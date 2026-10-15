// Epic pieces of the task page: phases (ROADMAP) → each phase's tasks (PLAN,
// live from the engine for the phase being built) → one task's detail.
// Contract: GET /api/board/:id/phases?root= → { phases, flat, live }.
import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, type BoardPhases, type BoardStage, type BoardTask, type PhaseBoard, type SubTaskRow } from "./api";
import { isMissing } from "./ui";

/** One fetch per task page. Polls while the task runs. */
export function usePhases(root: string, task: BoardTask) {
  return useQuery({
    // `updated` in the key: any task change (stage, status) refetches once.
    queryKey: ["board-phases", root, task.id, task.updated],
    queryFn: () => api.boardPhases(root, task.id),
    placeholderData: (prev) => prev,
    refetchInterval: (q) => (isMissing(q.state.error) ? false : task.status === "running" ? 4000 : false),
  });
}

// Step names for a phase's "Đang: …" line.
const STEP_LABEL: Partial<Record<BoardStage, string>> = {
  investigate: "Điều tra",
  intake: "Tiếp nhận",
  solution: "Lập kế hoạch",
  review: "PO duyệt",
  approval: "Chờ bạn duyệt",
  dev: "Phát triển",
  test: "Kiểm thử",
  security: "Review bảo mật",
  report: "Báo cáo",
  acceptance: "Chờ bạn nghiệm thu",
};
export const stepLabel = (s: BoardStage) => STEP_LABEL[s] ?? s;

const COLS: { status: SubTaskRow["status"]; label: string }[] = [
  { status: "todo", label: "Chưa làm" },
  { status: "in_progress", label: "Đang làm" },
  { status: "blocked", label: "Bị chặn" },
  { status: "done", label: "Xong" },
];
const STATUS_LABEL: Record<SubTaskRow["status"], string> = { todo: "Chưa làm", in_progress: "Đang làm", blocked: "Bị chặn", done: "Xong" };

export const doneN = (ts: SubTaskRow[]) => ts.filter((s) => s.status === "done").length;

/** Phase being worked on: the active one, else the next todo, else the last. */
export function currentPhase(p: BoardPhases): PhaseBoard | undefined {
  return p.phases.find((ph) => ph.state === "active") ?? p.phases.find((ph) => ph.state === "todo") ?? p.phases[p.phases.length - 1];
}

/** Overall progress, phase-weighted: unplanned phases have no tasks yet, so tasks alone would overstate it. */
export function progressPct(p: BoardPhases): number {
  if (p.phases.length) {
    const sum = p.phases.reduce((s, ph) => s + (ph.state === "done" ? 1 : ph.tasks.length ? doneN(ph.tasks) / ph.tasks.length : 0), 0);
    return Math.round((sum / p.phases.length) * 100);
  }
  return p.flat.length ? Math.round((doneN(p.flat) / p.flat.length) * 100) : 0;
}

// ── Phase list ─────────────────────────────────────────────────────────────
/** Compact phase selector: state icon, id + name, task count; the active phase also says what runs now. */
export function PhaseList({ phases, sel, now, onPick }: { phases: PhaseBoard[]; sel: string; now: string | null; onPick: (id: string) => void }) {
  // Narrow screens: the list scrolls sideways; keep the selected phase in view.
  const listRef = useRef<HTMLOListElement>(null);
  useEffect(() => {
    const ol = listRef.current;
    const on = ol?.querySelector<HTMLElement>(".tp-ph.is-on");
    if (ol && on && ol.scrollWidth > ol.clientWidth) ol.scrollTo({ left: Math.max(0, on.offsetLeft - 16) });
  }, [sel]);
  return (
    <nav className="tp-phases" aria-label="Phase">
      <ol ref={listRef}>
        {phases.map((ph) => {
          const on = ph.id === sel;
          const step = ph.steps.find((s) => s.state === "now");
          const doing = ph.state === "active" ? (step ? stepLabel(step.key) : now) : null;
          return (
            <li key={ph.id}>
              <button className={`tp-ph is-${ph.state}${on ? " is-on" : ""}`} aria-current={on ? "true" : undefined} onClick={() => onPick(ph.id)} title={ph.goal || undefined}>
                <span className="tp-ph-ico" aria-hidden="true">{ph.state === "done" ? "✓" : ph.state === "active" ? "●" : "○"}</span>
                <span className="tp-ph-main">
                  <span className="tp-ph-name"><span className="mono">{ph.id}</span> {ph.name}</span>
                  {doing ? <span className="tp-ph-now">Đang: {doing}</span> : null}
                </span>
                <span className="tp-ph-n">{ph.planned || ph.tasks.length ? `${doneN(ph.tasks)}/${ph.tasks.length} task` : "chưa có task"}</span>
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

// ── Task kanban ────────────────────────────────────────────────────────────
export function SubKanban({ tasks, onOpen }: { tasks: SubTaskRow[]; onOpen: (key: string) => void }) {
  return (
    <div className="ep-board">
      {COLS.map((c) => {
        const list = tasks.filter((s) => s.status === c.status);
        return (
          <section key={c.status} className={`ep-col is-${c.status}`} aria-label={`${c.label}: ${list.length}`}>
            <div className="ep-col-head">
              <span className="ep-col-name">{c.label}</span>
              <span className="ep-col-count">{list.length}</span>
            </div>
            <div className="ep-col-body">
              {list.length === 0 ? <p className="ep-col-empty">Không có</p> : list.map((s) => <SubCard key={s.key} s={s} onOpen={() => onOpen(s.key)} />)}
            </div>
          </section>
        );
      })}
    </div>
  );
}

function SubCard({ s, onOpen }: { s: SubTaskRow; onOpen: () => void }) {
  const top =
    s.status === "done" ? <span className="ep-k is-ok">{s.key} ✓{s.commit ? <> · <span className="mono">{s.commit}</span></> : null}</span>
    : s.status === "in_progress" ? <span className="ep-k is-run">{s.key} · đang làm</span>
    : s.status === "blocked" ? <span className="ep-k is-err">{s.key} · bị chặn</span>
    : <span className="ep-k">{s.key}{s.depends ? ` · sau ${s.depends}` : ""}</span>;
  return (
    <button className={`ep-card is-${s.status}`} onClick={onOpen} aria-label={`${s.key}: ${s.title || s.key}`}>
      {top}
      <span className="ep-card-title">{s.title || s.key}</span>
      {s.files ? <span className="ep-card-files mono">{s.files}</span> : null}
      {s.retries > 0 ? <span className="ep-card-warn">verify lỗi {s.retries} lần</span> : null}
    </button>
  );
}

// ── One task ───────────────────────────────────────────────────────────────
/** Focused view of one sub-task; replaces the kanban. Dependencies link to their tasks. */
export function SubDetail({ s, list, back, onBack, onOpen }: { s: SubTaskRow; list: SubTaskRow[]; back: string; onBack: () => void; onOpen: (key: string) => void }) {
  const deps = s.depends.split(/[,;\s]+/).filter(Boolean);
  const verify =
    s.retries > 0 ? `Lỗi ${s.retries} lần` : s.status === "done" ? "Đạt" : s.status === "in_progress" ? "Đang chạy" : "Chưa chạy";
  return (
    <article className="tp-sub" aria-labelledby="tp-sub-title">
      <button className="ghost tp-sub-back" onClick={onBack}>← {back}</button>
      <p className={`ep-k ${s.status === "done" ? "is-ok" : s.status === "in_progress" ? "is-run" : s.status === "blocked" ? "is-err" : ""}`}>
        <span className="mono">{s.key}</span> · {STATUS_LABEL[s.status]}
      </p>
      <h3 id="tp-sub-title" className="tp-sub-title">{s.title || s.key}</h3>
      <dl className="tp-dl">
        <dt>File</dt>
        <dd className={s.files ? "mono" : "faint"}>{s.files || "Chưa ghi"}</dd>
        <dt>Phụ thuộc</dt>
        <dd>
          {deps.length === 0 ? (
            <span className="faint">Không</span>
          ) : (
            deps.map((k, i) => {
              const hit = list.find((x) => x.key === k);
              return (
                <span key={`${k}-${i}`}>
                  {i > 0 ? ", " : ""}
                  {hit ? (
                    <button className="link" onClick={() => onOpen(k)} title={hit.title}>{k}</button>
                  ) : (
                    <span className="mono">{k}</span>
                  )}
                  {hit ? <span className="faint"> ({STATUS_LABEL[hit.status].toLowerCase()})</span> : null}
                </span>
              );
            })
          )}
        </dd>
        <dt>Tiêu chí (AC)</dt>
        <dd className={s.ac ? "" : "faint"}>{s.ac || "Chưa gắn"}</dd>
        <dt>Kiểm tra</dt>
        <dd>
          <span className={s.retries > 0 ? "tp-warn" : ""}>{verify}</span>
          {s.note ? <p className="tp-sub-note">{s.note}</p> : null}
        </dd>
        <dt>Commit</dt>
        <dd className={s.commit ? "mono" : "faint"}>{s.commit ?? "Chưa có"}</dd>
      </dl>
    </article>
  );
}

// Task page: one Board task as a full page with real browser history.
// URL: ?page=board&task=T-002&root=<abs>[&phase=M1][&sub=T4]. Every level is a
// history entry, so Back / Forward / reload / shared links all land where the
// boss was. Levels: task (phases + kanban) → phase → one sub-task.
import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BoardDetail, type BoardTask } from "./api";
import { EndpointMissing, ErrorState, Loading, isMissing } from "./ui";
import { Pipeline } from "./Steps";
import { AskPanel, askingYou } from "./Asks";
import { pauseLabel } from "./Interject";
import { PhaseList, SubDetail, SubKanban, currentPhase, doneN, progressPct, stepLabel, usePhases } from "./Epic";
import { Docs, Log, STAGE_LABEL, StatusPill, TaskActions, TaskPm, Thread, clock, isTerminal, money, needLabel, useEscape } from "./Board";
import { ContinuePlan } from "./ContinuePlan";
import { WorkflowChip, WorkflowPipeline, taskStageLabel } from "./Workflows";

// ── Route ──────────────────────────────────────────────────────────────────
export type TaskRoute = { task: string; root: string; phase: string | null; sub: string | null };
const KEYS = ["task", "root", "phase", "sub"] as const;
const ROUTE_EVENT = "ckit:route";

// A card button that opens the page lands the boss where they act: the open
// question, the reply box, or the gate's primary button. One-shot, set right
// before the navigation.
export type OpenFocus = "ask" | "reply" | "act";
let pendingFocus: OpenFocus | null = null;
export function focusOnOpen(f: OpenFocus) {
  pendingFocus = f;
}
function useOpenFocus() {
  useEffect(() => {
    const f = pendingFocus;
    pendingFocus = null;
    if (!f) return;
    const h = window.setTimeout(() => {
      const el = document.querySelector<HTMLElement>(f === "ask" ? ".ask-panel" : ".tp-act");
      el?.scrollIntoView({ block: "center" });
      const target = f === "act" ? el?.querySelector<HTMLElement>(".tp-act-btns .primary") : el?.querySelector<HTMLElement>("textarea, input, .ask-opt");
      target?.focus({ preventScroll: true });
    }, 0);
    return () => window.clearTimeout(h);
  }, []);
}

export function routeFromUrl(): TaskRoute | null {
  const p = new URLSearchParams(window.location.search);
  const task = p.get("task");
  if (!task || (p.get("page") ?? "board") !== "board") return null;
  return { task, root: p.get("root") ?? "", phase: p.get("phase"), sub: p.get("sub") };
}

function urlFor(r: TaskRoute | null): string {
  const p = new URLSearchParams(window.location.search);
  for (const k of KEYS) p.delete(k);
  p.set("page", "board");
  if (r) {
    p.set("task", r.task);
    if (r.root) p.set("root", r.root);
    if (r.phase) p.set("phase", r.phase);
    if (r.sub) p.set("sub", r.sub);
  }
  return `?${p}`;
}

// Same page regardless of param order or the bare "." the shell uses for the Board.
function norm(search: string): string {
  const p = new URLSearchParams(search);
  if (!p.get("page")) p.set("page", "board");
  p.sort();
  return p.toString();
}

/** Push a new level (or the Board when `r` is null). No-op when already there. */
export function goTo(r: TaskRoute | null) {
  const url = urlFor(r);
  if (norm(url) === norm(window.location.search)) return;
  window.history.pushState({ ckitFrom: window.location.search }, "", url);
  window.dispatchEvent(new Event(ROUTE_EVENT));
}

/** One level up. Steps back when the previous entry is that level, so history stays linear. */
export function goUp(r: TaskRoute) {
  const parent = r.sub ? { ...r, sub: null } : r.phase ? { ...r, phase: null } : null;
  const from = (window.history.state as { ckitFrom?: string } | null)?.ckitFrom;
  // A sub-task opened from the default (unpicked) phase came from the task level, which shows that same phase.
  const ups = [parent, ...(r.sub && r.phase ? [{ ...r, phase: null, sub: null }] : [])].map((x) => norm(urlFor(x)));
  if (from !== undefined && ups.includes(norm(from))) window.history.back();
  else goTo(parent);
}

/** Current route; follows pushes and Back / Forward. */
export function useTaskRoute(): TaskRoute | null {
  const [r, setR] = useState(routeFromUrl);
  useEffect(() => {
    const on = () => setR(routeFromUrl());
    window.addEventListener("popstate", on);
    window.addEventListener(ROUTE_EVENT, on);
    return () => {
      window.removeEventListener("popstate", on);
      window.removeEventListener(ROUTE_EVENT, on);
    };
  }, []);
  return r;
}

// ── Page ───────────────────────────────────────────────────────────────────
export function TaskPage({ route, updated }: { route: TaskRoute; updated?: string }) {
  const qc = useQueryClient();
  const { task: id, root } = route;
  const detail = useQuery({
    queryKey: ["board-task", root, id],
    queryFn: () => api.boardTask(root, id),
    refetchInterval: (q) => {
      const s = q.state.data?.task.status;
      return s === "running" || s === "queued" ? 2000 : false;
    },
  });
  // The board poll sees changes first (e.g. a queued task promoted); follow it.
  useEffect(() => {
    if (updated) qc.invalidateQueries({ queryKey: ["board-task", root, id] });
  }, [updated, id, root, qc]);
  useEscape(() => goUp(route));
  // A new task starts at the top of the page.
  useEffect(() => {
    document.querySelector(".main")?.scrollTo(0, 0);
  }, [id]);

  if (detail.data) return <TaskView d={detail.data} route={route} />;
  return (
    <div className="tp">
      <Crumbs route={route} />
      {isMissing(detail.error) ? (
        <EndpointMissing endpoint={`/api/board/${id}`} />
      ) : detail.error ? (
        <ErrorState message={/^404/.test((detail.error as Error).message) ? `Không tìm thấy ${id} trong dự án này.` : (detail.error as Error).message} />
      ) : (
        <Loading rows={5} />
      )}
    </div>
  );
}

type Crumb = { label: string; to?: TaskRoute | null };

function Crumbs({ route, items }: { route: TaskRoute; items?: Crumb[] }) {
  const list: Crumb[] = [{ label: "Board", to: null }, ...(items ?? [{ label: route.task }])];
  return (
    <div className="tp-nav">
      <button className="ghost tp-back" onClick={() => goUp(route)} title="Lên một cấp (Esc)">← Quay lại</button>
      <nav aria-label="Đường dẫn">
        <ol className="tp-crumbs">
          {list.map((c, i) => {
            const last = i === list.length - 1;
            return (
              <li key={i}>
                {last ? (
                  <span aria-current="page" className="tp-crumb is-here">{c.label}</span>
                ) : (
                  <button className="tp-crumb" onClick={() => goTo(c.to ?? null)}>{c.label}</button>
                )}
              </li>
            );
          })}
        </ol>
      </nav>
    </div>
  );
}

type Tab = "thread" | "docs" | "log";

function TaskView({ d, route }: { d: BoardDetail; route: TaskRoute }) {
  const t = d.task;
  const q = usePhases(route.root, t);
  const p = q.data;
  const [tab, setTab] = useState<Tab>("thread");
  useOpenFocus();

  const phases = p?.phases ?? [];
  const flat = Boolean(p) && phases.length === 0;
  const cur = p ? currentPhase(p) : undefined;
  const sel = phases.find((ph) => ph.id === route.phase) ?? cur;
  const list = flat ? p!.flat : sel?.tasks ?? [];
  const sub = route.sub ? list.find((s) => s.key === route.sub) : undefined;

  const top: TaskRoute = { ...route, phase: null, sub: null };
  const atPhase = (id: string): TaskRoute => ({ ...route, phase: id, sub: null });
  const openSub = (key: string) => goTo({ ...route, phase: flat ? null : sel?.id ?? null, sub: key });

  const crumbs: Crumb[] = [{ label: `${t.id} ${t.title}`, to: top }];
  if (!flat && route.phase && sel) crumbs.push({ label: `${sel.id}${sel.name ? ` ${sel.name}` : ""}`, to: atPhase(sel.id) });
  if (route.sub) crumbs.push({ label: sub ? `${sub.key} ${sub.title}` : route.sub });

  // Header summary: where the work is, what it cost, which branch.
  const closed = phases.filter((ph) => ph.state === "done").length;
  const curTasks = flat ? p!.flat : cur?.tasks ?? [];
  const summary: string[] = [];
  // Count phases actually closed (ROADMAP [x]); a finished task can still have
  // an open phase (e.g. an AC left NEEDS-CONFIRM at acceptance): say so.
  if (phases.length) summary.push(`Phase ${closed}/${phases.length} đã đóng`);
  const unclosed = t.status === "done" ? phases.filter((ph) => ph.state !== "done") : [];
  if (unclosed.length) summary.push(`Đã nghiệm thu khi ${unclosed.map((ph) => `${ph.id} chưa đóng${ph.ac_total ? ` (AC ${ph.ac_pass}/${ph.ac_total})` : ""}`).join(", ")}`);
  if (curTasks.length) summary.push(`Task ${doneN(curTasks)}/${curTasks.length}`);
  if (!phases.length && !isTerminal(t)) summary.push(`Đang: ${taskStageLabel(t) ?? STAGE_LABEL[t.stage]}`);
  summary.push(money(t.cost));
  const pct = p && (phases.length || p.flat.length) ? progressPct(p) : null;
  const nowStep = isTerminal(t) ? null : stepLabel(t.stage);

  return (
    <div className="tp">
      <Crumbs route={route} items={crumbs} />

      <header className="tp-head">
        <div className="tp-head-main">
          <div className="tp-title">
            <h2>
              <span className="mono tp-id">{t.id}</span> {t.title}
            </h2>
            <WorkflowChip t={t} />
            <Pill t={t} />
          </div>
          <p className="tp-sum">
            {summary.join(" · ")}
            {t.branch ? <> · nhánh <code>{t.branch}</code></> : null}
          </p>
          {pct !== null ? (
            <div className="tp-prog">
              <div className="ep-bar tp-bar" role="progressbar" aria-label="Tiến độ chung" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
                <span style={{ width: `${pct}%` }} />
              </div>
              <span className="tp-prog-n">{pct}%</span>
            </div>
          ) : null}
          {t.note && t.status !== "waiting" ? <p className={`bd-note ${t.status === "failed" ? "is-err" : ""}`}>{t.note}</p> : null}
        </div>
        <TaskActions t={t} />
        <ContinuePlan d={d} root={route.root} />
      </header>

      <AskPanel task={t} root={route.root} />

      <section className="tp-phase-panel" aria-label="Phase và task">
      <h3 className="tp-sec-title">{flat ? "Task" : "Phase và task"}</h3>
      <div className={`tp-body ${flat ? "is-flat" : ""}`}>
        {isMissing(q.error) ? (
          <EndpointMissing endpoint={`/api/board/${t.id}/phases`} />
        ) : q.error ? (
          <ErrorState message={(q.error as Error).message} />
        ) : !p ? (
          <Loading rows={3} />
        ) : (
          <>
            {!flat && sel ? <PhaseList phases={phases} sel={sel.id} now={nowStep} onPick={(id) => goTo(atPhase(id))} /> : null}
            <div className="tp-main">
              {route.sub ? (
                sub ? (
                  <SubDetail
                    key={sub.key}
                    s={sub}
                    list={list}
                    back={flat ? "Tất cả task" : `Task của ${sel?.id}`}
                    onBack={() => goUp(route)}
                    onOpen={openSub}
                  />
                ) : (
                  <div className="tp-sub">
                    <button className="ghost tp-sub-back" onClick={() => goUp(route)}>← Quay lại danh sách task</button>
                    <p className="bd-empty">Không tìm thấy task {route.sub}{sel && !flat ? ` trong ${sel.id}` : ""}.</p>
                  </div>
                )
              ) : (
                <>
                  {flat && !p.live ? null : (
                    <p className="tp-label">
                      {flat ? null : `Task của ${sel?.id}${sel?.name ? ` · ${sel.name}` : ""}`}
                      {p.live && (flat || sel?.state === "active") ? <span className="ep-live">trực tiếp từ engine</span> : null}
                    </p>
                  )}
                  {list.length > 0 || (sel?.planned && !flat) ? (
                    <SubKanban tasks={list} onOpen={openSub} />
                  ) : (
                    <p className="bd-empty ep-empty">
                      {flat ? "Chưa chia task. Task hiện ở đây khi Dev bắt đầu." : `${sel?.id} chưa có task. AI viết kế hoạch khi tới lượt phase này.`}
                    </p>
                  )}
                </>
              )}
            </div>
          </>
        )}
      </div>
      </section>

      <div className="tp-aux">
        <details className="bd-request">
          <summary>Các bước</summary>
          <div className="tp-aux-body">
            {t.workflow ? <WorkflowPipeline name={t.workflow.name} stage={t.stage} status={t.status} /> : <Pipeline task={t} root={route.root} />}
          </div>
        </details>
        <details className="bd-request">
          <summary>Yêu cầu gốc</summary>
          <div className="tp-aux-body">
            <p>{t.request}</p>
            <p className="tp-meta faint">
              Tạo {clock(t.created)}
              {t.base ? <> · từ nhánh <code>{t.base}</code></> : null}
              {t.feature ? <> · feature <code>{t.feature}</code></> : null}
              {t.rounds.solution > 0 ? ` · PO trả lại ${t.rounds.solution} lần` : ""}
            </p>
          </div>
        </details>
      </div>

      <section className="tp-more" aria-label="Thông tin thêm">
        <div className="tp-tabs" role="tablist" aria-label="Thông tin thêm">
          <TabBtn on={tab === "thread"} onClick={() => setTab("thread")} label="Trao đổi" n={t.thread.length} />
          <TabBtn on={tab === "docs"} onClick={() => setTab("docs")} label="Tài liệu" n={d.docs.length} />
          <TabBtn on={tab === "log"} onClick={() => setTab("log")} label="Nhật ký" n={t.runs.length} />
        </div>
        <div className="tp-tab" role="tabpanel">
          {tab === "thread" && <Thread msgs={t.thread} />}
          {tab === "docs" && <Docs d={d} docs={d.docs} />}
          {tab === "log" && (
            <>
              <TaskPm id={t.id} />
              <Log task={t} />
            </>
          )}
        </div>
      </section>
    </div>
  );
}

/** One pill: what the boss must do, else what is happening. */
function Pill({ t }: { t: BoardTask }) {
  if (!isTerminal(t) && (t.status === "waiting" || askingYou(t))) return <span className="tag warn">{needLabel(t)}</span>;
  if (t.status === "failed") return <span className="tag err">Lỗi, cần chạy lại</span>;
  return <StatusPill status={t.status} stage={t.stage} paused={pauseLabel(t)} />;
}

function TabBtn({ on, onClick, label, n }: { on: boolean; onClick: () => void; label: string; n: number }) {
  return (
    <button role="tab" aria-selected={on} className={on ? "seg-on" : ""} onClick={onClick}>
      {label} <span className="bd-tab-n">{n}</span>
    </button>
  );
}

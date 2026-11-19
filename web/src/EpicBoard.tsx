// "Bảng" view: one card per đầu mục (Board task = one feature), laid out by
// lifecycle instead of by pipeline step. Phases and their tasks live on the
// task page (TaskPage.tsx); this board only answers "where is each
// đầu mục and does it need me". A card that waits on the boss carries its
// action button (CardActions.tsx).
import type { CSSProperties } from "react";
import type { BoardTaskSummary } from "./api";
import { askingYou } from "./Asks";
import { isNew, isPaused, pauseLabel } from "./Interject";
import { STAGE_LABEL, StatusPill, ago, isTerminal, money, needLabel } from "./Board";
import { WorkflowChip, taskStageLabel } from "./Workflows";
import { PmsSourceChip } from "./PmsPicker";
import { CardAction } from "./CardActions";
import { ModelChip } from "./ModelChip";

export type Lane = "new" | "plan" | "you" | "dev" | "test" | "done";

export const LANES: { id: Lane; name: string; empty: string }[] = [
  { id: "new", name: "Chưa bắt đầu", empty: "Đầu mục mới tạo nằm đây, chờ bạn bấm Bắt đầu." },
  { id: "plan", name: "Lên kế hoạch", empty: "Leader và BA làm rõ yêu cầu, lập kế hoạch phase ở đây." },
  { id: "you", name: "Chờ bạn", empty: "Không có đầu mục nào chờ bạn." },
  { id: "dev", name: "Đang làm", empty: "Chưa có đầu mục nào đang lập trình." },
  { id: "test", name: "Kiểm thử", empty: "Chưa có đầu mục nào đang kiểm thử." },
  { id: "done", name: "Xong", empty: "Chưa có đầu mục nào xong." },
];

/** Lifecycle lane of a đầu mục. Failed tasks stay in the lane of their stage. */
export function laneOf(t: BoardTaskSummary): Lane {
  if (isTerminal(t)) return "done";
  if (isNew(t)) return "new";
  if (t.status === "waiting" || askingYou(t)) return "you";
  switch (t.stage) {
    case "dev":
      return "dev";
    case "test":
    case "security":
    case "report":
    case "acceptance":
      return "test";
    default:
      return "plan";
  }
}

export function EpicBoard({
  tasks, root, showDone, doneN, onShowDone, only, onOpen,
}: {
  tasks: BoardTaskSummary[];
  root: string;
  showDone: boolean;
  /** finished tasks hidden behind the collapsed "Xong" rail */
  doneN: number;
  onShowDone: () => void;
  /** status-strip filter: show just these lanes (tasks already filtered) */
  only?: Lane[];
  onOpen: (id: string) => void;
}) {
  const lanes = only ? LANES.filter((l) => only.includes(l.id)) : showDone ? LANES : LANES.filter((l) => l.id !== "done");
  const rail = !only && !showDone;
  const by = new Map<Lane, BoardTaskSummary[]>();
  for (const t of tasks) {
    const l = laneOf(t);
    by.set(l, [...(by.get(l) ?? []), t]);
  }
  const style = { "--lanes": lanes.length } as CSSProperties;
  return (
    <div className={`bd-board${rail ? " has-rail" : ""}${only?.length === 1 ? " is-one" : ""}`} role="list" aria-label="Đầu mục theo vòng đời" style={style}>
      {lanes.map((l) => {
        const list = by.get(l.id) ?? [];
        const need = l.id === "you" && list.length > 0;
        return (
          <section key={l.id} className={`bd-col lane-${l.id}${need ? " is-need" : ""}`} role="listitem" aria-label={`${l.name}: ${list.length}`}>
            <header className="bd-col-head">
              <span className="bd-col-dot" aria-hidden="true" />
              <span className="bd-col-name">{l.name}</span>
              <span className={`bd-count ${need ? "is-need" : ""}`}>{list.length}</span>
            </header>
            <div className="bd-col-body">
              {list.length === 0 ? (
                <div className="bd-col-empty">{l.empty}</div>
              ) : (
                list.map((t) => <EpicCard key={t.id} t={t} root={root} need={l.id === "you"} onOpen={onOpen} />)
              )}
            </div>
          </section>
        );
      })}
      {rail ? (
        <button className="bd-rail" onClick={onShowDone} title="Hiện cột Xong" aria-label={`Hiện cột Xong: ${doneN} đầu mục`}>
          <span className="bd-rail-name">Xong</span>
          <span className="bd-count is-ok">{doneN}</span>
        </button>
      ) : null}
    </div>
  );
}

function EpicCard({ t, root, need, onOpen }: { t: BoardTaskSummary; root: string; need: boolean; onOpen: (id: string) => void }) {
  const p = t.progress && t.progress.total > 0 ? t.progress : null;
  const meta = t.phase
    ? `Phase ${t.phase}${p ? ` · ${p.done}/${p.total} task` : ""}`
    : p
      ? `${p.done}/${p.total} task`
      : isTerminal(t)
        ? null
        : (taskStageLabel(t) ?? STAGE_LABEL[t.stage]);
  // A paused task's note only repeats "Tạm dừng"; the pill already says it.
  const note = t.note && !isPaused(t) ? t.note : null;
  const failed = t.status === "failed";
  const full = p !== null && p.done >= p.total;
  return (
    <article className={`bd-card is-${need ? "waiting" : t.status}`}>
      <button className="bd-card-open" onClick={() => onOpen(t.id)} aria-label={`${t.id}: ${t.title}`}>
        <span className="bd-card-top">
          <span className="mono">{t.id}</span>
          <WorkflowChip t={t} />
        </span>
        <span className="bd-card-title">{t.title}</span>
        <span className="bd-card-pill">
          {need ? (
            <span className="tag warn">{needLabel(t)}</span>
          ) : (
            <StatusPill status={t.status} stage={t.stage} asking={askingYou(t)} paused={pauseLabel(t)} />
          )}
        </span>
        {meta ? <span className="bd-card-meta">{meta}</span> : null}
        {t.status === "running" ? <ModelChip m={t.run_model} className="bd-card-model" /> : null}
        {p ? (
          <span className={`bd-card-bar${failed ? " is-err" : full ? " is-full" : ""}`} role="progressbar" aria-label="Tiến độ phase" aria-valuemin={0} aria-valuemax={p.total} aria-valuenow={p.done}>
            <span style={{ width: `${Math.round((p.done / p.total) * 100)}%` }} />
          </span>
        ) : null}
        {note ? <span className={`bd-card-note ${failed ? "err" : ""}`}>{note}</span> : null}
        <span className="bd-card-foot">
          <span className="bd-card-cost">{money(t.cost)}</span>
          <span>{ago(t.updated)}</span>
        </span>
      </button>
      {t.source ? (
        <div className="bd-card-src">
          <PmsSourceChip source={t.source} />
        </div>
      ) : null}
      <CardAction t={t} root={root} onOpen={() => onOpen(t.id)} />
    </article>
  );
}

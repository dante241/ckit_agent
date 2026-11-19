// Task page body (direction A): phase rail | timeline of runs grouped by phase | decisions.
// Each run is one entry: who, model, time, cost, verdict, ≤3 summary lines, the questions it
// asked with their answers, notes the boss / coordinator sent into it, files and commits, and
// "Xem log" (drawer). Gates (boss approvals, coordinator approvals on the boss's behalf, the
// merge) sit between runs. Filter chips narrow it to questions, sent-back runs or messages.
import { createContext, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useQuery } from "@tanstack/react-query";
import { api, type BoardAsk, type BoardDecision, type BoardDetail, type BoardMsg, type BoardRun, type BoardTask, type PhaseBoard, type BoardPhases } from "./api";
import { EndpointMissing, ErrorState, isMissing, scrollPane } from "./ui";
import { Inline, Markdown } from "./markdown";
import { ModelChip } from "./ModelChip";
import { Glyph } from "./icons";
import { asker, member, senderLabel } from "./Asks";
import { STAGE_WHO, clock, money, verdictTone } from "./Board";
import { doneN } from "./Epic";
import { hhmm, msgKind, nowActivity, runFailed, runLive, runSecs, runTitle, secsBetween, secsText, stepName, verdictText } from "./runs";

// ── Page-level actions (drawers, navigation) ───────────────────────────────
/** `openGoals(phase?)`: the "Mục tiêu" view (`&view=goals`), optionally filtered to one phase. */
export type TaskUi = { openLog: (run: number) => void; openDocs: (doc?: string) => void; openPhase: (id: string | null) => void; openSub: (phase: string | null, key: string) => void; openGoals: (phase?: string | null) => void };
const noop = () => {};
export const TaskUiCtx = createContext<TaskUi>({ openLog: noop, openDocs: noop, openPhase: noop, openSub: noop, openGoals: noop });
export const useTaskUi = () => useContext(TaskUiCtx);

export function useDecisions(root: string, t: BoardTask) {
  return useQuery({
    queryKey: ["board-decisions", root, t.id, t.updated],
    queryFn: () => api.boardDecisions(root, t.id),
    placeholderData: (prev) => prev,
    retry: (n, e) => !isMissing(e) && n < 2,
  });
}

// ── Index: asks + messages per run ─────────────────────────────────────────
// Messages the page shows; the Q/A copies in the thread are the asks themselves.
const SHOWN = new Set(["boss", "leader", "report", "coordinator", "delegated_approval", "system"]);

export type TaskIndex = {
  asks: Map<number, BoardAsk[]>;
  notes: Map<number, BoardMsg[]>;
  /** shown messages outside any run (gates, merges, notes between steps) */
  loose: BoardMsg[];
  /** every shown message, oldest first */
  msgs: BoardMsg[];
  unanswered: Set<string>;
};

/** Run whose window holds `at` (fallback when the server sent no `run`). */
function runAt(runs: BoardRun[], at: string): number {
  const x = Date.parse(at);
  for (let i = runs.length - 1; i >= 0; i--) {
    const r = runs[i]!;
    if (Date.parse(r.started) <= x && (r.ended === null || Date.parse(r.ended) >= x)) return i;
  }
  return -1;
}

export function indexTask(d: BoardDetail): TaskIndex {
  const t = d.task;
  const byLog = new Map(t.runs.map((r, i) => [r.log, i]));
  const asks = new Map<number, BoardAsk[]>();
  const notes = new Map<number, BoardMsg[]>();
  const loose: BoardMsg[] = [];
  const legacyQa = !(t.asks ?? []).length;
  const push = <T,>(m: Map<number, T[]>, k: number, v: T) => m.set(k, [...(m.get(k) ?? []), v]);
  for (const a of t.asks ?? []) {
    const i = a.run != null ? byLog.get(a.run) ?? -1 : runAt(t.runs, a.at);
    if (i >= 0) push(asks, i, a);
  }
  const msgs = t.thread.filter((m) => SHOWN.has(msgKind(m)) || (legacyQa && (msgKind(m) === "question" || msgKind(m) === "answer")));
  for (const m of msgs) {
    const i = m.run != null ? byLog.get(m.run) ?? -1 : m.kind ? -1 : runAt(t.runs, m.at);
    const k = msgKind(m);
    // A gate or the merge reads as its own row even when it lands inside a run's window.
    if (i >= 0 && k !== "delegated_approval" && k !== "system" && !(k === "boss" && !m.from.includes(" → "))) push(notes, i, m);
    else loose.push(m);
  }
  return { asks, notes, loose, msgs, unanswered: new Set((d.unanswered ?? []).map((u) => u.at)) };
}

// ── Groups ─────────────────────────────────────────────────────────────────
type Entry = { kind: "run"; i: number; at: string } | { kind: "msg"; m: BoardMsg; at: string };
/** `round` ≥ 2: runs of a "Vòng N" the boss opened from the chat (revise). */
export type Group = { key: string; phase: PhaseBoard | null; title: string; runs: number[]; entries: Entry[]; round: number };

function groupTitle(t: BoardTask, runs: number[], flat: boolean): string {
  if (flat) return "Các lần chạy";
  const stages = runs.map((i) => t.runs[i]!.stage);
  if (stages.every((s) => s === "intake" || s === "investigate")) return stepName(stages[0] ?? "intake");
  if (stages.includes("report")) return "Báo cáo và nghiệm thu";
  return "Ngoài phase";
}

export function buildGroups(t: BoardTask, phases: PhaseBoard[], ix: TaskIndex): Group[] {
  const groups: Group[] = [];
  const of = new Map<number, Group>();
  t.runs.forEach((r, i) => {
    let g = groups[groups.length - 1];
    const id = r.phase ?? null;
    const round = r.round ?? 1;
    if (!g || g.round !== round || (g.phase?.id ?? null) !== id || (id === null && g.phase === null && g.key.startsWith("x") && splitNull(t, g, r.stage))) {
      g = { key: `${id ?? "x"}-${groups.length}`, phase: id ? phases.find((p) => p.id === id) ?? stubPhase(id) : null, title: "", runs: [], entries: [], round };
      groups.push(g);
    }
    g.runs.push(i);
    g.entries.push({ kind: "run", i, at: r.started });
    of.set(i, g);
  });
  const flat = phases.length === 0;
  for (const g of groups) g.title = g.phase ? g.phase.name : g.round > 1 ? `Vòng ${g.round}` : groupTitle(t, g.runs, flat);
  for (const m of ix.loose) {
    const x = Date.parse(m.at);
    let i = -1;
    t.runs.forEach((r, k) => (Date.parse(r.started) <= x ? (i = k) : null));
    // "Vòng 2: <instruction>" opens its round: show it at the top of that round's first group.
    const rn = /^Vòng (\d+)\b/.exec(m.text)?.[1];
    let g = (rn ? groups.find((x) => x.round === Number(rn)) : undefined) ?? of.get(i) ?? groups[0];
    if (!g) {
      g = { key: "msgs-0", phase: null, title: "Trao đổi", runs: [], entries: [], round: 1 };
      groups.push(g);
    }
    g.entries.push({ kind: "msg", m, at: m.at });
  }
  for (const g of groups) g.entries.sort((a, b) => Date.parse(a.at) - Date.parse(b.at));
  return groups;
}

// An intake group must not swallow a later report run when both lack a phase.
function splitNull(t: BoardTask, g: Group, stage: BoardRun["stage"]): boolean {
  const first = t.runs[g.runs[0] ?? 0]?.stage;
  const startLike = (s: string | undefined) => s === "intake" || s === "investigate";
  return startLike(first) !== startLike(stage);
}

const stubPhase = (id: string): PhaseBoard => ({ id, name: "", state: "done", ac_pass: 0, ac_total: 0, goal: "", planned: false, steps: [], tasks: [] });

/** Runs, cost and time of a phase: the server's numbers when present, else from `runs[].phase`. */
export function phaseStats(t: BoardTask, ph: PhaseBoard) {
  const runs = ph.runs ?? t.runs.flatMap((r, i) => (r.phase === ph.id ? [i] : []));
  const rs = runs.map((i) => t.runs[i]).filter((r): r is BoardRun => Boolean(r));
  const started = ph.started ?? rs[0]?.started ?? null;
  const ended = rs.some(runLive) ? null : ph.ended ?? (rs.length && rs.every((r) => r.ended) ? rs[rs.length - 1]!.ended : null);
  return {
    runs,
    started,
    ended,
    secs: rs.reduce((s, r) => s + runSecs(r), 0),
    cost: ph.cost ?? rs.reduce((s, r) => s + r.cost, 0),
    back: ph.rounds_back ?? rs.filter((r) => r.stage === "review" && runFailed(r)).length,
    notes: ph.ac_notes ?? [],
    subagents: rs.reduce((s, r) => s + (r.digest?.subagents ?? 0), 0),
  };
}

// ── Small pieces ───────────────────────────────────────────────────────────
/** Long text shut to `lines`; "Xem thêm" only when it actually overflows. */
export function Clamp({ lines, children, className = "" }: { lines: number; children: ReactNode; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [over, setOver] = useState(false);
  useLayoutEffect(() => {
    const el = ref.current;
    if (el && !open) setOver(el.scrollHeight > el.clientHeight + 2);
  }, [open, children]);
  return (
    <div className={`clamp ${className}`.trim()}>
      <div ref={ref} className={open ? "" : "is-shut"} style={{ WebkitLineClamp: open ? undefined : lines }}>{children}</div>
      {over || open ? (
        <button className={`btn-sm ghost btn-more ${open ? "is-open" : ""}`} aria-expanded={open} onClick={() => setOpen(!open)}>
          {open ? "Thu gọn" : "Xem thêm"}<Glyph name="chevron" />
        </button>
      ) : null}
    </div>
  );
}

const stripQ = (s: string) => s.replace(/^(\s*\[[^\]\n]{1,24}\]\s*)+/, "");

export function AskBlock({ a }: { a: BoardAsk }) {
  const to = a.to === "boss" ? "bạn" : "Leader";
  const wait = a.answered_at ? secsBetween(a.at, a.answered_at) : null;
  return (
    <div className="tl-qa">
      <div className="tl-qa-h">
        <b>{a.id} · {asker(a.from)} hỏi {to}</b>
        <span>{hhmm(a.at)}</span>
        {wait !== null ? <span>trả lời sau {secsText(wait)}</span> : null}
        {a.status === "boss" ? <span className="tag warn">Chờ bạn trả lời</span> : a.status === "answering" ? <span className="tag info">Leader đang trả lời</span> : a.status === "timeout" || a.status === "closed" ? <span className="tag">Không có trả lời</span> : null}
      </div>
      <Clamp lines={2} className="tl-qa-q"><Inline text={stripQ(a.question)} /></Clamp>
      {a.answer ? (
        <div className="tl-qa-a">
          <span className="tl-qa-lbl">{member(a.by ?? "leader")} trả lời</span>
          <Clamp lines={3}><Inline text={stripQ(a.answer)} /></Clamp>
        </div>
      ) : null}
    </div>
  );
}

function Note({ m, ix }: { m: BoardMsg; ix: TaskIndex }) {
  const k = msgKind(m);
  const boss = k === "boss";
  const head = boss ? (m.from.includes(" → ") ? senderLabel(m.from).replace(/^Bạn → /, "Bạn nhắn ") : "Bạn nhắn Leader") : k === "leader" || k === "report" ? "Leader báo bạn" : senderLabel(m.from).replace(/ → /, " nhắn ");
  const waiting = !m.action && ix.unanswered.has(m.at);
  return (
    <div className={`tl-note ${boss ? "is-boss" : ""} ${k === "coordinator" ? "is-coord" : ""}`}>
      <div className="tl-note-h">
        <b>{head}</b>
        <span>{hhmm(m.at)}</span>
        {waiting ? <span className="tag warn">Chưa có phản hồi</span> : boss && m.action ? <span className="tag">Thao tác</span> : boss && m.ack ? <span className="tag ok">Đã nhận</span> : null}
      </div>
      {boss ? <p className="tl-note-text">{m.text}</p> : <Clamp lines={k === "leader" || k === "report" ? 4 : 3}><Markdown source={m.text} /></Clamp>}
      {boss && m.reply ? <p className="tl-note-reply"><span className="tl-qa-lbl">Trả lời</span>{m.reply}</p> : null}
    </div>
  );
}

function Gate({ m }: { m: BoardMsg }) {
  const k = msgKind(m);
  if (k === "system") return <div className="tl-gate is-sys"><span>{m.text}</span><span className="faint">{hhmm(m.at)}</span></div>;
  if (k === "delegated_approval") {
    return (
      <div className="tl-gate is-coord">
        <span className="tag warn">Thay bạn</span>
        <b>Điều phối duyệt thay bạn</b>
        <span className="faint">{hhmm(m.at)}</span>
        <span className="tl-gate-q">“{m.text}”</span>
      </div>
    );
  }
  if (k === "boss") {
    return (
      <div className="tl-gate">
        <span className="tag accent">Bạn</span>
        <b>{m.text}</b>
        <span className="faint">{hhmm(m.at)}</span>
      </div>
    );
  }
  return null;
}

const base = (p: string) => p.split("/").filter(Boolean).pop() ?? p;

function Files({ files }: { files: string[] }) {
  if (!files.length) return null;
  const shown = files.slice(0, 6);
  return (
    <div className="tl-out">
      <span>Tệp:</span>
      {shown.map((f) => <span key={f} className="tl-file" title={f}>{base(f)}</span>)}
      {files.length > shown.length ? <span title={files.slice(6).join("\n")}>+{files.length - shown.length}</span> : null}
    </div>
  );
}

function Commits({ list }: { list: { sha: string; subject: string }[] }) {
  if (!list.length) return null;
  const sha = (s: string) => s.slice(0, 10);
  if (list.length > 3) {
    return (
      <div className="tl-out">
        <span>{list.length} commit:</span>
        <code title={list[0]!.subject}>{sha(list[0]!.sha)}</code>
        <span>tới</span>
        <code title={list[list.length - 1]!.subject}>{sha(list[list.length - 1]!.sha)}</code>
      </div>
    );
  }
  return (
    <div className="tl-out">
      <span>Commit:</span>
      {list.map((c) => <span key={c.sha} className="tl-commit"><code>{sha(c.sha)}</code> {c.subject}</span>)}
    </div>
  );
}

export function subText(r: BoardRun): string {
  const d = r.digest;
  const n = d?.subagents ?? r.model?.subagents?.reduce((s, x) => s + x.count, 0) ?? 0;
  if (!n) return "";
  const models = d?.subagent_models ?? [];
  return models.length ? `${n} subagent (${models.join(", ")})` : `${n} subagent`;
}

export function toolText(r: BoardRun): string {
  const d = r.digest;
  if (!d) return "";
  const real = d.errors;
  return `${d.tools} lệnh · ${real} lỗi${d.harness_errors ? ` (+${d.harness_errors} do harness)` : ""}`;
}

const norm = (s: string) => s.replace(/[`*_\s]+/g, " ").trim().toLowerCase();
/** The Leader's step report whose opening is already the run summary: shown once. */
function repeatsSummary(m: BoardMsg, summary: string[]): boolean {
  const k = msgKind(m);
  if ((k !== "leader" && k !== "report") || !summary[0]) return false;
  return norm(m.text).includes(norm(summary[0]).slice(0, 48));
}

function RunCard({ t, i, ix, now }: { t: BoardTask; i: number; ix: TaskIndex; now: BoardDetail["now"] }) {
  const ui = useTaskUi();
  const r = t.runs[i]!;
  const live = runLive(r);
  const fail = runFailed(r);
  const summary = (r.summary ?? []).filter((s) => s && !(fail && r.fail_reason && s.trim() === r.fail_reason.trim())).slice(0, 3);
  const sub = subText(r);
  const tools = toolText(r);
  return (
    <article className={`tl-run ${fail ? "is-fail" : ""} ${live ? "is-live" : ""}`} id={`tl-run-${i}`} aria-label={runTitle(r)}>
      <div className="tl-r1">
        <h4>{live ? <span className="pulse" aria-hidden="true" /> : null}{runTitle(r)}</h4>
        <span className="tl-who">{STAGE_WHO[r.stage]}</span>
        {live ? <span className="tag info">Đang chạy</span> : r.verdict ? <span className={`tag ${verdictTone(r.verdict)}`}>{verdictText(r.verdict)}</span> : null}
        {r.exit !== null && r.exit !== 0 && !r.verdict ? <span className="tag err">Lỗi (exit {r.exit})</span> : null}
        <span className="tl-right">
          <time dateTime={r.started}>{hhmm(r.started)}</time>
          <b>{secsText(runSecs(r))}</b>
          <b>{money(r.cost)}</b>
        </span>
      </div>
      {r.model || sub || tools ? (
        <div className="tl-r2">
          <ModelChip m={r.model} />
          {sub ? <span>{sub}</span> : null}
          {tools ? <span>{tools}</span> : null}
          {r.summary_source === "log" && summary.length ? <span className="tag" title="Kết quả bước không có lời nhắn: tóm tắt lấy từ đoạn văn cuối của log">tóm tắt từ log</span> : null}
        </div>
      ) : null}
      {live && nowActivity(now) ? <p className="tl-live"><b>{nowActivity(now)!.replace(/^đ/, "Đ")}</b>{now?.at ? <span className="faint"> ({hhmm(now.at)})</span> : null}</p> : null}
      {summary.length ? <ul className="tl-sum">{summary.map((s, k) => <li key={k}><Inline text={s} /></li>)}</ul> : null}
      {fail && r.fail_reason ? <p className="tl-reason"><b>Lý do trả lại:</b> <Inline text={r.fail_reason} /></p> : null}
      <Files files={r.digest?.files ?? []} />
      <Commits list={r.commits ?? []} />
      {(ix.notes.get(i) ?? []).filter((m) => !repeatsSummary(m, summary)).map((m, k) => <Note key={`n${k}`} m={m} ix={ix} />)}
      {(ix.asks.get(i) ?? []).map((a) => <AskBlock key={a.id} a={a} />)}
      <div className="tl-foot">
        <button className="btn-sm" onClick={() => ui.openLog(i)}><Glyph name="log" />{live ? "Xem log trực tiếp" : "Xem log"}</button>
      </div>
    </article>
  );
}

// ── Timeline ───────────────────────────────────────────────────────────────
type Filter = "all" | "asks" | "back" | "msgs";

function groupLine(t: BoardTask, g: Group): string {
  const rs = g.runs.map((i) => t.runs[i]!);
  const cost = rs.reduce((s, r) => s + r.cost, 0);
  const first = rs[0]?.started ?? g.entries[0]?.at;
  const last = rs[rs.length - 1];
  const parts: string[] = [];
  if (g.phase) {
    parts.push(g.phase.state === "done" ? "Xong" : g.phase.state === "active" ? "Đang làm" : "Chưa làm");
    parts.push(`${hhmm(first)} → ${last?.ended ? hhmm(last.ended) : "nay"}`);
    parts.push(`${rs.length} lần chạy`);
    const back = rs.filter((r) => r.stage === "review" && runFailed(r)).length;
    if (back) parts.push(`${back} lần trả lại`);
  } else if (first) parts.push(clock(first));
  parts.push(money(cost));
  return parts.join(" · ");
}

function collapsedLine(t: BoardTask, g: Group, ix: TaskIndex): string {
  const asks = g.runs.reduce((s, i) => s + (ix.asks.get(i)?.length ?? 0), 0);
  const gates = g.entries.filter((e) => e.kind === "msg" && (msgKind(e.m) === "boss" || msgKind(e.m) === "delegated_approval")).length;
  const last = t.runs[g.runs[g.runs.length - 1] ?? -1];
  const res = last?.summary?.[0];
  return `${g.runs.length} lần chạy${asks ? ` · ${asks} câu hỏi` : ""}${gates ? ` · ${gates} lần duyệt` : ""}.${res ? ` Kết quả: ${res}` : ""}`;
}

export function Timeline({ d, groups, ix, sel = null }: { d: BoardDetail; groups: Group[]; ix: TaskIndex; sel?: string | null }) {
  const t = d.task;
  const ui = useTaskUi();
  const [filter, setFilter] = useState<Filter>("all");
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const nAsks = (t.asks ?? []).length;
  const nBack = t.runs.filter(runFailed).length;
  const nMsgs = ix.msgs.length;

  const keep = (e: Entry): boolean => {
    if (filter === "all") return true;
    if (filter === "msgs") return e.kind === "msg";
    if (e.kind !== "run") return false;
    return filter === "asks" ? (ix.asks.get(e.i)?.length ?? 0) > 0 : runFailed(t.runs[e.i]!);
  };
  const defOpen = (g: Group) => {
    if (!g.phase) return true;
    if (g.phase.state !== "done") return true;
    return g.runs.some((i) => runFailed(t.runs[i]!) || runLive(t.runs[i]!));
  };
  const isOpen = (g: Group) => filter !== "all" || (open[g.key] ?? defOpen(g));

  return (
    <div className="tl">
      <div className="tl-tools">
        <div className="seg tl-seg" role="tablist" aria-label="Lọc dòng thời gian">
          {([["all", "Tất cả", undefined], ["asks", "Câu hỏi", nAsks], ["back", "Trả lại", nBack], ["msgs", "Tin nhắn", nMsgs]] as const).map(([k, label, n]) => (
            <button key={k} role="tab" aria-selected={filter === k} className={filter === k ? "seg-on" : ""} onClick={() => setFilter(k)}>
              {label}{n !== undefined ? <span className="tl-seg-n">{n}</span> : null}
            </button>
          ))}
        </div>
      </div>
      {groups.length === 0 ? <p className="bd-empty">Chưa có lần chạy nào. Việc đang chờ tới lượt.</p> : null}
      {groups.map((g) => {
        const entries = filter === "msgs" ? msgEntries(g, ix) : g.entries.filter(keep);
        if (filter !== "all" && entries.length === 0) return null;
        const o = isOpen(g);
        return (
          <section key={g.key} className={`tl-grp${g.phase && g.phase.id === sel ? " is-sel" : ""}`} id={`tl-g-${g.key}`} aria-label={g.phase ? `${g.phase.id} ${g.phase.name}` : g.title}>
            {/* One row: the whole header toggles (chevron left); phase groups add one "Xem phase" button. */}
            <div className="tl-gh">
              <button className="tl-gh-tog" aria-expanded={o} onClick={() => setOpen({ ...open, [g.key]: !o })} disabled={filter !== "all"} aria-label={`${o ? "Thu gọn" : "Mở"} ${g.phase ? g.phase.id : g.title}`}>
                <span className="tl-gh-chev" aria-hidden="true"><Glyph name="chevron" /></span>
                <h3>{g.round > 1 && g.phase ? <span className="tag info tl-gh-round">Vòng {g.round}</span> : null}{g.phase ? <span className="mono">{g.phase.id}</span> : null}{g.title}</h3>
                <span className="tl-gh-s">{groupLine(t, g)}</span>
              </button>
              {g.phase ? <button className="btn-sm tl-gh-go" onClick={() => ui.openPhase(g.phase!.id)}>Xem phase<Glyph name="next" /></button> : null}
            </div>
            {o ? (
              <div className="tl-items">
                {entries.map((e) =>
                  e.kind === "run" ? <RunCard key={`r${e.i}`} t={t} i={e.i} ix={ix} now={d.now} /> : <MsgEntry key={`m${e.at}${e.m.from}`} m={e.m} ix={ix} />,
                )}
              </div>
            ) : (
              <p className="tl-collapsed"><Inline text={collapsedLine(t, g, ix)} /></p>
            )}
          </section>
        );
      })}
    </div>
  );
}

/** "Tin nhắn": every message of the group, the ones inside runs lifted out, oldest first. */
function msgEntries(g: Group, ix: TaskIndex): Entry[] {
  const inRuns = g.runs.flatMap((i) => (ix.notes.get(i) ?? []).map((m) => ({ kind: "msg" as const, m, at: m.at })));
  return [...g.entries.filter((e) => e.kind === "msg"), ...inRuns].sort((a, b) => Date.parse(a.at) - Date.parse(b.at));
}

function MsgEntry({ m, ix }: { m: BoardMsg; ix: TaskIndex }) {
  const k = msgKind(m);
  const gate = k === "system" || k === "delegated_approval" || (k === "boss" && !m.from.includes(" → ") && !ix.unanswered.has(m.at));
  return gate ? <Gate m={m} /> : <Note m={m} ix={ix} />;
}

// ── Phase rail ─────────────────────────────────────────────────────────────
// The phase last opened per task: its rail card and timeline group stay marked on return.
const lastPhase = new Map<string, string>();
export const rememberPhase = (task: string, phase: string) => lastPhase.set(task, phase);

export function PhaseRail({ t, p, groups, sel, onJump }: { t: BoardTask; p: BoardPhases; groups: Group[]; sel: string | null; onJump: (key: string) => void }) {
  const ui = useTaskUi();
  const loose = groups.filter((g) => !g.phase && g.runs.length);
  if (!p.phases.length) {
    return (
      <nav className="tl-rail" aria-label="Task">
        <h2 className="tl-side-h">Task <span className="tl-n">{doneN(p.flat)}/{p.flat.length}</span></h2>
        {p.flat.length === 0 ? <p className="bd-empty">Chưa chia task. Task hiện ở đây khi Dev bắt đầu.</p> : null}
        <ul className="tl-subs">
          {p.flat.map((s) => (
            <li key={s.key}>
              <button className="tl-sub" onClick={() => ui.openSub(null, s.key)} title={s.title}>
                <span className={`tl-sub-k is-${s.status}`}>{s.key}</span>
                <span className="tl-sub-t">{s.title || s.key}</span>
              </button>
            </li>
          ))}
        </ul>
      </nav>
    );
  }
  return (
    <nav className="tl-rail" aria-label="Phase">
      <h2 className="tl-side-h">Phase</h2>
      {p.phases.map((ph) => {
        const s = phaseStats(t, ph);
        const pct = ph.state === "done" ? 100 : ph.tasks.length ? Math.round((doneN(ph.tasks) / ph.tasks.length) * 100) : 0;
        const on = ph.id === sel;
        return (
          <button key={ph.id} className={`tl-ph tl-ph-main is-${ph.state}${on ? " is-sel" : ""}`} aria-current={on ? "true" : undefined} onClick={() => ui.openPhase(ph.id)} title={`${ph.goal ? `${ph.goal}\n` : ""}Mở trang phase ${ph.id}`}>
            <span className="tl-ph-top">
              <span className="tl-ph-ico" aria-hidden="true">{ph.state === "done" ? "✓" : ph.state === "active" ? "●" : "○"}</span>
              <span className="mono">{ph.id}</span>
              <span className="tl-ph-nm">{ph.name}</span>
              <span className="tl-ph-go" aria-hidden="true"><Glyph name="next" /></span>
            </span>
            <span className="tl-bar" role="progressbar" aria-label={`Tiến độ ${ph.id}`} aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}><span style={{ width: `${pct}%` }} /></span>
            <span className="tl-ph-st">
              {ph.ac_total ? <span>AC {ph.ac_pass}/{ph.ac_total}</span> : null}
              <span>{ph.planned || ph.tasks.length ? `${ph.tasks.length} task` : "chưa có task"}</span>
              {s.runs.length ? <span>{s.runs.length} lần chạy</span> : null}
              {s.secs ? <span>{secsText(s.secs)}</span> : null}
              {s.cost ? <span>{money(s.cost)}</span> : null}
              {s.back ? <span className="w">{s.back} lần trả lại</span> : null}
              {s.notes.length ? <span className="w">{s.notes.length} AC {ph.state === "done" ? "chấm lại" : "ghi chú (chưa chốt)"}</span> : null}
            </span>
            {on ? <span className="tl-ph-last">Vừa xem</span> : null}
          </button>
        );
      })}
      {loose.length ? (
        <button className="tl-ph is-extra tl-ph-main" onClick={() => onJump(loose[0]!.key)}>
          <span className="tl-ph-top"><span className="tl-ph-nm">Ngoài phase</span></span>
          <span className="tl-ph-st">
            <span>{[...new Set(loose.flatMap((g) => g.runs.map((i) => stepName(t.runs[i]!.stage))))].join(", ")}</span>
            <span>{money(loose.reduce((s, g) => s + g.runs.reduce((c, i) => c + t.runs[i]!.cost, 0), 0))}</span>
          </span>
        </button>
      ) : null}
    </nav>
  );
}

// ── Decisions ──────────────────────────────────────────────────────────────
const BY: Record<BoardDecision["by"], { label: string; tone: string }> = {
  boss: { label: "Bạn", tone: "accent" },
  leader: { label: "Leader", tone: "" },
  reviewer: { label: "Reviewer", tone: "" },
  coordinator: { label: "Điều phối", tone: "warn" },
};

export function DecisionList({ list, phase }: { list: BoardDecision[]; phase?: string }) {
  const shown = phase ? list.filter((x) => x.phase === phase) : list;
  if (!shown.length) return <p className="bd-empty">{phase ? `Chưa có quyết định nào ghi cho ${phase}.` : "Chưa có quyết định nào."}</p>;
  return (
    <ul className="tl-dec-list">
      {shown.map((x, i) => {
        const by = BY[x.by] ?? { label: x.by, tone: "" };
        const label = x.by === "coordinator" && x.kind === "delegated" ? "Điều phối thay bạn" : by.label;
        return (
          <li key={i}>
            <span className="tl-dec-h">
              <span className={`tag ${by.tone}`}>{label}</span>
              {x.phase && x.phase !== "all" && !phase ? <span className="mono">{x.phase}</span> : null}
              {x.at ? <span>{x.at.length > 10 ? hhmm(x.at) : x.at.slice(8, 10) + "/" + x.at.slice(5, 7)}</span> : null}
              {x.kind === "ac_regrade" ? <span className="tag warn">AC chấm lại</span> : null}
              {x.kind === "ac_change" ? <span className={`tag ${/^Không duyệt/.test(x.text) ? "err" : "accent"}`}>{/^Không duyệt/.test(x.text) ? "Không duyệt đổi AC" : "Duyệt đổi AC"}</span> : null}
            </span>
            <Clamp lines={4} className="tl-dec-t"><Inline text={x.text} /></Clamp>
            {x.kind === "ac_change" && x.why ? <span className="tl-dec-why">“{x.why.replace(/^(Không duyệt|Duyệt)\.\s*/, "")}”</span> : null}
            {x.ref ? <span className="tl-dec-ref">{x.ref}</span> : null}
          </li>
        );
      })}
    </ul>
  );
}

export function Decisions({ root, t }: { root: string; t: BoardTask }) {
  const q = useDecisions(root, t);
  const list = q.data ?? [];
  return (
    <section className="tl-dec" aria-label="Quyết định">
      <h2 className="tl-side-h">Quyết định {q.data ? <span className="tl-n">{list.length}</span> : null}</h2>
      {isMissing(q.error) ? <EndpointMissing endpoint={`/api/board/${t.id}/decisions`} /> : q.error ? <ErrorState message={(q.error as Error).message} /> : !q.data ? <p className="faint">Đang tải…</p> : <DecisionList list={list} />}
    </section>
  );
}

// ── Task page body ─────────────────────────────────────────────────────────
export function TaskBody({ d, p, root }: { d: BoardDetail; p: BoardPhases; root: string }) {
  const t = d.task;
  const ix = useMemo(() => indexTask(d), [d]);
  const groups = useMemo(() => buildGroups(t, p.phases, ix), [t, p.phases, ix]);
  const [sel] = useState(() => lastPhase.get(t.id) ?? null);
  const jump = (key: string, flash = false) => {
    const tog = document.getElementById(`tl-g-${key}`)?.querySelector<HTMLButtonElement>(".tl-gh-tog");
    if (tog?.getAttribute("aria-expanded") === "false") tog.click();
    // Land the group's header just under the sticky filter row (or the pinned crumbs on phones).
    window.setTimeout(() => {
      const el = document.getElementById(`tl-g-${key}`);
      const pane = scrollPane(el);
      if (el && pane) pane.scrollBy({ top: el.getBoundingClientRect().top - pane.getBoundingClientRect().top - 56, behavior: flash ? "auto" : "smooth" });
      if (el && flash) {
        el.classList.add("is-flash");
        window.setTimeout(() => el.classList.remove("is-flash"), 1600);
      }
    }, 0);
  };
  // Back from a phase page: land on that phase's group and flash it once.
  const selKey = groups.find((g) => g.phase?.id === sel)?.key;
  useEffect(() => {
    if (selKey) jump(selKey, true);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selKey]);
  // Wide: rail | timeline | decisions, each scrolling on its own. Medium: rail over decisions
  // in one left column. Narrow: one column (rail strip, timeline, decisions).
  return (
    <div className="tv">
      <div className="tv-side">
        <PhaseRail t={t} p={p} groups={groups} sel={sel} onJump={jump} />
        <Decisions root={root} t={t} />
      </div>
      <div className="tv-main">
        <Timeline d={d} groups={groups} ix={ix} sel={sel} />
      </div>
    </div>
  );
}

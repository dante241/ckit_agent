// Pinned region of the task / phase page, kept to ~170 px: one header bar (back, crumbs with
// the task title, status, model, key numbers, the actions for the state, Tài liệu), one live
// line (what happens now, or what the boss must do; expands for the details) and, on the
// phase page, a one-row phase switcher. Everything below scrolls.
import { useState, type ReactNode } from "react";
import type { BoardDetail, BoardPhases, BoardTask, PhaseBoard } from "./api";
import { acGateOf, askingYou, asker } from "./Asks";
import { isNew, pauseLabel } from "./Interject";
import { STAGE_WHO, StatusPill, TaskActions, clock, isTerminal, money, needLabel } from "./Board";
import { ContinuePlan } from "./ContinuePlan";
import { stepsOf } from "./Steps";
import { WorkflowChip } from "./Workflows";
import { PmsSourceChip } from "./PmsPicker";
import { ModelChip } from "./ModelChip";
import { Glyph } from "./icons";
import { Inline, Markdown } from "./markdown";
import { stepLabel } from "./Epic";
import { useTaskUi } from "./Timeline";
import { hhmm, msgKind, nowActivity, runSecs, runStep, secsBetween, secsText, stepName } from "./runs";

export type Crumb = { label: ReactNode; title?: string; onClick?: () => void; strong?: boolean };

/** One pill: what the boss must do, else what is happening. */
function Pill({ t }: { t: BoardTask }) {
  if (!isTerminal(t) && (t.status === "waiting" || askingYou(t))) return <span className="tag warn">{needLabel(t)}</span>;
  if (t.status === "failed") return <span className="tag err">Lỗi, cần chạy lại</span>;
  return <StatusPill status={t.status} stage={t.stage} paused={pauseLabel(t)} />;
}

/** Back + crumbs; the bar on its own while the task loads. */
export function Crumbs({ items, onBack }: { items: Crumb[]; onBack: () => void }) {
  return (
    <div className="tp-crumbline">
      <button className="tp-back" onClick={onBack} title="Lên một cấp (Esc)" aria-label="Quay lại (Esc)">←</button>
      <nav aria-label="Đường dẫn" className="tp-crumbnav">
        <ol className="tp-crumbs">
          {items.map((c, i) => {
            const last = i === items.length - 1;
            const cls = `tp-crumb${c.strong ? " is-task" : ""}${last ? " is-here" : ""}`;
            return (
              <li key={i} className={c.strong ? "is-task" : undefined}>
                {last || !c.onClick ? (
                  <span aria-current={last ? "page" : undefined} className={cls} title={c.title}>{c.label}</span>
                ) : (
                  <button className={cls} onClick={c.onClick} title={c.title}>{c.label}</button>
                )}
              </li>
            );
          })}
        </ol>
      </nav>
    </div>
  );
}

/** Header bar: crumbs (task title inside), pill, model, numbers; actions + Tài liệu on the right.
 *  The action components keep their own state (composer, confirm); here only their buttons show
 *  in the bar and an open composer drops to a full-width row under it. */
export function TaskBar({ d, p, root, crumbs, onBack, view = null }: { d: BoardDetail; p: BoardPhases | undefined; root: string; crumbs: Crumb[]; onBack: () => void; view?: string | null }) {
  const t = d.task;
  const ui = useTaskUi();
  const phases = p?.phases ?? [];
  const closed = phases.filter((ph) => ph.state === "done").length;
  const ac = phases.reduce((a, ph) => [a[0]! + ph.ac_pass, a[1]! + ph.ac_total], [0, 0]);
  const tasks = phases.length ? phases.reduce((s, ph) => s + ph.tasks.length, 0) : p?.flat.length ?? 0;
  return (
    <header className="tp-bar">
      <div className="tp-bar-main">
        <div className="tp-bar-l1">
          <Crumbs items={crumbs} onBack={onBack} />
          <Pill t={t} />
          {t.status === "running" ? <ModelChip m={t.run_model} /> : null}
          <WorkflowChip t={t} />
          <PmsSourceChip source={t.source} />
          {(t.round ?? 1) > 1 ? <span className="tag info" title="Số vòng làm thêm theo yêu cầu của bạn">Vòng {t.round}</span> : null}
        </div>
        <p className="tp-bar-l2">
          {phases.length ? <span><b>{closed}/{phases.length}</b> phase</span> : null}
          {ac[1] ? <span><b>{ac[0]}/{ac[1]}</b> AC</span> : null}
          {tasks ? <span><b>{tasks}</b> task</span> : null}
          <span><b>{t.runs.length}</b> lần chạy</span>
          <span><b>{money(t.cost)}</b></span>
          <span>{isTerminal(t) ? `${clock(t.created)} → ${clock(t.updated)} (${secsText(secsBetween(t.created, t.updated))})` : `từ ${clock(t.created)}`}</span>
          {t.branch ? <span className="tp-bar-br"><code>{t.branch}</code>{t.base ? <> → <code>{t.base}</code></> : null}</span> : null}
        </p>
      </div>
      <div className="tp-bar-act">
        <TaskActions t={t} />
        <ContinuePlan d={d} root={root} />
        {t.mr ? (
          <a className="tp-docs-btn tp-mr" href={t.mr.url} target="_blank" rel="noreferrer" title={`${t.mr.provider === "github" ? "Pull request" : "Merge request"} vào ${t.mr.target}: ${t.mr.url}`}>
            {t.mr.provider === "github" ? "PR" : "MR"}{/\/(\d+)\/?$/.exec(t.mr.url)?.[1] ? ` ${t.mr.provider === "github" ? "#" : "!"}${/\/(\d+)\/?$/.exec(t.mr.url)![1]}` : ""} ↗
          </a>
        ) : null}
        {/* Under 1400 px these two keep only icon (+ count) so the bar stays one row. */}
        <button className="tp-docs-btn" onClick={() => ui.openGoals()} aria-pressed={view === "goals"} aria-label="Mục tiêu" title="Mục tiêu: AC ↔ task ↔ commit"><Glyph name="check" /><span className="tp-btn-lbl">Mục tiêu</span></button>
        <button className="tp-docs-btn" onClick={() => ui.openDocs()} aria-label={`Tài liệu (${d.docs.length + 1})`} title="Tài liệu"><Glyph name="doc" /><span className="tp-btn-lbl">Tài liệu </span>({d.docs.length + 1})</button>
      </div>
    </header>
  );
}

/** What the gate wants from the boss, in one sentence (the action bar keeps only its buttons). */
function gateLine(t: BoardTask, phase: string | null): string {
  const ph = phase ? ` ${phase}` : "";
  if (t.stage === "approval") {
    return stepsOf(t).includes("dev")
      ? `Kế hoạch${ph} đã qua phản biện. Đọc CONTEXT và PLAN trong Tài liệu rồi duyệt để Dev bắt đầu.`
      : `Kế hoạch${ph} đã qua phản biện. Việc chỉ lập kế hoạch: duyệt và code luôn, hoặc chỉ lưu kế hoạch.`;
  }
  if (t.stage === "acceptance") return t.mr ? "Đã kiểm thử xong. Task đã có MR: nghiệm thu để đóng việc." : "Đã kiểm thử xong. Đọc REPORT trong Tài liệu rồi nghiệm thu: agent đẩy nhánh và tạo MR bằng skill của dự án.";
  if (t.stage === "investigate") return "Leader đã trả lời. Hỏi tiếp, hoặc tiếp tục / đóng.";
  return "Leader đang chờ bạn trả lời.";
}

/** The live line: one row of state; click opens recent actions, files, open questions,
 *  unanswered notes, the Leader's report or the "Sếp cần quyết" list. */
export function LiveLine({ d, p }: { d: BoardDetail; p: BoardPhases | undefined }) {
  const t = d.task;
  const ui = useTaskUi();
  const [open, setOpen] = useState(false);
  const last = t.runs[t.runs.length - 1];
  const liveIdx = last && last.ended === null ? t.runs.length - 1 : -1;
  const lr = liveIdx >= 0 ? last : undefined;
  // An action the boss confirmed runs while the task waits at acceptance or is already done.
  const running = (t.status === "running" || lr?.stage === "action") && lr;
  const recent = running ? d.now?.recent ?? [] : [];
  const files = running ? d.now?.files ?? lr?.digest?.files ?? [] : [];
  const act = nowActivity(d.now);
  const answering = (t.asks ?? []).filter((a) => a.status === "answering");
  const unanswered = isTerminal(t) ? [] : d.unanswered ?? [];
  const decide = t.status === "done" || t.stage === "acceptance" ? d.report?.decide ?? [] : [];
  const leaderMsg = t.status === "waiting" && !pauseLabel(t) ? [...t.thread].reverse().find((m) => msgKind(m) === "leader" || msgKind(m) === "report") : undefined;
  const lastSys = [...t.thread].reverse().find((m) => msgKind(m) === "system");
  const report = d.docs.find((x) => /REPORT/i.test(x.name))?.name;
  const unclosed = t.status === "done" ? (p?.phases ?? []).filter((ph) => ph.state !== "done") : [];
  const phase = lr?.phase ?? d.now?.phase ?? d.phase;

  let tone = "idle";
  let icon: ReactNode = null;
  let text: ReactNode = null;
  let btn: ReactNode = null;
  if (running) {
    tone = "live";
    icon = <span className="pulse" aria-hidden="true" />;
    text = (
      <>
        <b>{runStep(lr)}{lr.phase ? ` ${lr.phase}` : ""}</b> · {STAGE_WHO[lr.stage]} · {secsText(runSecs(lr))}
        {t.note === "đang xin tạm dừng…" ? <> · <span className="tp-live-warn">đang xin tạm dừng</span></> : null}
        {" · "}
        {act ? <span className="tp-live-act">{act}</span> : <span className="faint">chưa có thao tác nào</span>}
      </>
    );
    btn = <button className="btn-sm" onClick={() => ui.openLog(liveIdx)}><Glyph name="log" />Xem log trực tiếp</button>;
  } else if (askingYou(t)) {
    tone = "need";
    icon = <span className="tp-live-ico is-need" aria-hidden="true">?</span>;
    text = <><b>Team đang hỏi bạn.</b> Trả lời ngay bên dưới để họ làm tiếp.</>;
  } else if (t.status === "failed") {
    tone = "err";
    icon = <span className="tp-live-ico is-err" aria-hidden="true">!</span>;
    const why = last?.fail_reason ?? last?.digest?.last_error ?? t.note;
    text = <><b>Bước {stepName(t.stage)} bị lỗi.</b> {why ? <Inline text={why} /> : "Xem log rồi chạy lại."}</>;
    btn = last ? <button className="btn-sm" onClick={() => ui.openLog(t.runs.length - 1)}><Glyph name="log" />Xem log</button> : null;
  } else if (pauseLabel(t)) {
    icon = <span className="tp-live-ico" aria-hidden="true">‖</span>;
    text = isNew(t)
      ? <><b>Chưa bắt đầu.</b> Thêm chỉ dẫn nếu cần, rồi bấm Bắt đầu.</>
      : <><b>Tạm dừng</b> ở {stepName(t.stage)}{phase ? ` ${phase}` : ""}{last?.ended ? ` lúc ${hhmm(last.ended)}` : ""}. Bấm Tiếp tục để làm tiếp từ chỗ đã dừng.</>;
  } else if (acGateOf(t)) {
    tone = "need";
    icon = <span className="tp-live-ico is-need" aria-hidden="true">◆</span>;
    text = <><b>{t.note ?? "Chờ bạn duyệt thay đổi AC"}.</b> Duyệt hoặc không duyệt từng thẻ bên dưới; phase chỉ đóng khi bạn duyệt hết.</>;
    btn = <button className="btn-sm" onClick={() => ui.openGoals(t.ac_gate?.phase)}><Glyph name="check" />Xem mục tiêu</button>;
  } else if (t.status === "waiting") {
    tone = "need";
    icon = <span className="tp-live-ico is-need" aria-hidden="true">◆</span>;
    text = <><b>{needLabel(t)}{phase ? ` ${phase}` : ""}.</b> {gateLine(t, phase)}</>;
  } else if (t.status === "queued") {
    icon = <span className="tp-live-ico" aria-hidden="true">○</span>;
    text = <><b>Đang chờ tới lượt chạy.</b>{t.note ? ` ${t.note}` : ""}</>;
  } else if (t.status === "done") {
    tone = "done";
    icon = <span className="tp-live-ico is-ok" aria-hidden="true">✓</span>;
    const accepted = t.thread.some((m) => msgKind(m) === "boss" && /nghiệm thu/i.test(m.text));
    text = (
      <>
        <b>{accepted ? "Đã nghiệm thu" : "Đã xong"} lúc {clock(t.updated)}.</b> {lastSys ? lastSys.text : null}
        {unclosed.length ? ` ${unclosed.map((ph) => `${ph.id} chưa đóng${ph.ac_total ? ` (AC ${ph.ac_pass}/${ph.ac_total})` : ""}`).join(", ")}.` : ""}
        {d.can_continue ? ` Kế hoạch đã duyệt còn trên nhánh ${t.branch ?? ""}: bấm "Code theo kế hoạch này" để Dev làm tiếp.` : ""}
      </>
    );
    btn = report ? <button className="btn-sm" onClick={() => ui.openDocs(report)}><Glyph name="doc" />Báo cáo cuối</button> : null;
  } else if (t.status === "cancelled") {
    icon = <span className="tp-live-ico" aria-hidden="true">×</span>;
    text = <b>Đã huỷ lúc {clock(t.updated)}.</b>;
  }

  const more = recent.length + files.length + answering.length + unanswered.length + decide.length + (leaderMsg ? 1 : 0);
  const chips: ReactNode[] = [];
  if (unanswered.length) chips.push(<span key="u" className="tag warn">{unanswered.length} tin chưa phản hồi</span>);
  if (decide.length) chips.push(<span key="d" className="tag warn">{decide.length} điểm bạn cần quyết</span>);
  if (answering.length) chips.push(<span key="a" className="tag info">{answering.length} câu Leader đang trả lời</span>);

  return (
    <section className={`tp-live is-${tone} ${open ? "is-open" : ""}`} aria-label="Bây giờ">
      <div className="tp-live-row">
        {icon}
        <p className="tp-live-text" title={running && act ? act : undefined}>{text}</p>
        {chips}
        {btn}
        {more ? (
          <button className={`btn-sm ghost btn-more ${open ? "is-open" : ""}`} aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Thu gọn" : "Chi tiết"}<Glyph name="chevron" /></button>
        ) : null}
      </div>
      {open && more ? (
        <div className="tp-live-more">
          {recent.length ? (
            <div className="tp-live-sec">
              <h4>Thao tác gần nhất</h4>
              <ol className="tp-live-list">
                {recent.slice(-5).map((r, i) => <li key={i}>{r.at ? <time className="faint">{hhmm(r.at)}</time> : null} {r.tool ? <b>{r.tool}</b> : null} {r.tool ? r.text : <span className="muted">“{r.text}”</span>}</li>)}
              </ol>
            </div>
          ) : null}
          {files.length ? (
            <div className="tp-live-sec">
              <h4>Tệp đã sửa</h4>
              <p className="tl-out">{files.slice(0, 12).map((f) => <span key={f} className="tl-file" title={f}>{f.split("/").pop()}</span>)}{files.length > 12 ? <span>+{files.length - 12}</span> : null}</p>
            </div>
          ) : null}
          {answering.length ? (
            <div className="tp-live-sec">
              <h4>Câu hỏi đang mở</h4>
              <ul className="tp-live-list">
                {answering.map((a) => <li key={a.id}><b>{a.id}</b> {asker(a.from)} hỏi Leader, Leader đang trả lời: <span className="muted">{a.question.slice(0, 160)}{a.question.length > 160 ? "…" : ""}</span></li>)}
              </ul>
            </div>
          ) : null}
          {unanswered.length ? (
            <div className="tp-live-sec">
              <h4>Tin bạn nhắn chưa có phản hồi</h4>
              <ul className="tp-live-list">{unanswered.map((u) => <li key={u.at}><time className="faint">{hhmm(u.at)}</time> “{u.text}”</li>)}</ul>
            </div>
          ) : null}
          {leaderMsg ? (
            <div className="tp-live-sec">
              <h4>Leader báo lúc {hhmm(leaderMsg.at)}</h4>
              <Markdown source={leaderMsg.text} />
            </div>
          ) : null}
          {decide.length ? (
            <div className="tp-live-sec tp-decide">
              <h4>Còn {decide.length} điểm bạn cần quyết (từ REPORT.md)</h4>
              <ol>{decide.map((x, i) => <li key={i}><Inline text={x} /></li>)}</ol>
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

/** Phase switcher in one row: Tổng quan, then each phase with its state. */
export function PhaseNav({ t, phases, sel, onPick }: { t: BoardTask; phases: PhaseBoard[]; sel: string | null; onPick: (id: string | null) => void }) {
  const now = (ph: PhaseBoard) => {
    if (ph.state === "done") return ph.ac_total ? `Xong · AC ${ph.ac_pass}/${ph.ac_total}` : "Xong";
    if (ph.state === "todo") return "Chưa làm";
    const step = ph.steps.find((s) => s.state === "now")?.key ?? t.stage;
    return pauseLabel(t) ? `Tạm dừng: ${stepLabel(step)}` : `Đang: ${stepLabel(step)}`;
  };
  return (
    <nav className="tp-phnav" aria-label="Phase">
      <button className="tp-phnav-i is-ov" onClick={() => onPick(null)}>← Tổng quan</button>
      {phases.map((ph) => (
        <button key={ph.id} className={`tp-phnav-i is-${ph.state}${ph.id === sel ? " is-on" : ""}`} aria-current={ph.id === sel ? "page" : undefined} onClick={() => onPick(ph.id)} title={ph.name}>
          <span className="tp-phnav-ico" aria-hidden="true">{ph.state === "done" ? "✓" : ph.state === "active" ? "●" : "○"}</span>
          <span className="mono">{ph.id}</span>
          <span className="tp-phnav-s">{now(ph)}</span>
        </button>
      ))}
    </nav>
  );
}

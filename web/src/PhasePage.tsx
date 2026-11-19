// Phase page (direction B, `?phase=M1`). The phase switcher sits in the pinned region
// (TaskTop.PhaseNav); here: title, goal, numbers, results, then "Thực thi" (the phase's step
// pipeline with where it is now, a stage-relative banner, the sub-task table grouped by state;
// a row opens the sub-task drawer `&sub=T4`), then the runs table; beside it AC, questions,
// decisions and the phase's documents.
import { useMemo, useState, type KeyboardEvent } from "react";
import type { BoardDecision, BoardDetail, BoardPhases, BoardRun, BoardStage, PhaseBoard, SubTaskRow } from "./api";
import { Drawer } from "./ui";
import { ModelChip } from "./ModelChip";
import { Inline } from "./markdown";
import { Glyph } from "./icons";
import { STAGE_WHO, money, verdictTone } from "./Board";
import { pauseLabel } from "./Interject";
import { stepsOf } from "./Steps";
import { SubFacts, doneN, stepLabel } from "./Epic";
import { AskBlock, Clamp, DecisionList, buildGroups, indexTask, phaseStats, subText, useDecisions, useTaskUi, type Group, type TaskIndex } from "./Timeline";
import { hhmm, msgKind, nowActivity, runFailed, runLive, runSecs, runTitle, secsBetween, secsText, verdictText } from "./runs";

const STEP: Partial<Record<BoardStage, string>> = { solution: "Kế hoạch", review: "Phản biện", approval: "Bạn duyệt", dev: "Phát triển", test: "Kiểm thử", security: "Bảo mật" };
const STATUS: Record<SubTaskRow["status"], { label: string; tone: string }> = {
  todo: { label: "Chưa làm", tone: "" },
  in_progress: { label: "Đang làm", tone: "info" },
  blocked: { label: "Bị chặn", tone: "err" },
  done: { label: "Xong", tone: "ok" },
};
const GROUPS: SubTaskRow["status"][] = ["in_progress", "blocked", "todo", "done"];

type Ctx = { d: BoardDetail; p: BoardPhases; ix: TaskIndex; groups: Group[]; decisions: BoardDecision[] };

/** The phase's time window [started, ended or now]. */
function windowOf(ctx: Ctx, ph: PhaseBoard): [number, number] | null {
  const s = phaseStats(ctx.d.task, ph);
  if (!s.started) return null;
  return [Date.parse(s.started), s.ended ? Date.parse(s.ended) : Date.now()];
}

/** How the plan of the phase got approved: by the boss, or by the coordinator on their behalf. */
function approvalIn(ctx: Ctx, ph: PhaseBoard): { by: "boss" | "coordinator"; at: string } | null {
  const w = windowOf(ctx, ph);
  if (!w) return null;
  const inW = (at: string) => Date.parse(at) >= w[0] && Date.parse(at) <= w[1];
  const m = ctx.ix.msgs.find((x) => inW(x.at) && (msgKind(x) === "delegated_approval" || (msgKind(x) === "boss" && /duyệt/i.test(x.text))));
  return m ? { by: msgKind(m) === "delegated_approval" ? "coordinator" : "boss", at: m.at } : null;
}

/** Short verdict word for the attempt history ("#1 Bị ngắt · #2 Đạt"). */
function attemptWord(r: BoardRun): { text: string; tone: string } {
  if (runLive(r)) return { text: "Đang chạy", tone: "info" };
  if (r.verdict) return { text: verdictText(r.verdict), tone: verdictTone(r.verdict) };
  if (r.exit !== null && r.exit !== 0) return { text: "Lỗi", tone: "err" };
  return { text: "Xong", tone: "" };
}

const base = (p: string) => p.split("/").filter(Boolean).pop() ?? p;

/** One run of the phase: name + attempt, who, verdict, times, model, summary, why it was sent
 *  back, and chips that open its questions / files / commits inline; "Xem log" opens the drawer. */
function RunItem({ ctx, i }: { ctx: Ctx; i: number }) {
  const ui = useTaskUi();
  const r = ctx.d.task.runs[i]!;
  const [open, setOpen] = useState<"asks" | "files" | "commits" | null>(null);
  const fail = runFailed(r);
  const live = runLive(r);
  const v = attemptWord(r);
  const asks = ctx.ix.asks.get(i) ?? [];
  const files = r.digest?.files ?? [];
  const commits = r.commits ?? [];
  const sub = subText(r);
  // The reason box already shows the line a failed review leads with.
  const summary = (r.summary ?? []).filter((s) => s && !(fail && r.fail_reason && s.trim() === r.fail_reason.trim())).slice(0, 3);
  const chip = (k: NonNullable<typeof open>, label: string) => (
    <button className={`chip ${open === k ? "is-on" : ""}`} aria-expanded={open === k} onClick={() => setOpen(open === k ? null : k)}>{label}</button>
  );
  return (
    <li className={`pv-run ${fail ? "is-fail" : ""} ${live ? "is-live" : ""}`}>
      <div className="pv-run-h">
        <b className="pv-run-name">{live ? <span className="pulse" aria-hidden="true" /> : null}{runTitle(r)}</b>
        <span className="pv-run-who">{STAGE_WHO[r.stage]}</span>
        <span className={`tag ${v.tone}`}>{v.text}</span>
        <span className="pv-run-t">
          <time dateTime={r.started}>{hhmm(r.started)}</time> → {r.ended ? <time dateTime={r.ended}>{hhmm(r.ended)}</time> : "nay"}
          <b>{secsText(runSecs(r))}</b>
          <b>{money(r.cost)}</b>
        </span>
      </div>
      {r.model || r.digest ? (
        <div className="pv-run-m">
          <ModelChip m={r.model} />
          {r.digest ? <span>{r.digest.tools} lệnh · {r.digest.errors} lỗi{r.digest.harness_errors ? ` (+${r.digest.harness_errors} do harness)` : ""}</span> : null}
          {r.summary_source === "log" && summary.length ? <span className="tag" title="Bước không để lại lời nhắn: tóm tắt lấy từ log">tóm tắt từ log</span> : null}
        </div>
      ) : null}
      {fail && r.fail_reason ? <div className="pv-run-why"><b>Lý do</b><Clamp lines={3}><Inline text={r.fail_reason} /></Clamp></div> : null}
      {summary.length ? (
        <Clamp lines={3} className="pv-run-sum"><ul>{summary.map((s, k) => <li key={k}><Inline text={s} /></li>)}</ul></Clamp>
      ) : live ? null : <p className="pv-run-none">Chưa có tóm tắt.</p>}
      <div className="pv-run-f">
        {asks.length ? chip("asks", `${asks.length} câu hỏi`) : null}
        {files.length ? chip("files", `${files.length} tệp`) : null}
        {commits.length ? chip("commits", `${commits.length} commit`) : null}
        {sub ? <span className="chip is-static" title={r.digest?.subagents_list?.map((x) => x.name).join(", ")}>{sub}</span> : null}
        <button className="btn-sm pv-run-log" onClick={() => ui.openLog(i)}><Glyph name="log" />{live ? "Xem log trực tiếp" : "Xem log"}</button>
      </div>
      {open === "asks" ? <div className="pv-run-open">{asks.map((a) => <AskBlock key={a.id} a={a} />)}</div> : null}
      {open === "files" ? <p className="pv-run-open tl-out">{files.map((f) => <span key={f} className="tl-file" title={f}>{base(f)}</span>)}</p> : null}
      {open === "commits" ? (
        <ul className="pv-run-open pv-commits">{commits.map((c) => <li key={c.sha}><code>{c.sha.slice(0, 10)}</code> {c.subject}</li>)}</ul>
      ) : null}
    </li>
  );
}

/** The phase's runs grouped by step in pipeline order, each step with its attempt history;
 *  approvals sit in their own step. */
function RunList({ ctx, ph }: { ctx: Ctx; ph: PhaseBoard }) {
  const t = ctx.d.task;
  const entries = ctx.groups.filter((g) => g.phase?.id === ph.id).flatMap((g) => g.entries);
  const runIdx = entries.flatMap((e) => (e.kind === "run" ? [e.i] : []));
  if (!runIdx.length) return <section className="pv-sec" aria-label="Các lần chạy"><h3>Các lần chạy</h3><p className="bd-empty">{ph.id} chưa có lần chạy nào.</p></section>;
  const gates = entries.flatMap((e) => (e.kind === "msg" && (msgKind(e.m) === "delegated_approval" || (msgKind(e.m) === "boss" && /duyệt/i.test(e.m.text))) ? [e.m] : []));
  const keys: BoardStage[] = [...ph.steps.map((s) => s.key)];
  for (const i of runIdx) if (!keys.includes(t.runs[i]!.stage)) keys.push(t.runs[i]!.stage);
  return (
    <section className="pv-sec" aria-label="Các lần chạy">
      <h3>Các lần chạy <span className="pv-c">{runIdx.length} lần{gates.length ? ` + ${gates.length} lần duyệt` : ""}</span></h3>
      <ol className="pv-steps">
        {keys.map((k) => {
          const mine = runIdx.filter((i) => t.runs[i]!.stage === k);
          if (k === "approval") {
            if (!gates.length) return null;
            return (
              <li key={k} className="pv-stepg">
                <div className="pv-stepg-h"><span className="pv-stepg-ico is-ok" aria-hidden="true">✓</span><b>{STEP[k]}</b></div>
                <ul className="pv-gates">
                  {gates.map((m) => {
                    const coord = msgKind(m) === "delegated_approval";
                    return (
                      <li key={m.at} className={coord ? "is-coord" : ""}>
                        <span className={`tag ${coord ? "warn" : "accent"}`}>{coord ? "Điều phối thay bạn" : "Bạn"}</span>
                        <span className="faint">{hhmm(m.at)}</span>
                        <Clamp lines={2}><span>{m.text}</span></Clamp>
                      </li>
                    );
                  })}
                </ul>
              </li>
            );
          }
          if (!mine.length) return null;
          const rs = mine.map((i) => t.runs[i]!);
          const lastR = rs[rs.length - 1]!;
          const state = runLive(lastR) ? "live" : runFailed(lastR) ? "fail" : "ok";
          return (
            <li key={k} className={`pv-stepg is-${state}`}>
              <div className="pv-stepg-h">
                <span className={`pv-stepg-ico is-${state}`} aria-hidden="true">{state === "live" ? "●" : state === "fail" ? "✕" : "✓"}</span>
                <b>{STEP[k] ?? stepLabel(k)}</b>
                <span className="pv-attempts">
                  {rs.map((r) => { const v = attemptWord(r); return <span key={r.n} className={`tag ${v.tone}`}>#{r.n} {v.text}</span>; })}
                </span>
                <span className="pv-stepg-t">{secsText(rs.reduce((a, r) => a + runSecs(r), 0))} · {money(rs.reduce((a, r) => a + r.cost, 0))}</span>
              </div>
              <ol className="pv-runs">{mine.map((i) => <RunItem key={i} ctx={ctx} i={i} />)}</ol>
            </li>
          );
        })}
      </ol>
    </section>
  );
}


/** The phase's run, live right now (the task is running and its last run belongs to the phase). */
function liveRun(ctx: Ctx, ph: PhaseBoard): { i: number; r: BoardRun } | null {
  const t = ctx.d.task;
  const i = t.runs.length - 1;
  const r = t.runs[i];
  return r && runLive(r) && t.status === "running" && r.phase === ph.id ? { i, r } : null;
}

/** Kế hoạch → Phản biện → Bạn duyệt → Phát triển → Kiểm thử: done ✓ with time/cost, the current
 *  step highlighted with who + elapsed, upcoming greyed. */
function Flow({ ctx, ph }: { ctx: Ctx; ph: PhaseBoard }) {
  const t = ctx.d.task;
  const s = phaseStats(t, ph);
  const rs = s.runs.map((i) => t.runs[i]!);
  const live = liveRun(ctx, ph);
  const appr = approvalIn(ctx, ph);
  const paused = pauseLabel(t);
  const firstTodo = ph.steps.findIndex((x) => x.state === "todo");
  const planOnly = !stepsOf(t).includes("dev");
  return (
    <ol className="pv-flow" aria-label={`Các bước của ${ph.id}`}>
      {ph.steps.map((st, k) => {
        const mine = rs.filter((r) => r.stage === st.key);
        const secs = mine.reduce((a, r) => a + runSecs(r), 0);
        const cost = mine.reduce((a, r) => a + r.cost, 0);
        const name = st.key === "approval" && appr?.by === "coordinator" ? "Duyệt thay bạn" : STEP[st.key] ?? stepLabel(st.key);
        let meta = "";
        let warn = "";
        if (st.state === "done") {
          meta = st.key === "approval" ? (appr ? `${appr.by === "coordinator" ? "Điều phối" : "Bạn"} · ${hhmm(appr.at)}` : "Đã duyệt") : `${mine.length > 1 ? `${mine.length} lần · ` : ""}${secs ? secsText(secs) : ""}${cost ? ` · ${money(cost)}` : ""}`;
        } else if (st.state === "now") {
          const who = STAGE_WHO[st.key];
          const round = mine.length > 1 ? `Lần ${mine.length} · ` : "";
          meta = st.key === "approval" ? (t.status === "waiting" ? "Chờ bạn duyệt" : "Đang duyệt")
            : live && live.r.stage === st.key ? `${round}${who} · ${secsText(runSecs(live.r))}`
            : paused ? `${round}Tạm dừng` : t.status === "queued" ? `${round}Chờ tới lượt` : `${round}${who}`;
        } else {
          meta = k === firstTodo ? "Tiếp theo" : "";
        }
        if (st.key === "solution" && s.back) warn = `${s.back} lần trả lại`;
        if (st.key === "review" && mine.filter(runFailed).length) warn = `${mine.filter(runFailed).length} lần không đạt`;
        return (
          <li key={st.key} className={`pv-fs is-${st.state}${st.key === "approval" && appr?.by === "coordinator" ? " is-coord" : ""}`} aria-current={st.state === "now" ? "step" : undefined}>
            <span className="pv-fs-h">
              <span className="pv-fs-ico" aria-hidden="true">{st.state === "done" ? "✓" : st.state === "now" ? (live ? <span className="pulse" /> : "●") : "○"}</span>
              {name}
            </span>
            {meta ? <span className="pv-fs-m">{meta}</span> : null}
            {warn ? <span className="pv-fs-w">{warn}</span> : null}
          </li>
        );
      })}
      {planOnly ? <li className="pv-fs is-off"><span className="pv-fs-h">Không có Phát triển</span><span className="pv-fs-m">việc chỉ lập kế hoạch</span></li> : null}
    </ol>
  );
}

/** One sentence over the table while nobody codes yet: the rows are a plan, not work in progress. */
function preDevBanner(ctx: Ctx, ph: PhaseBoard): string | null {
  const t = ctx.d.task;
  const s = phaseStats(t, ph);
  const coding = ph.steps.some((x) => (x.key === "dev" || x.key === "test" || x.key === "security") && x.state !== "todo")
    || ph.tasks.some((x) => x.status !== "todo") || s.runs.some((i) => t.runs[i]!.stage === "dev");
  if (coding || ph.state === "done") return null;
  if (!stepsOf(t).includes("dev")) return "Việc này chỉ lập kế hoạch: các task dưới đây là kế hoạch, chưa ai code.";
  const now = ph.steps.find((x) => x.state === "now")?.key;
  if (ph.state === "todo") return "Phase chưa bắt đầu: task hiện ra khi Leader viết kế hoạch cho phase này.";
  if (now === "solution") {
    const n = s.runs.filter((i) => t.runs[i]!.stage === "solution").length;
    return `Chờ Dev (kế hoạch chưa duyệt): Leader + BA đang viết kế hoạch${n > 1 ? ` lần ${n}` : ""}, danh sách task có thể còn đổi.`;
  }
  if (now === "review") return "Chờ Dev (kế hoạch chưa duyệt): Reviewer đang phản biện kế hoạch.";
  if (now === "approval") return "Chờ Dev: kế hoạch chờ bạn duyệt. Duyệt xong Dev mới bắt đầu.";
  return "Chờ Dev: kế hoạch đã duyệt, Dev sắp bắt đầu.";
}


function SubRow({ s, ctx, ph, pre, open }: { s: SubTaskRow; ctx: Ctx; ph: PhaseBoard; pre: boolean; open: () => void }) {
  const t = ctx.d.task;
  const live = liveRun(ctx, ph);
  const devs = phaseStats(t, ph).runs.map((i) => t.runs[i]!).filter((r) => r.stage === "dev");
  const lastDev = devs[devs.length - 1];
  const st = STATUS[s.status];
  const files = s.files.split(/,\s*/).filter(Boolean);
  const who = pre ? "–"
    : s.status === "done" ? (lastDev ? `Dev · ${runTitle(lastDev)}` : "Dev")
    : s.status === "in_progress" ? `Dev · ${live && live.r.stage === "dev" ? secsText(runSecs(live.r)) : "đang làm"}`
    : "Dev";
  const act = nowActivity(ctx.d.now);
  const sub = s.status === "in_progress" && live && live.r.stage === "dev" && act ? act
    : s.status === "blocked" ? (s.note || "Bị chặn, xem chi tiết")
    : s.retries > 0 ? `verify lỗi ${s.retries} lần${s.note ? `: ${s.note}` : ""}` : "";
  const onKey = (e: KeyboardEvent) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), open());
  return (
    <tr className={`pv-trow is-${s.status}`} onClick={open} onKeyDown={onKey} tabIndex={0} aria-label={`${s.key}: ${s.title}`}>
      <td className="mono pv-tkey">{s.key}</td>
      <td className="pv-ttask">
        <span className="pv-tt" title={s.detail || s.title}><Inline text={s.title || s.key} /></span>
        {sub ? <span className={`pv-tsub ${s.status === "blocked" ? "is-err" : ""}`}>{sub}</span> : null}
      </td>
      <td className="pv-files hide-m" title={s.files}>{files.length ? <>{base(files[0]!)}{files.length > 1 ? <span className="faint"> +{files.length - 1}</span> : null}</> : <span className="faint">–</span>}</td>
      <td className="hide-m"><span className="pv-twho">{who}</span></td>
      <td>{pre ? <span className="faint">–</span> : <span className={`tag ${st.tone}`}>{st.label}</span>}</td>
      <td className="hide-m">{s.commit ? <code>{s.commit.slice(0, 10)}</code> : <span className="faint">–</span>}</td>
    </tr>
  );
}

/** "Thực thi": pipeline, banner, sub-tasks grouped Đang làm / Bị chặn / Chưa làm / Xong. */
function Exec({ ctx, ph }: { ctx: Ctx; ph: PhaseBoard }) {
  const ui = useTaskUi();
  const tasks = ph.tasks;
  const pre = preDevBanner(ctx, ph);
  const by = GROUPS.map((g) => ({ g, list: tasks.filter((x) => x.status === g) })).filter((x) => x.list.length);
  const onlyDone = by.length === 1 && by[0]!.g === "done";
  const [shut, setShut] = useState<Record<string, boolean>>({});
  const isShut = (g: string) => shut[g] ?? (g === "done" && !onlyDone);
  return (
    <section className="pv-sec pv-exec" aria-label="Thực thi">
      <h3>
        Thực thi <span className="pv-c">{doneN(tasks)}/{tasks.length} task xong</span>
        {ctx.p.live && ph.state === "active" ? <span className="ep-live">trực tiếp từ engine</span> : null}
      </h3>
      <Flow ctx={ctx} ph={ph} />
      {pre ? <p className="pv-banner">{pre}</p> : null}
      {tasks.length === 0 ? (
        <p className="bd-empty">Chưa có task. Task hiện ra khi Leader viết kế hoạch cho {ph.id}.</p>
      ) : (
        <div className="pv-tbl">
          <table className="pv-tasks">
            <thead><tr><th>#</th><th>Task</th><th className="hide-m">File</th><th className="hide-m">Người làm</th><th>Trạng thái</th><th className="hide-m">Commit</th></tr></thead>
            {by.map(({ g, list }) => (
              <tbody key={g} className={`pv-grp is-${g}`}>
                {by.length > 1 || g === "done" ? (
                  <tr className="pv-grp-h">
                    <th colSpan={6}>
                      <button className="pv-grp-tog" aria-expanded={!isShut(g)} onClick={() => setShut({ ...shut, [g]: !isShut(g) })}>
                        <span aria-hidden="true">{isShut(g) ? "▸" : "▾"}</span> {pre && g === "todo" ? "Trong kế hoạch" : STATUS[g].label} <span className="pv-c">{list.length}</span>
                      </button>
                    </th>
                  </tr>
                ) : null}
                {isShut(g) ? null : list.map((s) => <SubRow key={s.key} s={s} ctx={ctx} ph={ph} pre={Boolean(pre)} open={() => ui.openSub(ph.id, s.key)} />)}
              </tbody>
            ))}
          </table>
        </div>
      )}
    </section>
  );
}

function PhaseMain({ ctx, ph }: { ctx: Ctx; ph: PhaseBoard }) {
  const t = ctx.d.task;
  const s = phaseStats(t, ph);
  const asks = s.runs.reduce((n, i) => n + (ctx.ix.asks.get(i)?.length ?? 0), 0);
  const now = ph.steps.find((x) => x.state === "now");
  return (
    <div className="pv-main">
      <div className="pv-title">
        <h2><span className="pv-title-k">Phase <span className="mono">{ph.id}</span></span>{ph.name ? <> · {ph.name}</> : null}</h2>
        {ph.state === "done" ? <span className="tag ok">✓ Đã đóng{s.ended ? ` ${hhmm(s.ended)}` : ""}</span> : ph.state === "active" ? <span className="tag info">{pauseLabel(t) ? "Tạm dừng" : "Đang làm"}{now ? `: ${stepLabel(now.key)}` : ""}</span> : <span className="tag">Chưa bắt đầu</span>}
      </div>
      {ph.goal ? <p className="pv-goal"><Inline text={ph.goal} /></p> : null}
      <p className="pv-stats">
        {s.started ? <span><b>{hhmm(s.started)} → {s.ended ? hhmm(s.ended) : "nay"}</b> ({secsText(secsBetween(s.started, s.ended))})</span> : null}
        <span><b>{money(s.cost)}</b>{t.cost ? ` (${Math.round((s.cost / t.cost) * 100)}% việc)` : ""}</span>
        <span><b>{s.runs.length}</b> lần chạy</span>
        {s.back ? <span><b>{s.back}</b> lần trả lại</span> : null}
        {asks ? <span><b>{asks}</b> câu hỏi</span> : null}
        {s.subagents ? <span><b>{s.subagents}</b> subagent</span> : null}
      </p>
      {ph.result?.length ? (
        <section className="pv-sec" aria-label="Kết quả">
          <h3>Kết quả <span className="pv-c">trước → sau</span></h3>
          <div className="pv-res">
            {ph.result.map((x, i) => (
              <div key={i} className="pv-kv"><div className="k">{x.label}</div><div className="v">{x.before} → {x.after}</div></div>
            ))}
          </div>
        </section>
      ) : null}
      <Exec ctx={ctx} ph={ph} />
      <RunList ctx={ctx} ph={ph} />
    </div>
  );
}

function PhaseSide({ ctx, ph }: { ctx: Ctx; ph: PhaseBoard }) {
  const ui = useTaskUi();
  const t = ctx.d.task;
  const s = phaseStats(t, ph);
  const asks = s.runs.flatMap((i) => ctx.ix.asks.get(i) ?? []);
  const docs = ctx.d.docs.map((x) => x.name).filter((n) => n.startsWith(`${ph.id}-`) || n === ph.id);
  return (
    <aside className="pv-side">
      <section className="pv-sec" aria-label="Tiêu chí nghiệm thu">
        <h3>Tiêu chí nghiệm thu <button className="btn-sm pv-r" onClick={() => ui.openGoals(ph.id)} title={`AC của ${ph.id} với task, commit và bằng chứng`}>Mục tiêu<Glyph name="next" /></button></h3>
        {ph.ac_total ? (
          <p className="pv-acbig"><b>{ph.ac_pass}/{ph.ac_total}</b> <span className="muted">{ph.state === "done" ? "AC đạt" : "AC đạt tới giờ (phase chưa đóng)"}</span></p>
        ) : (
          <p className="bd-empty">Chưa có bảng AC.</p>
        )}
        {s.notes.length ? (
          <>
            {/* Notes only count as a re-grade once the phase closed; before that they are working notes. */}
            <h4 className="pv-sub-h">{ph.state === "done" ? "AC chấm lại" : "AC ghi chú (chưa chốt)"}</h4>
            <ul className="pv-list">
              {s.notes.map((n) => (
                <li key={n.id}>
                  <span className="pv-list-h"><span className={`tag ${ph.state === "done" ? "warn" : ""}`}>{n.status}</span><b>{n.id}</b></span>
                  <Inline text={n.note} />
                </li>
              ))}
            </ul>
          </>
        ) : null}
      </section>
      <section className="pv-sec" aria-label="Câu hỏi">
        <h3>Câu hỏi {asks.length ? <span className="pv-c">{asks.length}</span> : null}</h3>
        {asks.length ? <div className="pv-asks">{asks.map((a) => <AskBlock key={a.id} a={a} />)}</div> : <p className="bd-empty">Không có câu hỏi trong phase này.</p>}
      </section>
      <section className="pv-sec" aria-label="Quyết định">
        <h3>Quyết định</h3>
        <DecisionList list={ctx.decisions} phase={ph.id} />
      </section>
      <section className="pv-sec" aria-label="Tài liệu phase">
        <h3>Tài liệu phase</h3>
        {docs.length ? (
          <div className="pv-docs">{docs.map((n) => <button key={n} className="chip" onClick={() => ui.openDocs(n)}>{n}</button>)}</div>
        ) : (
          <p className="bd-empty">Chưa có tài liệu riêng của {ph.id}.</p>
        )}
      </section>
    </aside>
  );
}

export function PhasePage({ d, p, root, phase }: { d: BoardDetail; p: BoardPhases; root: string; phase: string }) {
  const ix = useMemo(() => indexTask(d), [d]);
  const groups = useMemo(() => buildGroups(d.task, p.phases, ix), [d.task, p.phases, ix]);
  const dq = useDecisions(root, d.task);
  const ctx: Ctx = { d, p, ix, groups, decisions: dq.data ?? [] };
  const ph = p.phases.find((x) => x.id === phase);
  if (!ph) return <div className="pv"><p className="bd-empty">Không có phase {phase} trong việc này.</p></div>;
  return (
    <div className="pv">
      <div className="pv-body">
        <PhaseMain ctx={ctx} ph={ph} />
        <PhaseSide ctx={ctx} ph={ph} />
      </div>
    </div>
  );
}

/** Sub-task drawer (`&sub=T4`): full text, files, dependencies, AC, verify, retries, note, commit. */
export function SubDrawer({ s, subKey, list, phase, d, onClose, onOpen }: { s: SubTaskRow | undefined; subKey: string; list: SubTaskRow[]; phase: PhaseBoard | null; d: BoardDetail; onClose: () => void; onOpen: (key: string) => void }) {
  const st = s ? STATUS[s.status] : null;
  const devs = d.task.runs.filter((r) => r.stage === "dev" && (!phase || r.phase === phase.id));
  const head = (
    <>
      <p className="dr-kicker"><span className="mono">{subKey}</span>{phase ? <span>{phase.id} {phase.name}</span> : null}{st ? <span className={`tag ${st.tone}`}>{st.label}</span> : null}</p>
      <h3 className="dr-title">{s ? <Inline text={s.title || s.key} /> : subKey}</h3>
      {s && devs.length ? <p className="dr-meta">Người làm: Dev · {devs.map(runTitle).join(", ")}</p> : null}
    </>
  );
  return (
    <Drawer label={`Task ${subKey}`} head={head} onClose={onClose} className="dr-sub">
      {s ? <SubFacts s={s} list={list} onOpen={onOpen} /> : <p className="bd-empty">Không tìm thấy task {subKey}{phase ? ` trong ${phase.id}` : ""}.</p>}
    </Drawer>
  );
}

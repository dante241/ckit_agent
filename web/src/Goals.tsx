// "Mục tiêu" (`&view=goals`): requirement → AC → task → commit → result + evidence, one row
// per AC, grouped by phase. Badges show an AC that was re-graded or reinterpreted and whether the
// boss approved that change; a warnings block lists ACs without a task and tasks without an AC.
// Contract: GET /api/board/:id/goals (BoardGoals).
import { useMemo, useState, type ReactNode } from "react";
import { AskPanel, acGateOf, askingYou } from "./Asks";
import { useQuery } from "@tanstack/react-query";
import { api, type BoardDetail, type BoardGoals, type BoardPhases, type GoalAc } from "./api";
import { EndpointMissing, ErrorState, Loading, isMissing } from "./ui";
import { Inline } from "./markdown";
import { Clamp, useTaskUi } from "./Timeline";
import { Glyph } from "./icons";

export function useGoals(root: string, id: string, live: boolean) {
  return useQuery({
    queryKey: ["board-goals", root, id],
    queryFn: () => api.boardGoals(root, id),
    refetchInterval: (q) => (isMissing(q.state.error) ? false : live ? 10000 : false),
  });
}

const STATUS: Record<GoalAc["status"], { label: string; tone: string }> = {
  pass: { label: "Đạt", tone: "ok" },
  fail: { label: "Không đạt", tone: "err" },
  needs_confirm: { label: "Cần xác nhận", tone: "warn" },
  pending: { label: "Chưa chấm", tone: "" },
};

/** The AC was re-graded / reinterpreted: did the boss approve it? */
function changeBadge(c: NonNullable<GoalAc["changed"]>): { label: string; tone: string; title: string } {
  const when = c.at ? ` lúc ${c.at.slice(11, 16)} ${c.at.slice(8, 10)}/${c.at.slice(5, 7)}` : "";
  if (c.decision === "approved" && c.approved_by === "boss") return { label: "Đổi AC · bạn đã duyệt", tone: "accent", title: `Bạn duyệt${when}${c.ask ? ` (${c.ask})` : ""}` };
  if (c.decision === "rejected") return { label: "Đổi AC · bạn không duyệt", tone: "err", title: `Bạn không duyệt${when}${c.ask ? ` (${c.ask})` : ""}` };
  if (c.decision === "pending") return { label: "Đổi AC · chờ bạn duyệt", tone: "warn", title: c.ask ? `Thẻ duyệt ${c.ask} ở đầu trang` : "Đang chờ bạn duyệt" };
  return { label: "Đổi AC · bạn chưa duyệt", tone: "warn", title: "AC được chấm lại hoặc hiểu lại mà bạn chưa từng duyệt" };
}

/** "M1/T4" → { phase: "M1", key: "T4" } (a bare "T4" has no phase). */
function taskRef(s: string): { phase: string | null; key: string } {
  const i = s.indexOf("/");
  return i > 0 ? { phase: s.slice(0, i), key: s.slice(i + 1) } : { phase: null, key: s };
}

const unapproved = (a: GoalAc) => a.changed !== null && !(a.changed.decision === "approved" && a.changed.approved_by === "boss");
const problem = (a: GoalAc) => a.status !== "pass" || unapproved(a) || a.tasks.length === 0;
// AC ids repeat across phases (M0/AC-07, M1/AC-07).
const rowId = (a: GoalAc) => `gv-${a.phase ?? "x"}-${a.id}`;
function jumpTo(id: string) {
  const el = document.getElementById(id);
  if (!el) return;
  el.scrollIntoView({ block: "center", behavior: "smooth" });
  el.classList.add("is-flash");
  window.setTimeout(() => el.classList.remove("is-flash"), 1600);
}

export function GoalsPage({ d, p, root, phase }: { d: BoardDetail; p: BoardPhases; root: string; phase: string | null }) {
  const t = d.task;
  const q = useGoals(root, t.id, t.status === "running" || t.status === "waiting");
  const cards = askingYou(t) || acGateOf(t) ? <AskPanel task={t} root={root} /> : null;
  return (
    <div className="gv" aria-label="Mục tiêu">
      {isMissing(q.error) ? (
        <EndpointMissing endpoint={`/api/board/${t.id}/goals`} />
      ) : q.error ? (
        <ErrorState message={(q.error as Error).message} />
      ) : !q.data ? (
        <Loading rows={5} />
      ) : (
        <Goals g={q.data} p={p} phase={phase} cards={cards} />
      )}
    </div>
  );
}

function Goals({ g, p, phase, cards }: { g: BoardGoals; p: BoardPhases; phase: string | null; cards: ReactNode }) {
  const ui = useTaskUi();
  const [onlyIssues, setOnlyIssues] = useState(false);
  const [showReq, setShowReq] = useState(false);
  const phaseIds = useMemo(() => {
    const ids = p.phases.map((ph) => ph.id);
    for (const a of g.acs) if (a.phase && !ids.includes(a.phase)) ids.push(a.phase);
    return ids;
  }, [p.phases, g.acs]);
  const inPhase = (x: string | null) => !phase || x === phase;
  const acs = g.acs.filter((a) => inPhase(a.phase) && (!onlyIssues || problem(a)));
  const all = g.acs.filter((a) => inPhase(a.phase));
  const n = (f: (a: GoalAc) => boolean) => all.filter(f).length;
  const pass = n((a) => a.status === "pass");
  const changed = n((a) => a.changed !== null);
  const notOk = n(unapproved);
  const noTask = g.orphans.acs_without_task.filter((s) => inPhase(taskRef(s).phase));
  const noAc = g.orphans.tasks_without_ac.filter((s) => inPhase(taskRef(s).phase));
  const groups = (phase ? [phase] : [...phaseIds, null]).map((id) => ({ id, rows: acs.filter((a) => (id === null ? !a.phase || !phaseIds.includes(a.phase) : a.phase === id)) })).filter((x) => x.rows.length);
  const name = (id: string | null) => (id ? p.phases.find((ph) => ph.id === id)?.name ?? "" : "");
  const reqOf = (uc: string) => g.acs.filter((a) => a.uc === uc).map((a) => a.id);

  return (
    <>
      <div className="gv-head">
        <div className="gv-title">
          <h2>Mục tiêu{phase ? <> · <span className="mono">{phase}</span> {name(phase)}</> : null}</h2>
          <p className="gv-sum">
            <span><b>{pass}/{all.length}</b> AC đạt</span>
            {all.length - pass ? <span className="w"><b>{all.length - pass}</b> chưa đạt</span> : null}
            {changed ? <span><b>{changed}</b> AC đã đổi{notOk ? <>, <b className="w">{notOk}</b> bạn chưa duyệt</> : ""}</span> : null}
            {g.requirements.length ? <span><b>{g.requirements.length}</b> yêu cầu</span> : null}
          </p>
        </div>
        <div className="gv-tools">
          <div className="seg" role="group" aria-label="Lọc theo phase">
            <button className={!phase ? "seg-on" : ""} aria-pressed={!phase} onClick={() => ui.openGoals(null)}>Tất cả</button>
            {phaseIds.map((id) => (
              <button key={id} className={phase === id ? "seg-on" : ""} aria-pressed={phase === id} onClick={() => ui.openGoals(id)}><span className="mono">{id}</span></button>
            ))}
          </div>
          <label className="gv-chk"><input type="checkbox" checked={onlyIssues} onChange={(e) => setOnlyIssues(e.target.checked)} /> Chỉ AC cần xem</label>
        </div>
      </div>

      {cards ?? (g.gate && inPhase(g.gate.phase) ? (
        <p className="gv-gate" role="status"><b>Phase {g.gate.phase} đang chờ bạn duyệt thay đổi AC</b> ({g.gate.asks.join(", ")}); phase chỉ đóng khi bạn duyệt hết.</p>
      ) : null)}

      {noTask.length || noAc.length || notOk ? (
        <section className="gv-warn" aria-label="Cảnh báo">
          <h3>Cần xem</h3>
          <ul>
            {notOk ? <li><b>{notOk} AC</b> bị chấm lại hoặc hiểu lại mà bạn chưa duyệt: {all.filter(unapproved).map((a) => <button key={`${a.phase}-${a.id}`} className="chip gv-task" onClick={() => jumpTo(rowId(a))}>{a.phase ? `${a.phase}/` : ""}{a.id}</button>)}</li> : null}
            {noTask.length ? <li><b>{noTask.length} AC chưa có task nào</b>: {noTask.map((s) => <span key={s} className="mono gv-ref">{s}</span>)}</li> : null}
            {noAc.length ? (
              <li>
                <b>{noAc.length} task không gắn AC nào</b>:{" "}
                {noAc.map((s) => { const r = taskRef(s); return <button key={s} className="chip gv-task" onClick={() => ui.openSub(r.phase, r.key)}>{s}</button>; })}
              </li>
            ) : null}
          </ul>
        </section>
      ) : null}

      {g.requirements.length ? (
        <section className="gv-req">
          <button className={`btn-sm ghost btn-more ${showReq ? "is-open" : ""}`} aria-expanded={showReq} onClick={() => setShowReq(!showReq)}>
            Yêu cầu gốc ({g.requirements.length})<Glyph name="chevron" />
          </button>
          {showReq ? (
            <ul className="gv-req-list">
              {g.requirements.map((r) => {
                const ids = reqOf(r.id);
                return (
                  <li key={r.id}>
                    <b className="mono">{r.id}</b> <Inline text={r.text} />
                    {ids.length ? <span className="gv-req-acs">{ids.join(", ")}</span> : <span className="gv-req-acs w">chưa có AC</span>}
                  </li>
                );
              })}
            </ul>
          ) : null}
        </section>
      ) : null}

      {groups.length === 0 ? (
        <p className="bd-empty">{onlyIssues ? "Không có AC nào cần xem." : g.acs.length ? "Phase này chưa có AC." : "Chưa có AC nào (REQUIREMENTS / VERIFICATION chưa viết)."}</p>
      ) : (
        <div className="gv-tbl">
          <table className="gv-acs">
            <thead>
              <tr><th className="gv-c-id">AC</th><th>Nội dung</th><th className="gv-c-task">Task</th><th className="gv-c-commit">Commit</th><th className="gv-c-res">Kết quả + bằng chứng</th></tr>
            </thead>
            {groups.map((grp) => (
              <tbody key={grp.id ?? "none"}>
                {!phase ? (
                  <tr className="gv-ph"><th colSpan={5}>{grp.id ? <><span className="mono">{grp.id}</span> {name(grp.id)}</> : "Không thuộc phase"} <span className="faint">{grp.rows.filter((a) => a.status === "pass").length}/{grp.rows.length} đạt</span></th></tr>
                ) : null}
                {grp.rows.map((a) => <AcRow key={`${a.phase}-${a.id}`} a={a} />)}
              </tbody>
            ))}
          </table>
        </div>
      )}
    </>
  );
}

function AcRow({ a }: { a: GoalAc }) {
  const ui = useTaskUi();
  const st = STATUS[a.status] ?? STATUS.pending;
  const ch = a.changed ? changeBadge(a.changed) : null;
  return (
    <tr id={rowId(a)} className={`gv-row is-${a.status}${a.changed && unapproved(a) ? " is-unapproved" : ""}`}>
      <td className="gv-c-id" data-label="AC"><b className="mono">{a.id}</b>{a.uc ? <span className="gv-uc mono">{a.uc}</span> : null}</td>
      <td className="gv-c-text" data-label="Nội dung">
        <Clamp lines={3}><Inline text={a.text} /></Clamp>
        {ch ? <span className={`tag ${ch.tone}`} title={ch.title}>{ch.label}</span> : null}
        {a.source ? <span className="gv-src">Nguồn: {a.source}</span> : null}
      </td>
      <td className="gv-c-task" data-label="Task">
        {a.tasks.length ? (
          <span className="gv-chips">{a.tasks.map((s) => { const r = taskRef(s); return <button key={s} className="chip gv-task" onClick={() => ui.openSub(r.phase ?? a.phase, r.key)} title={`Mở ${s}`}>{s}</button>; })}</span>
        ) : <span className="tag warn">Chưa có task</span>}
      </td>
      <td className="gv-c-commit" data-label="Commit">
        {a.commits.length ? (
          <ul className="gv-commits">{a.commits.slice(0, 4).map((c) => <li key={c.sha} title={c.subject}><code>{c.sha.slice(0, 8)}</code> <span>{c.subject}</span></li>)}{a.commits.length > 4 ? <li className="faint">+{a.commits.length - 4} commit</li> : null}</ul>
        ) : <span className="faint">-</span>}
      </td>
      <td className="gv-c-res" data-label="Kết quả">
        <span className={`tag ${st.tone}`}>{st.label}</span>
        {a.note ? <p className="gv-note"><Inline text={a.note} /></p> : null}
        {a.evidence ? <Clamp lines={2} className="gv-ev"><Inline text={a.evidence} /></Clamp> : null}
      </td>
    </tr>
  );
}

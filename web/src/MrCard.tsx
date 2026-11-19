// An action the boss confirms (chat proposal "Tạo merge request", the acceptance gate): board runs
// a normal omp session that picks the project's skill/command (e.g. gitlab-mr) like the boss typing
// in omp. The card shows what will run (title, instruction, the Leader's skill, writes outside the
// repo) and, when it pushes, the git rails: target branch, the exact commits, the foreign-commit
// choice. After confirming it shows the run live (log) and then the URLs it reported.
import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BoardRun, type BoardTask, type ChatFields, type ChatProposal, type MrCommit, type MrDraftResp } from "./api";
import { Drawer, ErrorState, Loading } from "./ui";
import { hhmm, runSecs, runStep, secsText } from "./runs";
import { serverMessage } from "./Steps";
import { Glyph } from "./icons";
import { useTaskUi } from "./Timeline";

/** Board's "Nghiệm thu" (when the task has no MR yet) dispatches this; the task page opens the drawer. */
export const ACCEPT_MR_EVENT = "ckit:accept-mr";
/** Messages for the Leader after the work (done task, acceptance gate) go to the task chat. */
export const FOCUS_CHAT_EVENT = "ckit:focus-chat";

/** Acceptance = the "Tạo merge request" action: the agent creates the MR with the project's skill.
 *  Done only when an MR URL comes back; otherwise the task stays at the gate with the agent's words. */
export function AcceptMrDrawer({ t, root, onClose }: { t: BoardTask; root: string; onClose: () => void }) {
  const q = useQuery({ queryKey: ["board-mr-draft", root, t.id], queryFn: () => api.boardMrDraft(root, t.id), staleTime: 0, gcTime: 0 });
  return (
    <Drawer label="Nghiệm thu: tạo merge request" head={<h3 className="dr-title">Nghiệm thu: tạo merge request · {t.id}</h3>} onClose={onClose} className="dr-mr">
      <p className="tc-note">Nghiệm thu = agent tạo MR cho nhánh của task bằng skill của dự án (như khi bạn gõ trong omp). Board không gộp nhánh trên máy; người review duyệt MR.</p>
      {q.error ? <ErrorState message={serverMessage(q.error as Error)} /> : !q.data ? <Loading rows={4} /> : <AcceptForm d={q.data} t={t} root={root} onClose={onClose} />}
    </Drawer>
  );
}

const actionRuns = (t: BoardTask) => t.runs.flatMap((r, i) => (r.stage === "action" ? [i] : []));

function AcceptForm({ d, t, root, onClose }: { d: MrDraftResp; t: BoardTask; root: string; onClose: () => void }) {
  const qc = useQueryClient();
  const form = useActionForm(d);
  // Action runs before this confirm: the one after them is ours.
  const before = useRef(actionRuns(t).length);
  const go = useMutation({
    // The server needs at least one field when the task has no MR yet: always send the target.
    mutationFn: () => api.boardAcceptMr(root, t.id, { target: form.f.target, ...form.fields }),
    onMutate: () => { before.current = actionRuns(t).length; },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task", root, t.id] });
      qc.invalidateQueries({ queryKey: ["board-chat", root, t.id] });
    },
    onError: () => qc.invalidateQueries({ queryKey: ["board-task", root, t.id] }),
  });
  const sent = go.isSuccess;
  const ri = sent ? actionRuns(t)[before.current] : undefined;
  const run = ri === undefined ? undefined : t.runs[ri];
  const done = sent && t.status === "done";
  const ended = Boolean(run?.ended);
  return (
    <div className={`tc-prop ${sent ? (done ? "is-done" : ended ? "is-failed" : "is-running") : "is-open"} dr-mr-card`}>
      <div className="tc-prop-h">
        <b>{d.title ?? "Tạo merge request"}</b>
        <StateTag state={!sent ? "open" : done ? "done" : ended ? "failed" : "running"} />
      </div>
      <ActionFields d={d} t={t} form={form} open={!sent} name="accept" fixedTitle />
      {sent ? (
        <>
          <RunRow t={t} idx={ri} />
          {done && t.mr ? <Urls urls={[t.mr.url]} /> : null}
          {done ? <p className="tc-res">Đã nghiệm thu: việc chuyển sang Xong.</p> : null}
          {!done && ended ? (
            <p className="tc-res is-err">
              Chưa có MR nên chưa nghiệm thu; việc vẫn chờ bạn.{run?.summary?.length ? ` Agent: ${run.summary.join(" ").replace(/^Thực hiện: [^:]+:\s*/, "")}` : " Xem log để biết lý do."}
            </p>
          ) : null}
        </>
      ) : null}
      {go.error ? <p className="tc-res is-err">{serverMessage(go.error as Error)}</p> : null}
      <div className="tc-prop-f">
        {sent ? (
          <button onClick={onClose}>Đóng</button>
        ) : (
          <>
            <button onClick={onClose} disabled={go.isPending}>Huỷ</button>
            <button className="primary" disabled={!form.ok || go.isPending} onClick={() => go.mutate()}>
              <Glyph name="check" /> {go.isPending ? "Đang chuẩn bị nhánh…" : "Xác nhận: push + tạo MR"}
            </button>
          </>
        )}
      </div>
    </div>
  );
}

/** What the card needs from a chat proposal or from `GET /api/board/:id/mr/draft`. */
export type ActionDraft = Pick<
  ChatProposal,
  "title" | "instruction" | "skill" | "pushes" | "writes_external" | "target" | "source" | "provider" | "host" | "targets" | "remote_default" | "commits" | "foreign" | "check_error" | "push"
>;

const PROVIDER: Record<string, string> = { gitlab: "GitLab", github: "GitHub" };

export function useActionForm(d: ActionDraft) {
  const init = () => ({ title: d.title ?? "", instruction: d.instruction ?? "", target: d.target ?? "" });
  const [f, setF] = useState(init);
  useEffect(() => setF(init()), [d.title, d.instruction, d.target]); // eslint-disable-line react-hooks/exhaustive-deps
  const pushes = Boolean(d.pushes);
  const commits = pushes ? d.commits ?? [] : [];
  const foreign = pushes ? d.foreign ?? commits.filter((c) => !c.from_task) : [];
  // Commits not made by the task: the boss must pick what goes out before confirming.
  const [push, setPush] = useState<"task" | "all" | null>(d.push ?? null);
  useEffect(() => setPush(d.push ?? null), [d.push]);
  const keys = pushes ? (["title", "instruction", "target"] as const) : (["title", "instruction"] as const);
  const changed = keys.filter((k) => f[k] !== (d[k] ?? ""));
  const fields: ChatFields = { ...Object.fromEntries(changed.map((k) => [k, f[k]])), ...(foreign.length && push ? { push } : {}) };
  const retarget = pushes && f.target !== (d.target ?? "");
  const ok =
    Boolean(f.title.trim() && f.instruction.trim()) &&
    (!pushes || (Boolean(f.target) && (!foreign.length || push !== null) && (!d.check_error || retarget)));
  return { f, setF, pushes, commits, foreign, push, setPush, fields, retarget, ok };
}
export type ActionForm = ReturnType<typeof useActionForm>;

/** `fixedTitle`: the acceptance always runs "Tạo merge request"; only the instruction is editable. */
export function ActionFields({ d, t, form, open, name, fixedTitle }: { d: ActionDraft; t: BoardTask; form: ActionForm; open: boolean; name: string; fixedTitle?: boolean }) {
  const { f, setF } = form;
  const ext = d.writes_external ?? [];
  return (
    <div className="tc-form">
      {open && !fixedTitle ? (
        <label className="tc-field"><span className="tc-lbl">Việc</span><input value={f.title} onChange={(e) => setF({ ...f, title: e.target.value })} /></label>
      ) : null}
      <label className="tc-field">
        <span className="tc-lbl">Yêu cầu cho agent</span>
        {open ? <textarea className="bd-textarea" rows={3} value={f.instruction} onChange={(e) => setF({ ...f, instruction: e.target.value })} /> : <span className="tc-pre">{d.instruction}</span>}
      </label>
      <p className="tc-row">
        <span className="tc-lbl">Skill</span>
        {d.skill ? <span className="tc-skill" title="Leader đề xuất; agent vẫn chọn skill / lệnh phù hợp của dự án">Skill: {d.skill}</span> : <span className="tc-skill is-auto">Agent tự chọn skill</span>}
        <span className="faint">chạy như khi bạn gõ trong omp: đủ công cụ, skill, MCP của dự án</span>
      </p>
      {ext.length ? (
        <div className="tc-ext" role="note">
          <span className="tc-lbl"><Glyph name="alert" /> Ghi ra ngoài repo ({ext.length})</span>
          <ul>{ext.map((w) => <li key={w}>{w}</li>)}</ul>
          {open ? <span className="tc-note">Agent ghi qua skill / MCP của dự án; board không hoàn tác được các thay đổi này.</span> : null}
        </div>
      ) : null}
      {form.pushes ? <GitPart d={d} t={t} form={form} open={open} name={name} /> : open ? <p className="tc-note">Việc này không đẩy code.</p> : null}
    </div>
  );
}

function GitPart({ d, t, form, open, name }: { d: ActionDraft; t: BoardTask; form: ActionForm; open: boolean; name: string }) {
  const { f, setF, commits, foreign, push, setPush, retarget } = form;
  const targets = [...new Set([...(d.targets ?? []), ...(d.target ? [d.target] : [])])];
  const src = d.source ?? t.branch ?? "?";
  const pushed = (c: MrCommit) => c.from_task || push !== "task";
  const setTarget = (e: { target: { value: string } }) => setF({ ...f, target: e.target.value });
  return (
    <div className="tc-git">
      <p className="tc-row">
        <span className="tc-lbl">Nơi tạo</span>
        {d.provider ? <span>{PROVIDER[d.provider] ?? d.provider}{d.host ? ` · ${d.host}` : ""}</span> : <span className="tp-warn">Không nhận ra remote (chỉ hỗ trợ GitHub / GitLab)</span>}
      </p>
      <p className="tc-row">
        <span className="tc-lbl">Nhánh</span>
        {/* Confirmed with "only the task's commits": the MR went out from the replayed `-mr` branch. */}
        <code>{!open && push === "task" ? `${src}-mr` : src}</code> →{" "}
        {open && targets.length > 1 ? (
          <select value={f.target} onChange={setTarget} aria-label="Nhánh đích">{targets.map((b) => <option key={b} value={b}>{b}{b === d.remote_default ? " (mặc định)" : ""}</option>)}</select>
        ) : open ? (
          <input value={f.target} onChange={setTarget} aria-label="Nhánh đích" className="tc-in-short" />
        ) : <code>{d.target}</code>}
      </p>
      {d.check_error ? <p className="tc-res is-err">{d.check_error}{open ? ". Chọn nhánh đích khác." : ""}</p> : null}
      {commits.length ? (
        <div className="tc-commits">
          <span className="tc-lbl">
            {open ? "Commit sẽ đẩy" : "Commit"} ({push === "task" ? commits.filter(pushed).length : commits.length})
            {open && retarget ? <span className="tc-note"> · tính theo <code>{d.target}</code>; đổi nhánh đích thì board tính lại khi xác nhận</span> : null}
          </span>
          <ol>
            {commits.map((c) => (
              <li key={c.sha} className={`${c.from_task ? "" : "is-foreign"} ${pushed(c) ? "" : "is-skip"}`.trim()} title={c.sha}>
                <code>{c.short}</code> <span className="tc-c-sub">{c.subject}</span>
                {c.from_task ? null : (
                  <>
                    <span className="tc-c-who">{c.author} · {hhmm(c.at)} {c.at.slice(8, 10)}/{c.at.slice(5, 7)}</span>
                    <span className="tc-c-warn">{pushed(c) ? "" : "Không đẩy. "}Commit này không phải của task, có sẵn trên nhánh gốc ở máy bạn.</span>
                  </>
                )}
              </li>
            ))}
          </ol>
        </div>
      ) : open ? <p className="tc-note">Không có commit mới so với <code>origin/{f.target}</code>.</p> : null}
      {open && foreign.length ? (
        <fieldset className="tc-choice">
          <legend><b>{foreign.length} commit không phải của task</b>: chọn cách đẩy</legend>
          <label><input type="radio" name={`push-${name}`} checked={push === "task"} onChange={() => setPush("task")} /> <b>Chỉ đẩy commit của task</b> <span className="faint">(nhánh mới <code>{src}-mr</code> từ <code>origin/{f.target}</code>, chỉ {commits.length - foreign.length} commit của task)</span></label>
          <label><input type="radio" name={`push-${name}`} checked={push === "all"} onChange={() => setPush("all")} /> <b>Đẩy cả các commit đó</b> <span className="faint">(đẩy nhánh <code>{src}</code> nguyên trạng)</span></label>
        </fieldset>
      ) : null}
      {open ? (
        <p className="tc-note">
          Agent chạy trong một worktree tạm trên nhánh nguồn, không đụng thư mục của bạn; chỉ được push nhánh <code>board/*</code>, không push đè. Tiêu đề và mô tả MR do skill viết.
          {foreign.length && push === null ? " Chọn cách đẩy ở trên để xác nhận." : ""}
        </p>
      ) : null}
    </div>
  );
}

export function StateTag({ state }: { state: ChatProposal["state"] }) {
  if (state === "done") return <span className="tag ok">Đã làm</span>;
  if (state === "failed") return <span className="tag err">Không xong</span>;
  if (state === "cancelled") return <span className="tag">Đã huỷ</span>;
  if (state === "running") return <span className="tag info">Đang chạy</span>;
  return <span className="tag warn">Chờ bạn xác nhận</span>;
}

/** The action's run: live (pulse, elapsed, Xem log trực tiếp) or finished (time, Xem log). */
export function RunRow({ t, idx }: { t: BoardTask; idx: number | undefined }) {
  const ui = useTaskUi();
  const r: BoardRun | undefined = idx === undefined ? undefined : t.runs[idx];
  if (!r) return <p className="tc-run"><span className="pulse" aria-hidden="true" /> Đang chuẩn bị nhánh và mở phiên agent…</p>;
  const live = r.ended === null;
  return (
    <p className={`tc-run ${live ? "is-live" : ""}`}>
      {live ? <span className="pulse" aria-hidden="true" /> : null}
      <b>{live ? "Agent đang chạy" : "Agent đã chạy xong"}</b> · {r.title ?? runStep(r)} · {secsText(runSecs(r))}
      <button className="btn-sm" onClick={() => ui.openLog(idx!)}><Glyph name="log" />{live ? "Xem log trực tiếp" : "Xem log"}</button>
    </p>
  );
}

export function Urls({ urls }: { urls: string[] }) {
  return (
    <ul className="tc-urls">
      {urls.map((u) => (
        <li key={u}><a href={u} target="_blank" rel="noreferrer">{/\/(merge_requests|pull)\/\d+/.test(u) ? "Mở merge request ↗" : "Mở ↗"}</a> <span className="faint">{u}</span></li>
      ))}
    </ul>
  );
}

/** The run of a chat action: the live one while it runs, else the latest with its title. */
export function actionRunIdx(t: BoardTask, p: ChatProposal): number | undefined {
  const idx = actionRuns(t);
  if (p.state === "running") return idx.reverse().find((i) => t.runs[i]!.ended === null);
  return idx.reverse().find((i) => !p.title || t.runs[i]!.title === p.title);
}

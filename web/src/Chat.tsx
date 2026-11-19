// "Trao đổi với Leader": the task chat at the bottom of every task page (done tasks too).
// The Leader answers read-only from a fork of the task's latest session (it may read the project's
// skills to name the right one); when the boss asks for something it answers with ONE proposal:
// an action (a normal omp run on confirm, e.g. "Tạo merge request" with the gitlab-mr skill; with
// `pushes` while the task waits at acceptance it is the acceptance) or revise (a new round).
// Contract: GET/POST /api/board/:id/chat, …/chat/:msg/confirm|cancel.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BoardChat, type BoardTask, type ChatFields, type ChatMessage, type ChatProposal } from "./api";
import { isMissing } from "./ui";
import { Markdown } from "./markdown";
import { serverMessage } from "./Steps";
import { money } from "./Board";
import { hhmm } from "./runs";
import { Glyph } from "./icons";
import { ActionFields, FOCUS_CHAT_EVENT, RunRow, StateTag, Urls, actionRunIdx, useActionForm } from "./MrCard";

const chatKey = (root: string, id: string) => ["board-chat", root, id];

export function useChat(root: string, id: string) {
  return useQuery({
    queryKey: chatKey(root, id),
    queryFn: () => api.boardChat(root, id),
    refetchInterval: (q) => (isMissing(q.state.error) ? false : q.state.data?.busy ? 2000 : 15000),
  });
}

/** Every write returns the whole chat; store it and refresh the task (status, round, mr). */
function useChatWrite<A>(root: string, t: BoardTask, fn: (a: A) => Promise<BoardChat>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: (chat) => {
      qc.setQueryData(chatKey(root, t.id), chat);
      qc.invalidateQueries({ queryKey: ["board-task", root, t.id] });
      qc.invalidateQueries({ queryKey: ["board"] });
    },
  });
}


export function ChatPanel({ t, root }: { t: BoardTask; root: string }) {
  const q = useChat(root, t.id);
  const msgs = q.data?.messages ?? [];
  const busy = Boolean(q.data?.busy);
  const openProposal = msgs.some((m) => m.proposal?.state === "open");
  // The chat stays busy while a confirmed action runs (chat proposal or the acceptance), not the Leader typing.
  const actionRun = t.runs.find((r) => r.stage === "action" && r.ended === null);
  const acting = msgs.find((m) => m.proposal?.state === "running")?.proposal?.title ?? actionRun?.title ?? (actionRun ? "việc đã xác nhận" : undefined);
  const qc = useQueryClient();
  const wasBusy = useRef(busy);
  useEffect(() => {
    // A Leader answer or an action just finished: the task (runs, status, mr) changed too.
    if (wasBusy.current && !busy) {
      qc.invalidateQueries({ queryKey: ["board-task", root, t.id] });
      qc.invalidateQueries({ queryKey: ["board"] });
    }
    wasBusy.current = busy;
  }, [busy, qc, root, t.id]);
  const [open, setOpen] = useState<boolean | null>(null);
  const shown = open ?? (busy || openProposal);
  // Opened because the Leader is busy or a proposal waits: stay open after it settles, so the
  // answer / the proposal's result stays in view until the boss folds the panel.
  useEffect(() => {
    if (open === null && (busy || openProposal)) setOpen(true);
  }, [open, busy, openProposal]);
  const [text, setText] = useState("");
  const send = useChatWrite(root, t, (v: string) => api.boardChatSend(root, t.id, v));
  const listRef = useRef<HTMLOListElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const atBottom = useRef(true);
  const cancelled = t.status === "cancelled";
  // "Yêu cầu sửa" at acceptance (and other after-work messages) land here.
  useEffect(() => {
    const on = () => {
      setOpen(true);
      window.setTimeout(() => {
        inputRef.current?.scrollIntoView({ block: "nearest" });
        inputRef.current?.focus({ preventScroll: true });
      }, 0);
    };
    window.addEventListener(FOCUS_CHAT_EVENT, on);
    return () => window.removeEventListener(FOCUS_CHAT_EVENT, on);
  }, []);

  // Follow new messages only when the boss is already at the bottom.
  useLayoutEffect(() => {
    const el = listRef.current;
    if (el && atBottom.current) el.scrollTop = el.scrollHeight;
  }, [msgs.length, busy, shown]);

  if (isMissing(q.error)) return null;
  const last = msgs[msgs.length - 1];
  const ok = text.trim() !== "" && !send.isPending && !busy && !cancelled;
  const submit = () => {
    if (!ok) return;
    atBottom.current = true;
    setOpen(true);
    send.mutate(text.trim(), { onSuccess: () => setText("") });
  };

  return (
    <section className={`tc ${shown ? "is-open" : ""}`} aria-label="Trao đổi với Leader">
      <button className="tc-h" aria-expanded={shown} onClick={() => setOpen(!shown)}>
        <span className="tc-h-chev" aria-hidden="true"><Glyph name="chevron" /></span>
        <b>Trao đổi với Leader</b>
        {msgs.length ? <span className="tc-n">{msgs.length}</span> : null}
        {acting ? (
          <span className="tc-typing"><span className="pulse" aria-hidden="true" />Đang thực hiện: {acting}…</span>
        ) : busy ? (
          <span className="tc-typing"><span className="tc-dots" aria-hidden="true"><i /><i /><i /></span>Leader đang trả lời…</span>
        ) : openProposal ? (
          <span className="tag warn">Đề xuất chờ bạn xác nhận</span>
        ) : !shown && last ? (
          <span className="tc-last">{last.from === "boss" ? "Bạn: " : last.from === "leader" ? "Leader: " : ""}{last.text.replace(/```[\s\S]*?```/g, " … ").replace(/[*_`#>]+/g, "").replace(/\s+/g, " ").slice(0, 160)}</span>
        ) : null}
      </button>
      {shown ? (
        <ol className="tc-list" ref={listRef} onScroll={(e) => { const el = e.currentTarget; atBottom.current = el.scrollTop + el.clientHeight >= el.scrollHeight - 24; }} aria-live="polite">
          {msgs.length === 0 && !busy ? (
            <li className="tc-empty">
              Hỏi Leader bất cứ điều gì về việc này: đã làm gì, vì sao, AC nào còn lo. Muốn làm tiếp thì nói ("tạo MR vào develop", "sửa lại phần …"): Leader đề xuất, bạn xác nhận ở đây.
            </li>
          ) : null}
          {msgs.map((m) => <Msg key={m.id} m={m} t={t} root={root} />)}
          {busy && !acting ? <li className="tc-msg is-leader is-typing"><span className="tc-dots" aria-hidden="true"><i /><i /><i /></span> Leader đang đọc lại việc này…</li> : null}
        </ol>
      ) : null}
      <div className="tc-compose">
        <textarea
          ref={inputRef}
          className="bd-textarea"
          rows={1}
          value={text}
          disabled={cancelled}
          onFocus={() => open === null && setOpen(true)}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
              e.preventDefault();
              submit();
            }
          }}
          placeholder={cancelled ? "Việc đã huỷ: không nhắn được." : acting ? "Đang thực hiện việc đã xác nhận; nhắn được khi xong." : busy ? "Leader đang trả lời…" : "Nhắn Leader về việc này (Enter gửi, Shift+Enter xuống dòng)"}
          aria-label="Nhắn Leader"
        />
        <button className="primary" disabled={!ok} onClick={submit}>{send.isPending ? "Đang gửi…" : "Gửi"}</button>
      </div>
      {send.error ? <p className="hint hint-err tc-err">{serverMessage(send.error as Error)}</p> : null}
    </section>
  );
}

function Msg({ m, t, root }: { m: ChatMessage; t: BoardTask; root: string }) {
  if (m.from === "system") return <li className="tc-msg is-system"><span>{m.text}</span> <time className="faint">{hhmm(m.at)}</time></li>;
  const boss = m.from === "boss";
  return (
    <li className={`tc-msg ${boss ? "is-boss" : "is-leader"}`}>
      <span className="tc-meta">
        <b>{boss ? "Bạn" : "Leader"}</b> <time>{hhmm(m.at)}</time>
        {!boss && m.cost ? <span>{money(m.cost)}</span> : null}
      </span>
      <div className="tc-text">{boss ? <p>{m.text}</p> : <Markdown source={m.text} />}</div>
      {m.proposal ? <Proposal p={m.proposal} msg={m.id} t={t} root={root} /> : null}
    </li>
  );
}

/** The Leader's proposal: editable fields, Huỷ / Xác nhận, then the run and its result. */
function Proposal({ p, msg, t, root }: { p: ChatProposal; msg: string; t: BoardTask; root: string }) {
  const [instruction, setInstruction] = useState(p.instruction ?? "");
  useEffect(() => setInstruction(p.instruction ?? ""), [p.instruction]);
  const act = useActionForm(p);
  const confirm = useChatWrite(root, t, (fields: ChatFields) => api.boardChatConfirm(root, t.id, msg, fields));
  const cancel = useChatWrite(root, t, () => api.boardChatCancel(root, t.id, msg));
  const open = p.state === "open";
  const busy = confirm.isPending || cancel.isPending;
  const round = (t.round ?? 1) + 1;
  const isAction = p.kind !== "revise";
  // A pushing action confirmed while the task waits at acceptance is the acceptance.
  const accepting = isAction && p.pushes && t.status === "waiting" && t.stage === "acceptance";
  const fields: ChatFields = isAction ? act.fields : instruction !== (p.instruction ?? "") ? { instruction } : {};
  // One action at a time: another one (e.g. the acceptance MR) is running.
  const other = t.runs.some((r) => r.stage === "action" && r.ended === null);
  const can = !busy && !other && (isAction ? act.ok : Boolean(instruction.trim()));
  const label = isAction ? `Xác nhận: ${p.pushes ? "push + " : ""}thực hiện${accepting ? " (nghiệm thu)" : ""}` : `Xác nhận: mở Vòng ${round}`;
  const err = confirm.error ?? cancel.error;
  const urls = [...new Set([...(p.result?.urls ?? []), ...(p.result?.url ? [p.result.url] : [])])];
  const ran = isAction && p.state !== "open" && p.state !== "cancelled";

  return (
    <div className={`tc-prop is-${p.state}`}>
      <div className="tc-prop-h">
        <b>{isAction ? `Đề xuất: ${p.title ?? "thực hiện"}` : "Đề xuất: làm thêm một vòng"}</b>
        <StateTag state={p.state} />
      </div>
      {isAction ? (
        <ActionFields d={p} t={t} form={act} open={open} name={msg} />
      ) : (
        <div className="tc-form">
          <label className="tc-field">
            <span className="tc-lbl">Yêu cầu cho Vòng {open ? round : t.round ?? round}</span>
            {open ? <textarea className="bd-textarea" rows={4} value={instruction} onChange={(e) => setInstruction(e.target.value)} /> : <span className="tc-pre">{p.instruction}</span>}
          </label>
          {open ? <p className="tc-note">Cùng việc này, cùng nhánh <code>{t.branch}</code>: Dev → Kiểm thử → Báo cáo → Nghiệm thu chạy lại; Dev nhớ phiên trước.</p> : null}
        </div>
      )}
      {ran ? <RunRow t={t} idx={actionRunIdx(t, p)} /> : null}
      {ran && urls.length ? <Urls urls={urls} /> : null}
      {ran && p.result?.message ? <p className={`tc-res ${p.state === "failed" ? "is-err" : ""}`}>Agent: {p.result.message}</p> : null}
      {p.state === "done" && !isAction ? <p className="tc-res">Đã mở Vòng {t.round ?? round}: Dev làm tiếp trên cùng nhánh.</p> : null}
      {p.state === "failed" && p.result?.error ? <p className="tc-res is-err">{p.result.error}</p> : null}
      {err ? <p className="hint hint-err">{serverMessage(err as Error)}</p> : null}
      {open ? (
        <div className="tc-prop-f">
          <button disabled={busy} onClick={() => cancel.mutate(undefined)}>{cancel.isPending ? "Đang huỷ…" : "Huỷ"}</button>
          {/* The card already says what happens; this click is the confirmation (no second dialog). */}
          <button className="primary" disabled={!can} onClick={() => confirm.mutate(fields)}>
            <Glyph name="check" /> {confirm.isPending ? (isAction ? "Đang chuẩn bị…" : "Đang xử lý…") : label}
          </button>
        </div>
      ) : null}
    </div>
  );
}

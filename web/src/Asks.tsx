// Mid-step team questions (board_ask): a running step (Dev, Tester, …) asks
// the Leader or the boss and waits. Questions that reach the boss show on the
// task page right under its header; the boss may also pre-empt the Leader's answer.
// Contract: task.asks, summary.asking, POST /api/board/:id/asks/:qid.
import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api, type BoardAsk, type BoardStage, type BoardStatus, type BoardTask } from "./api";
import { serverMessage } from "./Steps";

// Who asks, by the step that asked.
const ASKER: Partial<Record<BoardStage, string>> = {
  investigate: "Điều tra",
  intake: "Tiếp nhận",
  solution: "Kế hoạch",
  review: "Phản biện",
  dev: "Phát triển",
  test: "Kiểm thử",
  security: "Bảo mật",
  report: "Báo cáo",
};

// Short role of the asker for the room ("Đang trả lời Dev").
const ASKER_ROLE: Partial<Record<BoardStage, string>> = {
  dev: "Dev",
  test: "Tester",
  security: "Reviewer bảo mật",
  review: "Reviewer",
  solution: "BA",
  intake: "Leader",
  investigate: "Leader",
  report: "Leader",
};

// Legacy asks/threads addressed the BA or the PO: both read as the Leader now.
const MEMBER: Record<string, string> = { leader: "Leader", ba: "Leader", po: "Leader", reviewer: "Reviewer", boss: "Bạn", user: "Bạn", coordinator: "Điều phối" };

export const asker = (s: string) => ASKER[s as BoardStage] ?? s;
export const member = (s: string) => MEMBER[s] ?? asker(s);

/** Thread sender label: "dev → leader" → "Phát triển → Leader", "user → dev" → "Bạn → Phát triển";
 *  leader/reviewer/boss/coordinator → Leader / Reviewer / Bạn / Điều phối. */
export function senderLabel(from: string): string {
  if (from.includes(" → ")) {
    const [a = "", b = ""] = from.split(" → ");
    return `${member(a)} → ${member(b)}`;
  }
  return MEMBER[from] ?? from;
}

/** A running task with questions waiting for the boss counts as "Cần bạn". */
export function askingYou(t: { status: BoardStatus; asking?: number }): boolean {
  return (t.asking ?? 0) > 0 && (t.status === "running" || t.status === "queued");
}

const AC_GATE_NOTE = /^Chờ bạn duyệt thay đổi AC/;
/** Phase close held for the boss's ruling on AC changes (`ac_gate`, or its gate note on summaries). */
export function acGateOf(t: { status: BoardStatus; note?: string | null; ac_gate?: BoardTask["ac_gate"] }): boolean {
  return t.status === "waiting" && Boolean(t.ac_gate || AC_GATE_NOTE.test(t.note ?? ""));
}

/** Seats busy answering a teammate: the Leader ("Đang trả lời Dev"). */
export function answeringSeats(t: BoardTask): { seat: "leader"; state: string }[] {
  return (t.asks ?? [])
    .filter((a) => a.status === "answering" && a.to !== "boss")
    .map((a) => ({ seat: "leader" as const, state: `Đang trả lời ${ASKER_ROLE[a.from] ?? asker(a.from)}` }));
}

/** Open questions of the task: waiting for the boss, or being answered by the Leader. */
export function AskPanel({ task, root }: { task: BoardTask; root: string }) {
  const asks = (task.asks ?? []).filter((a) => a.status === "boss" || a.status === "answering");
  if (asks.length === 0) return null;
  const forBoss = asks.filter((a) => a.status === "boss");
  const acs = forBoss.filter((a) => a.kind === "ac_change");
  const plain = forBoss.filter((a) => a.kind !== "ac_change");
  return (
    <section className={`ask-panel ${forBoss.length ? "is-need" : ""}`} aria-label="Câu hỏi của team">
      {acs.length ? <p className="ac-chg-lead"><b>{acs.length} thay đổi AC chờ bạn duyệt.</b> Phase chỉ đóng khi bạn duyệt hết; không duyệt thì Dev làm lại AC đó theo nhận xét của bạn.</p> : null}
      {acs.map((a) => (
        <AcChangeAsk key={a.id} a={a} task={task} root={root} />
      ))}
      {plain.map((a) => (
        <BossAsk key={a.id} a={a} task={task} root={root} />
      ))}
      {asks
        .filter((a) => a.status === "answering")
        .map((a) => (
          <AnsweringAsk key={a.id} a={a} task={task} root={root} />
        ))}
      {plain.length > 0 && <p className="ask-note">Team đang chờ: không trả lời trong ~20 phút thì họ tự quyết.</p>}
    </section>
  );
}

/** One AC the team wants to change: current → proposed text, why, the evidence; the boss rules.
 *  Never times out. "Không duyệt" needs a comment (it goes to Dev with the AC). */
function AcChangeAsk({ a, task, root }: { a: BoardAsk; task: BoardTask; root: string }) {
  const qc = useQueryClient();
  const [note, setNote] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const c = a.ac;
  const send = useMutation({
    mutationFn: (decision: "approve" | "reject") =>
      api.boardAskAnswer(root, task.id, a.id, note.trim(), decision),
    onSuccess: () => {
      setNote("");
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task", root, task.id] });
      qc.invalidateQueries({ queryKey: ["board-goals", root, task.id] });
    },
  });
  const reject = () => {
    if (!note.trim()) return setErr("Ghi lý do không duyệt để Dev biết cần làm lại gì.");
    setErr(null);
    send.mutate("reject");
  };
  return (
    <article className="ask-item ac-chg" aria-label={`Đổi ${c?.ac ?? "AC"}`}>
      <div className="ask-head">
        <span className="tag warn">Đổi AC cần bạn duyệt</span>
        <b className="mono">{c?.ac ?? "AC"}</b>
        {a.phase ? <span className="mono faint">{a.phase}</span> : null}
        <span className="faint">{asker(a.from)} đề xuất</span>
        <span className="mono faint">{a.id}</span>
      </div>
      {c ? (
        <>
          <div className="ac-chg-diff">
            <div className="ac-chg-old"><span className="ac-chg-lbl">AC hiện tại</span><p>{c.old || "(trống)"}</p></div>
            <span className="ac-chg-arrow" aria-hidden="true">→</span>
            <div className="ac-chg-new"><span className="ac-chg-lbl">Đề xuất</span><p>{c.new || "(bỏ AC này)"}</p></div>
          </div>
          {c.reason ? <p className="ac-chg-row"><span className="ac-chg-lbl">Lý do</span>{c.reason}</p> : null}
          {c.evidence ? <p className="ac-chg-row"><span className="ac-chg-lbl">Bằng chứng</span><span className="ac-chg-ev">{c.evidence}</span></p> : null}
        </>
      ) : (
        <p className="ask-q">{a.question}</p>
      )}
      <textarea
        aria-label={`Nhận xét cho ${c?.ac ?? a.id}`}
        className="bd-textarea bd-reply"
        rows={2}
        value={note}
        onChange={(e) => { setNote(e.target.value); if (err) setErr(null); }}
        placeholder="Nhận xét (bắt buộc khi không duyệt)"
      />
      {err ? <p className="hint hint-err">{err}</p> : send.error ? <p className="hint hint-err">{serverMessage(send.error as Error)}</p> : null}
      <div className="bd-dfoot-row">
        <span className="faint hint-inline">Không tự quyết khi hết giờ: phase chờ tới khi bạn duyệt.</span>
        <span className="grow-spacer" />
        <button type="button" disabled={send.isPending} onClick={reject}>Không duyệt</button>
        <button type="button" className="primary" disabled={send.isPending} onClick={() => send.mutate("approve")}>
          {send.isPending ? "Đang gửi…" : "Duyệt thay đổi"}
        </button>
      </div>
    </article>
  );
}

function BossAsk({ a, task, root }: { a: BoardAsk; task: BoardTask; root: string }) {
  const head = a.to === "boss" ? `${asker(a.from)} hỏi bạn` : `${asker(a.from)} hỏi Leader, Leader chuyển lên bạn`;
  return (
    <div className="ask-item">
      <div className="ask-head">
        <span className="tag warn">{head}</span>
        {a.to !== "boss" && <span className="faint">Leader chưa tự quyết được</span>}
        <span className="mono faint">{a.id}</span>
      </div>
      <p className="ask-q">{a.question}</p>
      <AnswerForm a={a} task={task} root={root} />
    </div>
  );
}

function AnsweringAsk({ a, task, root }: { a: BoardAsk; task: BoardTask; root: string }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="ask-item is-answering">
      <div className="ask-line">
        <span className="bd-spin" aria-hidden="true" />
        <span className="ask-line-text">
          <strong>{asker(a.from)} hỏi Leader</strong> · Leader đang trả lời…
        </span>
        <button type="button" className="ask-toggle" aria-expanded={open} onClick={() => setOpen((v) => !v)}>
          {open ? "Thôi" : "Tôi trả lời"}
        </button>
      </div>
      {open && (
        <>
          <p className="ask-q">{a.question}</p>
          <AnswerForm a={a} task={task} root={root} />
        </>
      )}
    </div>
  );
}

function AnswerForm({ a, task, root }: { a: BoardAsk; task: BoardTask; root: string }) {
  const qc = useQueryClient();
  const [text, setText] = useState("");
  const send = useMutation({
    mutationFn: (answer: string) => api.boardAskAnswer(root, task.id, a.id, answer),
    onSuccess: () => {
      setText("");
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task", root, task.id] });
    },
  });
  const ok = text.trim() !== "" && !send.isPending;
  const submit = () => ok && send.mutate(text.trim());
  return (
    <div className="ask-form">
      {a.options.length > 0 && (
        <div className="ask-opts" role="group" aria-label="Gợi ý trả lời">
          {a.options.map((o) => (
            <button type="button" key={o} className={`ask-opt ${text === o ? "on" : ""}`} aria-pressed={text === o} onClick={() => setText(o)}>
              {o}
            </button>
          ))}
        </div>
      )}
      <textarea
        aria-label={`Câu trả lời cho ${a.id}`}
        className="bd-textarea bd-reply"
        rows={2}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
            e.preventDefault();
            submit();
          }
        }}
        placeholder="Trả lời team…"
      />
      {send.error ? <p className="hint hint-err">{serverMessage(send.error as Error)}</p> : null}
      <div className="bd-dfoot-row">
        <span className="faint hint-inline">⌘/Ctrl + Enter để gửi</span>
        <span className="grow-spacer" />
        <button type="button" className="primary" disabled={!ok} onClick={submit}>
          {send.isPending ? "Đang gửi…" : "Trả lời"}
        </button>
      </div>
    </div>
  );
}

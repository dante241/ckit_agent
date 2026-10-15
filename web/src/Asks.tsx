// Mid-step team questions (board_ask): a running step (Dev, Tester, …) asks
// the BA, the PO (Leader) or the boss and waits. Questions that reach the boss
// show on the task page right under its header; the boss may also pre-empt a BA/PO answer.
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
  review: "PO duyệt",
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
  solution: "Leader",
  intake: "Leader",
  investigate: "Leader",
  report: "Leader",
};

const MEMBER: Record<string, string> = { ba: "BA", po: "PO", boss: "Bạn", leader: "Leader", user: "Bạn", coordinator: "Điều phối" };

const asker = (s: string) => ASKER[s as BoardStage] ?? s;
const member = (s: string) => MEMBER[s] ?? asker(s);

/** Thread sender label: "dev → po" → "Phát triển → PO", "user → dev" → "Bạn → Phát triển";
 *  ba/po/boss/coordinator → BA / PO (Leader) / Bạn / Điều phối. */
export function senderLabel(from: string): string {
  if (from.includes(" → ")) {
    const [a = "", b = ""] = from.split(" → ");
    return `${member(a)} → ${member(b)}`;
  }
  if (from === "po") return "PO (Leader)";
  return MEMBER[from] ?? from;
}

/** A running task with questions waiting for the boss counts as "Cần bạn". */
export function askingYou(t: { status: BoardStatus; asking?: number }): boolean {
  return (t.asking ?? 0) > 0 && (t.status === "running" || t.status === "queued");
}

/** Seats busy answering a teammate: BA/PO answering → "ba" / "leader". */
export function answeringSeats(t: BoardTask): { seat: "ba" | "leader"; state: string }[] {
  return (t.asks ?? [])
    .filter((a) => a.status === "answering" && a.to !== "boss")
    .map((a) => ({ seat: a.to === "ba" ? "ba" : "leader", state: `Đang trả lời ${ASKER_ROLE[a.from] ?? asker(a.from)}` }));
}

/** Open questions of the task: waiting for the boss, or being answered by BA/PO. */
export function AskPanel({ task, root }: { task: BoardTask; root: string }) {
  const asks = (task.asks ?? []).filter((a) => a.status === "boss" || a.status === "answering");
  if (asks.length === 0) return null;
  const forBoss = asks.filter((a) => a.status === "boss");
  return (
    <section className={`ask-panel ${forBoss.length ? "is-need" : ""}`} aria-label="Câu hỏi của team">
      {forBoss.map((a) => (
        <BossAsk key={a.id} a={a} task={task} root={root} />
      ))}
      {asks
        .filter((a) => a.status === "answering")
        .map((a) => (
          <AnsweringAsk key={a.id} a={a} task={task} root={root} />
        ))}
      {forBoss.length > 0 && <p className="ask-note">Team đang chờ: không trả lời trong ~20 phút thì họ tự quyết.</p>}
    </section>
  );
}

function BossAsk({ a, task, root }: { a: BoardAsk; task: BoardTask; root: string }) {
  const head = a.to === "boss" ? `${asker(a.from)} hỏi bạn` : `${member(a.to)} chuyển lên bạn`;
  return (
    <div className="ask-item">
      <div className="ask-head">
        <span className="tag warn">{head}</span>
        {a.to !== "boss" && <span className="faint">câu hỏi của {asker(a.from)}</span>}
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
          {member(a.to)} đang trả lời câu hỏi của {asker(a.from)}…
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

// Interjections into a step: the boss notes or pauses a running step, and
// continues a paused one. Contract: POST /api/board/:id/inbox {kind,text},
// POST /api/board/:id/resume {kind:"note",text}.
import { useState, type ReactNode } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api, type BoardStatus, type BoardTask } from "./api";
import { serverMessage } from "./Steps";
import { askingYou } from "./Asks";

const PAUSING = "đang xin tạm dừng…";

/** Paused at a safe point. Summaries from older servers lack `paused`: fall back to the note. */
export function isPaused(t: { status: BoardStatus; paused?: boolean; note?: string | null }): boolean {
  return t.status === "waiting" && (t.paused ?? t.note === "Tạm dừng");
}

/** Created but never started: waits for the boss's Bắt đầu. */
export function isNew(t: { status: BoardStatus; paused?: boolean; note?: string | null; started?: boolean; runs?: unknown[] }): boolean {
  return isPaused(t) && (t.runs ? t.runs.length === 0 : t.started === false);
}

/** Pill text for a parked task, or undefined when it is not parked. */
export function pauseLabel(t: Parameters<typeof isNew>[0]): string | undefined {
  return isPaused(t) ? (isNew(t) ? "Chưa bắt đầu" : "Tạm dừng") : undefined;
}

type Send = { kind: "note" | "pause" | "resume"; text: string };

/** Task page action bar for a running or paused task. `cancel` is the page's Huỷ việc button. */
export function Interject({ t, root, seat, cancel, error }: { t: BoardTask; root: string; seat: string; cancel: ReactNode; error?: Error | null }) {
  const qc = useQueryClient();
  const [text, setText] = useState("");
  // Composer opens on Nhắn / Thêm chỉ dẫn so the bar stays one line by default.
  const [writing, setWriting] = useState(false);
  const send = useMutation({
    mutationFn: (s: Send) => (s.kind === "resume" ? api.boardResume(root, t.id, s.text) : api.boardInbox(root, t.id, s.kind, s.text)),
    onSuccess: () => {
      setText("");
      setWriting(false);
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task", root, t.id] });
    },
  });
  const paused = isPaused(t);
  const asking = askingYou(t);
  const fresh = isNew(t);
  const pausing = t.note === PAUSING;
  const busy = send.isPending ? send.variables?.kind : undefined;
  const v = text.trim();
  const go = (kind: Send["kind"]) => {
    if (busy || (kind === "note" && !v)) return;
    send.mutate({ kind, text: v });
  };

  // Questions sit right under the header (AskPanel); jump to the first answer box.
  const toAsk = () => {
    const panel = document.querySelector(".ask-panel");
    panel?.scrollIntoView({ behavior: "smooth", block: "center" });
    panel?.querySelector<HTMLElement>("textarea, input, .ask-opt")?.focus({ preventScroll: true });
  };

  return (
    <section className={`tp-act ij ${paused || asking ? "is-waiting" : ""}`} aria-label="Việc cần làm">
      <div className="tp-act-row">
        {paused ? (
          <p className="bd-gate-msg">{fresh ? "Việc chưa bắt đầu. Thêm chỉ dẫn nếu cần, rồi bấm Bắt đầu để team chạy." : "Việc đang tạm dừng. Bấm Tiếp tục để làm tiếp từ chỗ đã dừng."}</p>
        ) : pausing ? (
          <p className="bd-gate-msg ij-pausing"><span className="bd-spin" aria-hidden="true" /> Đang xin tạm dừng, {seat} sẽ dừng ở điểm an toàn.</p>
        ) : asking ? (
          <p className="bd-gate-msg">Team đang hỏi bạn. Trả lời ngay bên dưới để họ làm tiếp.</p>
        ) : (
          <p className="bd-gate-msg muted">{seat} đang làm. Bạn có thể nhắn thêm hoặc tạm dừng.</p>
        )}
        <div className="tp-act-btns">
          {cancel}
          {!paused && (
            <button onClick={() => go("pause")} disabled={busy !== undefined || pausing} title={pausing ? "Đã xin tạm dừng" : "Nội dung ô nhắn (nếu có) là lý do"}>
              {busy === "pause" ? "Đang gửi…" : pausing ? "Đang tạm dừng…" : "Tạm dừng"}
            </button>
          )}
          <button onClick={() => setWriting(!writing)} aria-expanded={writing}>
            {paused ? "Thêm chỉ dẫn" : "Nhắn"}
          </button>
          {paused && (
            <button className="primary" onClick={() => go("resume")} disabled={busy !== undefined}>
              {busy === "resume" ? "Đang xếp lượt…" : fresh ? "Bắt đầu" : "Tiếp tục"}
            </button>
          )}
          {!paused && asking && (
            <button className="primary" onClick={toAsk}>Trả lời câu hỏi</button>
          )}
        </div>
      </div>
      {writing && (
        <div className="tp-compose">
          <textarea
            className="bd-textarea bd-reply"
            rows={2}
            value={text}
            autoFocus
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                go(paused ? "resume" : "note");
              }
            }}
            placeholder={paused ? (fresh ? "Chỉ dẫn thêm cho team (không bắt buộc)…" : "Chỉ dẫn khi làm tiếp (không bắt buộc)…") : `Nhắn cho ${seat} đang làm…`}
            aria-label={paused ? "Chỉ dẫn khi tiếp tục" : `Nhắn cho ${seat}`}
          />
          {paused ? (
            <p className="ij-hint">Chỉ dẫn được gửi kèm khi bấm {fresh ? "Bắt đầu" : "Tiếp tục"}.</p>
          ) : (
            <div className="bd-dfoot-row">
              <p className="ij-hint">Tin được chèn vào phiên đang chạy ở lần gọi tool kế tiếp; Tạm dừng = dừng ở điểm an toàn (tối đa 5 phút).</p>
              <span className="grow-spacer" />
              <button className="primary" onClick={() => go("note")} disabled={!v || busy !== undefined}>
                {busy === "note" ? "Đang gửi…" : "Gửi"}
              </button>
            </div>
          )}
        </div>
      )}
      {send.error ? <p className="hint hint-err">{serverMessage(send.error as Error)}</p> : error ? <p className="hint hint-err">{error.message}</p> : null}
    </section>
  );
}

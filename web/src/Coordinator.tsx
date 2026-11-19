// The project's standing Coordinator: the boss chats with it, and (when
// enabled) it acts for the boss on the board. Contract: GET/POST
// /api/board/coord, POST /api/board/coord/enabled — all scoped by ?root=.
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type ChatMsg, type CoordState } from "./api";
import { Markdown } from "./markdown";
import { EndpointMissing, ErrorState, Loading, isMissing, useFollowBottom } from "./ui";
import { serverMessage } from "./Steps";

/** Coordinator state; polls every 3s while the chat is open (or busy), 10s otherwise. */
export function useCoord(root: string, open = false) {
  return useQuery({
    queryKey: ["board-coord", root],
    queryFn: () => api.coordGet(root),
    enabled: root !== "",
    refetchInterval: (q) => (isMissing(q.state.error) ? 60000 : open || q.state.data?.busy ? 3000 : 10000),
  });
}

function hhmm(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString("vi-VN", { hour: "2-digit", minute: "2-digit", day: "2-digit", month: "2-digit" });
}

const baseName = (p: string) => p.replace(/\/+$/, "").split("/").pop() || p;

/** Board header button (busy dot) + the chat drawer for the given project. */
export function CoordButton({ root }: { root: string }) {
  const [open, setOpen] = useState(false);
  const q = useCoord(root, open);
  const busy = Boolean(q.data?.busy);
  const off = q.data ? !q.data.enabled : false;
  return (
    <>
      <button
        className="co-btn"
        onClick={() => setOpen(true)}
        disabled={!root}
        aria-haspopup="dialog"
        title={busy ? "Điều phối đang xử lý" : off ? "Điều phối đang tắt" : "Trò chuyện với Điều phối"}
      >
        Điều phối
        {busy ? <span className="co-dot is-busy" aria-label="đang xử lý" /> : off ? <span className="co-dot is-off" aria-label="đang tắt" /> : null}
      </button>
      {/* Portal: the button lives in the page header, whose stacking context would trap the drawer. */}
      {open && createPortal(<CoordPanel root={root} onClose={() => setOpen(false)} />, document.body)}
    </>
  );
}

function CoordPanel({ root, onClose }: { root: string; onClose: () => void }) {
  const qc = useQueryClient();
  const q = useCoord(root, true);
  const [text, setText] = useState("");
  const panelRef = useRef<HTMLElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
    const h = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [onClose]);

  const say = useMutation({
    mutationFn: (v: string) => api.coordSay(root, v),
    onSuccess: () => {
      setText("");
      qc.invalidateQueries({ queryKey: ["board-coord", root] });
    },
  });
  const toggle = useMutation({
    mutationFn: (on: boolean) => api.coordEnabled(root, on),
    onSuccess: (r) => qc.setQueryData<CoordState>(["board-coord", root], (d) => (d ? { ...d, enabled: r.enabled } : d)),
  });

  const d = q.data;
  const enabled = toggle.isPending ? Boolean(toggle.variables) : (d?.enabled ?? true);
  const busy = Boolean(d?.busy) || say.isPending;
  const v = text.trim();
  const submit = () => {
    if (!v || say.isPending) return;
    say.mutate(v);
  };

  return (
    <>
      <div className="bd-scrim" onClick={onClose} />
      <aside className="bd-drawer co-drawer" role="dialog" aria-modal="true" aria-label="Điều phối" tabIndex={-1} ref={panelRef}>
        <header className="bd-dhead">
          <div className="bd-dhead-main">
            <span className="bd-dhead-id">
              <span className="mono faint" title={root}>{baseName(root)}</span>
              {busy ? <span className="tag info"><span className="bd-spin" aria-hidden="true" />Đang xử lý…</span> : null}
            </span>
            <h3>Điều phối</h3>
          </div>
          <button className="icon-btn ghost" onClick={onClose} aria-label="Đóng">✕</button>
        </header>
        <div className="co-power">
          <button
            type="button"
            role="switch"
            aria-checked={enabled}
            className={`co-switch ${enabled ? "on" : ""}`}
            onClick={() => toggle.mutate(!enabled)}
            disabled={!d || toggle.isPending}
          >
            <span className="co-knob" aria-hidden="true" />
            <span>Bật điều phối</span>
          </button>
          <p className="co-power-desc">
            {enabled
              ? "Trợ lý của bạn: duyệt kế hoạch thay bạn, nhắn/tạm dừng bước đang chạy, giao task. Câu hỏi của team do Leader trả lời. Không bao giờ nghiệm thu hay tạo MR."
              : "Đang tắt: Điều phối chỉ trò chuyện, không tự làm gì trên board."}
          </p>
          {toggle.error ? <p className="hint hint-err">{serverMessage(toggle.error as Error)}</p> : null}
        </div>
        <div className="bd-dbody">
          {isMissing(q.error) ? (
            <EndpointMissing endpoint="/api/board/coord" />
          ) : q.isLoading ? (
            <Loading rows={4} />
          ) : q.error ? (
            <ErrorState message={(q.error as Error).message} />
          ) : (
            <Chat msgs={d?.chat ?? []} busy={busy} />
          )}
        </div>
        <footer className="bd-dfoot">
          <textarea
            ref={inputRef}
            className="bd-textarea bd-reply"
            rows={3}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                submit();
              }
            }}
            placeholder="Nhắn Điều phối: hỏi tình hình, giao việc, nhờ xử lý…"
            aria-label="Tin nhắn cho Điều phối"
          />
          {say.error ? <p className="hint hint-err">{serverMessage(say.error as Error)}</p> : null}
          <div className="bd-dfoot-row">
            <span className="faint hint-inline">⌘/Ctrl + Enter để gửi</span>
            <span className="grow-spacer" />
            <button className="primary" onClick={submit} disabled={!v || say.isPending || isMissing(q.error)}>
              {say.isPending ? "Đang gửi…" : "Gửi"}
            </button>
          </div>
        </footer>
      </aside>
    </>
  );
}

function Chat({ msgs, busy }: { msgs: ChatMsg[]; busy: boolean }) {
  const endRef = useRef<HTMLDivElement>(null);
  // Pin the drawer body to the newest message only while the boss is at the bottom.
  useFollowBottom(endRef, `${msgs.length}:${busy}`);
  if (msgs.length === 0 && !busy) {
    return <p className="bd-empty">Chưa có trao đổi. Hỏi trợ lý Điều phối tình hình dự án, hoặc nhờ giao việc, duyệt kế hoạch thay bạn.</p>;
  }
  return (
    <div className="bd-thread co-chat" aria-live="polite">
      {msgs.map((m, i) =>
        m.from === "event" ? (
          <div key={i} className="bd-sys co-event"><span>{m.text}</span> <time className="faint">{hhmm(m.at)}</time></div>
        ) : (
          <div key={i} className={`bd-msg from-${m.from === "boss" ? "boss" : "coordinator"}`}>
            <div className="bd-msg-head">
              <strong>{m.from === "boss" ? "Bạn" : "Điều phối"}</strong>
              <time className="faint">{hhmm(m.at)}</time>
              {m.cost > 0 ? <span className="co-cost mono">${m.cost.toFixed(m.cost < 0.01 ? 4 : 2)}</span> : null}
            </div>
            {m.from === "boss" ? <p className="bd-msg-text">{m.text}</p> : <Markdown source={m.text} />}
          </div>
        ),
      )}
      {busy ? (
        <div className="co-typing"><span className="bd-spin" aria-hidden="true" /> Đang xử lý…</div>
      ) : null}
      <div ref={endRef} />
    </div>
  );
}

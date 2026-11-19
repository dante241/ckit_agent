// Telegram bridge (global, not per project): the board pings the boss on
// Telegram and takes replies. Contract: GET/POST /api/board/telegram,
// POST /api/board/telegram/test, POST /api/board/telegram/off.
// Pairing: the boss sends `/start <pair_code>` to the bot; we poll until paired.
import { useEffect, useState, type FormEvent } from "react";
import { createPortal } from "react-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type TelegramState } from "./api";
import { isMissing } from "./ui";
import { serverMessage } from "./Steps";

const KEY = ["telegram"];

function useTelegram(open: boolean) {
  return useQuery({
    queryKey: KEY,
    queryFn: api.telegramGet,
    // Waiting for `/start <code>`: poll fast while the dialog is open.
    refetchInterval: (q) => {
      const d = q.state.data;
      if (isMissing(q.state.error)) return false;
      return open && d?.configured && !d.paired ? 3000 : false;
    },
  });
}

type Phase = "off" | "pairing" | "on";
const phaseOf = (d?: TelegramState): Phase => (!d?.configured ? "off" : d.paired ? "on" : "pairing");
const TITLE: Record<Phase, string> = {
  off: "Telegram chưa kết nối",
  pairing: "Telegram đang chờ ghép đôi",
  on: "Telegram đã kết nối",
};

/** Board header button (status dot) + the setup dialog. */
export function TelegramButton() {
  const [open, setOpen] = useState(false);
  const q = useTelegram(open);
  if (isMissing(q.error)) return null;
  const ph = phaseOf(q.data);
  return (
    <>
      <button className="co-btn" onClick={() => setOpen(true)} aria-haspopup="dialog" title={TITLE[ph]}>
        Telegram
        <span className={`tg-dot is-${ph}`} aria-label={TITLE[ph]} />
      </button>
      {open && createPortal(<TelegramDialog onClose={() => setOpen(false)} />, document.body)}
    </>
  );
}

function TelegramDialog({ onClose }: { onClose: () => void }) {
  const qc = useQueryClient();
  const q = useTelegram(true);
  const [token, setToken] = useState("");
  const [copied, setCopied] = useState(false);
  const [sent, setSent] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const save = (d: TelegramState) => qc.setQueryData(KEY, d);
  const connect = useMutation({ mutationFn: (t: string) => api.telegramConnect(t), onSuccess: (d) => { save(d); setToken(""); } });
  const test = useMutation({ mutationFn: api.telegramTest, onSuccess: (d) => { save(d); setSent(true); } });
  const off = useMutation({ mutationFn: api.telegramOff, onSuccess: (d) => { save(d); setSent(false); } });
  const err = [connect.error, test.error, off.error].find(Boolean) as Error | undefined;

  const d = q.data;
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const v = token.trim();
    if (v && !connect.isPending) connect.mutate(v);
  };
  const disconnect = () => {
    if (window.confirm("Ngắt kết nối Telegram? Board sẽ không nhắn bạn nữa, muốn dùng lại phải dán token và ghép đôi lại.")) off.mutate();
  };
  const cmd = d?.configured && d.pair_code ? `/start ${d.pair_code}` : "/start";
  const copy = () => {
    void navigator.clipboard?.writeText(cmd).then(() => {
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    });
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal tg-modal" role="dialog" aria-modal="true" aria-label="Telegram" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <strong>Telegram</strong>
          <button type="button" className="icon-btn ghost" onClick={onClose} aria-label="Đóng">✕</button>
        </div>
        <div className="modal-body">
          {q.isLoading ? (
            <p className="faint tg-text">Đang tải…</p>
          ) : q.error ? (
            <p className="hint hint-err tg-text">{serverMessage(q.error as Error)}</p>
          ) : !d?.configured ? (
            <form className="tg-form" onSubmit={submit}>
              <p className="tg-text">Board nhắn bạn qua Telegram khi cần bạn, bạn trả lời ngay trong Telegram.</p>
              <ol className="tg-steps">
                <li>Mở Telegram, nhắn <a href="https://t.me/BotFather" target="_blank" rel="noreferrer">@BotFather</a> lệnh <code>/newbot</code> và đặt tên cho bot.</li>
                <li>BotFather gửi lại một token, dán vào ô dưới.</li>
              </ol>
              <label className="field">
                <span className="field-label">Token của bot</span>
                <input
                  type="password"
                  autoComplete="off"
                  spellCheck={false}
                  value={token}
                  onChange={(e) => setToken(e.target.value)}
                  placeholder="123456789:AA…"
                  autoFocus
                />
              </label>
              <div className="tg-actions">
                <button type="submit" className="primary" disabled={!token.trim() || connect.isPending}>
                  {connect.isPending ? "Đang kiểm tra…" : "Kết nối"}
                </button>
              </div>
            </form>
          ) : !d.paired ? (
            <>
              <p className="tg-text">Bot <strong>@{d.bot}</strong> đã sẵn sàng. Ghép đôi với tài khoản Telegram của bạn:</p>
              <ol className="tg-steps">
                <li>Mở <a href={`https://t.me/${d.bot}`} target="_blank" rel="noreferrer">t.me/{d.bot}</a></li>
                <li>
                  Gửi lệnh
                  <span className="tg-cmd">
                    <code>{cmd}</code>
                    <button type="button" onClick={copy}>{copied ? "Đã chép" : "Chép"}</button>
                  </span>
                </li>
              </ol>
              <p className="tg-wait"><span className="bd-spin" aria-hidden="true" /> Đang chờ bạn gửi lệnh…</p>
              <div className="tg-actions">
                <button type="button" className="danger" onClick={disconnect} disabled={off.isPending}>Huỷ, dùng bot khác</button>
              </div>
            </>
          ) : (
            <>
              <p className="tg-on"><span className="tg-dot is-on" aria-hidden="true" /> Đã kết nối <a href={`https://t.me/${d.bot}`} target="_blank" rel="noreferrer">@{d.bot}</a></p>
              <p className="tg-text muted">
                Board nhắn khi: cần duyệt kế hoạch, cần nghiệm thu, team hỏi bạn, việc chờ bấm Bắt đầu, có lỗi. Trả lời bằng reply hoặc bấm nút trong tin.
              </p>
              {sent && !test.isPending ? <p className="hint hint-ok tg-text">Đã gửi tin thử, mở Telegram để xem.</p> : null}
              <div className="tg-actions">
                <button type="button" onClick={() => { setSent(false); test.mutate(); }} disabled={test.isPending}>
                  {test.isPending ? "Đang gửi…" : "Gửi thử"}
                </button>
                <button type="button" className="danger" onClick={disconnect} disabled={off.isPending}>Ngắt kết nối</button>
              </div>
            </>
          )}
          {err ? <p className="hint hint-err tg-text">{serverMessage(err)}</p> : null}
        </div>
      </div>
    </div>
  );
}

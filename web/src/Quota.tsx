// "Quota" in the Board header: how much of the subscription the running tasks
// draw from is used up. Contract: GET /api/board/usage (omp usage, refreshed
// server-side at most once a minute, accounts redacted). The button shows the
// most-used window of the accounts running tasks use (all accounts when nothing
// runs); the panel lists every account and window.
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { useQuery } from "@tanstack/react-query";
import { api, type BoardUsage, type UsageAccount, type UsageWindow } from "./api";
import { isMissing } from "./ui";
import { serverMessage } from "./Steps";

const tone = (pct: number) => (pct > 90 ? "is-err" : pct > 70 ? "is-warn" : "is-ok");

function useUsage() {
  return useQuery({
    queryKey: ["board-usage"],
    queryFn: api.boardUsage,
    // First call starts the server's fetch: ask again soon until it lands.
    refetchInterval: (q) => (isMissing(q.state.error) ? false : q.state.data?.loading ? 3000 : 60000),
  });
}

type Peak = { acc: UsageAccount; w: UsageWindow };

/** Most-used window over the accounts running tasks use, else over every account. */
function peak(d: BoardUsage | undefined): Peak | null {
  const accs = d?.accounts ?? [];
  const used = accs.filter((a) => a.tasks.length > 0);
  let best: Peak | null = null;
  for (const acc of used.length ? used : accs) {
    for (const w of acc.windows) if (!best || w.used_pct > best.w.used_pct) best = { acc, w };
  }
  return best;
}

/** "2 giờ 10 phút" until `ms` (epoch), "" when unknown or past. */
function until(ms: number | null, now: number): string {
  if (!ms || ms <= now) return "";
  const m = Math.ceil((ms - now) / 60000);
  const d = Math.floor(m / 1440);
  const h = Math.floor((m % 1440) / 60);
  if (d > 0) return `${d} ngày${h ? ` ${h} giờ` : ""}`;
  if (h > 0) return `${h} giờ ${m % 60} phút`;
  return `${m} phút`;
}

const pctText = (p: number) => `${Math.round(p)}%`;

function Bar({ pct, label }: { pct: number; label: string }) {
  return (
    <span className={`qt-bar ${tone(pct)}`} role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}>
      <span style={{ width: `${Math.min(100, Math.max(0, pct))}%` }} />
    </span>
  );
}

export function QuotaButton() {
  const [open, setOpen] = useState(false);
  const q = useUsage();
  if (isMissing(q.error)) return null;
  const p = peak(q.data);
  const title = p
    ? `Quota: ${p.w.label} đã dùng ${pctText(p.w.used_pct)} (${p.acc.provider} ${p.acc.label})`
    : q.data?.error
      ? `Quota: ${q.data.error}`
      : "Quota: đang lấy";
  return (
    <>
      <button className="co-btn qt-btn" onClick={() => setOpen(true)} aria-haspopup="dialog" aria-label={title} title={title}>
        Quota
        {p ? (
          <span className="qt-btn-meter" aria-hidden="true">
            <Bar pct={p.w.used_pct} label={title} />
            <span className={`qt-pct ${tone(p.w.used_pct)}`}>{pctText(p.w.used_pct)}</span>
          </span>
        ) : (
          <span className="tg-dot is-off" aria-hidden />
        )}
      </button>
      {open && createPortal(<QuotaPanel onClose={() => setOpen(false)} />, document.body)}
    </>
  );
}

function QuotaPanel({ onClose }: { onClose: () => void }) {
  const q = useUsage();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    document.addEventListener("keydown", onKey);
    const tick = window.setInterval(() => setNow(Date.now()), 30000);
    return () => {
      document.removeEventListener("keydown", onKey);
      window.clearInterval(tick);
    };
  }, [onClose]);
  const d = q.data;
  const unassigned = (d?.running ?? []).filter((t) => !d?.accounts.some((a) => a.tasks.some((x) => x.project === t.project && x.id === t.id)));

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal qt-modal" role="dialog" aria-modal="true" aria-label="Quota tài khoản AI" onClick={(e) => e.stopPropagation()}>
        <div className="modal-head">
          <strong>Quota tài khoản AI</strong>
          <button type="button" className="icon-btn ghost" onClick={onClose} aria-label="Đóng">✕</button>
        </div>
        <div className="modal-body">
          {q.error ? (
            <p className="hint hint-err tg-text">{serverMessage(q.error as Error)}</p>
          ) : !d || d.loading ? (
            <p className="faint tg-text">Đang lấy quota từ omp…</p>
          ) : (
            <>
              {d.error ? <p className="qt-soft">{d.error}{d.accounts.length ? ", đang hiện số liệu lần trước." : "."}</p> : null}
              {d.accounts.length === 0 && !d.error ? <p className="faint tg-text">omp chưa đăng nhập tài khoản nào có quota (omp usage trống).</p> : null}
              {d.accounts.map((a, i) => (
                <section key={i} className={`qt-acc${a.tasks.length ? " is-used" : ""}`} aria-label={`${a.provider} ${a.label}`}>
                  <div className="qt-acc-head">
                    <strong>{a.provider}</strong>
                    {a.tasks.length ? <span className="tag accent">đang dùng</span> : null}
                    <span className="mono qt-acc-id">{a.label}</span>
                    {a.org ? <span className="faint">{a.org}</span> : null}
                    {a.plan ? <span className="tag">{a.plan}</span> : null}
                    {a.limit_reached ? <span className="tag err">Hết quota</span> : null}
                  </div>
                  {a.windows.length ? (
                    <ul className="qt-wins">
                      {a.windows.map((w) => {
                        const left = until(w.resets_at, now);
                        return (
                          <li key={w.id + w.label} className="qt-win">
                            <span className="qt-win-label">{w.label || w.id}</span>
                            <Bar pct={w.used_pct} label={`${w.label} đã dùng ${pctText(w.used_pct)}`} />
                            <span className={`qt-pct ${tone(w.used_pct)}`}>{pctText(w.used_pct)}</span>
                            <span className="qt-reset">{left ? `đặt lại sau ${left}` : ""}</span>
                          </li>
                        );
                      })}
                    </ul>
                  ) : (
                    <p className="faint qt-none">Không có số liệu giới hạn.</p>
                  )}
                  {a.tasks.length ? (
                    <ul className="qt-tasks" aria-label="Đang dùng tài khoản này">
                      {a.tasks.map((t) => (
                        <li key={t.project + t.id}>
                          <span className="mono">{t.id}</span> {t.title} <span className="faint">· {t.project} · {t.model}{t.effort ? ` · ${t.effort}` : ""}</span>
                        </li>
                      ))}
                    </ul>
                  ) : null}
                </section>
              ))}
              {unassigned.length ? (
                <p className="qt-soft">
                  Chưa rõ tài khoản: {unassigned.map((t) => `${t.id} (${t.provider}/${t.model})`).join(", ")}
                </p>
              ) : null}
              {d.disabled.length ? (
                <p className="faint qt-dis">Tài khoản bị tắt: {d.disabled.map((x) => `${x.provider} ${x.label}`).join(", ")}</p>
              ) : null}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

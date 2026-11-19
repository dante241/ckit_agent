// Shared page scaffolding + interaction states, used by every page (App.tsx)
// and by pages split into their own module (Board.tsx).
import { useEffect, useLayoutEffect, useRef, type ReactNode, type RefObject } from "react";
import { EndpointMissingError } from "./api";
import { Glyph } from "./icons";

/** App-shell page: header (title, summary, actions, optional toolbar) stays put; only the body
 *  scrolls. `fill` hands the body's full height to its content (Bảng columns scroll themselves). */
export function Page({ title, sub, action, toolbar, fill, children }: { title: string; sub?: ReactNode; action?: ReactNode; toolbar?: ReactNode; fill?: boolean; children: ReactNode }) {
  return (
    <section className="page">
      <div className="page-top">
        <header className="page-head">
          <div>
            <h2>{title}</h2>
            {sub ? <p className="sub">{sub}</p> : null}
          </div>
          {action ? <div className="page-action">{action}</div> : null}
        </header>
        {toolbar}
      </div>
      <div className={`page-body${fill ? " is-fill" : ""}`}>{children}</div>
    </section>
  );
}

/** Nearest ancestor that scrolls vertically (a pane), never the window. */
export function scrollPane(el: Element | null): HTMLElement | null {
  for (let n = el?.parentElement ?? null; n; n = n.parentElement) {
    if (/(auto|scroll)/.test(getComputedStyle(n).overflowY)) return n;
  }
  return null;
}

/** Scroll `el` into view inside its own pane only (scrollIntoView would also scroll every
 *  ancestor, overflow:hidden ones included, shifting the whole shell). */
export function revealInPane(el: Element | null, smooth = false) {
  const pane = scrollPane(el);
  if (!el || !pane) return;
  const e = el.getBoundingClientRect();
  const p = pane.getBoundingClientRect();
  const top = e.height > p.height ? e.top - p.top : e.top + e.height / 2 - (p.top + p.height / 2);
  pane.scrollBy({ top, behavior: smooth ? "smooth" : "auto" });
}

/** Chat/log follow: keeps the pane holding `ref` pinned to the bottom when `dep` changes, but
 *  only while the reader is already near the bottom; starts at the bottom (latest). */
export function useFollowBottom(ref: RefObject<HTMLElement | null>, dep: unknown) {
  // Pane height when we last looked: the reader was "near the bottom" if they still see its end.
  const last = useRef<number | null>(null);
  useLayoutEffect(() => {
    const p = scrollPane(ref.current);
    if (!p) return;
    const prev = last.current;
    if (prev === null || p.scrollTop + p.clientHeight >= prev - 48) p.scrollTop = p.scrollHeight;
    last.current = p.scrollHeight;
  }, [ref, dep]);
}

export function Loading({ rows = 4 }: { rows?: number }) {
  return (
    <div className="card" aria-busy="true" aria-live="polite">
      <div className="skeleton">
        {Array.from({ length: rows }).map((_, i) => (
          <div className="skeleton-row" key={i}>
            <span className="skel" style={{ width: `${68 - i * 9}%` }} />
            <span className="skel skel-tag" />
          </div>
        ))}
      </div>
    </div>
  );
}

// Type guard — keeps EndpointMissingError narrowing live at call sites.
export function isMissing(e: unknown): boolean {
  return e instanceof EndpointMissingError;
}

export function ErrorState({ message }: { message: string }) {
  return (
    <div className="card state-msg state-error" role="alert">
      <span className="state-ico" aria-hidden="true">!</span>
      <div>
        <strong>Couldn’t load this</strong>
        <p className="mono">{message}</p>
      </div>
    </div>
  );
}

// Honest "endpoint not ready" banner — used when the backend route isn't live.
export function EndpointMissing({ endpoint }: { endpoint: string }) {
  return (
    <div className="card missing state-msg">
      <span className="state-ico" aria-hidden="true"><Glyph name="alert" /></span>
      <div>
        <strong>Waiting on the backend</strong>
        <p>{endpoint} isn’t live in the running server yet. The screen fills in automatically once it ships; nothing to do here.</p>
      </div>
    </div>
  );
}

export function EmptyState({ title, hint, icon }: { title: string; hint?: ReactNode; icon?: ReactNode }) {
  return (
    <div className="card empty">
      <span className="empty-orb" aria-hidden="true">{icon}</span>
      <strong>{title}</strong>
      {hint ? <p className="muted">{hint}</p> : null}
    </div>
  );
}

/** Right-side panel over a scrim. Esc closes it first (capture phase, so the page's own Esc
 *  "one level up" does not fire too); focus returns to the opener. */
export function Drawer({ label, head, toolbar, onClose, children, className = "" }: { label: string; head: ReactNode; toolbar?: ReactNode; onClose: () => void; children: ReactNode; className?: string }) {
  const ref = useRef<HTMLElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    ref.current?.focus({ preventScroll: true });
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      close.current();
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      opener?.focus?.({ preventScroll: true });
    };
  }, []);
  return (
    <>
      <div className="bd-scrim" onClick={onClose} />
      <aside className={`bd-drawer ${className}`.trim()} role="dialog" aria-modal="true" aria-label={label} tabIndex={-1} ref={ref}>
        <header className="bd-dhead">
          <div className="bd-dhead-main">{head}</div>
          <button className="dr-close" onClick={onClose}>Đóng</button>
        </header>
        {toolbar}
        <div className="bd-dbody">{children}</div>
      </aside>
    </>
  );
}

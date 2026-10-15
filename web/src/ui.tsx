// Shared page scaffolding + interaction states, used by every page (App.tsx)
// and by pages split into their own module (Board.tsx).
import type { ReactNode } from "react";
import { EndpointMissingError } from "./api";
import { Glyph } from "./icons";

export function Page({ title, sub, action, children }: { title: string; sub?: ReactNode; action?: ReactNode; children: ReactNode }) {
  return (
    <section className="page">
      <header className="page-head">
        <div>
          <h2>{title}</h2>
          {sub ? <p className="sub">{sub}</p> : null}
        </div>
        {action ? <div className="page-action">{action}</div> : null}
      </header>
      {children}
    </section>
  );
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

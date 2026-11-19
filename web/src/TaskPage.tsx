// Task page: one Board task as a full page with real browser history.
// URL: ?page=board&task=T-002&root=<abs>[&phase=M1][&sub=T4]. Every level is a
// history entry, so Back / Forward / reload / shared links all land where the
// boss was. Levels: task (timeline of runs by phase) → phase page → one sub-task (drawer).
// Log and documents open in a right drawer (Esc closes it before leaving the level).
import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, type BoardDetail, type BoardPhases } from "./api";
import { EndpointMissing, ErrorState, Loading, isMissing, revealInPane } from "./ui";
import { AskPanel, acGateOf, askingYou } from "./Asks";
import { GoalsPage } from "./Goals";
import { ChatPanel } from "./Chat";
import { ACCEPT_MR_EVENT, AcceptMrDrawer } from "./MrCard";
import { usePhases } from "./Epic";
import { useEscape } from "./Board";
import { LogDrawer } from "./RunLog";
import { DocsDrawer } from "./TaskDocs";
import { PhasePage, SubDrawer } from "./PhasePage";
import { TaskBody, TaskUiCtx, rememberPhase, type TaskUi } from "./Timeline";
import { Crumbs, LiveLine, PhaseNav, TaskBar, type Crumb } from "./TaskTop";

// ── Route ──────────────────────────────────────────────────────────────────
/** `view: "goals"` = the Mục tiêu page (with `phase` as its filter, not the phase page). */
export type TaskRoute = { task: string; root: string; phase: string | null; sub: string | null; view?: "goals" | null };
const KEYS = ["task", "root", "phase", "sub", "view"] as const;
const ROUTE_EVENT = "ckit:route";

// A card button that opens the page lands the boss where they act: the open
// question, the reply box, or the gate's primary button. One-shot, set right
// before the navigation.
export type OpenFocus = "ask" | "reply" | "act";
let pendingFocus: OpenFocus | null = null;
export function focusOnOpen(f: OpenFocus) {
  pendingFocus = f;
}
function useOpenFocus() {
  useEffect(() => {
    const f = pendingFocus;
    pendingFocus = null;
    if (!f) return;
    const h = window.setTimeout(() => {
      const el = document.querySelector<HTMLElement>(f === "ask" ? ".ask-panel" : ".tp-act");
      revealInPane(el);
      const target = f === "act" ? el?.querySelector<HTMLElement>(".tp-act-btns .primary") : el?.querySelector<HTMLElement>("textarea, input, .ask-opt");
      target?.focus({ preventScroll: true });
    }, 0);
    return () => window.clearTimeout(h);
  }, []);
}

export function routeFromUrl(): TaskRoute | null {
  const p = new URLSearchParams(window.location.search);
  const task = p.get("task");
  if (!task || (p.get("page") ?? "board") !== "board") return null;
  return { task, root: p.get("root") ?? "", phase: p.get("phase"), sub: p.get("sub"), view: p.get("view") === "goals" ? "goals" : null };
}

function urlFor(r: TaskRoute | null): string {
  const p = new URLSearchParams(window.location.search);
  for (const k of KEYS) p.delete(k);
  p.set("page", "board");
  if (r) {
    p.set("task", r.task);
    if (r.root) p.set("root", r.root);
    if (r.phase) p.set("phase", r.phase);
    if (r.sub) p.set("sub", r.sub);
    if (r.view) p.set("view", r.view);
  }
  return `?${p}`;
}

// Same page regardless of param order or the bare "." the shell uses for the Board.
function norm(search: string): string {
  const p = new URLSearchParams(search);
  if (!p.get("page")) p.set("page", "board");
  p.sort();
  return p.toString();
}

/** Push a new level (or the Board when `r` is null). No-op when already there. */
export function goTo(r: TaskRoute | null) {
  const url = urlFor(r);
  if (norm(url) === norm(window.location.search)) return;
  window.history.pushState({ ckitFrom: window.location.search }, "", url);
  window.dispatchEvent(new Event(ROUTE_EVENT));
}

/** One level up. Steps back when the previous entry is that level, so history stays linear. */
export function goUp(r: TaskRoute) {
  // Mục tiêu goes back to where it was opened from (task level or that phase page).
  const parent = r.view ? { ...r, view: null, phase: null, sub: null } : r.sub ? { ...r, sub: null } : r.phase ? { ...r, phase: null } : null;
  const from = (window.history.state as { ckitFrom?: string } | null)?.ckitFrom;
  // A sub-task opened from the default (unpicked) phase came from the task level, which shows that same phase.
  const ups = [parent, ...(r.sub && r.phase ? [{ ...r, phase: null, sub: null }] : []), ...(r.view && r.phase ? [{ ...r, view: null, sub: null }] : [])].map((x) => norm(urlFor(x)));
  if (from !== undefined && ups.includes(norm(from))) window.history.back();
  else goTo(parent);
}

/** Current route; follows pushes and Back / Forward. */
export function useTaskRoute(): TaskRoute | null {
  const [r, setR] = useState(routeFromUrl);
  useEffect(() => {
    const on = () => setR(routeFromUrl());
    window.addEventListener("popstate", on);
    window.addEventListener(ROUTE_EVENT, on);
    return () => {
      window.removeEventListener("popstate", on);
      window.removeEventListener(ROUTE_EVENT, on);
    };
  }, []);
  return r;
}

// ── Page ───────────────────────────────────────────────────────────────────
export function TaskPage({ route, updated }: { route: TaskRoute; updated?: string }) {
  const qc = useQueryClient();
  const { task: id, root } = route;
  const detail = useQuery({
    queryKey: ["board-task", root, id],
    queryFn: () => api.boardTask(root, id),
    refetchInterval: (q) => {
      const task = q.state.data?.task;
      // Also while an action runs on a waiting / done task (acceptance MR, chat action).
      return task && (task.status === "running" || task.status === "queued" || task.runs.some((r) => r.ended === null)) ? 2000 : false;
    },
  });
  // The board poll sees changes first (e.g. a queued task promoted); follow it.
  useEffect(() => {
    if (updated) qc.invalidateQueries({ queryKey: ["board-task", root, id] });
  }, [updated, id, root, qc]);
  useEscape(() => goUp(route));

  if (detail.data) return <TaskView d={detail.data} route={route} />;
  return (
    <div className="tp">
      <header className="tp-bar"><Crumbs items={[{ label: "Board", onClick: () => goTo(null) }, { label: id, strong: true }]} onBack={() => goUp(route)} /></header>
      {isMissing(detail.error) ? (
        <EndpointMissing endpoint={`/api/board/${id}`} />
      ) : detail.error ? (
        <ErrorState message={/^404/.test((detail.error as Error).message) ? `Không tìm thấy ${id} trong dự án này.` : (detail.error as Error).message} />
      ) : (
        <Loading rows={5} />
      )}
    </div>
  );
}

type Panel = { kind: "log"; run: number } | { kind: "docs"; doc?: string } | { kind: "accept" };
const NO_PHASES: BoardPhases = { phases: [], flat: [], live: false };

function TaskView({ d, route }: { d: BoardDetail; route: TaskRoute }) {
  const t = d.task;
  const q = usePhases(route.root, t);
  // Without the phases endpoint the timeline still works (one group, no rail).
  const p = q.data ?? (q.error ? NO_PHASES : undefined);
  const [panel, setPanel] = useState<Panel | null>(null);
  useOpenFocus();
  // "Nghiệm thu: tạo MR" in the action bar opens the MR card.
  useEffect(() => {
    const on = () => setPanel({ kind: "accept" });
    window.addEventListener(ACCEPT_MR_EVENT, on);
    return () => window.removeEventListener(ACCEPT_MR_EVENT, on);
  }, []);

  const phases = p?.phases ?? [];
  const goals = route.view === "goals";
  const sel = route.phase && !goals ? phases.find((ph) => ph.id === route.phase) : undefined;
  const subList = sel?.tasks ?? (p?.flat.length ? p.flat : phases.flatMap((ph) => ph.tasks));
  const subRow = route.sub ? subList.find((s) => s.key === route.sub) : undefined;
  const top: TaskRoute = { ...route, phase: null, sub: null, view: null };
  const ui: TaskUi = {
    openLog: (run) => setPanel({ kind: "log", run }),
    openDocs: (doc) => setPanel({ kind: "docs", doc }),
    openPhase: (id) => goTo({ ...route, phase: id, sub: null, view: null }),
    openSub: (phase, key) => goTo({ ...route, phase, sub: key, view: null }),
    openGoals: (phase) => goTo({ ...route, phase: phase ?? null, sub: null, view: "goals" }),
  };

  // The task title is the task crumb; phase and sub-task follow it.
  const crumbs: Crumb[] = [
    { label: "Board", onClick: () => goTo(null) },
    { label: <><span className="mono tp-id">{t.id}</span> {t.title}</>, title: `${t.id} ${t.title}`, strong: true, onClick: () => goTo(top) },
  ];
  if (goals) crumbs.push({ label: "Mục tiêu" });
  else if (route.phase) crumbs.push({ label: sel ? `${sel.id}${sel.name ? ` ${sel.name}` : ""}` : route.phase, onClick: () => goTo({ ...route, sub: null }) });
  if (route.sub) crumbs.push({ label: subRow ? `${subRow.key} ${subRow.title}` : route.sub });
  const logRun = panel?.kind === "log" ? t.runs[panel.run] : undefined;
  const atPhase = Boolean(route.phase) && !goals;
  if (sel) rememberPhase(t.id, sel.id);
  // The tab title says where you are: "M1 Read path core · T-004 … · Ckit Harness".
  const docTitle = `${goals ? "Mục tiêu · " : sel ? `${sel.id}${sel.name ? ` ${sel.name}` : ""} · ` : ""}${t.id} ${t.title} · Ckit Harness`;
  useEffect(() => {
    const prev = document.title;
    document.title = docTitle;
    return () => { document.title = prev; };
  }, [docTitle]);

  return (
    <TaskUiCtx.Provider value={ui}>
      <div className={`tp ${goals ? "is-goals" : atPhase ? "is-phase" : "is-task"}`}>
        <div className="tp-top">
          <TaskBar d={d} p={p} root={route.root} crumbs={crumbs} onBack={() => goUp(route)} view={route.view ?? null} />
          <LiveLine d={d} p={p} />
          {/* On Mục tiêu the cards sit in the page next to the AC table, not pinned. */}
          {(askingYou(t) || acGateOf(t)) && !goals ? <AskPanel task={t} root={route.root} /> : null}
          {atPhase && phases.length ? <PhaseNav t={t} phases={phases} sel={route.phase} onPick={ui.openPhase} /> : null}
        </div>

        {!p ? (
          <Loading rows={4} />
        ) : (
          <>
            {isMissing(q.error) ? <EndpointMissing endpoint={`/api/board/${t.id}/phases`} /> : q.error ? <ErrorState message={(q.error as Error).message} /> : null}
            {goals ? <GoalsPage d={d} p={p} root={route.root} phase={route.phase} /> : atPhase ? <PhasePage d={d} p={p} root={route.root} phase={route.phase!} /> : <TaskBody d={d} p={p} root={route.root} />}
          </>
        )}
        <ChatPanel t={t} root={route.root} />

        {route.sub ? (
          <SubDrawer key={route.sub} s={subRow} subKey={route.sub} list={subList} phase={sel ?? null} d={d} onClose={() => goUp(route)} onOpen={(k) => ui.openSub(route.phase, k)} />
        ) : null}
        {panel?.kind === "log" && logRun ? (
          <LogDrawer key={panel.run} task={t} run={panel.run} phaseName={phases.find((ph) => ph.id === logRun.phase)?.name} onClose={() => setPanel(null)} />
        ) : null}
        {panel?.kind === "docs" ? <DocsDrawer d={d} root={route.root} pick={panel.doc} onClose={() => setPanel(null)} /> : null}
        {panel?.kind === "accept" ? <AcceptMrDrawer t={t} root={route.root} onClose={() => setPanel(null)} /> : null}
      </div>
    </TaskUiCtx.Provider>
  );
}

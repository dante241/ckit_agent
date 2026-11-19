// Sidebar "Dự án": every project's live state at a glance (status monitor).
// The switcher at the top stays the picker; this section answers "who is
// running, what needs me" without opening a page. Same poll as the Board's
// nav badge (["board", "all"]).
import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api, type BoardTaskSummary, type ProjectBoard } from "./api";
import { Glyph } from "./icons";
import { askingYou } from "./Asks";
import { STAGE_WHO, needLabel, useBoardAll } from "./Board";
import { goTo } from "./TaskPage";
import { ModelChip } from "./ModelChip";

const OPEN_KEY = "ckit.nav.projects";

type Tone = "need" | "fail" | "run" | "idle";
type Line = { task: BoardTaskSummary; text: string; tone: Tone; model?: boolean };

const needsYou = (t: BoardTaskSummary) => t.status === "waiting" || askingYou(t);

// Minutes since `iso`: 12' or 1h05'.
function elapsed(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  const m = Math.max(0, Math.floor((Date.now() - t) / 60000));
  if (m < 1) return "<1'";
  if (m < 60) return `${m}'`;
  return `${Math.floor(m / 60)}h${String(m % 60).padStart(2, "0")}'`;
}

// Same precedence as the all-projects sort: needs you, then failed, then running.
function summarize(p: ProjectBoard): { tone: Tone; need: number; failed: number; lines: Line[] } {
  const need = p.tasks.filter(needsYou);
  const failed = p.tasks.filter((t) => t.status === "failed");
  const running = p.tasks.filter((t) => t.status === "running" && !askingYou(t));
  const queued = p.tasks.filter((t) => t.status === "queued" && !askingYou(t));
  const n = { need: need.length, failed: failed.length };
  const runLine = (t: BoardTaskSummary): Line => {
    const when = elapsed(t.run_started ?? t.updated);
    return { task: t, tone: "run", model: true, text: `${STAGE_WHO[t.stage] || "AI"} đang làm${when ? ` · ${when}` : ""}` };
  };
  // One line: what needs the boss (or failed) first, else what is running, else the queue.
  const first: Line | null = need[0]
    ? { task: need[0], tone: "need", text: needLabel(need[0]) }
    : failed[0]
      ? { task: failed[0], tone: "fail", text: "Lỗi" }
      : null;
  const line = first ?? (running[0] ? runLine(running[0]) : queued[0] ? { task: queued[0], tone: "idle" as const, text: "Chờ lượt" } : null);
  const lines: Line[] = line ? [line] : [];
  const tone: Tone = first ? first.tone : running[0] ? "run" : "idle";
  return { tone, ...n, lines };
}

const TONE_LABEL: Record<Tone, string> = { need: "Cần bạn", fail: "Có lỗi", run: "Đang chạy", idle: "Rảnh" };

// Board of the picked project, kanban view; App + BoardPage follow popstate.
function openBoard() {
  window.history.pushState(null, "", "?page=board&view=kanban");
  window.dispatchEvent(new PopStateEvent("popstate"));
}

export function SidebarProjects() {
  const qc = useQueryClient();
  const { data } = useBoardAll();
  const [open, setOpenState] = useState(() => localStorage.getItem(OPEN_KEY) !== "0");
  const setOpen = (v: boolean) => {
    setOpenState(v);
    localStorage.setItem(OPEN_KEY, v ? "1" : "0");
  };
  const pick = useMutation({
    mutationFn: (root: string) => api.activateProject(root),
    onSuccess: () => {
      qc.invalidateQueries();
      openBoard();
    },
  });

  const projects = data?.projects ?? [];
  if (projects.length === 0) return null;
  const rows = projects.map((p) => ({ p, s: summarize(p) }));
  const runN = projects.reduce((s, p) => s + p.running, 0);
  const needN = rows.reduce((s, r) => s + r.s.need, 0);
  const failN = rows.reduce((s, r) => s + r.s.failed, 0);
  const summary = [
    runN > 0 && `${runN} chạy`,
    needN > 0 && `${needN} chờ`,
    failN > 0 && `${failN} lỗi`,
  ].filter(Boolean).join(" · ");

  const openProject = (p: ProjectBoard) => {
    if (p.current) openBoard();
    else pick.mutate(p.project);
  };
  const openTask = (p: ProjectBoard, t: BoardTaskSummary) => {
    goTo({ task: t.id, root: p.project, phase: null, sub: null });
    window.dispatchEvent(new PopStateEvent("popstate"));
  };

  return (
    <div className="nav-section sp">
      <button className="nav-more sp-head" aria-expanded={open} aria-controls="sp-list" onClick={() => setOpen(!open)}>
        <span>Dự án</span>
        {summary && <span className="sp-sum">{summary}</span>}
        <span className="nav-more-chev"><Glyph name="chevron" /></span>
      </button>
      {open && (
        <ul id="sp-list" className="sp-list" aria-label="Trạng thái dự án">
          {rows.map(({ p, s }) => (
            <li key={p.project} className={`sp-row${p.current ? " is-current" : ""}`}>
              <button
                className="sp-main"
                onClick={() => openProject(p)}
                disabled={pick.isPending}
                aria-current={p.current ? "true" : undefined}
                title={`${p.name}: ${TONE_LABEL[s.tone]}. Mở bảng\n${p.project}`}
              >
                <span className={`sp-dot is-${s.tone}`} role="img" aria-label={TONE_LABEL[s.tone]} />
                <span className="sp-name">{p.name}</span>
                {pick.isPending && pick.variables === p.project ? (
                  <span className="sp-busy">Đang mở…</span>
                ) : s.need + s.failed > 0 ? (
                  // One count, amber when something waits on the boss; the dot already says question vs error.
                  <span className={`sp-count${s.need > 0 ? " is-need" : ""}`} aria-label={[s.need > 0 && `${s.need} chờ bạn`, s.failed > 0 && `${s.failed} lỗi`].filter(Boolean).join(", ")}>
                    {s.need + s.failed}
                  </span>
                ) : null}
              </button>
              {s.lines.map((l) => (
                <button key={l.task.id} className={`sp-task is-${l.tone}`} onClick={() => openTask(p, l.task)} title={l.task.title}>
                  <span className="sp-task-id mono">{l.task.id}</span>
                  <span className="sp-task-text">{l.text}</span>
                  {l.model ? <ModelChip m={l.task.run_model} className="sp-task-model" /> : null}
                </button>
              ))}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

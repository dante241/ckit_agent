// Board — kanban for the AI delivery pipeline. The user only talks to the
// Leader; AI stages run one at a time per project and two user gates
// (approval, acceptance) sit between them. Contract: /api/board*.
import { createContext, useContext, useEffect, useRef, useState, type FormEvent, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { useMutation, useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  type BoardDetail,
  type BoardLogLine,
  type BoardMsg,
  type BoardRun,
  type BoardStage,
  type BoardStatus,
  type BoardTask,
  type BoardTaskSummary,
  type PmEntry,
  type ProjectBoard,
} from "./api";
import { Markdown } from "./markdown";
import { Glyph } from "./icons";
import { Page, Loading, isMissing, ErrorState, EndpointMissing, EmptyState } from "./ui";
import { FULL_STEPS, StepPicker, hasStepAfter, serverMessage, useStepCatalog, validateSteps } from "./Steps";
import { answeringSeats, askingYou, senderLabel } from "./Asks";
import { Interject, isNew, isPaused, pauseLabel } from "./Interject";
import { CoordButton, useCoord } from "./Coordinator";
import { TaskPage, goTo, useTaskRoute } from "./TaskPage";
import { TelegramButton } from "./Telegram";
import { EpicBoard, laneOf } from "./EpicBoard";
import { StatusStrip, matches, type StripFilter } from "./StatusStrip";
import { CardAction, cardAction } from "./CardActions";
import { WorkflowChip, WorkflowPicker, WorkflowSteps, taskStageLabel } from "./Workflows";

// ── Vocabulary ─────────────────────────────────────────────────────────────
// Board stage → the `/feature` step it runs.
export const STAGE_LABEL: Record<BoardStage, string> = {
  investigate: "Điều tra",
  intake: "Tiếp nhận (new)",
  solution: "Kế hoạch phase (plan)",
  review: "PO duyệt",
  approval: "Bạn duyệt kế hoạch",
  dev: "Phát triển (go)",
  test: "Kiểm thử (ship)",
  security: "Review bảo mật",
  report: "Báo cáo",
  acceptance: "Nghiệm thu",
  done: "Hoàn tất",
  cancelled: "Đã huỷ",
};

// Who works the stage. User gates read "Bạn".
export const STAGE_WHO: Record<BoardStage, string> = {
  investigate: "Leader",
  intake: "Leader",
  solution: "Leader + BA",
  review: "Leader (PO)",
  approval: "Bạn",
  dev: "Dev",
  test: "Tester",
  security: "Reviewer bảo mật",
  report: "Leader",
  acceptance: "Bạn",
  done: "",
  cancelled: "",
};

// investigate waits like a gate (answer → ask more / continue / close).
const GATES = new Set<BoardStage>(["investigate", "approval", "acceptance"]);

// What the user has to do for a waiting (or asking) task, by stage.
export function needLabel(t: { stage: BoardStage; status: BoardStatus; asking?: number; paused?: boolean; note?: string | null }): string {
  if (askingYou(t)) return "Trả lời câu hỏi";
  if (isPaused(t)) return isNew(t) ? "Bấm Bắt đầu" : "Đang tạm dừng";
  const stage = t.stage;
  if (stage === "approval") return "Duyệt kế hoạch";
  if (stage === "investigate") return "Đọc trả lời";
  if (stage === "acceptance") return "Nghiệm thu";
  return "Trả lời Leader";
}

export const isTerminal = (t: { stage: BoardStage; status: BoardStatus }) =>
  t.status === "done" || t.status === "cancelled" || t.stage === "done" || t.stage === "cancelled";
// Waiting on the user, failed, or a running step asking the boss a question.
const needsYou = (t: BoardTaskSummary) => t.status === "waiting" || t.status === "failed" || askingYou(t);
const askingN = (p: ProjectBoard) => p.tasks.filter(askingYou).length;

// ── Formatting ─────────────────────────────────────────────────────────────
export function money(c: number): string {
  if (!c) return "$0";
  if (c < 0.01) return "<$0.01";
  return `$${c.toFixed(2)}`;
}

export function ago(iso: string): string {
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return "";
  const s = Math.max(0, (Date.now() - t) / 1000);
  if (s < 45) return "vừa xong";
  if (s < 3600) return `${Math.round(s / 60)} phút trước`;
  if (s < 86400) return `${Math.round(s / 3600)} giờ trước`;
  return `${Math.round(s / 86400)} ngày trước`;
}

export function clock(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString("vi-VN", { hour: "2-digit", minute: "2-digit", day: "2-digit", month: "2-digit" });
}

function duration(run: BoardRun): string {
  const a = Date.parse(run.started);
  const b = run.ended ? Date.parse(run.ended) : Date.now();
  if (Number.isNaN(a) || Number.isNaN(b)) return "";
  const s = Math.max(0, Math.round((b - a) / 1000));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

// ── Data ───────────────────────────────────────────────────────────────────
// Shared by the page and the nav badge (same key → one poll). A missing route
// backs off to a slow probe so the page lights up once the backend ships.
function useBoard() {
  return useQuery({
    queryKey: ["board"],
    queryFn: api.board,
    refetchInterval: (q) => (isMissing(q.state.error) ? 30000 : 3000),
  });
}

// Project every query/action below the list is bound to (the list's `project`;
// the task page pins the project its task came from). Part of every query key.
const RootCtx = createContext("");
const useRoot = () => useContext(RootCtx);

// Every project's board in one poll. Key sits under ["board"] so any
// invalidation of the current board also refreshes the multi-project view.
export function useBoardAll() {
  return useQuery({
    queryKey: ["board", "all"],
    queryFn: api.boardAll,
    refetchInterval: (q) => (isMissing(q.state.error) ? 30000 : 3000),
  });
}

/** Tasks waiting on the user or failed, across every project. */
export function BoardNavBadge() {
  const { data } = useBoardAll();
  const n = data?.projects.reduce((s, p) => s + p.waiting + p.failed + askingN(p), 0) ?? 0;
  if (!n) return null;
  return (
    <span className="nav-badge" title={`${n} việc cần bạn`} aria-label={`${n} việc cần bạn`}>
      {n}
    </span>
  );
}

// ── Status pill ────────────────────────────────────────────────────────────
// Amber is reserved for what needs the boss; a parked task's state is neutral.
export function StatusPill({ status, stage, asking, paused }: { status: BoardStatus; stage: BoardStage; asking?: boolean; paused?: string }) {
  if (asking) return <span className="tag warn">Cần bạn</span>;
  if (paused) return <span className="tag">{paused}</span>;
  switch (status) {
    case "running":
      return <span className="tag info"><span className="bd-spin" aria-hidden="true" />{STAGE_WHO[stage] || "AI"} đang làm</span>;
    case "waiting":
      return <span className="tag warn">Cần bạn</span>;
    case "failed":
      return <span className="tag err">Lỗi</span>;
    case "queued":
      return <span className="tag">Chờ lượt</span>;
    case "done":
      return <span className="tag ok">Hoàn tất</span>;
    case "cancelled":
      return <span className="tag">Đã huỷ</span>;
  }
}

// ── Page ───────────────────────────────────────────────────────────────────
// View lives in the URL next to the page id: ?page=board&view=all|room|kanban.
// "all" (every project) is the default; room/kanban show the current project.
type View = "all" | "room" | "kanban";
const SHOW_DONE_KEY = "ckit.board.showDone";
function viewFromUrl(): View {
  const v = new URLSearchParams(window.location.search).get("view");
  return v === "room" || v === "kanban" ? v : "all";
}

export function BoardPage() {
  const qc = useQueryClient();
  const { data, isLoading, error } = useBoard();
  const all = useBoardAll();
  // The task page stays bound to the project its task came from (`root` in
  // the URL), even if the dashboard's active project changes while it is open.
  const route = useTaskRoute();
  const openTask = (id: string, root = data?.project ?? "") => goTo({ task: id, root, phase: null, sub: null });
  const [creating, setCreating] = useState(false);
  const [view, setViewState] = useState<View>(viewFromUrl);
  const setView = (v: View) => {
    setViewState(v);
    const p = new URLSearchParams(window.location.search);
    p.set("page", "board");
    p.set("view", v);
    window.history.replaceState(null, "", `?${p}`);
  };
  // The sidebar's project monitor navigates by URL (?view=kanban); follow an
  // explicit view param, keep the current view on a bare "." (Board nav click).
  useEffect(() => {
    const on = () => {
      if (new URLSearchParams(window.location.search).has("view")) setViewState(viewFromUrl());
    };
    window.addEventListener("popstate", on);
    return () => window.removeEventListener("popstate", on);
  }, []);
  // Finished (done/cancelled) tasks stay hidden unless the boss asks for them.
  const [showDone, setShowDoneState] = useState(() => localStorage.getItem(SHOW_DONE_KEY) === "1");
  const setShowDone = (on: boolean) => {
    setShowDoneState(on);
    localStorage.setItem(SHOW_DONE_KEY, on ? "1" : "0");
  };
  // Status strip tile = filter of the current view (toggle; null = everything).
  const [filter, setFilter] = useState<StripFilter | null>(null);
  const keep = (t: BoardTaskSummary) => (filter ? matches(filter, t) : true);
  // Project header in the All view: make it current, then show its board.
  const pick = useMutation({
    mutationFn: (root: string) => api.activateProject(root),
    onSuccess: () => {
      qc.invalidateQueries();
      setView("kanban");
    },
  });
  const missing = isMissing(error);

  const tasks = data?.tasks ?? [];
  const active = tasks.filter((t) => !isTerminal(t));
  const closed = tasks.filter(isTerminal);
  const projects = all.data?.projects ?? [];
  const waitingN =
    view === "all"
      ? projects.reduce((s, p) => s + p.waiting + askingN(p), 0)
      : active.filter((t) => t.status === "waiting" || askingYou(t)).length;
  const hiddenN = view === "all" ? projects.reduce((s, p) => s + p.tasks.filter(isTerminal).length, 0) : closed.length;
  const live = view === "all" ? projects.flatMap((p) => p.tasks.filter((t) => !isTerminal(t))) : active;
  const runningN = live.filter((t) => t.status === "running" && !askingYou(t)).length;
  const failedN = live.filter((t) => t.status === "failed").length;
  // Freshest summary of the open task, from whichever list carries its project.
  const routeRoot = route ? route.root || data?.project || "" : "";
  const openList =
    route && routeRoot === data?.project ? tasks : projects.find((p) => p.project === routeRoot)?.tasks;
  const open = route ? openList?.find((t) => t.id === route.task) : undefined;
  // New tasks go to the project being viewed (kanban/room), else the active one.
  const createRoot = data?.project || all.data?.current || "";
  // What the strip counts: every project's tasks in "Tất cả dự án", else this project's.
  const scope = view === "all" ? projects.flatMap((p) => p.tasks) : tasks;
  const shown = scope.filter(keep);
  const filterEmpty = filter !== null && shown.length === 0 ? (
    <p className="bd-filter-empty">
      Không có việc nào ở trạng thái này. <button className="link" onClick={() => setFilter(null)}>Bỏ lọc</button>
    </p>
  ) : null;

  if (route) {
    return (
      <RootCtx.Provider value={routeRoot}>
        <div className="bd-page">
          <TaskPage key={`${routeRoot}#${route.task}`} route={{ ...route, root: routeRoot }} updated={open?.updated} />
        </div>
      </RootCtx.Provider>
    );
  }

  return (
    <RootCtx.Provider value={data?.project ?? ""}>
    <div className="bd-page">
      <Page
        title="Board"
        sub={
          waitingN + runningN + failedN > 0 ? (
            <span className="bd-sum">
              {waitingN > 0 && <span className="bd-sum-item is-need">{waitingN} việc cần bạn</span>}
              {runningN > 0 && <span className="bd-sum-item is-run">{runningN} đang chạy</span>}
              {failedN > 0 && <span className="bd-sum-item is-fail">{failedN} lỗi</span>}
            </span>
          ) : (
            "Giao việc cho Leader. Team lập kế hoạch từng phase, code, kiểm thử theo AC; bạn chỉ duyệt kế hoạch và nghiệm thu."
          )
        }
        action={
          <div className="bd-head-actions">
            <CoordButton root={data?.project || all.data?.current || ""} />
            <TelegramButton />
            <button className="primary bd-new" onClick={() => setCreating(true)} disabled={missing}>
              <Glyph name="plus" /> Giao việc
            </button>
          </div>
        }
      >
        {scope.length > 0 && <StatusStrip tasks={scope} active={filter} onPick={setFilter} />}
        <div className="bd-toolbar">
          <div className="seg bd-views" role="tablist" aria-label="Kiểu xem">
            <button role="tab" aria-selected={view === "all"} className={view === "all" ? "seg-on" : ""} onClick={() => setView("all")}>Tất cả dự án</button>
            <button role="tab" aria-selected={view === "room"} className={view === "room" ? "seg-on" : ""} onClick={() => setView("room")}>Phòng dự án</button>
            <button role="tab" aria-selected={view === "kanban"} className={view === "kanban" ? "seg-on" : ""} onClick={() => setView("kanban")}>Bảng</button>
          </div>
          <label className="bd-done-toggle">
            <input type="checkbox" checked={showDone} onChange={(e) => setShowDone(e.target.checked)} />
            Hiện việc đã xong ({hiddenN})
          </label>
        </div>
        {filterEmpty ? (
          filterEmpty
        ) : view === "all" ? (
          isMissing(all.error) ? (
            <EndpointMissing endpoint="/api/board/all" />
          ) : all.isLoading ? (
            <Loading rows={4} />
          ) : all.error ? (
            <ErrorState message={(all.error as Error).message} />
          ) : (
            <AllProjects
              projects={projects}
              onOpen={openTask}
              onPick={(root) => pick.mutate(root)}
              picking={pick.isPending ? pick.variables : undefined}
              pickError={pick.error ? (pick.error as Error).message : null}
              onCreate={() => setCreating(true)}
              canCreate={!missing}
              showDone={showDone}
              keep={filter ? keep : null}
            />
          )
        ) : (
          <>
            {(data?.others.length ?? 0) > 0 && <OtherProjects others={data!.others} />}
            {missing ? (
              <EndpointMissing endpoint="/api/board" />
            ) : isLoading ? (
              <Loading rows={4} />
            ) : error ? (
              <ErrorState message={(error as Error).message} />
            ) : view === "room" ? (
              <>
                <Room tasks={filter ? active.filter(keep) : active} onOpen={openTask} />
                {(showDone || filter === "done") && closed.filter(keep).length > 0 && <Closed tasks={closed.filter(keep)} onOpen={openTask} />}
              </>
            ) : tasks.length === 0 ? (
              <EmptyState
                icon={<Glyph name="plus" />}
                title="Chưa có việc nào"
                hint="Bấm “Giao việc” và mô tả bạn cần gì. Leader làm rõ yêu cầu, chia phase, lập kế hoạch kèm tiêu chí nghiệm thu (AC), rồi gửi bạn duyệt trước khi code."
              />
            ) : (
              <EpicBoard
                tasks={filter ? shown : showDone ? tasks : active}
                root={data?.project ?? ""}
                showDone={showDone}
                doneN={closed.length}
                onShowDone={() => setShowDone(true)}
                only={filter ? (filter === "fail" ? [...new Set(shown.map(laneOf))] : [filter]) : undefined}
                onOpen={openTask}
              />
            )}
          </>
        )}
      </Page>
      {creating && (
        <CreateModal
          root={createRoot}
          name={projects.find((p) => p.project === createRoot)?.name || createRoot.split("/").pop() || createRoot}
          onClose={() => setCreating(false)}
          onCreated={(id, root) => {
            setCreating(false);
            openTask(id, root);
          }}
        />
      )}
    </div>
    </RootCtx.Provider>
  );
}

// ── All projects ───────────────────────────────────────────────────────────
// One section per project; the boss sees everything that needs them first.
function AllProjects({
  projects, onOpen, onPick, picking, pickError, onCreate, canCreate, showDone, keep,
}: {
  projects: ProjectBoard[];
  onOpen: (id: string, root: string) => void;
  onPick: (root: string) => void;
  picking?: string;
  pickError: string | null;
  onCreate: () => void;
  canCreate: boolean;
  showDone: boolean;
  /** status-strip filter; null = no filter */
  keep: ((t: BoardTaskSummary) => boolean) | null;
}) {
  const pass = keep ?? (() => true);
  const needs = projects.flatMap((p) =>
    p.tasks.filter((t) => !isTerminal(t) && needsYou(t) && pass(t)).map((t) => ({ p, t })),
  );
  // Filtering: only projects with a matching task.
  const listed = keep ? projects.filter((p) => p.tasks.some(keep)) : projects;
  if (projects.every((p) => p.tasks.length === 0)) {
    return (
      <EmptyState
        icon={<Glyph name="plus" />}
        title="Chưa có việc nào ở dự án nào"
        hint={
          <>
            Giao việc cho Leader ở bất kỳ dự án nào. Việc của mọi dự án hiện chung ở đây; việc cần bạn duyệt nổi lên đầu.
            <span className="bd-empty-act">
              <button className="primary bd-new" onClick={onCreate} disabled={!canCreate}>
                <Glyph name="plus" /> Giao việc
              </button>
            </span>
          </>
        }
      />
    );
  }
  return (
    <>
      {needs.length > 0 && (
        <div className="bd-attn" role="region" aria-label="Việc cần bạn ở mọi dự án">
          <div className="bd-attn-label">Cần bạn <span className="bd-count is-need">{needs.length}</span></div>
          {needs.map(({ p, t }) => {
            const act = cardAction(t);
            return (
              <div key={`${p.project}#${t.id}`} className={`bd-attn-item ${t.status === "failed" ? "is-failed" : ""}`}>
                <button className="bd-attn-open" onClick={() => onOpen(t.id, p.project)} aria-label={`${t.id}: ${t.title}`}>
                  <span className="bd-attn-proj" title={p.project}>{p.name}</span>
                  <span className="mono faint">{t.id}</span>
                  <span className="bd-attn-title">{t.title}</span>
                  {act ? null : (
                    <span className={`tag ${t.status === "failed" ? "err" : "warn"}`}>
                      {t.status === "failed" ? "Lỗi, cần chạy lại" : needLabel(t)}
                    </span>
                  )}
                </button>
                <CardAction t={t} root={p.project} onOpen={() => onOpen(t.id, p.project)} />
              </div>
            );
          })}
        </div>
      )}
      {pickError ? <p className="hint hint-err">{pickError}</p> : null}
      <div className="bd-projs">
        {listed.map((p) => (
          <ProjectSection key={p.project} p={p} onOpen={onOpen} onPick={onPick} picking={picking === p.project} showDone={showDone} keep={keep} />
        ))}
      </div>
    </>
  );
}

function ProjectSection({
  p, onOpen, onPick, picking, showDone, keep,
}: {
  p: ProjectBoard;
  onOpen: (id: string, root: string) => void;
  onPick: (root: string) => void;
  picking: boolean;
  showDone: boolean;
  keep: ((t: BoardTaskSummary) => boolean) | null;
}) {
  const pass = keep ?? (() => true);
  const active = p.tasks.filter((t) => !isTerminal(t) && pass(t));
  const closed = p.tasks.filter((t) => isTerminal(t) && pass(t));
  const need = p.waiting + askingN(p);
  return (
    <section className="card bd-proj" aria-label={p.name}>
      <button className="bd-proj-head" onClick={() => onPick(p.project)} disabled={picking} title={`Mở bảng của ${p.name}`}>
        <span className="bd-proj-id">
          <span className="bd-proj-name">{p.name}</span>
          {p.current && <span className="tag accent">Đang mở</span>}
          <span className="bd-proj-path mono">{p.project}</span>
        </span>
        <span className="bd-proj-chips">
          {need > 0 && <span className="tag warn">{need} chờ bạn</span>}
          {p.failed > 0 && <span className="tag err">{p.failed} lỗi</span>}
          {p.running > 0 && <span className="tag info">{p.running} đang chạy</span>}
          {p.queued > 0 && <span className="tag">{p.queued} chờ lượt</span>}
          <span className="bd-proj-cost mono">{money(p.cost)}</span>
          <span className="bd-proj-go">{picking ? "Đang mở…" : "Mở bảng →"}</span>
        </span>
      </button>
      {active.length === 0 ? (
        <p className="bd-proj-empty faint">
          {closed.length > 0 ? `Không có việc đang làm · ${closed.length} đã xong` : "Không có việc đang làm."}
        </p>
      ) : (
        <div className="list bd-proj-list">
          {active.map((t) => (
            <TaskRow key={t.id} t={t} onOpen={() => onOpen(t.id, p.project)} />
          ))}
        </div>
      )}
      {(showDone || keep !== null) && closed.length > 0 && (
        <div className="bd-proj-closed">
          <div className="bd-proj-closed-label">{closed.length} đã xong / huỷ</div>
          <div className="list bd-proj-list">
            {closed.map((t) => (
              <TaskRow key={t.id} t={t} onOpen={() => onOpen(t.id, p.project)} />
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

function TaskRow({ t, onOpen }: { t: BoardTaskSummary; onOpen: () => void }) {
  return (
    <button className={`row bd-trow is-${askingYou(t) ? "waiting" : t.status}`} onClick={onOpen} aria-label={`${t.id}: ${t.title}`}>
      <span className="mono faint bd-trow-id">{t.id}{phaseLabel(t)}</span>
      <span className="row-main bd-trow-main">
        <span className="row-title bd-trow-title">{t.title} <WorkflowChip t={t} /></span>
        {t.note ? <span className={`bd-trow-note ${t.status === "failed" ? "err" : ""}`}>{t.note}</span> : null}
      </span>
      <span className="bd-trow-stage faint">{taskStageLabel(t) ?? STAGE_LABEL[t.stage]}</span>
      <StatusPill status={t.status} stage={t.stage} asking={askingYou(t)} paused={pauseLabel(t)} />
    </button>
  );
}

// Tasks keep running in other projects while the dashboard shows this one.
function OtherProjects({ others }: { others: { project: string; active: number; waiting: number }[] }) {
  const qc = useQueryClient();
  const go = useMutation({ mutationFn: (p: string) => api.activateProject(p), onSuccess: () => qc.invalidateQueries() });
  return (
    <div className="bd-others" role="region" aria-label="Dự án khác">
      <span className="faint">Dự án khác đang có việc:</span>
      {others.map((o) => (
        <button key={o.project} className="bd-other" onClick={() => go.mutate(o.project)} disabled={go.isPending} title={o.project}>
          <strong>{o.project.split("/").pop()}</strong>
          {o.waiting > 0 ? <span className="tag warn">{o.waiting} chờ bạn</span> : null}
          {o.active > 0 ? <span className="tag">{o.active} đang chạy</span> : null}
        </button>
      ))}
    </div>
  );
}

function Closed({ tasks, onOpen }: { tasks: BoardTaskSummary[]; onOpen: (id: string) => void }) {
  const done = tasks.filter((t) => t.status === "done" || t.stage === "done").length;
  return (
    <details className="card bd-closed" open>
      <summary>
        <span>Đã đóng</span>
        <span className="tag ok">{done} hoàn tất</span>
        {tasks.length - done > 0 && <span className="tag">{tasks.length - done} đã huỷ</span>}
      </summary>
      <div className="list">
        {tasks.map((t) => (
          <button key={t.id} className="row bd-closed-row" onClick={() => onOpen(t.id)}>
            <span className="mono faint">{t.id}</span>
            <span className="row-main"><span className="row-title">{t.title}</span></span>
            <span className="muted mono">{money(t.cost)}</span>
            <StatusPill status={t.status} stage={t.stage} />
          </button>
        ))}
      </div>
    </details>
  );
}

// ── Create ─────────────────────────────────────────────────────────────────
// The project is already chosen (sidebar switcher / the board being viewed);
// the modal only names it.
function CreateModal({
  root, name, onClose, onCreated,
}: {
  root: string;
  name: string;
  onClose: () => void;
  onCreated: (id: string, root: string) => void;
}) {
  const qc = useQueryClient();
  const [title, setTitle] = useState("");
  const [request, setRequest] = useState("");
  const [steps, setSteps] = useState<BoardStage[]>(FULL_STEPS);
  const [workflow, setWorkflow] = useState<string | null>(null);
  const { catalog } = useStepCatalog();
  const stepsErr = workflow ? null : validateSteps(steps, catalog);
  const create = useMutation({
    mutationFn: (r: string) => api.boardCreate(r, title.trim(), request.trim(), workflow ? undefined : steps, workflow),
    onSuccess: (t, r) => {
      qc.invalidateQueries({ queryKey: ["board"] });
      onCreated(t.id, r);
    },
  });
  useEscape(onClose);
  const ok = root !== "" && title.trim() !== "" && request.trim() !== "" && !stepsErr && !create.isPending;
  const send = () => ok && create.mutate(root);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    send();
  };
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <form className="modal" role="dialog" aria-modal="true" aria-label="Giao việc" onClick={(e) => e.stopPropagation()} onSubmit={submit}>
        <div className="modal-head">
          <strong>Giao việc cho Leader</strong>
          <button type="button" className="icon-btn ghost" onClick={onClose} aria-label="Đóng">✕</button>
        </div>
        <div className="modal-body">
          <p className="bd-create-proj" title={root}>Dự án: <strong>{name || "chưa chọn"}</strong></p>
          <label className="field">
            <span className="field-label">Tiêu đề</span>
            <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="VD: Thêm xuất Excel cho danh sách đơn hàng" autoFocus />
          </label>
          <label className="field">
            <span className="field-label">Yêu cầu</span>
            <textarea
              className="bd-textarea"
              value={request}
              onChange={(e) => setRequest(e.target.value)}
              onKeyDown={(e) => submitOnMetaEnter(e, send)}
              placeholder="Cần làm gì, cho ai, kết quả mong muốn. Leader sẽ hỏi lại nếu còn thiếu."
              rows={5}
            />
          </label>
          <WorkflowPicker root={root} value={workflow} onChange={setWorkflow} />
          {workflow ? <WorkflowSteps name={workflow} /> : <StepPicker value={steps} onChange={setSteps} />}
          {create.error ? <p className="hint hint-err">{serverMessage(create.error as Error)}</p> : null}
        </div>
        <div className="modal-foot">
          <span className="faint hint-inline">⌘/Ctrl + Enter để gửi</span>
          <button type="submit" className="primary" disabled={!ok}>
            {create.isPending ? "Đang gửi…" : "Giao cho Leader"}
          </button>
        </div>
      </form>
    </div>
  );
}

// ── Task actions (task page header) ────────────────────────────────────────
type Act = { kind: "approve" | "approve-code" | "retry" | "cancel" } | { kind: "reply"; text: string };

/** What the boss can do now: gate approve / reply, retry, or note / pause / resume a running step. */
export function TaskActions({ t }: { t: BoardTask }) {
  const qc = useQueryClient();
  const root = useRoot();

  const act = useMutation({
    mutationFn: (a: Act): Promise<BoardTask> => {
      switch (a.kind) {
        case "approve": return api.boardApprove(root, t.id);
        case "approve-code": return api.boardApprove(root, t.id, true);
        case "retry": return api.boardRetry(root, t.id);
        case "cancel": return api.boardCancel(root, t.id);
        case "reply": return api.boardReply(root, t.id, a.text);
      }
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["board"] });
      qc.invalidateQueries({ queryKey: ["board-task", root, t.id] });
      qc.invalidateQueries({ queryKey: ["board-log", root, t.id] });
    },
  });

  const cancel = () => {
    if (window.confirm(`Huỷ ${t.id}? ${t.status === "running" ? "Giai đoạn đang chạy sẽ bị dừng. " : ""}Không hoàn tác được.`)) {
      act.mutate({ kind: "cancel" });
    }
  };

  return <Actions t={t} busy={act.isPending ? act.variables?.kind : undefined} error={act.error as Error | null} onAct={(a) => act.mutate(a)} onCancel={cancel} />;
}

// ── Thread ─────────────────────────────────────────────────────────────────
export function Thread({ msgs }: { msgs: BoardMsg[] }) {
  const endRef = useRef<HTMLDivElement>(null);
  // Follow NEW messages only: scrolling on mount would yank the task page down
  // to the thread the moment it opens, past the header and the action bar.
  const seen = useRef(msgs.length);
  // Block body: newer Chromium returns a Promise from scrollIntoView, which
  // React would otherwise call as the effect cleanup.
  useEffect(() => {
    if (msgs.length > seen.current) endRef.current?.scrollIntoView({ block: "end" });
    seen.current = msgs.length;
  }, [msgs.length]);
  if (msgs.length === 0) return <p className="bd-empty">Chưa có trao đổi. Leader sẽ nhắn ở đây khi cần bạn.</p>;
  return (
    <div className="bd-thread">
      {msgs.map((m, i) =>
        m.from === "system" ? (
          <div key={i} className="bd-sys"><span>{m.text}</span> <time className="faint">{clock(m.at)}</time></div>
        ) : (
          <div key={i} className={`bd-msg from-${threadClass(m.from)}`}>
            <div className="bd-msg-head">
              <strong>{senderLabel(m.from)}</strong>
              <time className="faint">{clock(m.at)}</time>
            </div>
            {m.from === "user" || m.from === "boss" || m.from.startsWith("user → ") ? <p className="bd-msg-text">{m.text}</p> : <Markdown source={m.text} />}
          </div>
        ),
      )}
      <div ref={endRef} />
    </div>
  );
}

// ── Artifacts ──────────────────────────────────────────────────────────────
export function Docs({ d, docs }: { d: BoardDetail; docs: BoardDetail["docs"] }) {
  const names = docs.map((x) => x.name);
  const fallback = d.focus && names.includes(d.focus) ? d.focus : names[names.length - 1];
  const [pick, setPick] = useState<string | null>(null);
  const cur = pick && names.includes(pick) ? pick : fallback;
  if (!cur) return <p className="bd-empty">Chưa có tài liệu. ROADMAP xuất hiện sau bước tiếp nhận.</p>;
  return (
    <div className="bd-docs">
      <div className="bd-doc-tabs" role="tablist" aria-label="Tài liệu">
        {names.map((k) => (
          <button key={k} role="tab" aria-selected={k === cur} className={`link ${k === cur ? "is-on" : ""}`} aria-current={k === cur} onClick={() => setPick(k)}>
            {k}
          </button>
        ))}
      </div>
      <div className="bd-doc">
        <Markdown source={docs.find((x) => x.name === cur)?.content ?? ""} />
      </div>
    </div>
  );
}

// ── Runs + live log ────────────────────────────────────────────────────────
export function Log({ task }: { task: BoardTask }) {
  const root = useRoot();
  const [runIdx, setRunIdx] = useState<number | null>(null); // null = latest
  const latest = task.runs.length - 1;
  const sel = runIdx ?? latest;
  const log = useQuery({
    queryKey: ["board-log", root, task.id, runIdx],
    queryFn: () => api.boardLog(root, task.id, runIdx ?? undefined),
    enabled: task.runs.length > 0,
    refetchInterval: (q) => (q.state.data?.running || (runIdx === null && task.status === "running") ? 2000 : false),
  });

  const boxRef = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const lines = log.data?.lines ?? [];
  useEffect(() => {
    const el = boxRef.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [lines.length, sel]);

  if (task.runs.length === 0) return <p className="bd-empty">Chưa có lần chạy nào. Việc đang chờ tới lượt.</p>;

  return (
    <div className="bd-logwrap">
      <div className="bd-runs" role="listbox" aria-label="Các lần chạy">
        {task.runs.map((r, i) => (
          <button
            key={i}
            role="option"
            aria-selected={i === sel}
            className={`bd-run ${i === sel ? "is-on" : ""}`}
            onClick={() => { stick.current = true; setRunIdx(i === latest ? null : i); }}
          >
            <span className="bd-run-name">
              {r.ended === null && <span className="bd-spin" aria-hidden="true" />}
              {STAGE_LABEL[r.stage]} #{r.n}
            </span>
            <span className="bd-run-meta">
              {r.verdict ? <span className={`tag ${verdictTone(r.verdict)}`}>{r.verdict}</span> : null}
              {r.exit !== null && r.exit !== 0 ? <span className="tag err">exit {r.exit}</span> : null}
              <span className="mono faint">{money(r.cost)} · {duration(r)}</span>
            </span>
          </button>
        ))}
      </div>
      {isMissing(log.error) ? (
        <EndpointMissing endpoint={`/api/board/${task.id}/log`} />
      ) : log.error ? (
        <ErrorState message={(log.error as Error).message} />
      ) : (
        <div
          className="bd-log"
          ref={boxRef}
          onScroll={(e) => {
            const el = e.currentTarget;
            stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
          }}
          aria-live={log.data?.running ? "polite" : undefined}
          aria-busy={log.isLoading}
        >
          {log.isLoading ? (
            <span className="faint">Đang tải log…</span>
          ) : lines.length === 0 ? (
            <span className="faint">Log trống.</span>
          ) : (
            lines.map((l, i) => <LogRow key={i} l={l} />)
          )}
          {log.data?.running ? <div className="bd-log-live"><span className="bd-spin" aria-hidden="true" /> đang chạy</div> : null}
        </div>
      )}
    </div>
  );
}

function LogRow({ l }: { l: BoardLogLine }) {
  return <div className={`bd-line k-${l.kind}`}>{l.text}</div>;
}

function verdictTone(v: string): string {
  const s = v.toLowerCase();
  if (/(pass|ok|approve|đạt)/.test(s)) return "ok";
  if (/(fail|reject|error|không đạt)/.test(s)) return "err";
  return "";
}

// ── Gate actions (task page header) ────────────────────────────────────────
/** Thread bubble class: boss interjections sit with the boss, coordinator ones with the coordinator. */
function threadClass(from: string): string {
  if (from.startsWith("user → ")) return "user";
  if (from.startsWith("coordinator → ")) return "coordinator";
  return from.includes(" → ") ? "ask" : from;
}

function Actions({
  t, busy, error, onAct, onCancel,
}: {
  t: BoardTask;
  busy: Act["kind"] | undefined;
  error: Error | null;
  onAct: (a: Act) => void;
  onCancel: () => void;
}) {
  const root = useRoot();
  const [text, setText] = useState("");
  // Gate composer (Yêu cầu sửa / Hỏi tiếp) opens on demand; a plain question from Leader keeps it open.
  const [writing, setWriting] = useState(false);
  const sentRef = useRef(false);
  // Clear the composer once a reply lands.
  useEffect(() => {
    if (sentRef.current && busy === undefined && !error) {
      setText("");
      setWriting(false);
      sentRef.current = false;
    }
  }, [busy, error]);

  if (isTerminal(t)) return null;
  const cancelBtn = (
    <button className="ghost danger" onClick={onCancel} disabled={busy !== undefined}>
      {busy === "cancel" ? "Đang huỷ…" : "Huỷ việc"}
    </button>
  );
  if (t.status === "running" || isPaused(t)) {
    return <Interject t={t} root={root} seat={STAGE_WHO[t.stage] || "AI"} cancel={cancelBtn} error={error} />;
  }
  const waiting = t.status === "waiting";
  const failed = t.status === "failed";
  const gate = GATES.has(t.stage);
  const investigate = t.stage === "investigate";
  // A "Chỉ kế hoạch" task at its approval gate: approve and code it, or just keep the plan.
  const planOnly = waiting && t.stage === "approval" && !(t.steps ?? []).includes("dev");
  const composing = waiting && (writing || !gate);
  const send = () => {
    const v = text.trim();
    if (!v || busy) return;
    sentRef.current = true;
    onAct({ kind: "reply", text: v });
  };
  const approve = () => {
    if (t.stage === "acceptance") {
      const into = t.base ? ` vào ${t.base}` : "";
      const msg = t.branch ? `Nghiệm thu sẽ merge nhánh ${t.branch}${into}. Tiếp tục?` : "Nghiệm thu và đóng việc này?";
      if (!window.confirm(msg)) return;
    }
    onAct({ kind: "approve" });
  };

  return (
    <section className={`tp-act ${waiting ? "is-waiting" : ""} ${failed ? "is-err" : ""}`} aria-label="Việc cần làm">
      <div className="tp-act-row">
        <p className={`bd-gate-msg ${failed ? "err" : ""}`}>
          {failed
            ? `Bước ${STAGE_LABEL[t.stage]} bị lỗi. Xem Nhật ký, rồi chạy lại.`
            : !waiting
              ? "Việc đang chờ tới lượt chạy."
              : planOnly
                ? "Kế hoạch đã qua PO duyệt. Đọc CONTEXT và PLAN ở tab Tài liệu. Việc này chỉ lập kế hoạch: duyệt và code luôn, hoặc chỉ lưu kế hoạch để code sau."
              : t.stage === "approval"
                ? "Kế hoạch phase đã qua PO duyệt. Đọc CONTEXT (Goal + bảng AC) và PLAN ở tab Tài liệu, rồi duyệt để Dev bắt đầu."
                : t.stage === "acceptance"
                  ? "Đã kiểm thử xong. Đọc REPORT ở tab Tài liệu rồi nghiệm thu để merge."
                  : investigate
                    ? "Leader đã trả lời. Hỏi tiếp, hoặc tiếp tục / đóng."
                    : "Leader đang chờ bạn trả lời."}
        </p>
        <div className="tp-act-btns">
          {cancelBtn}
          {waiting && gate && (
            <button onClick={() => setWriting(!writing)} aria-expanded={writing} disabled={busy !== undefined}>
              {investigate ? "Hỏi tiếp" : "Yêu cầu sửa"}
            </button>
          )}
          {planOnly && (
            <button onClick={approve} disabled={busy !== undefined}>
              {busy === "approve" ? "Đang xử lý…" : "Duyệt, chỉ lưu kế hoạch"}
            </button>
          )}
          {planOnly && (
            <button className="primary" onClick={() => onAct({ kind: "approve-code" })} disabled={busy !== undefined}>
              <Glyph name="check" /> {busy === "approve-code" ? "Đang xếp lượt…" : "Duyệt và code luôn"}
            </button>
          )}
          {waiting && gate && !planOnly && (
            <button className="primary" onClick={approve} disabled={busy !== undefined}>
              <Glyph name="check" /> {busy === "approve" ? "Đang xử lý…" : t.stage === "approval" ? "Duyệt kế hoạch" : investigate ? (hasStepAfter(t, "investigate") ? "Tiếp tục" : "Đóng") : "Nghiệm thu"}
            </button>
          )}
          {failed && (
            <button className="primary" onClick={() => onAct({ kind: "retry" })} disabled={busy !== undefined}>
              <Glyph name="refresh" /> {busy === "retry" ? "Đang xếp lượt…" : "Chạy lại"}
            </button>
          )}
        </div>
      </div>
      {composing && (
        <div className="tp-compose">
          <textarea
            className="bd-textarea bd-reply"
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={(e) => submitOnMetaEnter(e, send)}
            placeholder={
              t.stage === "approval"
                ? "Muốn sửa gì ở kế hoạch / AC?"
                : t.stage === "acceptance"
                  ? "Chỗ nào chưa đạt?"
                  : investigate
                    ? "Hỏi thêm Leader…"
                    : "Trả lời Leader…"
            }
            rows={3}
            autoFocus={gate}
            aria-label="Trả lời Leader"
          />
          <div className="bd-dfoot-row">
            <span className="faint tp-hint">⌘/Ctrl + Enter để gửi</span>
            <span className="grow-spacer" />
            <button className="primary" onClick={send} disabled={!text.trim() || busy !== undefined}>
              {busy === "reply" ? "Đang gửi…" : investigate ? "Gửi câu hỏi" : gate ? "Gửi yêu cầu sửa" : "Gửi trả lời"}
            </button>
          </div>
        </div>
      )}
      {error ? <p className="hint hint-err">{error.message}</p> : null}
    </section>
  );
}

// ── Project room ───────────────────────────────────────────────────────────
type SeatId = "boss" | "coord" | "leader" | "ba" | "dev" | "tester" | "pm";
const SEATS: { id: SeatId; name: string; role: string; initials: string }[] = [
  { id: "boss", name: "Sếp", role: "Bạn · duyệt và nghiệm thu", initials: "S" },
  { id: "coord", name: "Điều phối", role: "Thay bạn trả lời, duyệt kế hoạch, giao việc", initials: "ĐP" },
  { id: "leader", name: "Leader", role: "Trưởng nhóm · PO", initials: "L" },
  { id: "ba", name: "BA", role: "Phân tích, thiết kế", initials: "BA" },
  { id: "dev", name: "Dev", role: "Lập trình", initials: "D" },
  { id: "tester", name: "Tester", role: "Kiểm thử độc lập", initials: "T" },
  { id: "pm", name: "PM", role: "Theo dõi tiến độ", initials: "PM" },
];

// ── Phase label for list rows ──────────────────────────────────────────────
/** ` · M1 · 3/5` — active phase and its sub-task progress, when known. */
function phaseLabel(t: BoardTaskSummary): string {
  const p = t.progress && t.progress.total > 0 ? ` · ${t.progress.done}/${t.progress.total}` : "";
  return t.phase ? ` · ${t.phase}${p}` : "";
}

// Running stage → who sits at work, and what they're doing.
const STAGE_SEATS: Partial<Record<BoardStage, { seat: SeatId; doing: string }[]>> = {
  investigate: [{ seat: "leader", doing: "Điều tra" }],
  intake: [{ seat: "leader", doing: "Tiếp nhận yêu cầu" }],
  solution: [{ seat: "leader", doing: "Lập kế hoạch phase" }, { seat: "ba", doing: "Phân tích yêu cầu, AC" }],
  review: [{ seat: "leader", doing: "PO duyệt" }],
  dev: [{ seat: "dev", doing: "Lập trình" }],
  test: [{ seat: "tester", doing: "Kiểm thử" }],
  security: [{ seat: "tester", doing: "Review bảo mật" }],
  report: [{ seat: "leader", doing: "Viết báo cáo" }],
};

function usePm() {
  const root = useRoot();
  return useQuery({
    queryKey: ["board-pm", root],
    queryFn: () => api.boardPm(root),
    enabled: root !== "",
    refetchInterval: (q) => (isMissing(q.state.error) ? 60000 : 10000),
  });
}

function useNow(ms: number): number {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const h = window.setInterval(() => setNow(Date.now()), ms);
    return () => window.clearInterval(h);
  }, [ms]);
  return now;
}

function elapsed(fromIso: string, now: number): string {
  const a = Date.parse(fromIso);
  if (Number.isNaN(a)) return "";
  const s = Math.max(0, Math.floor((now - a) / 1000));
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}:${String(s % 60).padStart(2, "0")}`;
  return `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, "0")}m`;
}

function Room({ tasks, onOpen }: { tasks: BoardTaskSummary[]; onOpen: (id: string) => void }) {
  const pm = usePm();
  const root = useRoot();
  const coord = useCoord(root);
  const entries = pm.data?.entries ?? [];
  const running = tasks.filter((t) => t.status === "running");
  const waiting = tasks.filter((t) => t.status === "waiting" || askingYou(t));
  const failed = tasks.filter((t) => t.status === "failed");
  const queued = tasks.filter((t) => t.status === "queued");

  // Seat → task and its full state line ("Đang làm · Lập trình", "Đang trả lời Dev").
  const busy = new Map<SeatId, { t: BoardTaskSummary; doing: string }>();
  for (const t of running) {
    for (const s of STAGE_SEATS[t.stage] ?? []) if (!busy.has(s.seat)) busy.set(s.seat, { t, doing: `Đang làm · ${s.doing}` });
  }
  // BA/PO answering a teammate's question (shares the task page's cache).
  const details = useQueries({
    queries: running.map((t) => ({ queryKey: ["board-task", root, t.id], queryFn: () => api.boardTask(root, t.id), refetchInterval: 5000 })),
  });
  for (const q of details) {
    const d = q.data?.task;
    if (d) for (const s of answeringSeats(d)) busy.set(s.seat, { t: d, doing: s.state });
  }
  const lastStatus = (id: string): PmEntry | undefined => {
    for (let i = entries.length - 1; i >= 0; i--) {
      const e = entries[i];
      if (e && e.task === id && e.kind === "status") return e;
    }
    return undefined;
  };

  return (
    <div className="rm">
      <div className="rm-seats" role="list" aria-label="Thành viên">
        {SEATS.map((s) => {
          const b = busy.get(s.id);
          const bossBusy = s.id === "boss" && waiting.length + failed.length > 0;
          const on = Boolean(b) || bossBusy || (s.id === "coord" && Boolean(coord.data?.busy));
          return (
            <section key={s.id} className={`rm-seat seat-${s.id} ${on ? "is-on" : ""} ${bossBusy ? "is-need" : ""}`} role="listitem" aria-label={s.name}>
              <header className="rm-seat-head">
                <span className="rm-av" aria-hidden="true">{s.initials}</span>
                <span className="rm-who">
                  <strong>{s.name}</strong>
                  <span className="faint">{s.role}</span>
                </span>
              </header>
              {s.id === "boss" ? (
                <BossSeat waiting={waiting} failed={failed} onOpen={onOpen} />
              ) : s.id === "coord" ? (
                coord.data?.busy ? (
                  <span className="rm-state is-busy"><span className="bd-spin" aria-hidden="true" /> Đang xử lý</span>
                ) : (
                  <span className="rm-state is-idle">{coord.data && !coord.data.enabled ? "Đang tắt" : "Rảnh"}</span>
                )
              ) : s.id === "pm" ? (
                <PmSeat feed={pm.data} missing={isMissing(pm.error)} />
              ) : b ? (
                <WorkingSeat t={b.t} doing={b.doing} status={lastStatus(b.t.id)} onOpen={onOpen} />
              ) : (
                <span className="rm-state is-idle">Rảnh</span>
              )}
            </section>
          );
        })}
      </div>

      {queued.length > 0 && (
        <div className="rm-queue" role="region" aria-label="Hàng chờ">
          <span className="rm-queue-label">Hàng chờ</span>
          {queued.map((t) => (
            <button key={t.id} className="rm-queue-item" onClick={() => onOpen(t.id)}>
              <span className="mono faint">{t.id}</span>
              <span className="rm-queue-title">{t.title}</span>
              {t.note ? <span className="rm-queue-note">{t.note}</span> : null}
            </button>
          ))}
        </div>
      )}

      <section className="card rm-feed" aria-label="Nhật ký PM">
        <h3 className="card-title">Nhật ký PM</h3>
        {isMissing(pm.error) ? (
          <EndpointMissing endpoint="/api/board/pm" />
        ) : pm.isLoading ? (
          <Loading rows={3} />
        ) : pm.error ? (
          <ErrorState message={(pm.error as Error).message} />
        ) : entries.length === 0 ? (
          <p className="bd-empty">PM chưa ghi gì. Khi có việc đang chạy, PM báo tình hình mỗi {Math.round((pm.data?.interval_secs ?? 300) / 60)} phút.</p>
        ) : (
          <PmList entries={entries} onOpen={onOpen} />
        )}
      </section>
    </div>
  );
}

function BossSeat({ waiting, failed, onOpen }: { waiting: BoardTaskSummary[]; failed: BoardTaskSummary[]; onOpen: (id: string) => void }) {
  const root = useRoot();
  if (waiting.length + failed.length === 0) return <span className="rm-state is-idle">Không có việc chờ bạn</span>;
  return (
    <div className="rm-boss">
      {waiting.length > 0 && <span className="rm-state is-need">Chờ bạn duyệt: {waiting.length}</span>}
      {[...waiting, ...failed].map((t) => (
        <div key={t.id} className="rm-task">
          <button className="rm-task-open" onClick={() => onOpen(t.id)} aria-label={`${t.id}: ${t.title}`}>
            <span className="mono faint">{t.id}</span>
            <span className="rm-task-title">{t.title}</span>
          </button>
          <CardAction t={t} root={root} onOpen={() => onOpen(t.id)} />
        </div>
      ))}
    </div>
  );
}

function WorkingSeat({ t, doing, status, onOpen }: { t: BoardTaskSummary; doing: string; status?: PmEntry; onOpen: (id: string) => void }) {
  // Run start lives on the task detail; shares the task page's cache entry.
  const root = useRoot();
  const detail = useQuery({ queryKey: ["board-task", root, t.id], queryFn: () => api.boardTask(root, t.id), refetchInterval: 5000 });
  const now = useNow(1000);
  const runs = detail.data?.task.runs ?? [];
  let started: string | undefined;
  for (let i = runs.length - 1; i >= 0; i--) {
    const r = runs[i];
    if (r && r.ended === null) { started = r.started; break; }
  }
  return (
    <button className="rm-work" onClick={() => onOpen(t.id)}>
      <span className="rm-state is-busy">
        <span className="bd-spin" aria-hidden="true" /> {doing}
        {started ? <span className="rm-elapsed">{elapsed(started, now)}</span> : null}
      </span>
      <span className="rm-task-line">
        <span className="mono faint">{t.id}{phaseLabel(t)}</span> <span className="rm-task-title">{t.title}</span>
      </span>
      {status ? <span className="rm-pm-status" title={`PM · ${clock(status.at)}`}>{status.text}</span> : null}
    </button>
  );
}

function PmSeat({ feed, missing }: { feed?: { reporting: boolean; interval_secs: number; last_tick: string | null }; missing: boolean }) {
  const qc = useQueryClient();
  const root = useRoot();
  const report = useMutation({
    mutationFn: () => api.boardPmReport(root),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["board-pm", root] }),
  });
  const pending = report.isPending || Boolean(feed?.reporting);
  const mins = Math.round((feed?.interval_secs ?? 300) / 60);
  return (
    <div className="rm-pm">
      <span className="rm-state is-watch"><span className="live-dot" aria-hidden="true" /> Theo dõi mỗi {mins} phút</span>
      <span className="faint rm-pm-tick">{feed?.last_tick ? `Lần quét cuối ${clock(feed.last_tick)}` : "Chưa quét lần nào"}</span>
      <button onClick={() => report.mutate()} disabled={pending || missing}>
        {pending ? <><span className="bd-spin" aria-hidden="true" /> Đang viết báo cáo…</> : "Viết báo cáo"}
      </button>
      {report.error ? <span className="hint-inline err">{(report.error as Error).message}</span> : null}
    </div>
  );
}

// Newest first in the UI; the API sends newest last.
function PmList({ entries, onOpen }: { entries: PmEntry[]; onOpen?: (id: string) => void }) {
  const list = [...entries].reverse();
  return (
    <ol className="pm-list">
      {list.map((e, i) => {
        const open = e.task && onOpen ? () => onOpen(e.task as string) : undefined;
        const head = (
          <span className="pm-head">
            <time className="faint">{clock(e.at)}</time>
            {e.task ? <span className="mono faint">{e.task}</span> : null}
            {e.stage ? <span className="faint">{STAGE_LABEL[e.stage]}</span> : null}
          </span>
        );
        if (e.kind === "report") {
          return (
            <li key={i} className="pm-report">
              <div className="pm-report-head">
                <strong>Báo cáo PM</strong>
                {head}
                {open ? <button className="link" onClick={open}>Mở {e.task}</button> : null}
              </div>
              <Markdown source={e.text} />
            </li>
          );
        }
        const body = (
          <>
            {e.kind === "alert" ? <span className="pm-alert-ico" aria-hidden="true"><Glyph name="alert" /></span> : null}
            {head}
            <span className="pm-text">{e.text}</span>
          </>
        );
        return (
          <li key={i} className={`pm-${e.kind}`}>
            {open ? <button className="pm-row" onClick={open}>{body}</button> : <div className="pm-row">{body}</div>}
          </li>
        );
      })}
    </ol>
  );
}

// PM entries for one task, shown in the task page's Nhật ký tab. Silent when the
// feed is unavailable or empty for this task.
export function TaskPm({ id }: { id: string }) {
  const pm = usePm();
  const mine = (pm.data?.entries ?? []).filter((e) => e.task === id);
  if (mine.length === 0) return null;
  return (
    <section className="bd-taskpm" aria-label="PM theo dõi">
      <h4 className="bd-taskpm-title">PM theo dõi</h4>
      <PmList entries={mine} />
    </section>
  );
}

// ── Small hooks ────────────────────────────────────────────────────────────
export function useEscape(fn: () => void) {
  const ref = useRef(fn);
  ref.current = fn;
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // Don't throw away a half-written message.
      const el = e.target as HTMLElement | null;
      if (el instanceof HTMLTextAreaElement && el.value.trim() !== "") return;
      ref.current();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);
}

function submitOnMetaEnter(e: ReactKeyboardEvent<HTMLTextAreaElement>, fn: () => void) {
  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
    e.preventDefault();
    fn();
  }
}

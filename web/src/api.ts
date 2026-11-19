// Typed fetch wrappers for the 8sync harness web API. No `any` — every response
// is typed so the UI consumes a known shape.
//
// The backend serves an embedded SPA, so an unmatched /api/* path returns the
// index.html fallback with HTTP 200 (not a 404). We detect that (non-JSON body
// or json content-type missing) and throw a clean, human error so pages can
// render an honest "endpoint not ready yet" state instead of a JSON parse crash
// or raw HTML.

export type EngineStatus = { present: boolean; version: string; registered?: boolean; runner?: boolean };
export type Engines = {
  codegraph: EngineStatus;
  cbm: EngineStatus;
  // serena reports presence via mcp.json + uvx, and carries registration state.
  serena: EngineStatus;
  mnemopi_on: boolean;
};
export type SkillEntry = {
  name: string;
  tier: "always" | "on-demand" | "off";
  source: string;
  global: boolean;
  local: boolean;
};
export type BenchReport = {
  upfront: number;
  deferred: number;
  a1_pass: boolean;
  a2_saved_pct: number;
  force_load_prefix: number;
  core_tok: number;
  spine_tok: number;
  naive_tok: number;
  spine_advice: string | null;
};
export type RoleScore = { role: string; pct: number; detail: string[] };
export type EvalReport = { overall: number; total: number; present: number; roles: RoleScore[] };
export type StateInfo = { project: string; profile: string; state_md: string };
export type WorkspaceInfo = { profiles: string[]; project: string; session: string };
export type TeamInfo = {
  roster: { type: string; role: string; skills: string }[];
  readiness: EvalReport | null;
};
export type Submodule = { name: string; path: string; url: string; initialized: boolean };

// Normalized context — the new backend field names. normalizeContext() maps the
// legacy shape onto this so the page never branches on which backend shipped.
export type CtxInfo = {
  usedTok: number;
  windowTok: number;
  pct: number;
  thresholdPct: number;
  willCompact: boolean;
  assumed: boolean; // true when the window is an assumption, not measured
  model: string;
  project: string;
  session: string;
  lastCompactAt: number | null;
  compactionObserved: boolean;
  stale?: boolean; // session idle/ended — snapshot, not a live run
  sessionAgeSecs?: number;
  note?: string;
};

export type McpServer = { name: string; command: string; args: string[]; type: string; present: boolean };
export type Rule = { scope: string; name: string; path: string; bytes: number };
export type MarketInstall = {
  type: "stdio" | "http" | "sse" | "skill" | "link";
  command?: string;
  args?: string[];
  url?: string;
  spec?: string;
  env?: Record<string, string>;
  headers?: Record<string, string>;
};
export type MarketItem = {
  id: string;
  name: string;
  description: string;
  kind: "mcp" | "skill";
  source: string;
  stars: number;
  updated: string;
  url: string;
  new: boolean;
  install: MarketInstall;
};
export type MarketResponse = { kind: string; count: number; items: MarketItem[] };

// Curated knowledge — sindresorhus/awesome catalog + apply-to-project result.
export type KnowledgeEntry = { name: string; url: string; desc: string };
export type KnowledgeCategory = { name: string; count: number; entries: KnowledgeEntry[] };
export type KnowledgeCatalog = { source: string; categories: KnowledgeCategory[]; category_count: number; total: number };
export type KnowledgeApplyResult = { ok: boolean; added: number; path: string };
export type ProjectCreateResult = { ok: boolean; path: string; skills: { spec: string; ok: boolean }[]; mcp_added: number; knowledge_added: number };

// Codegraph — codebase-memory-mcp knowledge graph, bridged read-only for viz.
export type CgPackage = { name: string; node_count: number; fan_in: number; fan_out: number };
export type CgBoundary = { from: string; to: string; call_count: number };
export type CgCluster = {
  id: number; label: string; members: number; cohesion: number;
  top_nodes: string[]; packages: string[]; edge_types: string[];
};
export type CgHotspot = { name: string; qualified_name: string; fan_in: number };
export type CgOverview = {
  total_nodes: number; total_edges: number;
  node_labels: { label: string; count: number }[];
  languages: { language: string; file_count: number }[];
  packages: CgPackage[];
  boundaries: CgBoundary[];
  clusters: CgCluster[];
  hotspots: CgHotspot[];
};
export type CgSearchResult = {
  name: string; qualified_name: string; label: string;
  file_path: string; start_line: number; end_line: number; rank: number;
};
export type CgSearchResponse = { total: number; has_more: boolean; results: CgSearchResult[] };
export type CgTraceNode = { name: string; qualified_name: string; hop: number };
export type CgTrace = { function: string; direction: string; callers: CgTraceNode[]; callees: CgTraceNode[] };

// Live `/auto` engine run — the real gsd-pi state.json the engine drives.
export type EngineTaskView = { id?: string; title: string; status: "pending" | "in_progress" | "done" | "blocked"; retries?: number };
export type EngineSliceView = { id?: string; title: string; tasks: EngineTaskView[] };
export type EngineRun = {
  active: boolean;
  goal?: string;
  updatedAt?: string;
  total?: number;
  done?: number;
  blocked?: number;
  current?: { slice: string; task: string; status: string } | null;
  slices?: EngineSliceView[];
};

// Project switcher. `current` = the project the dashboard is viewing; `active` =
// current OR used within 2h (green dot).
export type ProjectEntry = {
  name: string;
  path: string;
  current?: boolean;
  active: boolean;
  lastModified?: number | null;
};

// Model routing config (`~/.config/ckit/models.toml`). roles = named slots;
// tasks = {class: model} keyed by class. Values are omp role refs (`@task`),
// model ids, or "" (omp default). `board` = what each Board step really runs,
// resolved through omp `modelRoles` (`~/.omp/agent/config.yml`).
export type ModelRole = "default" | "plan" | "smol" | "slow" | "vision";
export type ModelTask = "plan" | "review" | "debug" | "code" | "trivial";
export type ModelResolved = {
  setting: string;
  model: string | null;
  via: string;
};
export type ModelBoardRow = ModelResolved & {
  step: string;
  label: string;
  who: string;
  class: ModelTask | null;
};
export type ModelConfig = {
  path: string;
  roles: Partial<Record<ModelRole, string>>;
  tasks: Partial<Record<string, string>>;
  classes?: string[];
  advisor?: boolean;
  resolved?: Partial<Record<ModelTask, ModelResolved>>;
  board?: ModelBoardRow[];
  omp_config?: string;
  omp_roles?: Record<string, string>;
  omp_reviewer?: string | null;
};

// ── Board (AI delivery pipeline). User talks only to the Leader; two user
// gates (approval, acceptance) sit between the AI stages.
export type BoardStage =
  | "investigate" | "intake" | "solution" | "review" | "approval" | "dev" | "test" | "security" | "report" | "acceptance" | "action"
  | "done" | "cancelled";
/** One step of the fixed catalog (GET /api/board/steps). */
export type BoardStepDef = { key: BoardStage; label: string; who: string; gate: boolean; requires: BoardStage[] };
export type BoardStepPreset = { name: string; steps: BoardStage[] };
export type BoardStepCatalog = { catalog: BoardStepDef[]; presets: BoardStepPreset[] };
// waiting = needs the USER (questions at intake, approval gate, acceptance gate).
export type BoardStatus = "queued" | "running" | "waiting" | "failed" | "done" | "cancelled";
/** `from`: "user" | "leader" | "reviewer" | "system" | "boss", or a mid-step question "dev → leader". Legacy threads may hold "ba" | "po" (shown as Leader). */
export type BoardMsg = {
  from: string; text: string; at: string;
  /** log name of the run the message belongs to (by time window); null between runs */
  run?: string | null;
  kind?: BoardMsgKind;
  /** a boss note into a running step that the step has taken (inbox consumed / steer seen) */
  ack?: boolean;
  /** the agent's next words after the note, when known */
  reply?: string | null;
  /** a boss button action ([Tạm dừng], [Tiếp tục], Duyệt…): never "waiting for a reply" */
  action?: boolean;
};
export type BoardMsgKind = "boss" | "leader" | "report" | "question" | "answer" | "coordinator" | "delegated_approval" | "system";
/** A mid-step question from a running step to the Leader or the boss (board_ask). */
export type BoardAsk = {
  id: string;
  from: BoardStage;
  /** legacy task.json asks may still say "ba" | "po": read as "leader" */
  to: "leader" | "boss" | "ba" | "po";
  question: string;
  options: string[];
  at: string;
  /** answering = Leader session answering; boss = waiting for the boss; closed = run ended unanswered */
  status: "answering" | "boss" | "answered" | "timeout" | "closed";
  answer: string | null;
  by: string | null;
  answered_at: string | null;
  /** log name of the run that asked */
  run?: string | null;
  /** `ac_change`: a run wants to reinterpret / loosen / drop an AC; only the boss decides (never times out) */
  kind?: "ac_change" | null;
  ac?: { ac: string; old: string; new: string; reason: string; evidence: string } | null;
  phase?: string | null;
};
/** Phase close blocked until the boss rules on every AC change (`asks` = their ids). */
export type AcGate = { phase: string; route: string | null; asks: string[] };
/** GET /api/board/:id/goals: requirement → AC → task → commit → result, plus orphans. */
export type GoalAc = {
  id: string; phase: string | null; text: string; uc: string | null; source: string | null;
  status: "pass" | "fail" | "needs_confirm" | "pending"; note: string | null;
  /** non-null = the AC was re-graded / reinterpreted; `approved_by: "boss"` only after an approved ac_change ask */
  changed: { approved_by: "boss" | null; at: string | null; ask: string | null; decision: "approved" | "rejected" | "pending" | null } | null;
  tasks: string[]; commits: { sha: string; subject: string }[]; evidence: string | null;
};
export type BoardGoals = {
  requirements: { id: string; text: string }[];
  acs: GoalAc[];
  orphans: { acs_without_task: string[]; tasks_without_ac: string[] };
  gate: AcGate | null;
};
/** Merge request / PR of the task: made at acceptance ("Nghiệm thu" = create the MR) or from the chat. */
export type BoardMr = { url: string; provider: "gitlab" | "github"; target: string; at: string; source?: string; head?: string };
/** A commit the MR would carry (`origin/<target>..branch`, oldest first); `from_task` false = not made by the task. */
export type MrCommit = { sha: string; short: string; subject: string; author: string; at: string; from_task: boolean };
/** A Leader proposal in the task chat; the boss confirms (optionally edited fields) or cancels.
 *  action = a normal omp run on confirm (the agent uses the project's skill/command, e.g.
 *  gitlab-mr); old merge_request / accept proposals are served as action with `pushes`. */
export type ChatProposal = {
  kind: "action" | "revise";
  state: "open" | "running" | "done" | "failed" | "cancelled";
  /** the action run's outcome: URLs it reported (an MR/PR URL also sets task.mr), its message, or the error */
  result?: { url?: string; urls?: string[]; error?: string; message?: string } | null;
  title?: string;
  instruction?: string;
  /** skill / command the Leader suggests (e.g. gitlab-mr); null = the agent picks */
  skill?: string | null;
  /** the action pushes the task branch: the git part below is present */
  pushes?: boolean;
  /** writes outside the repo the Leader expects (e.g. "PMS ticket #26809 (solution + status)") */
  writes_external?: string[];
  target?: string; source?: string;
  /** remote branches (`git ls-remote --heads origin`, board/* excluded); `remote_default` = origin HEAD */
  provider?: "gitlab" | "github" | null; host?: string | null; targets?: string[]; remote_default?: string | null;
  commits?: MrCommit[]; foreign?: MrCommit[];
  /** e.g. "nhánh đích develop không có trên origin" */
  check_error?: string | null;
  /** what the boss chose when confirming ("task" = only the task's commits on `<source>-mr`) */
  push?: "task" | "all" | null;
};
export type ChatMessage = { id: string; from: "boss" | "leader" | "system"; text: string; at: string; cost: number; proposal?: ChatProposal | null };
export type BoardChat = { messages: ChatMessage[]; busy: boolean; mr: BoardMr | null };
/** `push`: "task" = only the task's commits (cherry-picked onto origin/<target>, the default), "all" = the branch as is. */
export type ChatFields = { target?: string; title?: string; instruction?: string; push?: "task" | "all" };
/** GET /api/board/:id/mr/draft: the acceptance action (create the MR; computed live from origin, ~1 s). */
export type MrDraftResp = Omit<ChatProposal, "state" | "result"> & { mr: BoardMr | null };
export type BoardRun = {
  stage: BoardStage;
  n: number;
  /** action runs: what the boss confirmed ("Tạo merge request") */
  title?: string;
  /** revise round (Vòng N), present when ≥ 2 */
  round?: number;
  started: string;
  ended: string | null;
  exit: number | null;
  cost: number;
  verdict: string | null;
  log: string;
  /** model/effort of this run, from its log (detail only); null before its first turn */
  model?: RunModel | null;
  /** `/feature` phase the run worked on; null for intake / report */
  phase?: string | null;
  /** ≤3 lines: the result message, else the log's last assistant text */
  summary?: string[];
  /** where the summary comes from: the Leader's thread message, the result message, or the log's last text */
  summary_source?: "thread" | "result" | "log" | null;
  /** computed once when the run ends */
  digest?: RunDigest | null;
  /** task-branch commits made during the run */
  commits?: { sha: string; subject: string }[];
  /** verdict fail: why */
  fail_reason?: string | null;
};
export type RunDigest = {
  tools: number; errors: number; harness_errors: number; files: string[]; last_error: string | null;
  subagents: number; subagent_models: string[];
  /** subagent sessions of the run; `id` → GET /log?run=N&agent=<id> */
  subagents_list?: { id: string; name: string; model: string | null }[];
};
/** The model a run is on: last assistant turn of its log; `subagents` = child sessions (detail only). */
export type RunModel = {
  provider: string;
  model: string;
  effort: string | null;
  credential_id: number | null;
  subagents?: { model: string; effort: string | null; count: number }[];
};
/** GET /api/board/usage: subscription quota per provider account (`omp usage`), redacted. */
export type UsageWindow = { id: string; label: string; used_pct: number; resets_at: number | null; status: string };
export type UsageTask = { project: string; id: string; title: string; provider: string; model: string; effort: string | null; credential_id: number | null };
export type UsageAccount = {
  provider: string;
  label: string;
  org: string | null;
  plan: string | null;
  credential_ids: number[];
  limit_reached: boolean;
  fetched_at: number | null;
  windows: UsageWindow[];
  /** running tasks drawing from this account */
  tasks: UsageTask[];
};
export type BoardUsage = {
  loading: boolean;
  error: string | null;
  generated_at: number | null;
  accounts: UsageAccount[];
  disabled: { provider: string; label: string }[];
  running: UsageTask[];
};
export type BoardTaskSummary = {
  id: string;
  title: string;
  stage: BoardStage;
  status: BoardStatus;
  note: string | null;
  branch: string | null;
  /** active `/feature` phase (e.g. "M1"), null before planning or after the last ship */
  phase: string | null;
  /** sub-task progress of the active phase (null before a PLAN exists) */
  progress: { done: number; total: number } | null;
  cost: number;
  /** open questions waiting for the boss (asks with status `boss`); absent on old servers */
  asking?: number;
  /** paused at a safe point (status `waiting`); absent on summaries of old servers, see `isPaused` */
  paused?: boolean;
  /** waiting for the boss to rule on AC changes before a phase closes */
  ac_gate?: AcGate | null;
  /** current round (1 = original request); each confirmed "revise" adds one */
  round?: number;
  mr?: BoardMr | null;
  /** has run at least once; false → parked as "Chưa bắt đầu" */
  started?: boolean;
  /** start of the current run while running (for elapsed time) */
  run_started?: string | null;
  /** model of the current (or last) run; absent on old servers */
  run_model?: RunModel | null;
  created: string;
  updated: string;
  /** workflow command name (`cook`) of a slash-command task; null = `/feature` */
  workflow?: string | BoardWorkflow | null;
  /** the PMS task/ticket the boss pulled this from; absent on old tasks */
  source?: BoardSource | null;
};
/** Where a task came from: a CloudGO PMS task (`PT…`), HelpDesk ticket (`#…`) or a report row of any other module. */
export type BoardSource = { system: "pms"; kind: PmsKind | "record"; id: string; no: string; url: string };

/** One finding of `ckit review`. `inline`: blocking on an added line (posted as a line comment). */
export type MrFinding = { path: string; line: number; severity: string; title: string; body: string; inline: boolean };
/** The last review of a PR/MR (`.cache/ckit/reviews/<n>.json`); `at`/`posted_at`: unix seconds. */
export type MrReview = { number: number; title: string; url: string; head_sha: string; at: number; model: string | null; summary: string; findings: MrFinding[]; posted_at: number | null };
/** `queued`: waiting for one of the server's review slots (2 run at once). `posted_head`/`posted_by`:
 *  commit that ckit's summary comment on the MR covers and who posted it (CI bot, this account);
 *  `other_review`: a "Code Review" comment written without ckit (`sha`: the commit it cites).
 *  All three are known once the MR's comments were loaded. */
export type MrState = {
  queued: boolean; running: boolean; error: string | null; review: MrReview | null;
  posted_head: string | null; posted_by: string | null; other_review: { author: string; at: string; sha: string | null } | null;
  /** Last run of the project's own `/review-pr` on this MR (projects that have the command). */
  command_run: MrCommandRun | null;
};
/** `/review-pr` flags the boss picked. */
export type MrFlags = { no_merge: boolean; dry_run: boolean };
/** Outcome of `/review-pr` run by the Board; `at`/`ended`: unix seconds; `blocking`: CRITICAL/HIGH/PERFORMANCE
 *  count. `verdict: "ask"`: stopped at the command's question, `draft` = the comment it would post. */
export type MrCommandRun = { head_sha: string; at: number; ended: number; flags: MrFlags; verdict: "pass" | "fail" | "ask"; merged: boolean; posted: boolean; blocking: number; message: string; draft: string; cost: number; log: string };
/** A comment on the MR (everyone's). `path`/`line`: a line comment; `ckit`: posted by ckit's review. */
export type MrComment = { id: number; author: string; body: string; at: string; path: string; line: number | null; resolved: boolean | null; ckit: boolean };
export type MrLabel = { name: string; color: string };
export type MrItem = MrState & { number: number; title: string; author: string; url: string; draft: boolean; head_sha: string; source: string; target: string; labels: MrLabel[]; created: string; updated: string };
export type PmsKind = "task" | "ticket";
export type PmsStatusFilter = "open" | "done" | "all";
/** Extra PMS search filters; "" = not set. `statuses` (exact) wins over the preset. */
export type PmsFilters = {
  statuses: string[]; project: string; view: string; priority: string; category: string; owner: string;
  /** Names of a project / person found by the server-side search (not in the cached lists). */
  projectLabel?: string; ownerLabel?: string;
};
/** One filter option. Saved lists carry their group, picklists their PMS colour, projects their status. */
export type PmsOpt = { value: string; label: string; group?: string; color?: string };
export type PmsMeta = { projects: PmsOpt[]; views: PmsOpt[]; statuses: PmsOpt[]; priorities: PmsOpt[]; categories: PmsOpt[]; users: PmsOpt[] };
/** The boss's own PMS account (`~/.config/ckit/pms.json`). Never carries the password or access key.
 * `report`: an Access Key is saved, so PMS reports can be opened. */
export type PmsStatus = { configured: boolean; base_url: string | null; user: string | null; display_name: string | null; report: boolean };
/** What "Dùng tài khoản từ MCP pms" would import (no password); `report`: it carries an Access Key. */
export type PmsMcpPreview = { found: boolean; base_url: string | null; user: string | null; report: boolean };
/** One row of a PMS report: the non-empty columns in the report's order. */
export type PmsReportRow = { id: string; title: string; cells: { label: string; value: string }[]; url: string };
/** One page of a PMS report. `kind` is set when the rows are tasks / tickets (they open like search results). */
export type PmsReport = { id: string; name: string; module: string; kind: PmsKind | null; chart: boolean; rows: PmsReportRow[]; has_more: boolean; url: string };
/** One search row. Task lists carry no number (`no` empty); the detail has it. */
export type PmsItem = {
  kind: PmsKind; id: string; no: string; title: string; status: string; priority: string;
  project: string; assignee: string; due: string; updated: string; created: string; url: string;
  /** task type (`Coding`) / ticket category (`Bug`) */
  category: string;
  /** task progress (`40%`), empty for tickets */
  progress: string;
  /** planned start (tasks) */
  start: string;
  /** logged hours, empty when none */
  hours: string;
};
export type PmsDetail = PmsItem & {
  description: string;
  checklist: { name: string; done: boolean }[];
  subtasks: { title: string; status: string }[];
  comments: { author: string; text: string; at: string }[];
};
export type BoardTask = BoardTaskSummary & {
  request: string;
  base: string | null;
  /** `/feature` slug; planning tree at agents/planning/<feature>/ */
  feature: string | null;
  thread: BoardMsg[];
  runs: BoardRun[];
  rounds: { solution: number };
  /** enabled steps, catalog order; absent on old tasks = Feature đầy đủ */
  steps?: BoardStage[];
  /** mid-step questions (board_ask); absent on old tasks */
  asks?: BoardAsk[];
  /** the boss's slash-command workflow; absent/null = the `/feature` pipeline */
  workflow?: BoardWorkflow | null;
};
/** A workflow task's command (`/cook`) and the plan file its plan step wrote. */
export type BoardWorkflow = { name: string; path: string; plan?: string | null };
/** An omp slash command the board can run as a task's workflow, per project. */
export type BoardCommand = { name: string; path: string; description: string; argument_hint: string; board: boolean; enabled: boolean };
/** One document of the task: `/feature` tree file (PROJECT, ROADMAP, M0-CONTEXT…) or board REVIEW/REPORT. */
export type BoardDoc = { name: string; content: string };
export type BoardDetail = {
  task: BoardTask; docs: BoardDoc[]; focus: string | null; phase: string | null;
  progress: { done: number; total: number } | null;
  /** finished plan-only task whose branch is still unmerged: "Code theo kế hoạch này" applies; absent on old servers */
  can_continue?: boolean;
  /** REPORT.md "Sếp cần quyết" bullets */
  report?: { decide: string[] } | null;
  /** what happens now: latest PM tick, else the current run's log tail */
  now?: BoardNow | null;
  /** boss messages with no reply after them */
  unanswered?: { at: string; text: string }[];
};
export type BoardNow = {
  /** pm = PM status tick; log = read from the run log tail; run = no readable log yet ("đang chạy") */
  activity: string | null; at: string | null; source: "pm" | "log" | "run" | null;
  /** last ≤5 human-readable actions, oldest first; `tool` null = the agent's own words */
  recent?: { at: string | null; tool: string | null; text: string }[];
  /** edit / write paths seen in the log tail */
  files?: string[];
  stage?: string | null; phase?: string | null; started?: string | null;
};
/** GET /api/board/:id/decisions: one decision, who made it, where it is written. */
export type BoardDecision = {
  at: string | null; by: "boss" | "leader" | "reviewer" | "coordinator"; phase: string | null;
  kind: "gate" | "delegated" | "answer" | "plan" | "ac_regrade" | "ac_change"; text: string; ref: string | null;
  /** ac_change: the boss's comment ("Duyệt. …" / "Không duyệt. …") */
  why?: string | null;
};
/** `harness`: a tool call the harness blocked or cut short (code-intel-first, eval timeout); `error`: a real failure. */
export type BoardLogLine = { kind: "text" | "tool" | "error" | "harness"; text: string };
export type BoardLog = { run: BoardRun | null; running: boolean; lines: BoardLogLine[] };
export type BoardPhase = { id: string; name: string; state: "done" | "active" | "todo"; ac_pass: number; ac_total: number };
export type BoardSubTask = {
  /** `title` = first line (≤120 chars), `detail` = the full text */
  key: string; title: string; detail?: string; files: string; depends: string; ac: string;
  status: "todo" | "in_progress" | "done" | "blocked"; retries: number; note: string;
};
/** One step chip of a phase: done / now / todo. */
export type PhaseStep = { key: BoardStage; state: "done" | "now" | "todo" };
/** Sub-task row of the epic view; `commit` = short hash of the branch commit naming the task key. */
export type SubTaskRow = BoardSubTask & { commit: string | null };
/** Phase of the epic view; `planned=false` → its tasks are not written yet. */
export type PhaseBoard = BoardPhase & {
  goal: string; planned: boolean; steps: PhaseStep[]; tasks: SubTaskRow[];
  /** indexes into task.runs */
  runs?: number[];
  started?: string | null; ended?: string | null; cost?: number;
  /** reviewer fails (plan sent back) */
  rounds_back?: number;
  /** re-graded / noted ACs */
  ac_notes?: { id: string; status: string; note: string }[];
  /** before/after table of the phase VERIFICATION */
  result?: { label: string; before: string; after: string }[];
};
/** GET /api/board/:id/phases. Quick presets (no `/feature` tree): `phases: []`, tasks in `flat`. */
export type BoardPhases = { phases: PhaseBoard[]; flat: SubTaskRow[]; live: boolean };
/** Global Telegram bridge: bot token + pairing with the boss's chat. */
export type TelegramState =
  | { configured: false }
  | { configured: true; bot: string; paired: boolean; pair_code: string | null };
// Every project with board tasks + the current one, sorted by need.
export type ProjectBoard = {
  /** absolute root; pass as ?root= to every per-task call */
  project: string;
  name: string;
  current: boolean;
  waiting: number;
  failed: number;
  running: number;
  queued: number;
  cost: number;
  /** ALL tasks, newest first */
  tasks: BoardTaskSummary[];
};
export type BoardAll = { current: string; projects: ProjectBoard[] };
/** Coordinator chat line, oldest first. `event` = something the coordinator did. */
export type ChatMsg = { from: "boss" | "coordinator" | "event"; text: string; at: string; cost: number };
export type CoordState = { enabled: boolean; busy: boolean; chat: ChatMsg[] };
// PM: deterministic status lines every interval, AI alerts on anomalies, and
// markdown board reports. Entries arrive newest LAST.
export type PmEntry = { at: string; task: string | null; stage: BoardStage | null; kind: "status" | "alert" | "report"; text: string };
export type PmFeed = { entries: PmEntry[]; reporting: boolean; interval_secs: number; last_tick: string | null };

// Error thrown when a backend endpoint is not implemented yet (SPA fallback).
export class EndpointMissingError extends Error {
  endpoint: string;
  constructor(endpoint: string) {
    super(
      `Backend is still building ${endpoint}. This screen will light up once the ` +
        `route ships — no action needed here.`,
    );
    this.endpoint = endpoint;
    this.name = "EndpointMissingError";
  }
}

async function json<T>(url: string, init?: RequestInit): Promise<T> {
  const r = await fetch(url, init);
  // SPA fallback: backend returns 200 + index.html for unknown /api paths. A
  // missing endpoint therefore looks like a successful HTML response.
  const ct = r.headers.get("content-type") ?? "";
  const looksLikeHtml = ct.includes("text/html") || !ct.includes("json");
  if (r.ok && looksLikeHtml) {
    const text = await r.text();
    if (text.trimStart().startsWith("<")) throw new EndpointMissingError(url);
    return JSON.parse(text) as T;
  }
  if (!r.ok) throw new Error(`${r.status} ${await r.text().catch(() => r.statusText)}`);
  return (await r.json()) as T;
}

const POST_JSON = (body: unknown): RequestInit => ({
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
});

/// Map any context response shape (legacy or new) onto the normalized CtxInfo.
/// `assumed` defaults to true when absent — the window is an assumption until a
/// backend reports a measured one, and the page must not imply false precision.
export function normalizeContext(raw: Record<string, unknown>): CtxInfo {
  const num = (v: unknown, d = 0): number =>
    typeof v === "number" ? v : typeof v === "string" && v.trim() !== "" ? Number(v) || d : d;
  const str = (v: unknown): string => (typeof v === "string" ? v : v == null ? "" : String(v));
  const usedTok = num(raw.usedTok ?? raw.used);
  const windowTok = num(raw.windowTok ?? raw.window, 1_000_000);
  const thresholdPct = num(raw.thresholdPct ?? raw.threshold_pct, 50);
  const willCompact =
    raw.willCompact != null ? Boolean(raw.willCompact) : Boolean(raw.over_threshold);
  const assumed = raw.assumed != null ? Boolean(raw.assumed) : true;
  return {
    usedTok,
    windowTok,
    pct: num(raw.pct, windowTok > 0 ? (usedTok * 100) / windowTok : 0),
    thresholdPct,
    willCompact,
    assumed,
    model: str(raw.model),
    project: str(raw.project),
    session: str(raw.session),
    lastCompactAt:
      raw.lastCompactAt != null
        ? num(raw.lastCompactAt)
        : raw.last_compact_at != null
          ? num(raw.last_compact_at)
          : null,
    compactionObserved: Boolean(raw.compaction_observed ?? raw.compactionObserved),
    stale: raw.stale != null ? Boolean(raw.stale) : undefined,
    sessionAgeSecs: raw.sessionAgeSecs != null ? num(raw.sessionAgeSecs) : undefined,
    note: str(raw.note) || undefined,
  };
}

export const api = {
  state: () => json<StateInfo>("/api/state"),
  skills: () => json<SkillEntry[]>("/api/skills"),
  toggleSkill: (name: string, when: SkillEntry["tier"]) =>
    json<{ name: string; tier: SkillEntry["tier"] }>("/api/skills/toggle", POST_JSON({ name, when })),
  skillAdd: (spec: string) => json<{ ok: boolean; log: string }>("/api/skills/add", POST_JSON({ spec })),
  skillUpdate: (name?: string) => json<{ ok: boolean; log: string }>("/api/skills/update", POST_JSON({ name })),
  memory: (file: string) => json<{ file: string; content: string }>(`/api/memory/${file}`),
  saveMemory: (file: string, content: string) =>
    json<{ ok: boolean }>("/api/memory/" + file, POST_JSON({ content })),
  engines: () => json<Engines>("/api/engines"),
  engine: () => json<EngineRun>("/api/engine"),
  bench: () => json<BenchReport>("/api/bench"),
  evalProject: () => json<EvalReport>("/api/eval"),
  workspaces: () => json<WorkspaceInfo>("/api/workspaces"),
  activateWorkspace: (profile?: string, project?: string) =>
    json<unknown>("/api/workspaces/activate", POST_JSON({ profile, project })),
  team: () => json<TeamInfo>("/api/team"),
  submodules: () => json<Submodule[]>("/api/submodules"),
  submoduleAdd: (url: string, path?: string) =>
    json<{ ok: boolean; path: string }>("/api/submodules/add", POST_JSON({ url, path })),
  submodulePull: (path: string) => json<{ ok: boolean }>("/api/submodules/pull", POST_JSON({ path })),
  submoduleRemove: (path: string) =>
    json<{ ok: boolean }>("/api/submodules/remove", POST_JSON({ path })),
  context: () => json<Record<string, unknown>>("/api/context").then(normalizeContext),
  mcp: () => json<{ servers: McpServer[] }>("/api/mcp"),
  rules: () => json<Rule[]>("/api/rules"),
  ruleAdd: (name: string, content: string, scope?: string) =>
    json<{ ok: boolean; path: string }>("/api/rules/add", POST_JSON({ name, content, scope })),
  ruleDelete: (path: string) => json<{ ok: boolean }>("/api/rules/delete", POST_JSON({ path })),

  // ── New endpoints (backend ships these in parallel; FE degrades cleanly) ──
  projects: () => json<ProjectEntry[]>("/api/projects"),
  activateProject: (path: string) =>
    json<unknown>("/api/workspaces/activate", POST_JSON({ project: path })),
  models: () => json<ModelConfig>("/api/models"),
  modelSet: (section: "roles" | "tasks", key: string, value: string) =>
    json<ModelConfig>("/api/models", POST_JSON({ section, key, value })),

  // ── Codegraph (codebase-memory-mcp bridge) ──
  codegraphOverview: () => json<CgOverview>("/api/codegraph/overview"),
  codegraphSearch: (q: string, limit = 20) =>
    json<CgSearchResponse>(`/api/codegraph/search?q=${encodeURIComponent(q)}&limit=${limit}`),
  codegraphTrace: (symbol: string, depth = 2) =>
    json<CgTrace>(`/api/codegraph/trace?symbol=${encodeURIComponent(symbol)}&depth=${depth}`),

  // ── Marketplace (discover + install skills / MCP from external registries) ──
  marketplace: (kind: "mcp" | "skill", search = "", sort: "top" | "new" = "top") =>
    json<MarketResponse>(
      `/api/marketplace?kind=${kind}&sort=${sort}${search ? `&search=${encodeURIComponent(search)}` : ""}`,
    ),
  mcpAdd: (body: { name: string; command?: string; args?: string[]; type?: string; url?: string; spec?: string; env?: Record<string, string>; headers?: Record<string, string> }) =>
    json<{ ok: boolean; name: string; note: string }>("/api/mcp/add", POST_JSON(body)),
  mcpRemove: (name: string) => json<{ ok: boolean; removed: boolean }>("/api/mcp/remove", POST_JSON({ name })),
  ruleImport: (source: string, scope?: string) =>
    json<{ ok: boolean; imported: number; files: string[] }>("/api/rules/import", POST_JSON({ source, scope })),

  // ── Curated knowledge (sindresorhus/awesome) ──
  knowledge: (search = "", refresh = false) =>
    json<KnowledgeCatalog>(
      `/api/knowledge?${refresh ? "refresh=1&" : ""}${search ? `search=${encodeURIComponent(search)}` : ""}`,
    ),
  knowledgeApply: (items: (KnowledgeEntry & { category: string })[], project?: string) =>
    json<KnowledgeApplyResult>("/api/knowledge/apply", POST_JSON({ items, project })),

  // ── Create project (scaffold + skills + mcp + knowledge) ──
  projectCreate: (body: {
    name?: string;
    path?: string;
    skills?: string[];
    mcp?: { name: string; spec: string }[];
    knowledge?: (KnowledgeEntry & { category: string })[];
  }) =>
    json<ProjectCreateResult>("/api/projects/create", POST_JSON(body)),

  // ── Board (AI delivery pipeline) ──
  // Every call after the list carries the `project` the list returned (`root`):
  // task ids repeat across repos, and the dashboard's active project can change
  // while a task page is open — actions must hit the task that was shown.
  board: () =>
    json<{ project: string; tasks: BoardTaskSummary[]; others: { project: string; active: number; waiting: number }[] }>("/api/board"),
  boardAll: () => json<BoardAll>("/api/board/all"),
  /** `workflow`: the boss's slash command (`cook`) instead of `/feature`; fixed steps, `steps` ignored. */
  boardCreate: (root: string, title: string, request: string, steps?: BoardStage[], workflow?: string | null, source?: BoardSource | null) =>
    json<BoardTask>(`/api/board?${rootQ(root)}`, POST_JSON({ title, request, steps, workflow: workflow ?? undefined, source: source ?? undefined })),
  /** CloudGO PMS: connect the boss's account, then search tasks/tickets (read-only) → one item's detail. */
  pmsStatus: () => json<PmsStatus>("/api/board/pms/status"),
  pmsConnect: (p: { base_url: string; username: string; password: string; access_key?: string }) => json<PmsStatus>("/api/board/pms", POST_JSON(p)),
  /** Save (checked against PMS first) or clear (`""`) the Access Key for reports. */
  pmsKey: (access_key: string) => json<PmsStatus>("/api/board/pms/key", POST_JSON({ access_key })),
  /** One page (1-based, ≤500 rows) of a PMS report. */
  pmsReport: (id: string, page: number) => json<PmsReport>(`/api/board/pms/report?${new URLSearchParams({ id, page: String(page) })}`),
  pmsTest: () => json<PmsStatus>("/api/board/pms/test", { method: "POST" }),
  pmsOff: () => json<PmsStatus>("/api/board/pms/off", { method: "POST" }),
  pmsMcpPreview: () => json<PmsMcpPreview>("/api/board/pms/import-mcp"),
  pmsMcpImport: () => json<PmsStatus>("/api/board/pms/import-mcp", { method: "POST" }),
  pmsSearch: (p: { kind: PmsKind; q: string; mine: boolean; status: PmsStatusFilter; page: number } & PmsFilters) =>
    json<{ items: PmsItem[]; has_more: boolean }>(
      `/api/board/pms/search?${new URLSearchParams({
        kind: p.kind, q: p.q, mine: p.mine ? "1" : "0", status: p.status, page: String(p.page),
        statuses: p.statuses.join(","), project: p.project, view: p.view, priority: p.priority, category: p.category, owner: p.owner,
      })}`,
    ),
  /** Filter options for a kind (saved lists, picklists, open projects, people); server-cached 10 min. */
  pmsMeta: (kind: PmsKind) => json<PmsMeta>(`/api/board/pms/meta?${new URLSearchParams({ kind })}`),
  /** Any project / person by name, server-side (the cached lists are capped). */
  pmsProjects: (q: string) => json<{ items: PmsOpt[] }>(`/api/board/pms/projects?${new URLSearchParams({ q })}`).then((r) => r.items),
  pmsUsers: (q: string) => json<{ items: PmsOpt[] }>(`/api/board/pms/users?${new URLSearchParams({ q })}`).then((r) => r.items),
  pmsItem: (kind: PmsKind, id: string) => json<PmsDetail>(`/api/board/pms/item?${new URLSearchParams({ kind, id })}`),
  boardWorkflows: (root: string) => json<{ commands: BoardCommand[] }>(`/api/board/workflows?${rootQ(root)}`),
  boardWorkflowToggle: (root: string, name: string, enabled: boolean) =>
    json<{ commands: BoardCommand[] }>(`/api/board/workflows?${rootQ(root)}`, POST_JSON({ name, enabled })),
  /** Board view "Review MR": open PRs/MRs with their last review. `command`: the project's
   *  `.omp/commands/review-pr.md` when it has one (reviews then run that command). */
  boardMrs: (root: string) => json<{ host: "gitlab" | "github"; items: MrItem[]; command: string | null }>(`/api/board/mrs?${rootQ(root)}`),
  boardMr: (root: string, n: number) => json<MrState>(`/api/board/mrs/${n}?${rootQ(root)}`),
  boardMrComments: (root: string, n: number) => json<{ items: MrComment[]; state: MrState }>(`/api/board/mrs/${n}/comments?${rootQ(root)}`),
  boardMrReview: (root: string, n: number, flags: MrFlags = { no_merge: false, dry_run: false }) => json<MrState>(`/api/board/mrs/${n}/review?${rootQ(root)}`, POST_JSON(flags)),
  /** Answer `/review-pr`'s question (post / don't post, optional edit); resumes the run. */
  boardMrAnswer: (root: string, n: number, post: boolean, note = "") => json<MrState>(`/api/board/mrs/${n}/answer?${rootQ(root)}`, POST_JSON({ post, note })),
  boardMrPost: (root: string, n: number) => json<MrState>(`/api/board/mrs/${n}/post?${rootQ(root)}`, { method: "POST" }),
  boardSteps: (root: string, id: string, steps: BoardStage[]) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/steps?${rootQ(root)}`, POST_JSON({ steps })),
  boardStepCatalog: () => json<BoardStepCatalog>("/api/board/steps"),
  boardTask: (root: string, id: string) => json<BoardDetail>(`/api/board/${encodeURIComponent(id)}?${rootQ(root)}`),
  /** `agent`: one subagent session of the run (`digest.subagents_list[i].id`). */
  boardLog: (root: string, id: string, run?: number, agent?: string) =>
    json<BoardLog>(`/api/board/${encodeURIComponent(id)}/log?${rootQ(root)}${run != null ? `&run=${run}` : ""}${agent ? `&agent=${encodeURIComponent(agent)}` : ""}`),
  boardDecisions: (root: string, id: string) => json<BoardDecision[]>(`/api/board/${encodeURIComponent(id)}/decisions?${rootQ(root)}`),
  boardGoals: (root: string, id: string) => json<BoardGoals>(`/api/board/${encodeURIComponent(id)}/goals?${rootQ(root)}`),
  boardChat: (root: string, id: string) => json<BoardChat>(`/api/board/${encodeURIComponent(id)}/chat?${rootQ(root)}`),
  boardChatSend: (root: string, id: string, text: string) => json<BoardChat>(`/api/board/${encodeURIComponent(id)}/chat?${rootQ(root)}`, POST_JSON({ text })),
  boardChatConfirm: (root: string, id: string, msg: string, fields?: ChatFields) =>
    json<BoardChat>(`/api/board/${encodeURIComponent(id)}/chat/${encodeURIComponent(msg)}/confirm?${rootQ(root)}`, POST_JSON(fields ? { fields } : {})),
  boardChatCancel: (root: string, id: string, msg: string) =>
    json<BoardChat>(`/api/board/${encodeURIComponent(id)}/chat/${encodeURIComponent(msg)}/cancel?${rootQ(root)}`, { method: "POST" }),
  boardReply: (root: string, id: string, text: string) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/reply?${rootQ(root)}`, POST_JSON({ text })),
  /** `thenCode`: a plan-only task also turns on Dev, Test, Report, Acceptance (→ Dev queued). */
  boardApprove: (root: string, id: string, thenCode = false) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/approve?${rootQ(root)}${thenCode ? "&then=code" : ""}`, { method: "POST" }),
  boardMrDraft: (root: string, id: string) => json<MrDraftResp>(`/api/board/${encodeURIComponent(id)}/mr/draft?${rootQ(root)}`),
  /** Acceptance = create the MR with these fields (or link the existing one: empty body when `task.mr`). */
  boardAcceptMr: (root: string, id: string, fields: ChatFields) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/approve?${rootQ(root)}`, POST_JSON(fields)),
  /** A finished plan-only task goes on to code its approved plan (→ Dev queued). */
  boardContinue: (root: string, id: string) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/continue?${rootQ(root)}`, { method: "POST" }),
  boardRetry: (root: string, id: string) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/retry?${rootQ(root)}`, { method: "POST" }),
  boardCancel: (root: string, id: string) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/cancel?${rootQ(root)}`, { method: "POST" }),
  /** `decision` (AC-change asks): the server turns it into "Duyệt. <answer>" / "Không duyệt. <answer>". */
  boardAskAnswer: (root: string, id: string, qid: string, answer: string, decision?: "approve" | "reject") =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/asks/${encodeURIComponent(qid)}?${rootQ(root)}`, POST_JSON(decision ? { answer, decision } : { answer })),
  boardPm: (root: string, limit = 100) => json<PmFeed>(`/api/board/pm?${rootQ(root)}&limit=${limit}`),
  boardInbox: (root: string, id: string, kind: "note" | "pause", text: string) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/inbox?${rootQ(root)}`, POST_JSON({ kind, text })),
  boardResume: (root: string, id: string, text: string) =>
    json<BoardTask>(`/api/board/${encodeURIComponent(id)}/resume?${rootQ(root)}`, POST_JSON({ kind: "note", text })),
  coordGet: (root: string) => json<CoordState>(`/api/board/coord?${rootQ(root)}`),
  coordSay: (root: string, text: string) => json<{ ok: boolean }>(`/api/board/coord?${rootQ(root)}`, POST_JSON({ text })),
  coordEnabled: (root: string, enabled: boolean) =>
    json<{ enabled: boolean }>(`/api/board/coord/enabled?${rootQ(root)}`, POST_JSON({ enabled })),
  boardPmReport: (root: string) => json<{ ok: boolean }>(`/api/board/pm/report?${rootQ(root)}`, { method: "POST" }),
  boardPhases: (root: string, id: string) =>
    json<BoardPhases>(`/api/board/${encodeURIComponent(id)}/phases?${rootQ(root)}`),

  // ── Telegram (global, not per project) ──
  telegramGet: () => json<TelegramState>("/api/board/telegram"),
  telegramConnect: (token: string) => json<TelegramState>("/api/board/telegram", POST_JSON({ token })),
  telegramTest: () => json<TelegramState>("/api/board/telegram/test", { method: "POST" }),
  telegramOff: () => json<TelegramState>("/api/board/telegram/off", { method: "POST" }),
  boardUsage: () => json<BoardUsage>("/api/board/usage"),
};

function rootQ(root: string): string {
  return `root=${encodeURIComponent(root)}`;
}

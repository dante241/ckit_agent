// ckit engine - a gsd-pi-style automation engine built 100% on omp core.
//
// It does NOT patch omp: it lives in omp's config dir (~/.omp/agent/extensions/
// global + <root>/.omp/extensions/ project, auto-loaded by the native extension
// provider) and exposes model-callable TOOLS that carry the parts gsd-pi enforces
// in CODE rather than prose:
//   - a durable milestone/slice/task state machine (JSON at .cache/ckit/engine/),
//   - verify-with-auto-retry (the tool runs the commands + counts attempts +
//     blocks a task once max retries is hit - the agent can't skip the gate),
//   - an independent reviewer (a fresh `omp -p --no-tools` run that never saw the
//     work) that audits red-first tests for fairness and gates worktree merges,
//   - acceptance criteria that only close with evidence,
//   - git worktree open / squash-merge / remove (code, not "please run git").
// The `/auto` slash command orchestrates these tools into a run-to-done loop.
// This is THE automation path: a single `/auto` command (no competing `/gs`).
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { extname, join } from "node:path";
import { spawn, spawnSync } from "node:child_process";

type TaskStatus = "pending" | "in_progress" | "done" | "blocked" | "dropped";

interface EngineTask {
  id: string;
  key: string;
  title: string;
  status: TaskStatus;
  retries: number;
  verified: boolean;
  failStreak: number;
  lastFailureHash: string;
  verify: string[];
  depends: string[];
  files: string[];
  note: string;
  /** Red-first: engine_red must see the verify FAIL before the task's non-test files change. */
  red: boolean;
  /** Paths inside `files` that hold the task's tests: the only files allowed to change before engine_red. */
  tests: string[];
  /** Declared check-free (docs/config): the only kind of task that may have no verify commands. */
  trivial: boolean;
  /** engine_red evidence (time + failing output excerpt); empty until recorded. */
  redProof: string;
  /** Acceptance criteria the red-first tests are audited against (required for new red tasks). */
  spec: string;
  /** Fingerprint of the test files the reviewer last judged; the red is bound to these bytes. */
  auditHash: string;
  /** The test files that fingerprint covers (files a test run creates later are not part of it). */
  auditFiles: string[];
  /** Last fairness verdict ("fair …" / "unfair …"); empty until audited. */
  auditNote: string;
  /** Repo-relative worktree the task runs in (engine_ready worktree:true); empty = main checkout. */
  worktree: string;
  branch: string;
  /** Fingerprint of the last diff the merge reviewer FAILED: the same diff is not re-reviewed. */
  failedReviewHash: string;
}
interface EngineSlice {
  id: string;
  title: string;
  tasks: EngineTask[];
}
interface Criterion {
  text: string;
  /** Empty = incomplete: nothing observed yet, so it cannot count as met. */
  evidence: string;
  at: string;
}
interface EngineState {
  goal: string;
  createdAt: string;
  updatedAt: string;
  maxRetries: number;
  slices: EngineSlice[];
  criteria: Criterion[];
}

const STATE_REL = ".cache/ckit/engine/state.json";
const WT_REL = ".cache/ckit/engine/wt";
const MAX_OUTPUT = 2000;
/** Prompt budget for what the reviewer reads (diff or test files). */
const MAX_REVIEW_INPUT = 60_000;
const REVIEW_TIMEOUT_MS = 240_000;
const SHOT_EXT = /\.(png|jpe?g|webp|gif)$/i;

export default function (pi: ExtensionAPI) {
  const { z } = pi.zod;
  pi.setLabel("ckit engine (gsd-pi-style, on omp core)");

  const taskSchema = z.object({
    id: z.string(),
    key: z.string().default(""),
    title: z.string(),
    status: z.enum(["pending", "in_progress", "done", "blocked", "dropped"]),
    retries: z.number(),
    verified: z.boolean().default(false),
    failStreak: z.number().default(0),
    lastFailureHash: z.string().default(""),
    verify: z.array(z.string()),
    depends: z.array(z.string()).default(() => []),
    files: z.array(z.string()).default(() => []),
    note: z.string(),
    red: z.boolean().default(false),
    tests: z.array(z.string()).default(() => []),
    // Plans saved before this field existed never declared it: keep their old behaviour.
    trivial: z.boolean().default(true),
    redProof: z.string().default(""),
    // Older plans have no spec: their red tasks keep running without the fairness audit.
    spec: z.string().default(""),
    auditHash: z.string().default(""),
    auditFiles: z.array(z.string()).default(() => []),
    auditNote: z.string().default(""),
    worktree: z.string().default(""),
    branch: z.string().default(""),
    failedReviewHash: z.string().default(""),
  });
  const stateSchema = z.object({
    goal: z.string(),
    createdAt: z.string(),
    updatedAt: z.string(),
    maxRetries: z.number(),
    slices: z.array(z.object({ id: z.string(), title: z.string(), tasks: z.array(taskSchema) })),
    criteria: z.array(z.object({ text: z.string(), evidence: z.string(), at: z.string() })).default(() => []),
  });
  /** One task as engine_plan / engine_unblock split accept it. */
  const planTaskSchema = z.object({
    title: z.string(),
    key: z.string().optional(),
    verify: z.array(z.string()).default(() => []),
    depends: z.array(z.string()).default(() => []),
    files: z.array(z.string()).default(() => []),
    red: z.boolean().default(false),
    tests: z.array(z.string()).default(() => []),
    spec: z.string().default(""),
    trivial: z.boolean().default(false),
  });
  type PlanTask = { title: string; key?: string; verify: string[]; depends: string[]; files: string[]; red: boolean; tests: string[]; spec: string; trivial: boolean };

  /** The main checkout. A worker whose cwd is a task worktree still shares the one plan. */
  function root(): string {
    const cwd = process.cwd();
    const i = cwd.indexOf(`/${WT_REL}/`);
    return i >= 0 ? cwd.slice(0, i) : cwd;
  }

  /** Where a task's files live: its worktree, else the main checkout. */
  function taskDir(t: EngineTask): string {
    return t.worktree ? join(root(), t.worktree) : root();
  }

  function load(): EngineState | null {
    const p = join(root(), STATE_REL);
    if (!existsSync(p)) return null;
    try {
      const parsed = stateSchema.safeParse(JSON.parse(readFileSync(p, "utf8")));
      return parsed.success ? parsed.data : null;
    } catch {
      return null;
    }
  }

  function save(state: EngineState): void {
    state.updatedAt = new Date().toISOString();
    mkdirSync(join(root(), ".cache/ckit/engine"), { recursive: true });
    writeFileSync(join(root(), STATE_REL), JSON.stringify(state, null, 2));
  }

  /** `total` leaves dropped tasks out: they are closed without being done. */
  function counts(state: EngineState): { total: number; done: number; blocked: number; dropped: number } {
    const c = { total: 0, done: 0, blocked: 0, dropped: 0 };
    for (const t of allTasks(state)) {
      if (t.status === "dropped") {
        c.dropped += 1;
        continue;
      }
      c.total += 1;
      if (t.status === "done") c.done += 1;
      if (t.status === "blocked") c.blocked += 1;
    }
    return c;
  }

  function allTasks(state: EngineState): EngineTask[] {
    return state.slices.flatMap((s) => s.tasks);
  }

  function byKey(state: EngineState): Map<string, EngineTask> {
    return new Map(allTasks(state).map((t) => [t.key || t.id, t]));
  }

  function findTask(state: EngineState, id: string): EngineTask | undefined {
    return allTasks(state).find((t) => t.id === id || t.key === id);
  }

  /** A dropped dependency no longer holds anything back. */
  function depsDone(state: EngineState, task: EngineTask): boolean {
    const keys = byKey(state);
    return task.depends.every((k) => {
      const s = keys.get(k)?.status;
      return s === "done" || s === "dropped";
    });
  }

  /** Path-prefix overlap: `dir/` collides with every file inside it. */
  function overlaps(a: string[], b: string[]): boolean {
    const norm = (p: string) => p.replace(/^\.\//, "").replace(/\/+$/, "");
    return a.some((x) =>
      b.some((y) => {
        const nx = norm(x);
        const ny = norm(y);
        return nx === ny || nx.startsWith(`${ny}/`) || ny.startsWith(`${nx}/`);
      }),
    );
  }

  /** A pending task whose dependency is blocked can never run: block it too. Returns newly blocked keys. */
  function propagateBlocked(state: EngineState): string[] {
    const keys = byKey(state);
    const newly: string[] = [];
    let changed = true;
    while (changed) {
      changed = false;
      for (const t of allTasks(state)) {
        if (t.status !== "pending") continue;
        const dead = t.depends.find((k) => keys.get(k)?.status === "blocked");
        if (!dead) continue;
        t.status = "blocked";
        t.note = `dependency ${dead} blocked`;
        newly.push(t.key || t.id);
        changed = true;
      }
    }
    return newly;
  }

  /** Undo propagateBlocked below `key` once it is no longer blocked. Returns reopened keys. */
  function reopenDependents(state: EngineState, key: string): string[] {
    const reopened: string[] = [];
    const queue = [key];
    while (queue.length) {
      const k = queue.shift() as string;
      for (const t of allTasks(state)) {
        if (t.status !== "blocked" || t.note !== `dependency ${k} blocked`) continue;
        t.status = "pending";
        t.note = "";
        reopened.push(t.key);
        queue.push(t.key);
      }
    }
    return reopened;
  }

  function findNext(state: EngineState): { slice: EngineSlice; task: EngineTask } | null {
    for (const s of state.slices) {
      for (const t of s.tasks) {
        if (t.status === "in_progress") return { slice: s, task: t };
        if (t.status === "pending" && depsDone(state, t)) return { slice: s, task: t };
      }
    }
    return null;
  }

  /** Reply when no task is left to start: done only once every task is closed and every criterion has evidence. */
  function finished(state: EngineState): string {
    const c = counts(state);
    const waiting = allTasks(state).filter((t) => t.status === "pending" || t.status === "in_progress");
    if (waiting.length) return `NOT DONE — ${waiting.length} task(s) still open but none can start now: ${waiting.map((t) => `${t.key} [${t.status}]`).join(", ")}. Read the errors below and engine_status.`;
    if (c.blocked) return `All tasks resolved but ${c.blocked} BLOCKED — review engine_status, then engine_unblock each root cause.`;
    const open = state.criteria.map((k, i) => ({ k, i })).filter(({ k }) => !k.evidence);
    if (open.length) {
      return `TASKS DONE, goal INCOMPLETE — ${open.length} acceptance criteria have no evidence:\n${open.map(({ k, i }) => `  ${i + 1}. ${k.text}`).join("\n")}\nCall engine_accept for each (a passing task verify, a file:line, or a screenshot). Unobserved is not met.`;
    }
    return "DONE — every task is complete and every acceptance criterion has evidence.";
  }

  /** Git runs directly with omp's PATH: a login shell (`bash -lc`) can resolve a different, broken git (macOS path_helper → /usr/bin/git). */
  function git(args: string[], cwd: string = root()): { ok: boolean; output: string } {
    const r = spawnSync("git", args, { cwd, encoding: "utf8" });
    const raw = `${r.stdout ?? ""}${r.stderr ?? ""}${r.error ? String(r.error) : ""}`.trim();
    return { ok: r.status === 0, output: raw.length > MAX_OUTPUT ? `${raw.slice(0, MAX_OUTPUT)}\n…[truncated]` : raw };
  }

  /** Returns an error message for unknown keys, duplicate keys, or a dependency cycle. */
  function validateGraph(tasks: EngineTask[]): string | null {
    const keys = new Map<string, EngineTask>();
    for (const t of tasks) {
      if (keys.has(t.key)) return `duplicate task key ${t.key}`;
      keys.set(t.key, t);
    }
    for (const t of tasks) {
      const missing = t.depends.find((k) => !keys.has(k));
      if (missing) return `task ${t.key} depends on unknown key ${missing}`;
    }
    const mark = new Map<string, 1 | 2>();
    const visit = (k: string, path: string[]): string | null => {
      if (mark.get(k) === 2) return null;
      if (mark.get(k) === 1) return `dependency cycle: ${[...path, k].join(" -> ")}`;
      mark.set(k, 1);
      for (const d of keys.get(k)?.depends ?? []) {
        const err = visit(d, [...path, k]);
        if (err) return err;
      }
      mark.set(k, 2);
      return null;
    };
    for (const k of keys.keys()) {
      const err = visit(k, []);
      if (err) return err;
    }
    return null;
  }

  /** p is one of roots or sits under one of them. */
  function inside(p: string, roots: string[]): boolean {
    const norm = (x: string) => x.replace(/^\.\//, "").replace(/\/+$/, "");
    const np = norm(p);
    return roots.some((r) => np === norm(r) || np.startsWith(`${norm(r)}/`));
  }

  /** Gate rules for newly planned tasks; returns the first violation. */
  function validateTasks(tasks: EngineTask[]): string | null {
    for (const t of tasks) {
      if (!t.verify.length && !t.trivial) {
        return `task ${t.key} has no verify commands — give it the real lint/test that proves it, or mark it trivial:true (docs/config only)`;
      }
      if (!t.red) continue;
      if (!t.verify.length) return `task ${t.key} is red:true but has no verify commands that could fail`;
      if (t.tests.some((p) => !inside(p, t.files))) return `task ${t.key}: every tests path must be inside its files`;
      if (!t.files.some((f) => !inside(f, t.tests))) return `task ${t.key} is red:true but owns no non-test file — red-first proves a code change`;
      if (!t.spec.trim()) return `task ${t.key} is red:true but has no spec — state the acceptance criteria the tests must check; a reviewer audits the tests against it before engine_red`;
    }
    return null;
  }

  function newTask(id: string, t: PlanTask): EngineTask {
    return { id, key: t.key || id, title: t.title, status: "pending", retries: 0, verify: t.verify, depends: t.depends, files: t.files, note: "", verified: false, failStreak: 0, lastFailureHash: "", red: t.red, tests: t.tests, trivial: t.trivial, redProof: "", spec: t.spec, auditHash: "", auditFiles: [], auditNote: "", worktree: "", branch: "", failedReviewHash: "" };
  }

  /** Non-login shell with omp's own env: a login shell re-sources /etc/profile, whose macOS path_helper puts /usr/bin first (wrong git/php). */
  function run(cmd: string, cwd: string = root()): { ok: boolean; output: string; code: number | null } {
    const r = spawnSync("bash", ["-c", cmd], { cwd, env: process.env, encoding: "utf8" });
    const raw = `${r.stdout ?? ""}${r.stderr ?? ""}`.trim();
    const output = raw.length > MAX_OUTPUT ? `${raw.slice(0, MAX_OUTPUT)}\n…[truncated]` : raw;
    return { ok: r.status === 0, output, code: r.status };
  }

  /** FNV-1a 32-bit fingerprint of a failure output — lets the no-progress
   * detector spot byte-identical consecutive failures (doom-loop guard). */
  function fnv1a(s: string): string {
    let h = 0x811c9dc5;
    for (let i = 0; i < s.length; i++) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 0x01000193) >>> 0;
    }
    return h.toString(16);
  }

  function text(s: string) {
    return { content: [{ type: "text" as const, text: s }] };
  }

  /**
   * The task's test files with their contents and one fingerprint: `only` (the
   * audited set; a deleted file still changes the fingerprint), else every
   * tracked or new, non-ignored file under its tests paths.
   */
  function readTests(t: EngineTask, only?: string[]): { files: { path: string; body: string }[]; hash: string } {
    const dir = taskDir(t);
    const paths = only ?? testPaths(t);
    const files = [...paths].sort().map((path) => {
      try {
        return { path, body: readFileSync(join(dir, path), "utf8") };
      } catch {
        return { path, body: "\0missing" };
      }
    });
    return { files, hash: fnv1a(files.map((f) => `${f.path}\0${f.body}`).join("\0")) };
  }

  function testPaths(t: EngineTask): string[] {
    const r = spawnSync("git", ["ls-files", "-co", "--exclude-standard", "--", ...t.tests], { cwd: taskDir(t), encoding: "utf8" });
    return (r.status === 0 ? r.stdout : "").split("\n").filter(Boolean);
  }

  /**
   * The tests changed since the fairness verdict (so the red no longer covers them): an audited
   * file edited or deleted, or a new file of the same kind (e.g. a conftest.py that skips them).
   * New files of other kinds (snapshots, fixtures a test run writes) do not count.
   */
  function testsChangedSinceAudit(t: EngineTask): boolean {
    if (!(t.red && t.spec && t.auditHash)) return false;
    // Plans audited before auditFiles existed fingerprinted every file under the tests paths.
    if (readTests(t, t.auditFiles.length ? t.auditFiles : undefined).hash !== t.auditHash) return true;
    const kinds = new Set(t.auditFiles.map((p) => extname(p)).filter(Boolean));
    return testPaths(t).some((p) => !t.auditFiles.includes(p) && kinds.has(extname(p)));
  }

  /** Paths staged in the main checkout, uncapped and unquoted (NUL-separated). */
  function stagedPaths(filter?: string): string[] {
    const args = ["diff", "--cached", "--no-renames", "--name-only", "-z", ...(filter ? [`--diff-filter=${filter}`] : [])];
    const r = spawnSync("git", args, { cwd: root(), encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
    return (r.stdout ?? "").split("\0").filter(Boolean);
  }

  /**
   * Undo a squash-merge staged onto a clean index. `reset --merge` first; when it refuses (a
   * verify command or hook rewrote a merged file): unstage, restore the paths the merge touched
   * to HEAD, delete the ones it added. Returns "" when the checkout is clean again, else what is left.
   */
  function undoSquash(touched: string[], added: string[]): string {
    if (git(["reset", "--merge"]).ok) return "";
    git(["reset", "-q"]);
    const errors: string[] = [];
    const addedSet = new Set(added);
    const restore = touched.filter((p) => !addedSet.has(p));
    if (restore.length) {
      // Literal: a name like app/[id].tsx must not glob-match (and reset) app/i.tsx.
      const r = spawnSync("git", ["checkout", "HEAD", "--pathspec-from-file=-", "--pathspec-file-nul"], { cwd: root(), encoding: "utf8", input: restore.join("\0"), env: { ...process.env, GIT_LITERAL_PATHSPECS: "1" } });
      if (r.status !== 0) errors.push(`${r.stderr ?? ""}`.trim());
    }
    for (const p of added) {
      try {
        rmSync(join(root(), p), { force: true });
      } catch (e) {
        errors.push(String(e));
      }
    }
    const st = spawnSync("git", ["status", "--porcelain", "-z", "--untracked-files=all"], { cwd: root(), encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
    const touchedSet = new Set(touched);
    const left = (st.stdout ?? "").split("\0").filter((e) => touchedSet.has(e.slice(3)));
    if (!left.length && !errors.length) return "";
    return cap(`MAIN CHECKOUT LEFT DIRTY — clean it before any other advance (${left.length} path(s)):\n${left.slice(0, 20).join("\n")}\n${errors.join("\n")}`.trim());
  }

  /**
   * One independent judgement: a fresh `omp -p --no-tools` that never saw this
   * session. It answers with one JSON line; anything else is an error, never a pass.
   */
  function askReviewer(prompt: string, verdicts: string[], signal?: AbortSignal): Promise<{ verdict: string; reasons: string[] } | { error: string }> {
    const model = process.env.CKIT_ENGINE_REVIEW_MODEL;
    const args = ["-p", "--no-session", "--no-tools", "--thinking", "low", ...(model ? ["--model", model] : []), prompt];
    return new Promise((resolve) => {
      let out = "";
      let settled = false;
      const done = (v: { verdict: string; reasons: string[] } | { error: string }) => {
        if (settled) return;
        settled = true;
        clearTimeout(timer);
        resolve(v);
      };
      // stdin closed: `omp -p` reads stdin until EOF, so an open pipe would hang it until the timeout.
      const child = spawn("omp", args, { cwd: root(), env: process.env, signal, stdio: ["ignore", "pipe", "pipe"] });
      const timer = setTimeout(() => {
        child.kill("SIGKILL");
        done({ error: `reviewer timed out after ${REVIEW_TIMEOUT_MS / 1000}s` });
      }, REVIEW_TIMEOUT_MS);
      child.stdout.on("data", (d) => (out += d));
      child.stderr.on("data", (d) => (out += d));
      child.on("error", (e) => done({ error: `reviewer could not start (omp on PATH?): ${e.message}` }));
      child.on("close", () => {
        const line = out
          .split("\n")
          .map((l) => l.trim())
          .reverse()
          .find((l) => l.startsWith("{") && l.includes('"verdict"'));
        try {
          const j = JSON.parse(line ?? "");
          const verdict = String(j.verdict).toLowerCase();
          if (!verdicts.includes(verdict)) throw new Error("bad verdict");
          const reasons = Array.isArray(j.reasons) ? j.reasons.map(String) : [];
          done({ verdict, reasons });
        } catch {
          done({ error: `reviewer gave no {"verdict":…} line:\n${out.trim().slice(-800)}` });
        }
      });
    });
  }

  function cap(s: string): string {
    return s.length > MAX_REVIEW_INPUT ? `${s.slice(0, MAX_REVIEW_INPUT)}\n…[truncated]` : s;
  }

  /**
   * Red-first fairness gate: the reviewer judges the tests against the spec
   * before any code exists. Returns an error text, or null once the current test
   * bytes carry a "fair" verdict (cached by fingerprint, so re-calls are free).
   */
  async function auditTests(t: EngineTask, signal?: AbortSignal): Promise<string | null> {
    const { files, hash } = readTests(t);
    if (!files.length) return `REFUSED ${t.id}: no test files found under ${t.tests.join(", ") || "(no tests paths)"} — write the tests first.`;
    if (t.auditHash === hash && t.auditNote.startsWith("fair")) return null;
    if (t.auditHash === hash && t.auditNote.startsWith("unfair")) return `REFUSED ${t.id}: these exact tests were already judged UNFAIR (${t.auditNote}) — change them first; asking again does not re-roll the verdict.`;
    const prompt = [
      "You audit tests written BEFORE the implementation exists. Decide whether they are FAIR: every reasonable implementation that meets the spec must pass, and a wrong one must fail.",
      "UNFAIR when a test depends on names, import paths, file layout, call counts, call order, exact message wording, or default values the spec does not state; asserts behaviour the spec does not ask for; or cannot fail for a wrong implementation (vacuous). Judge only fairness, not style.",
      `Task: ${t.title}`,
      `Spec: ${t.spec}`,
      "Tests:",
      cap(files.map((f) => `--- ${f.path} ---\n${f.body}`).join("\n")),
      'Reply with ONE line of JSON and nothing else: {"verdict":"fair"|"unfair","reasons":["<test>: <why>"]}',
    ].join("\n\n");
    const r = await askReviewer(prompt, ["fair", "unfair"], signal);
    if ("error" in r) return `REFUSED ${t.id}: the fairness audit could not run — ${r.error}`;
    const state = load();
    const live = state && findTask(state, t.id);
    if (!state || !live) return `REFUSED ${t.id}: plan changed during the audit — call again.`;
    live.auditHash = hash;
    live.auditFiles = files.map((f) => f.path);
    live.auditNote = `${r.verdict} ${new Date().toISOString()}${r.reasons.length ? `: ${r.reasons.join("; ")}` : ""}`;
    save(state);
    if (r.verdict === "fair") return null;
    return `REFUSED ${t.id}: the reviewer judged the tests UNFAIR to the spec:\n  - ${r.reasons.join("\n  - ") || "(no reasons given)"}\nRewrite the tests so any implementation meeting the spec passes, then call engine_red again. The spec is the contract: a missing requirement means re-planning the task, not a test that assumes it.`;
  }

  function appendKnowledge(line: string): void {
    const dir = join(root(), "agents");
    mkdirSync(dir, { recursive: true });
    appendFileSync(join(dir, "KNOWLEDGE.md"), `${line}\n`);
  }

  function removeWorktree(t: EngineTask): void {
    if (!t.worktree) return;
    git(["worktree", "remove", t.worktree, "--force"]);
    if (t.branch) git(["branch", "-D", t.branch]);
    t.worktree = "";
    t.branch = "";
  }

  /** Commits whatever the task left in its worktree. Returns an error text or null. */
  function commitWorktree(t: EngineTask, msg: string): string | null {
    const dir = taskDir(t);
    const add = git(["add", "-A"], dir);
    if (!add.ok) return `git add failed in ${t.worktree}:\n${add.output}`;
    if (git(["diff", "--cached", "--quiet"], dir).ok) return null;
    const scan = run("if command -v gitleaks >/dev/null 2>&1; then gitleaks protect --staged --no-banner; fi", dir);
    if (!scan.ok) {
      git(["reset", "-q"], dir);
      return `gitleaks flagged a secret in ${t.worktree} — remove it:\n${scan.output}`;
    }
    const c = git(["commit", "-m", msg], dir);
    return c.ok ? null : `git commit failed in ${t.worktree}:\n${c.output}`;
  }

  pi.registerTool({
    name: "engine_plan",
    label: "Engine: plan",
    description:
      "Create/replace the run-to-done plan: a goal decomposed into slices, each with atomic tasks and verify commands (the project's real lint/test). Optional per task: `key` (e.g. T3, defaults to the task id), `depends` (keys that must be done first) and `files` (paths/dirs the task owns) — engine_ready runs independent tasks in parallel from these. A task with no verify commands is REJECTED unless `trivial: true` (docs/config only). `red: true` makes the task red-first: `tests` (paths inside `files`) may change first, then engine_red must see the verify FAIL before any other file of the task changes; engine_verify refuses the task until that red is recorded. A red task needs `spec` (its acceptance criteria): engine_red first has an independent reviewer check the tests are fair to any implementation of that spec. `criteria` = the goal's acceptance criteria; the plan is not DONE until engine_accept attaches evidence to each. `append: true` keeps the current plan (goal, tasks, statuses, criteria) and adds the slices and criteria after it, so new tasks may depend on existing keys. Rejects duplicate/unknown keys and dependency cycles. Persists to .cache/ckit/engine/state.json.",
    parameters: z.object({
      goal: z.string(),
      maxRetries: z.number().int().min(0).max(10).default(3),
      append: z.boolean().default(false),
      criteria: z.array(z.string().min(1)).default(() => []),
      slices: z
        .array(
          z.object({
            title: z.string(),
            tasks: z.array(planTaskSchema),
          }),
        )
        .min(1),
    }),
    async execute(_id, params) {
      const now = new Date().toISOString();
      const base = params.append ? load() : null;
      let si = base?.slices.length ?? 0;
      const added: EngineSlice[] = params.slices.map((s) => {
        si += 1;
        let ti = 0;
        return {
          id: `s${si}`,
          title: s.title,
          tasks: s.tasks.map((t) => {
            ti += 1;
            return newTask(`s${si}.t${ti}`, t);
          }),
        };
      });
      const criteria = params.criteria.map((text) => ({ text, evidence: "", at: "" }));
      const state: EngineState = base
        ? { ...base, updatedAt: now, slices: [...base.slices, ...added], criteria: [...base.criteria, ...criteria] }
        : { goal: params.goal, createdAt: now, updatedAt: now, maxRetries: params.maxRetries, slices: added, criteria };
      const invalid = validateGraph(allTasks(state)) ?? validateTasks(added.flatMap((s) => s.tasks));
      if (invalid) return text(`Plan REJECTED: ${invalid}. Nothing saved.`);
      save(state);
      const c = counts(state);
      const crit = state.criteria.length ? `, ${state.criteria.length} acceptance criteria` : "";
      const head = base
        ? `Appended ${added.length} slice(s) "${params.goal}" to plan "${state.goal}" — ${c.done}/${c.total} tasks done${crit}.`
        : `Plan saved: "${params.goal}" — ${state.slices.length} slices, ${c.total} tasks${crit}.`;
      return text(`${head} Call engine_ready (parallel) or engine_next (one at a time) to start.`);
    },
  });

  pi.registerTool({
    name: "engine_status",
    label: "Engine: status",
    description: "Report the current plan: per-slice task statuses, overall progress (done/total, blocked, dropped) and each acceptance criterion with its evidence or `incomplete`.",
    parameters: z.object({}),
    async execute() {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const c = counts(state);
      const dropped = c.dropped ? `, ${c.dropped} dropped` : "";
      const lines = [`Goal: ${state.goal}`, `Progress: ${c.done}/${c.total} done, ${c.blocked} blocked${dropped}`, ""];
      for (const s of state.slices) {
        lines.push(`# ${s.id} ${s.title}`);
        for (const t of s.tasks) {
          const key = t.key && t.key !== t.id ? ` ${t.key}` : "";
          const deps = t.depends.length ? ` (depends: ${t.depends.join(",")})` : "";
          const retries = t.retries ? ` (retries:${t.retries})` : "";
          const note = (t.status === "blocked" || t.status === "dropped") && t.note ? ` — ${t.note}` : "";
          const files = t.status === "in_progress" && t.files.length ? ` [files: ${t.files.join(", ")}]` : "";
          const wt = t.status === "in_progress" && t.worktree ? ` [worktree: ${t.worktree}]` : "";
          const red = t.red ? (t.redProof ? " (red ✓)" : " (red-first: engine_red pending)") : "";
          const audit = t.red && t.auditNote ? ` (audit: ${t.auditNote.split(" ")[0]})` : "";
          lines.push(`  [${t.status}] ${t.id}${key} ${t.title}${deps}${retries}${files}${wt}${red}${audit}${note}`);
        }
      }
      if (state.criteria.length) {
        lines.push("", "Acceptance criteria:");
        state.criteria.forEach((k, i) => lines.push(`  ${i + 1}. ${k.evidence ? "✓" : "incomplete"} ${k.text}${k.evidence ? ` — ${k.evidence}` : ""}`));
      }
      return text(lines.join("\n"));
    },
  });

  pi.registerTool({
    name: "engine_next",
    label: "Engine: next task",
    description: "Return the next pending task whose dependencies are done (with its slice) and mark it in_progress. When no task is left it reports DONE only if every acceptance criterion has evidence.",
    parameters: z.object({}),
    async execute() {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      propagateBlocked(state);
      const next = findNext(state);
      if (!next) {
        save(state);
        return text(finished(state));
      }
      next.task.status = "in_progress";
      save(state);
      const verify = next.task.verify.length ? `\nVerify with: ${next.task.verify.join(" && ")}` : "";
      const steps = next.task.red
        ? `RED-FIRST: write/adjust the test in ${next.task.tests.join(", ") || "the task's tests"} against the spec, call engine_red (fairness audit + red) BEFORE editing any other file of the task, then implement, engine_verify, engine_advance.${next.task.spec ? `\nSpec: ${next.task.spec}` : ""}`
        : "Implement it, then call engine_verify, then engine_advance.";
      return text(`NEXT ${next.task.id} (slice ${next.slice.title}): ${next.task.title}${verify}\n${steps}`);
    },
  });

  pi.registerTool({
    name: "engine_ready",
    label: "Engine: ready tasks (parallel)",
    description:
      "Return EVERY pending task whose dependencies are done and whose files do not overlap (path prefix) with an in-progress task or with each other, up to `limit`, and mark them in_progress — dispatch them in parallel. `worktree: true` gives each task its own git worktree + branch off HEAD: the worker edits only there, engine_verify runs there, and engine_advance commits it, has an independent reviewer judge the diff, and squash-merges only on PASS. Also lists tasks still in progress. When no task is left it reports DONE only if every acceptance criterion has evidence.",
    parameters: z.object({ limit: z.number().int().min(1).max(32).optional(), worktree: z.boolean().default(false) }),
    async execute(_id, params) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const newlyBlocked = propagateBlocked(state);
      const tasks = allTasks(state);
      const running = tasks.filter((t) => t.status === "in_progress");
      const busy = running.flatMap((t) => t.files);
      const limit = (params.limit ?? Number.MAX_SAFE_INTEGER) - running.length;
      const ready: EngineTask[] = [];
      const errors: string[] = [];
      for (const t of tasks) {
        if (ready.length >= limit) break;
        if (t.status !== "pending" || !depsDone(state, t) || overlaps(t.files, busy)) continue;
        if (params.worktree && !(t.worktree && existsSync(taskDir(t)))) {
          // Unique per open: a re-plan reusing the key, or two keys that sanitize alike, never meet a leftover branch.
          const slug = `${t.key.replace(/[^a-zA-Z0-9._-]/g, "-")}-${Date.now().toString(36)}`;
          const rel = join(WT_REL, `task-${slug}`);
          const branch = `ckit/task-${slug}`;
          mkdirSync(join(root(), WT_REL), { recursive: true });
          const r = git(["worktree", "add", rel, "-b", branch]);
          if (!r.ok) {
            errors.push(`${t.key}: worktree add failed — left pending:\n${r.output}`);
            continue;
          }
          t.worktree = rel;
          t.branch = branch;
        }
        t.status = "in_progress";
        busy.push(...t.files);
        ready.push(t);
      }
      save(state);

      const tail = [
        newlyBlocked.length ? `Blocked because a dependency is blocked: ${newlyBlocked.join(", ")}` : "",
        ...errors,
      ].filter(Boolean);
      const blockedNote = tail.length ? `\n${tail.join("\n")}` : "";
      if (!ready.length && !running.length) return text(finished(state) + blockedNote);
      const fmt = (t: EngineTask) => {
        const files = t.files.length ? `\n    files: ${t.files.join(", ")}` : "";
        const verify = t.verify.length ? `\n    verify: ${t.verify.join(" && ")}` : "";
        const red = t.red ? `\n    RED-FIRST: tests ${t.tests.join(", ") || "—"} first → engine_red (fairness audit + red) → then the other files${t.spec ? `\n    spec: ${t.spec}` : ""}` : "";
        const wt = t.worktree ? `\n    worktree: ${taskDir(t)} (edit ONLY there; based on HEAD, uncommitted main-tree changes are not in it)` : "";
        return `  ${t.id} ${t.key} ${t.title}${files}${verify}${red}${wt}`;
      };
      const lines = [`READY ${ready.length} (now in_progress — dispatch in parallel):`, ...ready.map(fmt)];
      if (running.length) lines.push(`STILL IN PROGRESS ${running.length}:`, ...running.map((t) => `  ${t.id} ${t.key} ${t.title}${t.files.length ? ` [files: ${t.files.join(", ")}]` : ""}${t.worktree ? ` [worktree: ${t.worktree}]` : ""}`));
      lines.push("Per task: (red-first: engine_red before touching non-test files) → engine_verify → engine_advance {files:[changed files]} (worktree tasks: engine_advance reviews + merges, no files needed); call engine_ready again after each advance.");
      return text(lines.join("\n") + blockedNote);
    },
  });

  pi.registerTool({
    name: "engine_red",
    label: "Engine: red-first proof",
    description:
      "For a red:true task: run its verify commands BEFORE the code change and require them to FAIL. If the task has a spec, an independent reviewer (fresh omp, no tools, never saw this session) first audits the tests against it and REFUSES unfair ones (tied to unstated names, paths, call counts, message wording, defaults, or vacuous). Refused when any of the task's non-test files already changed (git status), when the commands already pass (the test does not capture the behaviour), or when every failure is exit 126/127 (command missing, not a real red). Records the failing output as evidence; engine_verify refuses the task until this is recorded, and later edits to the audited tests cost a verify attempt.",
    parameters: z.object({ taskId: z.string() }),
    async execute(_id, params, signal) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const target = findTask(state, params.taskId);
      if (!target) return text(`No task ${params.taskId}.`);
      if (!target.red) return text(`Task ${target.id} is not red-first (red:false) — no red proof needed.`);
      if (target.redProof) return text(`RED already recorded for ${target.id}:\n${target.redProof}`);
      if (target.status !== "pending" && target.status !== "in_progress") return text(`Task ${target.id} is ${target.status}.`);
      const dir = taskDir(target);

      const st = git(["status", "--porcelain", "--untracked-files=all", "--", ...target.files], dir);
      if (!st.ok) return text(`REFUSED ${target.id}: git status failed:\n${st.output}`);
      // git() trims the output, so the first line may have lost its leading status space: strip "XY " by pattern.
      const changed = st.output
        .split("\n")
        .filter(Boolean)
        .map((l) => l.replace(/^[ MADRCTU?!]{1,2}\s+/, "").replace(/^.* -> /, "").replace(/^"|"$/g, ""))
        .filter((p) => !inside(p, target.tests));
      if (changed.length) {
        return text(`REFUSED ${target.id}: non-test files already changed — the red must run before the code change:\n  ${changed.join("\n  ")}\nRevert/stash them (only ${target.tests.join(", ") || "the tests"} may change first), then call engine_red again.`);
      }

      if (target.spec) {
        const refused = await auditTests(target, signal);
        if (refused) return text(refused);
      }

      const results = target.verify.map((cmd) => ({ cmd, ...run(cmd, dir) }));
      const failed = results.filter((r) => !r.ok);
      if (!failed.length) {
        return text(`REFUSED ${target.id}: the verify commands already PASS before the change, so they do not capture the required behaviour. Strengthen the test in ${target.tests.join(", ") || "the task's tests"}, then call engine_red again.`);
      }
      if (failed.every((r) => r.code === 126 || r.code === 127)) {
        return text(`REFUSED ${target.id}: every failure is exit 126/127 (command not found / not executable) — that is a broken command, not a red test:\n\n${failed.map((r) => `$ ${r.cmd}\n${r.output}`).join("\n\n")}`);
      }
      const excerpt = failed.map((r) => `$ ${r.cmd} (exit ${r.code})\n${r.output.slice(0, 600)}`).join("\n\n");
      // The audit may have rewritten state: reload so its verdict is kept.
      const fresh = load() ?? state;
      const live = findTask(fresh, target.id) ?? target;
      live.redProof = `${new Date().toISOString()}\n${excerpt}`;
      save(fresh);
      const audited = target.spec ? "Tests audited FAIR. " : "";
      return text(`${audited}RED recorded for ${target.id} — check the failure is the expected assertion, not a syntax/setup error:\n\n${excerpt}\n\nNow implement, then engine_verify.`);
    },
  });

  pi.registerTool({
    name: "engine_verify",
    label: "Engine: verify (auto-retry gate)",
    description:
      "Run the task's planned verify commands (in its worktree, if it has one) plus any extra `commands` (extras ADD to the plan's commands, never replace them). All must pass to advance. A red-first task is refused until engine_red recorded its red; editing its audited tests after the red counts as a failed attempt. On failure the retry counter increments; once it reaches maxRetries the task is BLOCKED. Identical consecutive failures trip the no-progress guard: 2x warns, 3x BLOCKS early (doom-loop). The gate is code-enforced — engine_advance refuses an unverified task.",
    parameters: z.object({ taskId: z.string(), commands: z.array(z.string()).optional() }),
    async execute(_id, params) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const target = findTask(state, params.taskId);
      if (!target) return text(`No task ${params.taskId}.`);
      if (target.status !== "pending" && target.status !== "in_progress") return text(`Task ${target.id} is ${target.status}.`);
      if (target.red && !target.redProof) {
        return text(`REFUSED ${target.id}: red-first task with no recorded red — call engine_red (before changing its non-test files) first.`);
      }
      const cmds = [...target.verify, ...(params.commands ?? []).filter((c) => !target.verify.includes(c))];
      if (!cmds.length) return text(`Task ${target.id} is trivial (no verify commands) — call engine_advance directly.`);
      const dir = taskDir(target);

      const failures: string[] = [];
      if (testsChangedSinceAudit(target)) {
        failures.push(`$ (red-first guard) the tests in ${target.tests.join(", ")} changed after the fairness audit + red — the red no longer covers them. Revert the test edits (git diff -- ${target.tests.join(" ")}); if the test itself was wrong this task needs engine_unblock once blocked.`);
      } else {
        for (const cmd of cmds) {
          const r = run(cmd, dir);
          if (!r.ok) failures.push(`$ ${cmd}\n${r.output}`);
        }
      }
      if (!failures.length) {
        target.verified = true;
        target.failStreak = 0;
        target.lastFailureHash = "";
        save(state);
        return text(`VERIFIED ${target.id}: all ${cmds.length} checks passed. Call engine_advance.`);
      }

      target.verified = false;
      target.retries += 1;
      const hash = fnv1a(failures.join("\n\n"));
      target.failStreak = hash === target.lastFailureHash ? target.failStreak + 1 : 1;
      target.lastFailureHash = hash;
      if (target.failStreak >= 3 || target.retries >= state.maxRetries) {
        target.status = "blocked";
        const doom = target.failStreak >= 3;
        target.note = doom ? `no progress — ${target.failStreak} identical failures` : `blocked after ${target.retries} failed verifies`;
        const dependents = propagateBlocked(state);
        save(state);
        const also = dependents.length ? `\nAlso blocked (depend on it): ${dependents.join(", ")}.` : "";
        const head = doom
          ? `BLOCKED ${target.id}: NO PROGRESS — ${target.failStreak} consecutive identical failures (doom-loop guard). Repeating the same attempt cannot pass.`
          : `BLOCKED ${target.id} after ${target.retries} attempts (maxRetries=${state.maxRetries}).`;
        return text(`${head} Diagnose the failure below, then engine_unblock {taskId:"${target.id}", diagnosis, action: retry|split|drop} — it logs the failure to agents/KNOWLEDGE.md.${also}\n\n${failures.join("\n\n")}`);
      }
      save(state);
      const loop = target.failStreak === 2 ? " WARNING: same failure twice in a row — a third identical failure BLOCKS the task (no-progress guard). Change the approach, don't retry the same fix." : "";
      return text(`FAILED ${target.id} (attempt ${target.retries}/${state.maxRetries}).${loop} Fix the cause, then call engine_verify again:\n\n${failures.join("\n\n")}`);
    },
  });

  pi.registerTool({
    name: "engine_unblock",
    label: "Engine: unblock (diagnose + decide)",
    description:
      "Resolve a BLOCKED task with a written diagnosis — a block is evidence, not a dead end. The diagnosis is appended to agents/KNOWLEDGE.md as a `failure:` entry. action retry: back to pending with fresh retry counters (if its audited tests changed after the red, the red and audit are cleared so the red-first cycle reruns). action split: replace it with `tasks` (same shape as engine_plan tasks) in its slice; the original is dropped and everything that depended on it now depends on the new tasks. action drop: close it without doing it; dependents run without it. Tasks blocked only because a dependency is blocked are reopened automatically.",
    parameters: z.object({
      taskId: z.string(),
      diagnosis: z.string().min(15),
      action: z.enum(["retry", "split", "drop"]),
      tasks: z.array(planTaskSchema).optional(),
    }),
    async execute(_id, params) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const target = findTask(state, params.taskId);
      if (!target) return text(`No task ${params.taskId}.`);
      if (target.status !== "blocked") return text(`REFUSED ${target.id}: it is ${target.status}, not blocked.`);
      const root_cause = target.note.match(/^dependency (.+) blocked$/);
      if (root_cause) return text(`REFUSED ${target.id}: it is blocked only because ${root_cause[1]} is — unblock ${root_cause[1]}; this one reopens with it.`);

      const why = target.note;
      let outcome: string;
      if (params.action === "retry") {
        target.status = "pending";
        target.retries = 0;
        target.failStreak = 0;
        target.lastFailureHash = "";
        target.verified = false;
        target.note = "";
        if (testsChangedSinceAudit(target)) {
          target.redProof = "";
          target.auditHash = "";
          target.auditFiles = [];
          target.auditNote = "";
          outcome = `${target.id} back to pending. Its tests changed after the red, so the red-first cycle reruns: stash the code change, engine_red (re-audit + red), then re-apply.`;
        } else {
          outcome = `${target.id} back to pending with fresh retry counters.`;
        }
      } else if (params.action === "split") {
        const parts = params.tasks ?? [];
        if (!parts.length) return text("REFUSED: action split needs `tasks` (the replacement tasks).");
        const slice = state.slices.find((s) => s.tasks.includes(target));
        if (!slice) return text(`No slice holds ${target.id}.`);
        let n = Math.max(0, ...slice.tasks.map((t) => Number(t.id.split(".t")[1]) || 0));
        const fresh = parts.map((p) => newTask(`${slice.id}.t${++n}`, p));
        const at = slice.tasks.indexOf(target) + 1;
        slice.tasks.splice(at, 0, ...fresh);
        const keys = fresh.map((t) => t.key);
        for (const t of allTasks(state)) {
          if (!fresh.includes(t) && t.depends.includes(target.key)) t.depends.push(...keys.filter((k) => !t.depends.includes(k)));
        }
        target.status = "dropped";
        target.note = `split into ${keys.join(", ")}`;
        const invalid = validateGraph(allTasks(state)) ?? validateTasks(fresh);
        if (invalid) return text(`Split REJECTED: ${invalid}. Nothing saved.`);
        removeWorktree(target);
        outcome = `${target.id} dropped, split into ${keys.join(", ")}; its dependents now wait for them.`;
      } else {
        target.status = "dropped";
        target.note = `dropped: ${params.diagnosis}`;
        removeWorktree(target);
        outcome = `${target.id} dropped; its dependents no longer wait for it.`;
      }
      const reopened = reopenDependents(state, target.key);
      save(state);
      appendKnowledge(`- failure: ${new Date().toISOString().slice(0, 10)} engine task ${target.key} "${target.title}" blocked (${why}). Diagnosis: ${params.diagnosis.replace(/\s+/g, " ")} → ${params.action}.`);
      const re = reopened.length ? `\nReopened (were blocked behind it): ${reopened.join(", ")}.` : "";
      return text(`${outcome}${re}\nLogged the diagnosis to agents/KNOWLEDGE.md. Call engine_ready or engine_next.`);
    },
  });

  pi.registerTool({
    name: "engine_advance",
    label: "Engine: advance",
    description:
      "Mark a verified task done and optionally commit the change (a gitleaks gate blocks the commit if a secret is staged, matching the ckit pre-commit hook). REFUSES a task whose verify commands never passed — the agent's own say-so is not a stop signal. With `files`, stages and commits ONLY those paths (required while other tasks are in progress — `git add -A` would sweep their unfinished work). A task with its own worktree is always committed there, then an independent reviewer (fresh omp, no tools) judges the diff against the task: FAIL returns the findings and un-verifies the task; PASS squash-merges the branch into the main checkout and removes the worktree. Advances the plan.",
    parameters: z.object({
      taskId: z.string(),
      commit: z.boolean().default(false),
      message: z.string().optional(),
      files: z.array(z.string()).optional(),
    }),
    async execute(_id, params, signal) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const target = findTask(state, params.taskId);
      if (!target) return text(`No task ${params.taskId}.`);
      if (target.status !== "pending" && target.status !== "in_progress") return text(`Task ${target.id} is ${target.status}.`);
      if (target.verify.length && !target.verified) {
        return text(
          `REFUSED ${target.id}: it has ${target.verify.length} verify command(s) but no passing engine_verify run. The gate is code-enforced — call engine_verify {taskId:"${target.id}"} first.`,
        );
      }
      const msg = params.message ?? `feat: ${target.title}`;

      if (target.worktree) {
        const err = commitWorktree(target, msg);
        if (err) return text(`FAILED ${target.id}: ${err}\nTask stays in_progress.`);
        const full = spawnSync("git", ["diff", `HEAD...${target.branch}`], { cwd: root(), encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
        if (full.status !== 0) return text(`FAILED ${target.id}: cannot diff ${target.branch}:\n${(full.stderr ?? "").slice(0, MAX_OUTPUT)}`);
        let merged = "\nNothing to merge (the branch has no changes).";
        if (full.stdout.trim()) {
          // The squash commit takes the whole index and an aborted merge resets it: never touch someone's staged work.
          if (!git(["diff", "--cached", "--quiet"]).ok) return text(`REFUSED ${target.id}: the main checkout has staged changes — commit or unstage them, then call engine_advance again (the squash-merge would sweep them in or reset them).`);
          const diffHash = fnv1a(full.stdout);
          if (diffHash === target.failedReviewHash) return text(`REFUSED ${target.id}: this exact diff already FAILED review — change the code in ${taskDir(target)}, engine_verify, then engine_advance again.`);
          const prompt = [
            "You are a strict code reviewer. Judge this diff against the task. FAIL only for real defects: behaviour wrong or missing versus the task/spec, broken edge cases, tests weakened or deleted, debug leftovers, secrets, changes unrelated to the task. Do not fail on style or naming taste.",
            `Task: ${target.title}`,
            `Spec: ${target.spec || "(none — judge against the task title)"}`,
            `Verify commands (already passing): ${target.verify.join(" && ") || "(none)"}`,
            "Diff:",
            cap(full.stdout),
            'Reply with ONE line of JSON and nothing else: {"verdict":"pass"|"fail","reasons":["<file:line>: <problem>"]}',
          ].join("\n\n");
          const r = await askReviewer(prompt, ["pass", "fail"], signal);
          if ("error" in r) return text(`REFUSED ${target.id}: the merge review could not run — ${r.error}\nTask stays in_progress; call engine_advance again.`);
          if (r.verdict === "fail") {
            const fresh = load() ?? state;
            const live = findTask(fresh, target.id) ?? target;
            live.verified = false;
            live.failedReviewHash = diffHash;
            save(fresh);
            return text(`REVIEW FAILED ${target.id} — not merged:\n  - ${r.reasons.join("\n  - ") || "(no reasons given)"}\nFix it in ${taskDir(target)}, engine_verify, then engine_advance again.`);
          }
          const m = git(["merge", "--squash", target.branch]);
          if (!m.ok) {
            const undo = git(["reset", "--merge"]);
            const undone = undo.ok ? "merge aborted" : `MAIN CHECKOUT LEFT DIRTY (reset --merge failed: ${undo.output}) — clean it before any other advance`;
            return text(`FAILED ${target.id}: squash-merge of ${target.branch} conflicts with the main checkout — ${undone}, task stays in_progress. Rebase the worktree onto HEAD (cd ${taskDir(target)} && git rebase <main branch>), engine_verify, engine_advance again.\n${m.output}`);
          }
          // The index was clean before the merge, so everything staged now is the merge.
          const touched = stagedPaths();
          const added = stagedPaths("A");
          const aborted = () => undoSquash(touched, added) || "merge aborted";
          // The worktree verified against an older HEAD: tasks merged since may break it. Re-check on the merged tree.
          const broken = target.verify.map((cmd) => ({ cmd, ...run(cmd) })).filter((v) => !v.ok);
          if (broken.length) {
            const undone = aborted();
            const fresh = load() ?? state;
            const live = findTask(fresh, target.id) ?? target;
            live.verified = false;
            save(fresh);
            const inMain = allTasks(fresh).filter((t) => t.status === "in_progress" && !t.worktree && t.id !== target.id);
            const cause = inMain.length
              ? `work merged since the worktree branched, or the unfinished edits of ${inMain.map((t) => t.key || t.id).join(", ")} in the main checkout, conflict with it`
              : "work merged since the worktree branched conflicts with it";
            return text(`FAILED ${target.id}: verify fails on the merged tree (${cause}) — ${undone}, task un-verified. Rebase the worktree onto HEAD, fix, engine_verify, engine_advance again.\n\n${broken.map((v) => `$ ${v.cmd}\n${v.output}`).join("\n\n")}`);
          }
          const c = git(["commit", "-m", msg]);
          if (!c.ok) {
            return text(`FAILED ${target.id}: commit of the squash-merge failed — ${aborted()}, task stays in_progress.\n${c.output}`);
          }
          merged = `\nReview PASS${r.reasons.length ? ` (notes: ${r.reasons.join("; ")})` : ""}. Squash-merged ${target.branch}: ${msg}`;
        }
        const fresh = load() ?? state;
        const live = findTask(fresh, target.id) ?? target;
        removeWorktree(live);
        live.status = "done";
        save(fresh);
        const c = counts(fresh);
        return text(`DONE ${target.id}. Progress ${c.done}/${c.total}.${merged}\nCall engine_ready (parallel) or engine_next for the next task.`);
      }

      const files = params.files ?? [];
      const others = allTasks(state).filter((t) => t.status === "in_progress" && t !== target && !t.worktree);
      if (params.commit && !files.length && others.length) {
        return text(
          `REFUSED ${target.id}: ${others.length} other task(s) in progress (${others.map((t) => t.key || t.id).join(", ")}) — a whole-tree commit would sweep their unfinished files. Pass files:[<paths this task changed>].`,
        );
      }
      // Commit first, mark done only on success: a dependent must never start on uncommitted code.
      let committed = "";
      if (params.commit) {
        const add = git(files.length ? ["add", "--", ...files] : ["add", "-A"]);
        if (!add.ok) return text(`FAILED ${target.id}: git add failed — task stays in_progress. Fix the file list, then call engine_advance again.\n${add.output}`);
        const staged = git(["diff", "--cached", "--quiet", "--", ...files]);
        if (staged.ok) {
          committed = "\nNothing to commit (no changes in the given files).";
        } else {
          // Secret gate before every autonomous commit — same check as the ckit
          // pre-commit hook. gitleaks absent → `if` runs no branch, exits 0 (best-
          // effort skip); present + a finding → non-zero → abort and unstage.
          const scan = run("if command -v gitleaks >/dev/null 2>&1; then gitleaks protect --staged --no-banner; fi");
          if (!scan.ok) {
            git(files.length ? ["reset", "-q", "--", ...files] : ["reset", "-q"]);
            return text(`ABORTED ${target.id}: gitleaks flagged a secret in the staged diff — task stays in_progress. Remove the secret, then call engine_advance again.\n${scan.output}`);
          }
          const r = git(["commit", "-m", msg, ...(files.length ? ["--", ...files] : [])]);
          if (!r.ok) return text(`FAILED ${target.id}: git commit failed — task stays in_progress.\n${r.output}`);
          committed = `\nCommitted: ${msg}`;
        }
      }
      target.status = "done";
      save(state);
      const c = counts(state);
      return text(`DONE ${target.id}. Progress ${c.done}/${c.total}.${committed}\nCall engine_ready (parallel) or engine_next for the next task.`);
    },
  });

  pi.registerTool({
    name: "engine_accept",
    label: "Engine: accept criterion (with evidence)",
    description:
      "Attach evidence to acceptance criterion `criterion` (1-based, see engine_status). Evidence kinds — verify: `taskId` of a DONE task whose verify passed; file: `path` + `line` that exist now (the line text is recorded); shot: `path` to a screenshot image created after the plan started. Without evidence a criterion stays `incomplete` and the plan is never DONE; there is no way to mark one met by assertion.",
    parameters: z.object({
      criterion: z.number().int().min(1),
      kind: z.enum(["verify", "file", "shot"]),
      taskId: z.string().optional(),
      path: z.string().optional(),
      line: z.number().int().min(1).optional(),
      note: z.string().optional(),
    }),
    async execute(_id, params) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const k = state.criteria[params.criterion - 1];
      if (!k) return text(`No criterion ${params.criterion} (plan has ${state.criteria.length}).`);
      let evidence: string;
      if (params.kind === "verify") {
        const t = params.taskId ? findTask(state, params.taskId) : undefined;
        if (!t) return text("REFUSED: kind verify needs the taskId of a plan task.");
        if (t.status !== "done" || !t.verify.length || !t.verified) return text(`REFUSED: ${t.id} is ${t.status}${t.verify.length ? (t.verified ? "" : ", never verified") : ", has no verify commands"} — evidence must be a done task whose verify passed.`);
        evidence = `verify ${t.key}: ${t.verify.join(" && ")}`;
      } else {
        if (!params.path) return text(`REFUSED: kind ${params.kind} needs path.`);
        const p = join(root(), params.path);
        if (!existsSync(p)) return text(`REFUSED: ${params.path} does not exist.`);
        if (params.kind === "file") {
          if (!params.line) return text("REFUSED: kind file needs line.");
          const lines = readFileSync(p, "utf8").split("\n");
          const at = lines[params.line - 1];
          if (at === undefined) return text(`REFUSED: ${params.path} has ${lines.length} lines, no line ${params.line}.`);
          evidence = `${params.path}:${params.line} "${at.trim().slice(0, 120)}"`;
        } else {
          const st = statSync(p);
          if (!SHOT_EXT.test(params.path) || !st.size) return text(`REFUSED: ${params.path} is not a non-empty png/jpg/webp/gif.`);
          if (st.mtime.toISOString() < state.createdAt) return text(`REFUSED: ${params.path} predates the plan (${state.createdAt}) — capture fresh evidence.`);
          evidence = `shot ${params.path}`;
        }
      }
      k.evidence = params.note ? `${evidence} (${params.note})` : evidence;
      k.at = new Date().toISOString();
      save(state);
      const open = state.criteria.filter((c) => !c.evidence).length;
      return text(`ACCEPTED criterion ${params.criterion}: ${k.text}\n  evidence: ${k.evidence}\n${open ? `${open} criteria still incomplete.` : "Every criterion has evidence."}`);
    },
  });

  pi.registerTool({
    name: "engine_worktree",
    label: "Engine: git worktree",
    description:
      "Isolate a slice in its own git worktree (open), squash-merge it back to the current branch (merge), or discard it (remove). open: git worktree add .cache/ckit/engine/wt/<slug> -b ckit/<slug>. For one worktree per task with a review-gated merge, use engine_ready {worktree:true} instead.",
    parameters: z.object({ action: z.enum(["open", "merge", "remove"]), slug: z.string() }),
    async execute(_id, params) {
      const slug = params.slug.replace(/[^a-zA-Z0-9._-]/g, "-");
      const wt = join(WT_REL, slug);
      // Worktrees opened before the rename live on `8sync/<slug>`; merge/remove still find them.
      const hasBranch = (b: string) => git(["rev-parse", "--verify", "-q", `refs/heads/${b}`]).ok;
      const legacy = `8sync/${slug}`;
      const branch = params.action !== "open" && !hasBranch(`ckit/${slug}`) && hasBranch(legacy) ? legacy : `ckit/${slug}`;
      if (params.action === "open") {
        mkdirSync(join(root(), WT_REL), { recursive: true });
        const r = git(["worktree", "add", wt, "-b", branch]);
        return text(r.ok ? `Worktree ${wt} on ${branch}. cd there to work the slice.` : `worktree add failed:\n${r.output}`);
      }
      if (params.action === "merge") {
        const m = git(["merge", "--squash", branch]);
        if (!m.ok) return text(`squash-merge failed (resolve, then retry):\n${m.output}`);
        const c = git(["commit", "-m", `merge ${branch} (squash)`]);
        git(["worktree", "remove", wt, "--force"]);
        git(["branch", "-D", branch]);
        return text(c.ok ? `Squash-merged ${branch} and removed the worktree.` : `merged but commit failed:\n${c.output}`);
      }
      git(["worktree", "remove", wt, "--force"]);
      const r = git(["branch", "-D", branch]);
      return text(`Removed worktree ${wt}${r.ok ? ` and branch ${branch}` : ""}.`);
    },
  });

  pi.registerCommand("engine", {
    description: "ckit engine status — show plan progress",
    handler: async (_args, ctx) => {
      const state = load();
      if (!state) {
        ctx.ui.notify("ckit engine: no plan. Use /auto <goal> or call engine_plan.", "info");
        return;
      }
      const c = counts(state);
      const open = state.criteria.filter((k) => !k.evidence).length;
      const crit = state.criteria.length ? `, ${state.criteria.length - open}/${state.criteria.length} criteria met` : "";
      ctx.ui.notify(`ckit engine: ${c.done}/${c.total} done, ${c.blocked} blocked${crit} — goal: ${state.goal}`, "info");
    },
  });
}

// ckit engine — a gsd-pi-style automation engine built 100% on omp core.
//
// It does NOT patch omp: it lives in omp's config dir (~/.omp/agent/extensions/
// global + <root>/.omp/extensions/ project, auto-loaded by the native extension
// provider) and exposes model-callable TOOLS that carry the parts gsd-pi enforces
// in CODE rather than prose:
//   - a durable milestone/slice/task state machine (JSON at .cache/ckit/engine/),
//   - verify-with-auto-retry (the tool runs the commands + counts attempts +
//     blocks a task once max retries is hit — the agent can't skip the gate),
//   - git worktree open / squash-merge / remove (code, not "please run git").
// The `/auto` slash command orchestrates these tools into a run-to-done loop.
// This is THE automation path: a single `/auto` command (no competing `/gs`).
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";
import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

type TaskStatus = "pending" | "in_progress" | "done" | "blocked";

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
}
interface EngineSlice {
  id: string;
  title: string;
  tasks: EngineTask[];
}
interface EngineState {
  goal: string;
  createdAt: string;
  updatedAt: string;
  maxRetries: number;
  slices: EngineSlice[];
}

const STATE_REL = ".cache/ckit/engine/state.json";
const WT_REL = ".cache/ckit/engine/wt";
const MAX_OUTPUT = 2000;

export default function (pi: ExtensionAPI) {
  const { z } = pi.zod;
  pi.setLabel("ckit engine (gsd-pi-style, on omp core)");

  const taskSchema = z.object({
    id: z.string(),
    key: z.string().default(""),
    title: z.string(),
    status: z.enum(["pending", "in_progress", "done", "blocked"]),
    retries: z.number(),
    verified: z.boolean().default(false),
    failStreak: z.number().default(0),
    lastFailureHash: z.string().default(""),
    verify: z.array(z.string()),
    depends: z.array(z.string()).default(() => []),
    files: z.array(z.string()).default(() => []),
    note: z.string(),
  });
  const stateSchema = z.object({
    goal: z.string(),
    createdAt: z.string(),
    updatedAt: z.string(),
    maxRetries: z.number(),
    slices: z.array(z.object({ id: z.string(), title: z.string(), tasks: z.array(taskSchema) })),
  });

  function load(): EngineState | null {
    const p = join(process.cwd(), STATE_REL);
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
    const p = join(process.cwd(), STATE_REL);
    mkdirSync(join(process.cwd(), ".cache/ckit/engine"), { recursive: true });
    writeFileSync(p, JSON.stringify(state, null, 2));
  }

  function counts(state: EngineState): { total: number; done: number; blocked: number } {
    let total = 0;
    let done = 0;
    let blocked = 0;
    for (const s of state.slices) {
      for (const t of s.tasks) {
        total += 1;
        if (t.status === "done") done += 1;
        if (t.status === "blocked") blocked += 1;
      }
    }
    return { total, done, blocked };
  }

  function allTasks(state: EngineState): EngineTask[] {
    return state.slices.flatMap((s) => s.tasks);
  }

  function byKey(state: EngineState): Map<string, EngineTask> {
    return new Map(allTasks(state).map((t) => [t.key || t.id, t]));
  }

  function depsDone(state: EngineState, task: EngineTask): boolean {
    const keys = byKey(state);
    return task.depends.every((k) => keys.get(k)?.status === "done");
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

  function findNext(state: EngineState): { slice: EngineSlice; task: EngineTask } | null {
    for (const s of state.slices) {
      for (const t of s.tasks) {
        if (t.status === "in_progress") return { slice: s, task: t };
        if (t.status === "pending" && depsDone(state, t)) return { slice: s, task: t };
      }
    }
    return null;
  }

  /** Git runs directly with omp's PATH: a login shell (`bash -lc`) can resolve a different, broken git (macOS path_helper → /usr/bin/git). */
  function git(args: string[]): { ok: boolean; output: string } {
    const r = spawnSync("git", args, { cwd: process.cwd(), encoding: "utf8" });
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

  /** Non-login shell with omp's own env: a login shell re-sources /etc/profile, whose macOS path_helper puts /usr/bin first (wrong git/php). */
  function run(cmd: string): { ok: boolean; output: string } {
    const r = spawnSync("bash", ["-c", cmd], { cwd: process.cwd(), env: process.env, encoding: "utf8" });
    const raw = `${r.stdout ?? ""}${r.stderr ?? ""}`.trim();
    const output = raw.length > MAX_OUTPUT ? `${raw.slice(0, MAX_OUTPUT)}\n…[truncated]` : raw;
    return { ok: r.status === 0, output };
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

  pi.registerTool({
    name: "engine_plan",
    label: "Engine: plan",
    description:
      "Create/replace the run-to-done plan: a goal decomposed into slices, each with atomic tasks and optional verify commands (lint/test). Optional per task: `key` (e.g. T3, defaults to the task id), `depends` (keys that must be done first) and `files` (paths/dirs the task owns) — engine_ready runs independent tasks in parallel from these. `append: true` keeps the current plan (goal, tasks, statuses) and adds the slices after it, so new tasks may depend on existing keys. Rejects duplicate/unknown keys and dependency cycles. Persists to .cache/ckit/engine/state.json.",
    parameters: z.object({
      goal: z.string(),
      maxRetries: z.number().int().min(0).max(10).default(3),
      append: z.boolean().default(false),
      slices: z
        .array(
          z.object({
            title: z.string(),
            tasks: z.array(
              z.object({
                title: z.string(),
                key: z.string().optional(),
                verify: z.array(z.string()).default(() => []),
                depends: z.array(z.string()).default(() => []),
                files: z.array(z.string()).default(() => []),
              }),
            ),
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
            const id = `s${si}.t${ti}`;
            return { id, key: t.key || id, title: t.title, status: "pending", retries: 0, verify: t.verify, depends: t.depends, files: t.files, note: "", verified: false, failStreak: 0, lastFailureHash: "" };
          }),
        };
      });
      const state: EngineState = base
        ? { ...base, updatedAt: now, slices: [...base.slices, ...added] }
        : { goal: params.goal, createdAt: now, updatedAt: now, maxRetries: params.maxRetries, slices: added };
      const invalid = validateGraph(allTasks(state));
      if (invalid) return text(`Plan REJECTED: ${invalid}. Nothing saved.`);
      save(state);
      const c = counts(state);
      const head = base
        ? `Appended ${added.length} slice(s) "${params.goal}" to plan "${state.goal}" — ${c.done}/${c.total} tasks done.`
        : `Plan saved: "${params.goal}" — ${state.slices.length} slices, ${c.total} tasks.`;
      return text(`${head} Call engine_ready (parallel) or engine_next (one at a time) to start.`);
    },
  });

  pi.registerTool({
    name: "engine_status",
    label: "Engine: status",
    description: "Report the current plan: per-slice task statuses + overall progress (done/total, blocked).",
    parameters: z.object({}),
    async execute() {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const c = counts(state);
      const lines = [`Goal: ${state.goal}`, `Progress: ${c.done}/${c.total} done, ${c.blocked} blocked`, ""];
      for (const s of state.slices) {
        lines.push(`# ${s.id} ${s.title}`);
        for (const t of s.tasks) {
          const key = t.key && t.key !== t.id ? ` ${t.key}` : "";
          const deps = t.depends.length ? ` (depends: ${t.depends.join(",")})` : "";
          const retries = t.retries ? ` (retries:${t.retries})` : "";
          const note = t.status === "blocked" && t.note ? ` — ${t.note}` : "";
          const files = t.status === "in_progress" && t.files.length ? ` [files: ${t.files.join(", ")}]` : "";
          lines.push(`  [${t.status}] ${t.id}${key} ${t.title}${deps}${retries}${files}${note}`);
        }
      }
      return text(lines.join("\n"));
    },
  });

  pi.registerTool({
    name: "engine_next",
    label: "Engine: next task",
    description: "Return the next pending task whose dependencies are done (with its slice) and mark it in_progress. Returns done when every task is done/blocked.",
    parameters: z.object({}),
    async execute() {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      propagateBlocked(state);
      const next = findNext(state);
      if (!next) {
        const c = counts(state);
        return text(c.blocked ? `All tasks resolved but ${c.blocked} BLOCKED — review engine_status.` : "DONE — every task is complete.");
      }
      next.task.status = "in_progress";
      save(state);
      const verify = next.task.verify.length ? `\nVerify with: ${next.task.verify.join(" && ")}` : "";
      return text(`NEXT ${next.task.id} (slice ${next.slice.title}): ${next.task.title}${verify}\nImplement it, then call engine_verify, then engine_advance.`);
    },
  });

  pi.registerTool({
    name: "engine_ready",
    label: "Engine: ready tasks (parallel)",
    description:
      "Return EVERY pending task whose dependencies are done and whose files do not overlap (path prefix) with an in-progress task or with each other, up to `limit`, and mark them in_progress — dispatch them in parallel. Also lists tasks still in progress. Returns done when every task is done/blocked.",
    parameters: z.object({ limit: z.number().int().min(1).max(32).optional() }),
    async execute(_id, params) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      const newlyBlocked = propagateBlocked(state);
      const tasks = allTasks(state);
      const running = tasks.filter((t) => t.status === "in_progress");
      const busy = running.flatMap((t) => t.files);
      const cap = (params.limit ?? Number.MAX_SAFE_INTEGER) - running.length;
      const ready: EngineTask[] = [];
      for (const t of tasks) {
        if (ready.length >= cap) break;
        if (t.status !== "pending" || !depsDone(state, t) || overlaps(t.files, busy)) continue;
        t.status = "in_progress";
        busy.push(...t.files);
        ready.push(t);
      }
      save(state);

      const blockedNote = newlyBlocked.length ? `\nBlocked because a dependency is blocked: ${newlyBlocked.join(", ")}` : "";
      if (!ready.length && !running.length) {
        const c = counts(state);
        return text((c.blocked ? `All tasks resolved but ${c.blocked} BLOCKED — review engine_status.` : "DONE — every task is complete.") + blockedNote);
      }
      const fmt = (t: EngineTask) => {
        const files = t.files.length ? `\n    files: ${t.files.join(", ")}` : "";
        const verify = t.verify.length ? `\n    verify: ${t.verify.join(" && ")}` : "";
        return `  ${t.id} ${t.key} ${t.title}${files}${verify}`;
      };
      const lines = [`READY ${ready.length} (now in_progress — dispatch in parallel):`, ...ready.map(fmt)];
      if (running.length) lines.push(`STILL IN PROGRESS ${running.length}:`, ...running.map((t) => `  ${t.id} ${t.key} ${t.title}${t.files.length ? ` [files: ${t.files.join(", ")}]` : ""}`));
      lines.push("Per task: engine_verify → engine_advance {files:[changed files]}; call engine_ready again after each advance.");
      return text(lines.join("\n") + blockedNote);
    },
  });

  pi.registerTool({
    name: "engine_verify",
    label: "Engine: verify (auto-retry gate)",
    description:
      "Run the task's verify commands (or the ones passed). All must pass to advance. On failure the retry counter increments; once it reaches maxRetries the task is BLOCKED. Identical consecutive failures trip the no-progress guard: 2x warns, 3x BLOCKS early (doom-loop). The gate is code-enforced — engine_advance refuses an unverified task.",
    parameters: z.object({ taskId: z.string(), commands: z.array(z.string()).optional() }),
    async execute(_id, params) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      let target: EngineTask | undefined;
      for (const s of state.slices) for (const t of s.tasks) if (t.id === params.taskId) target = t;
      if (!target) return text(`No task ${params.taskId}.`);
      const cmds = params.commands?.length ? params.commands : target.verify;
      if (!cmds.length) return text(`Task ${target.id} has no verify commands — add some or advance manually if truly trivial.`);

      const failures: string[] = [];
      for (const cmd of cmds) {
        const r = run(cmd);
        if (!r.ok) failures.push(`$ ${cmd}\n${r.output}`);
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
          ? `BLOCKED ${target.id}: NO PROGRESS — ${target.failStreak} consecutive identical failures (doom-loop guard). Repeating the same attempt cannot pass. Record a failure: in agents/KNOWLEDGE.md, then change approach, split the task, or escalate.`
          : `BLOCKED ${target.id} after ${target.retries} attempts (maxRetries=${state.maxRetries}). Record a failure: in agents/KNOWLEDGE.md and move on / escalate.`;
        return text(`${head}${also}\n\n${failures.join("\n\n")}`);
      }
      save(state);
      const loop = target.failStreak === 2 ? " WARNING: same failure twice in a row — a third identical failure BLOCKS the task (no-progress guard). Change the approach, don't retry the same fix." : "";
      return text(`FAILED ${target.id} (attempt ${target.retries}/${state.maxRetries}).${loop} Fix the cause, then call engine_verify again:\n\n${failures.join("\n\n")}`);
    },
  });

  pi.registerTool({
    name: "engine_advance",
    label: "Engine: advance",
    description:
      "Mark a verified task done and optionally commit the change (a gitleaks gate blocks the commit if a secret is staged, matching the ckit pre-commit hook). REFUSES a task whose verify commands never passed — the agent's own say-so is not a stop signal. With `files`, stages and commits ONLY those paths (required while other tasks are in progress — `git add -A` would sweep their unfinished work). Advances the plan.",
    parameters: z.object({
      taskId: z.string(),
      commit: z.boolean().default(false),
      message: z.string().optional(),
      files: z.array(z.string()).optional(),
    }),
    async execute(_id, params) {
      const state = load();
      if (!state) return text("No plan yet. Call engine_plan first.");
      let target: EngineTask | undefined;
      for (const s of state.slices) for (const t of s.tasks) if (t.id === params.taskId) target = t;
      if (!target) return text(`No task ${params.taskId}.`);
      if (target.verify.length && !target.verified) {
        return text(
          `REFUSED ${target.id}: it has ${target.verify.length} verify command(s) but no passing engine_verify run. The gate is code-enforced — call engine_verify {taskId:"${target.id}"} first.`,
        );
      }
      const files = params.files ?? [];
      const others = allTasks(state).filter((t) => t.status === "in_progress" && t !== target);
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
          const msg = params.message ?? `feat: ${target.title}`;
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
    name: "engine_worktree",
    label: "Engine: git worktree",
    description:
      "Isolate a slice in its own git worktree (open), squash-merge it back to the current branch (merge), or discard it (remove). open: git worktree add .cache/ckit/engine/wt/<slug> -b ckit/<slug>.",
    parameters: z.object({ action: z.enum(["open", "merge", "remove"]), slug: z.string() }),
    async execute(_id, params) {
      const slug = params.slug.replace(/[^a-zA-Z0-9._-]/g, "-");
      const wt = join(WT_REL, slug);
      // Worktrees opened before the rename live on `8sync/<slug>`; merge/remove still find them.
      const hasBranch = (b: string) => git(["rev-parse", "--verify", "-q", `refs/heads/${b}`]).ok;
      const legacy = `8sync/${slug}`;
      const branch = params.action !== "open" && !hasBranch(`ckit/${slug}`) && hasBranch(legacy) ? legacy : `ckit/${slug}`;
      if (params.action === "open") {
        mkdirSync(join(process.cwd(), WT_REL), { recursive: true });
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
      ctx.ui.notify(`ckit engine: ${c.done}/${c.total} done, ${c.blocked} blocked — goal: ${state.goal}`, "info");
    },
  });
}

---
name: feature
argument-hint: '[new <slug> | plan | go | ship | status | switch <slug> | list] [--auto | --all [--to M<n>]]'
description: Large multi-phase feature scopes (GSD). Drives the `feature` skill — scaffold a planning tree (agents/planning/<slug>/), plan a phase (Goal+AC), execute it via the engine_* loop, and verify against the AC matrix. Cross-session, switchable. Small/single-concern work → /auto.
---

# /feature — large-scope GSD framework

`$ARGUMENTS` first word selects the subcommand; `--auto` anywhere = autonomous full-phase;
`--all` anywhere = autonomous every remaining phase.

Use this for **large, multi-phase features** (>10 files, multiple milestones, spanning
sessions). Small/single-concern work → **`/auto`** (drive the engine directly). This layer
owns the multi-feature ROADMAP + per-phase Acceptance-Criteria contract *above* the
`engine_*` loop.

## 0. Load the skill + ground (do this first, every subcommand)
Read the bundled skill and its rules BEFORE acting:
- `~/.omp/skills/feature/SKILL.md` + `~/.omp/skills/feature/references/feature-rules.md`
  (the always-applied rules), then ONLY the reference for this subcommand
  (`new`/`plan`/`execute`/`ship`; + `auto` when `--auto`/`--all`). Load each with `read`, once;
  never `cat` several references in one bash call or re-read one you already have — every
  byte loaded stays in this session's context for every later turn.
Then ground on state (except for `new`, which creates it):
- `agents/planning/ACTIVE.md` line 1 → active slug; `agents/planning/<slug>/STATE.md`
  frontmatter (`status`/`active_phase`/`next_action`); `agents/planning/config.json`
  (`workflow.*`, `paths.*`). Obey `~/.omp/agent/APPEND_SYSTEM.md` (code-intel first;
  always-on skills). Explore with **codegraph / codebase-memory-mcp / serena** via `xd://`
  devices — never grep/Read-all; summarize long tool output (> ~300 lines), never paste it raw.

## 1. Dispatch (`$ARGUMENTS` first word)
| word | do |
|------|----|
| `new <slug>` | `references/new.md` — the deterministic scaffold is also `8sync feature new <slug>`; then fill PROJECT/REQUIREMENTS/ROADMAP with the user, cut phases by dependency. Gate 1: user approves the architecture. |
| `plan` | `references/plan.md` — discuss + write `M<x>-CONTEXT.md` (📌 Requirement scope + 🎯 Goal + ✅ AC table) + `M<x>-NN-PLAN.md` (tasks ↔ UC ↔ AC, each with `[file:]` + `[depends:]` + scoped `[verify:]`). Plan-review per `config.workflow.plan_review`. Gate 2: user approves the AC + plan. |
| `go` | `references/execute.md` — `engine_plan` (goal = phase Goal, one task per PLAN task with `key`/`depends`/`files`/scoped `verify`), then the dispatch loop: `engine_ready {limit}` returns every task whose deps are done and whose files don't overlap a running task → spawn dev subagents for them; when one finishes run `engine_verify` while the others keep coding; PASS → `engine_advance {commit:true, files:[changed files]}`; FAIL → re-dispatch dev with the failure. Dependency order, file overlap, doom-loop guard and blocked-dependents are engine-enforced. Tick STATE.Log + PLAN checkbox per task. |
| `ship` | `references/ship.md` — review (multi-lens via `task` reviewers) + test (`task` tester) against the AC → write `M<x>-VERIFICATION.md` AC-matrix. Any AC FAIL → converge: classify (missing code / wrong code / bad AC → ask user / blocked → NEEDS-CONFIRM), append converge tasks to PLAN, run them through the `go` loop, re-verify; repeat up to `workflow.max_converge_rounds` (default 3), stop early when the same AC fails twice with the same evidence. Phase done ⇔ every AC PASS. Archive on the final phase. |
| `status` | print the active STATE position (same as `8sync feature status`). |
| `switch <slug>` | flip ACTIVE (same as `8sync feature switch <slug>`), then re-ground. |
| `list` | list features + archived (same as `8sync feature list`). |
| _(empty)_ | read STATE → suggest the next action from `next_action`. |

## 2. `--auto` (autonomous full-phase)
If `$ARGUMENTS` contains `--auto`, load `references/auto.md` FIRST, then run the phase
autonomously: replace user gates with a `task` discuss-agent + plan-review; run `go` via the
engine to DONE; a hard blocker (credential / real external data) → SKIP the item, record
NEEDS-CONFIRM in VERIFICATION/STATE, and finish the rest of the phase. Stop at the next phase
boundary (never auto-advance phases without `--all`). This mirrors `/auto`'s engine discipline,
scoped to one phase. `ship --auto` follows every `--auto` branch of `references/ship.md`.

## 3. `--all` (every remaining phase, this session)
If `$ARGUMENTS` contains `--all`, load `references/auto.md` (incl. §`--all`) FIRST. Loop in THIS
session — never wrap a phase in a `task` subagent (it costs the dev/reviewer agents their
`task` tool at the default `task.maxRecursionDepth: 2`): per phase run `--auto` (plan → go) →
`ship --auto` → `phase_gate.py --check all`; advance only on `DONE` + `active_phase` moved; stop
on `CONDITIONAL`/`NOT DONE`, at `--to M<n>`, or when the ROADMAP is finished. Rewrite STATE
Session Continuity before each next phase. Unattended (away from the machine) → `8sync feature run`
(one fresh `omp -p` process per step; report in `agents/planning/<slug>/RUN-REPORT.md`).

## Guardrails
Work must belong to the `active_phase` (stop + ask if it drifts; `--auto` → SKIP+NEEDS-CONFIRM).
Verify-gate before every commit (`engine_verify` enforces it; `engine_advance` refuses
unverified tasks). Scope edits to the change + `agents/` memory. NO `git push` / PR unless
asked. Every AC maps to ≥1 UC in REQUIREMENTS; every task maps to ≥1 AC — no orphan work.

## Model + context budget
Use the models in `~/.config/8sync/models.toml` (view/edit: `8sync harness model`); route per
task class. The orchestrator's cost ≈ turns × context length (a real M0 run: context 43k → 325k,
orchestrator ≈ all dev agents combined), so:
- **One stage per session.** STATE/PLAN/engine state live in files, so a stage never needs the
  previous stage's transcript. After gate 1 (`new`), gate 2 (`plan`) and the end of `go`, write
  the STATE handoff and tell the user to continue in a fresh session (`8sync new`, then
  `/feature <next>`). Unattended → `8sync feature run` already starts a fresh process per stage.
- Long documents (PROJECT/REQUIREMENTS/ROADMAP/CONTEXT/PLAN) and all code/test changes are
  written by subagents; the orchestrator keeps decisions, ids and one-line summaries.
- Batch independent calls into one turn; never re-read a file you just wrote.
- When context nears the limit (auto-compacts at 50%), write a handoff into the active
  feature's `agents/planning/<slug>/STATE.md` (< 100 lines, digest) BEFORE it fires.

Begin: load the skill, ground on ACTIVE + STATE + config, then act on `$ARGUMENTS`.

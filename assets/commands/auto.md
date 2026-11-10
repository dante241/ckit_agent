---
name: auto
argument-hint: '[<goal> | status | resume]'
description: 8sync autonomous engine — decompose a goal into slices/tasks and run to DONE on omp core via the code-enforced engine_* tools (durable state, verify-with-retry gate, git worktree). Right-sized, token-lean (codegraph/cbm/serena), ponytail/karpathy discipline.
---

# /auto — run to done on the 8sync engine

`$ARGUMENTS` first word: `<goal>` = plan + run · `status` = report · `resume`/empty = continue the saved plan.

You drive the **8sync-engine** (model-callable `engine_*` tools). It owns the durable plan state, the verify gate, and worktrees in CODE — you supply judgement. Obey `~/.omp/agent/APPEND_SYSTEM.md` (code-intel first; always-on skills).

## 0. Ground (token-lean)
Read `agents/STATE.md` + the recent `failure:` entries in `agents/KNOWLEDGE.md`. Explore with **codegraph / codebase-memory-mcp / serena** — never grep/Read-all. Summarize long tool output (> ~300 lines); never paste it raw.

## 1. Right-size first (ponytail)
Trivial / small (a few files, clear path) → just do it, no engine ceremony. Medium / large / multi-slice → use the engine.

## 2. Research, then plan (research is INTEGRATED into planning)
First **research** the unknowns so the plan is grounded — never guess:
- Scout the codebase: **codegraph** (callers/deps/impact) + **codebase-memory-mcp** (architecture) + **serena** (symbols).
- Domain/external unknowns: **feynman** skills (`deep-research` / `autoresearch` / `literature-review`) + `web_search` / `last30days`. Log decisions under `## Assumptions` in `agents/STATE.md`.
Then call **engine_plan** with the goal + slices; each slice's atomic tasks; each task's `verify` commands = the project's REAL lint/test/build (this is the gate). A task without `verify` is REJECTED unless marked `trivial: true` (docs/static config only). Bug fixes and behaviour changes with an automated test: `red: true` + `tests: [test paths]` (inside `files`) + `spec` (the acceptance criteria the tests must check — a red task without it is REJECTED). Put the goal's Definition-of-Done in `criteria` (one observable line each): the plan is not DONE until each has evidence. Smallest-first.

## 3. Loop until done (in an autonomous run, do not yield between tasks)
1. **engine_next** → next task + scoped context. Understand before editing (codegraph callers/deps + `git log/blame` + `agents/DECISIONS.md`).
2. Red-first task → write/adjust ONLY its `tests` files, call **engine_red** `{taskId}`. It first has an independent reviewer (fresh omp, no tools, never saw your work) audit the tests against `spec`: UNFAIR (tied to names, paths, call counts, message wording or defaults the spec doesn't state, or vacuous) → rewrite the tests, call again. Then it is refused if code files already changed, if the test already passes, or on exit 126/127; check the failure is the expected assertion. Then implement at the right size — do not touch the audited tests again (editing them costs a verify attempt). Prefer **serena** symbol-level edits over blind whole-file rewrites.
3. **engine_verify** `{taskId}` — the gate runs the plan's commands; extra `commands` are added, never substituted. FAILED → fix the cause, call engine_verify again — with a DIFFERENT fix: 2 identical failures warn, 3 BLOCK the task early (no-progress/doom-loop guard). BLOCKED → diagnose the failure output, then **engine_unblock** `{taskId, diagnosis, action}` — `retry` (cause fixed outside the task), `split` (replacement `tasks`; dependents wait for them) or `drop` (not needed; dependents run without it). It writes the `failure:` entry to `agents/KNOWLEDGE.md` itself.
4. **engine_advance** `{taskId, commit:true}` — code-REFUSED unless engine_verify passed (your own "done" is not a stop signal); gitleaks clean.
5. Tick `agents/STATE.md` (Current/Next); distill a `validated:` runbook into `agents/PLAYBOOKS.md` when a multi-step procedure worked.
6. Loop. Parallel or risky tasks: **engine_ready** `{worktree:true}` gives each task its own worktree + branch — the worker edits only there; engine_advance commits it, an independent reviewer judges the diff, and only PASS squash-merges, after re-running verify on the merged tree (FAIL returns findings and un-verifies the task; the same diff is never re-reviewed; staged changes in the main checkout make it refuse). **engine_worktree** still isolates a whole slice (open → work → merge squash → remove).

## 4. Closeout — re-evaluate HARD before handing back (large / multi-slice)
When engine_next/engine_ready report DONE (every task done **and** every `criteria` line has evidence) hand over ONLY if green. Attach evidence with **engine_accept** `{criterion, kind}`: `verify` (a done task whose verify passed), `file` (`path`+`line`) or `shot` (a screenshot taken after the plan started). A criterion you could not observe stays `incomplete` — report it as such, never as met.
1. **Full test suite** (not just changed tests) + add missing coverage.
2. **QA / UAT the running thing** — exercise real end-to-end flows in a **browser** (web app or the desktop web-debug port; see below). Catch what unit tests miss.
3. **Independent re-review** — a fresh `reviewer` (+ `senior-security` for anything sensitive) subagent reads the whole diff vs the goal + Definition-of-Done.
4. Doc-hygiene (`8sync harness audit`) + handoff summary; every goal item ↔ concrete evidence. Any gap → back into the loop.

## Use omp's tools (they are strong)
- **browser** — verify any web/visual change for real (open, screenshot, click), never guess. **Web app** → point it at the dev URL. **Desktop app (Tauri v2 / WRY-WebKit)** → run it with its web-inspector/remote-debug port enabled, then point the SAME `browser` tool at that port — identical DOM/console/network verification as web.
- **task** subagents — independent parallel work / context-heavy isolation / specialization you lack.
- **irc** — coordinate with siblings before editing a shared file.

## Guardrails
Verify-gate before every commit (engine_verify enforces it; engine_advance refuses unverified tasks) · scope to the change + `agents/` memory · NO `git push` / PR unless asked · unattended needs omp `tools.approvalMode: yolo` AND a hard token ceiling (budget `+Nk!`) — a turn/token cap is a stop signal, agent say-so is not · stop only on a true blocker (missing credential, irreversible/destructive action), never on ambiguity — pick the reversible option and log it.

## Model + context budget
Use the models in `~/.config/8sync/models.toml` (view/edit: `8sync harness model`); route per task class — cheaper for trivial, stronger for review/debug — **never above the configured ceiling**, and let omp fall back to an authenticated model when the configured one isn't logged in. When context nears the limit (auto-compacts at 50%), write a structured handoff into `agents/STATE.md` BEFORE it fires so the next session resumes clean.

Begin: ground, right-size, then act on `$ARGUMENTS`.

## Your step: Dev — execute the approved phase = `/feature go` (references/execute.md)

The boss approved phase `{{PHASE}}` (Gate 2) at the board — do the plan.md Step 4 bookkeeping first: STATE `status: planned` → `next_action: execute-phase`, ROADMAP phase `[ ]` → `[~]`.
Then follow `references/execute.md` with `--auto` semantics (`references/auto.md`): `engine_plan` from the PLAN (one task per PLAN task with `key`/`depends`/`files`/scoped `verify`), dispatch loop with `engine_ready`, `engine_verify`, `engine_advance {commit: true, files: [...]}` per task. Blocked item → SKIP + NEEDS-CONFIRM, code the rest.
Every `engine_advance` commit passes `message: "feat({{PHASE}}/<key>): <task title>"` (e.g. `feat(M1/T3): lưu tin`) so the board can link each PLAN task to its commit.
Stop when the phase's tasks are done — do NOT run `/feature ship` (the Tester does).

Result file: `{"verdict": "pass", "message": ""}`; `"fail"` only if nothing could be built, with `message` explaining the blocker for the boss.

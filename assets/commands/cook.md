---
name: cook
argument-hint: '<#ticket | "description"> [--simple | --complex] [--skip-test] [--resume]'
description: Code a VTiger ticket/feature end-to-end via the Primary Workflow, executed through the engine_* loop — classify, knowledge lookup, load domain skills, research, plan + user gate, branch, then engine_plan→next→verify→advance with a real php-l/SQL/test verify-gate, review, complete. User-gated (plan needs approval); autonomous engine only after approval. Small/single-concern → /auto; large multi-phase GSD → /feature.
---

# /cook — VTiger feature/bugfix end-to-end (Primary Workflow on the engine)

`$ARGUMENTS` = a PMS ticket `#NNNNN` **or** an inline `"description"`, plus flags:
`--simple` / `--complex` force complexity · `--skip-test` skip the test tier (not recommended) ·
`--resume` continue from the saved plan/todolist.

Self-contained: this command IS the Primary Workflow — no separate skill. It is **user-gated**
(the plan needs approval before code); after approval, execution runs on the **engine** (the
code-enforced `engine_*` verify-gate + doom-loop guard), same discipline as `/feature go`.
For a fully autonomous run use **`/auto`**; for large multi-phase GSD scopes use **`/feature`**.
Obey `~/.omp/agent/APPEND_SYSTEM.md` (code-intel first; always-on skills). REJECT if neither a ticket nor a description is given.

## 0. Ground (token-lean)
Read `agents/STATE.md` + recent `failure:` in `agents/KNOWLEDGE.md`. `--resume` → also read the
existing `.omp/plans/<ticket>-<slug>/todolist.md` and resume at the first unchecked task.
Explore with **codegraph / codebase-memory-mcp / serena** — never grep/Read-all;
`headroom_compress` any tool output > ~300 lines.

## 1. Classify + knowledge lookup (Phase 1–1.5)
- Ticket `#NNNNN` → `pms_ticket_detail` with the raw numeric **internal** id; if the displayed
  number ≠ internal id, resolve via `pms_tickets` filter `{name:"ticket_no", value:"NNNNN",
  operator:"c"}`. Read title/description + `steps_to_reproduce`/`expected_result`/
  `actual_result` + `imagename` images (read them for UI/bug reference).
- Inline `"..."` → parse directly; ask only if genuinely ambiguous.
- Classify: `feature` / `bugfix` / `config` / `report` / `integration`. A `bugfix` runs the
  Debugging path — locate via codegraph/serena, reproduce, fix root cause, confirm the repro no
  longer triggers.
- Complexity: `simple` ≤3 files · `medium` ≤10 · `complex` >10 (override with `--simple` /
  `--complex`).
- **MANDATORY** — read `docs/knowledge/INDEX.md` → the module `docs/knowledge/modules/<Module>.md`
  and cross-module `flows/<flow>.md`. Summarize 1–3 business points, or note "no knowledge file
  for `<Module>`". Do not skip: understand the business before coding.

## 2. Load domain skills + research (Phase 2–3)
Auto-load the matching VTiger domain skills (open their `SKILL.md`; lite=SKILL.md only for
simple, full=+`references/` for medium/complex): view / action / database / field / migration /
language / handler / cron / config / integration / report / notification / export / ui / module /
inventory / error-handling / testing. Research token-lean: serena symbols → codegraph
callers/deps → cbm architecture; spawn a `scout` for unfamiliar modules. Skip research for
known-pattern CRUD.

## 3. Plan + gates (Phase 4–6.5)
- Plan under `.omp/plans/<ticket>-<slug>/` (`plan.md` + `todolist.md`; slug = `<ticket>-<kebab>`,
  or `<kebab>` if no ticket). Open every plan with the **Approach Declaration**:
  - **Loại thay đổi** · **Pattern chọn** (đối chiếu convention; chọn khác → 1 câu lý do) ·
    **Base class** · **Schema/field** (không đụng / BFR+quick_repair / migration CPMigration) ·
    **Exemplar đã đọc** (path file cùng loại trong repo — CẤM viết code trước khi đọc exemplar;
    không có thì `codegraph_explore` tìm 2 file gần nhất) · **Files tạo/sửa**.
- `simple` → lite todolist, self-approved (no gate). `medium`/`complex` → write `plan.md`, then
  **STOP and wait for user approval** (Phase 5). UI involved → confirm layout (text/ASCII, or a
  `designer` mockup for complex) before coding (Phase 6).
- **Phase 6.5 Sync & Branch** (before any code write): `git status` — unrelated uncommitted
  changes → **stop + ask** (never carry them onto master). On `master`/`dev` → `git checkout
  master` + `git pull origin master` + create `<type>/#<ticket>-<slug>` (`feature/`/`bug/`/
  `hotfix/`/`refactor/`; confirm the name). Already on a feature/bug branch (e.g. `--resume`) →
  stay, skip sync.

## 4. Execute on the engine (Phase 7–9)
After the plan is approved, **delegate execution to the engine** (like `/feature go`):
1. `engine_plan` — goal = the ticket/feature; slices/tasks = the plan; each task's `verify` = its
   REAL gate:
   - `php -l <file>` for every touched PHP file;
   - **test tier** by what changed: Action/Model/Helper → `must-test` (spawn `tester`: unit +
     security) · Report/SQL/migration-with-SELECT → `verify-sql` (run the query on the real DB) ·
     DDL migration / language / config / CSS → `verify-only` (`php -l` + review) · UI (TPL/JS) →
     E2E via the **browser** tool. `--skip-test` drops the tier (keep `php -l`).
2. Loop `engine_next → implement → engine_verify {taskId} → engine_advance {taskId}`:
   - **Implement** at the right size — prefer serena symbol-level edits over whole-file rewrites.
     Enforce **file separation** (NO inline CSS/JS): CSS→`modules/<M>/resources/<V>.css`; JS
     core→`layouts/v7/modules/<M>/resources/<V>.js`, custom→`modules/<M>/resources/<V>.js`; TPL
     core→`layouts/v7/modules/<M>/<V>.tpl`, custom→`modules/<M>/tpls/<V>.tpl`. Runtime-check
     before advancing: parent/base methods exist (serena `find_symbol`), classes in
     `extends`/`new` exist, DB columns match schema, config keys valid.
   - `engine_verify` runs the gate. FAILED → fix the CAUSE and re-verify with a DIFFERENT fix (2
     identical fails warn, 3 BLOCK the task — no-progress guard). BLOCKED → write a `failure:` to
     `agents/KNOWLEDGE.md`, move on or escalate.
   - `engine_advance {taskId}` marks the verified task done (REFUSED if verify never passed).
     **Do NOT `commit:true`** — `/cook` defers commit to Phase 10 / `/commit` (unless the user
     asked to commit).
3. **Code review gate (Phase 8)** — before completion, run a `reviewer` subagent on the full diff:
   runtime correctness · security (prepared `pquery(?)`, `.text()` not `.html()`, `(string)`/
   `(int)` cast on `$request->get()`, `checkPermission()` on controllers) · performance (no query
   in loops, indexed WHERE/JOIN, LIMIT) · cleanliness. Must pass before Phase 10.

## 5. Completion + capture gates (Phase 10–10.6)
- **Phase 10** — update `.omp/plans/<ticket>-<slug>/todolist.md` (status Completed, tick tasks,
  fill Test Results + Completion Notes); summarize (what/files/warnings); **migration reminder**
  if any migration was created; offer `/commit #NNNNN`.
- **Phase 10.5 Knowledge (USER GATE)** — new business / changed logic / important discovery /
  new-or-touched module without a knowledge file, or a decision the user locked this session →
  draft a proposal (show FULL text) and ask via `AskUserQuestion`
  (Approve all / Edit / Skip / Approve partial). **NEVER write to `docs/knowledge/` (or a `D-NNN`
  to `agents/DECISIONS.md`) without approval.** Skip for label/CSS/typo-only changes.
- **Phase 10.6 Error pattern** — mostly `bugfix`. On a generalizable anti-pattern / framework
  misuse / semantic bug: **auto-draft `PEND-<ticket>` into `.omp/rules/error-patterns-pending.md`
  (no ask)** with `Trigger keywords` = verbatim substrings from the buggy code. At flow end, if
  pending has new entries → `AskUserQuestion` to **promote** (assign real `EP-NNN`, append to
  `.omp/rules/error-patterns.md`, remove from pending). **NEVER write `error-patterns.md` without
  approval; pending is free.**

## Rules (enforced)
Ticket or description required · knowledge lookup mandatory (Phase 1.5) · exemplar-before-code ·
todolist for ALL levels · user gate for medium/complex before code · sync-before-branch · file
separation (no inline CSS/JS) · `php -l` every PHP file (in the verify gate) · SQL verified on the
real DB · review gate must pass · test tier mandatory (unless `--skip-test`) · completion updates
the todolist · migration reminder · **no AI/Claude mention in code or commits** · when editing
others' code add `// #NNNNN: Added/Modified by <Name> on <DATE> to <REASON>`.

## Guardrails
User-gated: medium/complex plans need approval before code. Verify-gate before advance
(`engine_verify` enforces it; `engine_advance` refuses unverified tasks). Scope edits to the
change + `agents/` + `.omp/plans/` memory. NO `git push` / PR / auto-commit unless asked. Stop on
a true blocker (missing credential, irreversible action) — never on ambiguity you can resolve.

## Model + context budget
Use the models in `~/.config/ckit/models.toml` (view/edit: `ckit harness model`); route per task
class — cheaper for trivial, stronger for review/debug. Near the context limit (auto-compacts at
50%) write a handoff into `agents/STATE.md` + `.omp/plans/<ticket>-<slug>/todolist.md` BEFORE it
fires so `--resume` picks up clean.

Begin: ground, classify + knowledge-lookup, plan (gate), then run the engine loop on `$ARGUMENTS`.

---
name: fix
disable-model-invocation: true
description: "Fix a bug end-to-end following the debugging flow. Handles bug analysis, root cause investigation, fix implementation, review, testing, and completion. Use when given a bug report or ticket with 'fix', 'error', 'broken', 'not working'."
user-invokable: true
---

# Fix Skill

> Fix a bug from report to verified resolution, following the debugging flow from primary workflow.

## Usage

```
/fix #20686                          -> Fix bug from PMS ticket
/fix #20686 --simple                 -> Force simple complexity
/fix #20686 --medium                 -> Force medium complexity
/fix #20686 --complex                -> Force complex complexity (debugger agent)
/fix #20686 --skip-test              -> Skip Phase 5 testing (not recommended)
/fix #20686 --resume                 -> Resume from last todolist checkpoint
/fix "Kanban count not updating"     -> Fix from inline description
```

## Workflow Overview

```
Step 1: Receive & Classify Bug     ->  Analyze report, assess bug complexity
Step 2: Load Skills                ->  Auto-detect relevant skills from bug context
Step 3: Investigate                ->  Locate root cause (Serena / debugger agent)
Step 4: Create TodoList            ->  Plan directory + todolist (ALL levels)
Step 5: Fix Implementation         ->  Apply fix with mandatory checks
Step 6: Code Review                ->  Security, performance, correctness
Step 7: Testing & Verification     ->  Verify fix + no regressions
Step 8: Completion                 ->  Update todolist, summarize, offer commit
Step 9: Error Pattern Capture      ->  Propose EP-NNN entry -> USER GATE -> append to error-patterns.md
```

## Step 1: Receive & Classify Bug

### Extract Bug Report

1. **From ticket number** (`#NNNNN`):
   - Use `pms_get_ticket` with raw numeric ID (NOT `17x` prefixed)
   - Read ticket title, description, and **check images** (`imagename` field)
   - If ticket has images, read them for visual reference of the bug
   - Extract: module(s), steps to reproduce, expected vs actual behavior

2. **From inline description** (quoted string):
   - Parse the description directly
   - Ask clarifying questions if ambiguous

### Assess Bug Complexity

| Level | Criteria | Flow |
|-------|----------|------|
| `simple` | Single file, obvious cause, clear fix | Step 1 -> 3(quick) -> 4(lite) -> 5 -> 6(built-in) -> 7(verify-only/E2E) -> 8 |
| `medium` | 2-5 files, needs call chain tracing | Step 1 -> 2 -> 3(Serena trace) -> 4 -> 5 -> 6(reviewer) -> 7(must-test/E2E) -> 8 |
| `complex` | Cross-module, unclear cause, intermittent | Step 1 -> 2 -> 3(debugger agent) -> 4 -> 5 -> 6(reviewer) -> 7(tester agent) -> 8 |

**Complexity can be overridden** with `--simple`, `--medium`, or `--complex` flags.

### Clarification Checklist

If missing info, ask user (use `AskUserQuestion`):
- Which module(s) affected?
- Steps to reproduce?
- When did it start happening? (recent change? always broken?)
- Any error messages or screenshots?
- Which environment? (dev/staging/production)

**Output:** 1-2 sentence root cause hypothesis before proceeding.

## Step 2: Load Skills

Auto-detect required skills from bug context:

| Bug Context | Skill to Load |
|-------------|---------------|
| UI rendering / layout broken | `view`, `ui` |
| AJAX endpoint returning error | `action` |
| Wrong data / missing records | `database` |
| Field not showing / wrong type | `field` |
| Migration failed / schema issue | `migration` |
| Wrong label / missing translation | `language` |
| Handler not firing / wrong trigger | `handler` |
| Cron not running / stuck queue | `cron` |
| Config not taking effect | `config` |
| API call failing / wrong response | `integration` |
| Report wrong data / SQL error | `report` |
| Notification not sent | `notification` |
| Export corrupted / wrong format | `export` |
| Permission denied unexpectedly | `error-handling` |

**How to load:**
- **Simple bugs:** Only read `SKILL.md`, skip `references/*.md`
- **Medium/Complex bugs:** Read `SKILL.md` + relevant `references/*.md`

## Step 3: Investigate Root Cause

### Simple Bug (single file, obvious cause)

1. Use Serena `search_for_pattern` to locate error message or relevant code
2. Use `find_symbol` with `include_body=True` to read the buggy code
3. Identify root cause directly
4. **Skip to Step 4** (lite todolist)

### Medium Bug (2-5 files, needs tracing)

1. Use Serena `get_symbols_overview` on suspected file(s)
2. Use `find_symbol` to locate the entry point (controller/view/action)
3. Use `find_referencing_symbols` to trace the call chain:
   - Controller -> Model -> Helper -> Database
   - View -> Template -> JavaScript -> AJAX -> Action
4. Read symbol bodies along the chain until root cause is found
5. Document the call chain for the fix plan

### Complex Bug (cross-module, unclear cause)

1. Spawn `debugger` agent with:
   - Bug description and reproduction steps
   - Suspected modules/files
   - Any error logs or screenshots
   - Work context: `<git root path>`
2. Read debugger report for root cause analysis
3. If debugger cannot identify root cause:
   - Try manual investigation with Serena
   - Check git blame for recent changes: `git log --oneline -20 -- <suspected-files>`
   - If still unclear, ask user for more context

### Investigation Shortcuts

| Symptom | Quick Check |
|---------|-------------|
| PHP error/blank page | `php -l <file>`, check error log |
| AJAX returning error | Find Action controller, check `process()` method |
| Data not saving | Trace `save()` call chain, check handler conflicts |
| UI not rendering | Check TPL syntax, JS console errors (Playwright) |
| SQL error | Extract query, run on real DB via MCP |
| Permission denied | Check `checkPermission()`, user role settings |
| Cron not executing | Check cron status table, handler file path |
| JS not working | Check controller name, `registerEvents()`, browser cache |

## Step 4: Create TodoList

**MANDATORY for ALL bug complexity levels.**

All plans live in `.claude/plans/<ticket-id>-<bug-slug>/` directory.

### Bug Slug Derivation

Generate directory name from bug context:
- **With ticket:** `#20686 "Kanban count not updating"` -> `20686-fix-kanban-activity-count`
- **Without ticket:** `"Export overlay won't close"` -> `fix-export-overlay-close`
- Format: `<ticket-id>-fix-<slug>`, lowercase, kebab-case, max 50 chars for slug part

### TodoList Template

```markdown
# Fix: <Bug Description> -- TodoList

**Date:** YYYY-MM-DD | **Ticket:** #XXXXX | **Complexity:** simple/medium/complex
**Status:** In progress

## Bug Summary
- **Module:** <affected module(s)>
- **Symptom:** <what user sees>
- **Root Cause:** <identified cause>
- **Fix Strategy:** <1-2 sentence approach>

## Tasks
- [ ] Investigate root cause
- [ ] Apply fix — <description>
- [ ] php -l syntax check
- [ ] Code review
- [ ] Verify fix (tier: verify-only / E2E / must-test)
- [ ] Regression check

## Files to modify
- path/to/file.php — <what changes>

## Test Results
_(filled in Step 7)_

## Completion Notes
_(filled in Step 8)_
```

### Medium/Complex: User Confirmation

- **Simple bugs:** No user gate — proceed directly to fix
- **Medium/Complex bugs:** Present root cause analysis and fix strategy to user
  - Use `AskUserQuestion`: "Root cause: X. Fix strategy: Y. Proceed?"
  - **NEVER code a complex fix without user confirmation**

## Step 5: Fix Implementation

### Apply the Fix

1. **Edit the minimum number of files** — fix the root cause, don't refactor surroundings
2. Follow `cloudgo-development-rules.md` conventions strictly
3. Add modification tracking comment:
   ```php
   // Modified by <Author> on YYYY-MM-DD to fix #XXXXX: <brief description>
   ```

### Mandatory Checks (after each file)

1. **`php -l <file>`** on every modified PHP file
2. **Method existence:** If calling parent/base methods, verify with Serena `find_symbol`
3. **File separation:** NO inline CSS/JS in PHP/TPL — if fix needs CSS/JS changes, use separate files
4. **SQL verification:** If fix involves SQL changes, test on real DB via MCP mysql tools
5. **No side effects:** Verify fix doesn't break related functionality using `find_referencing_symbols`

### Fix Principles

| Principle | Detail |
|-----------|--------|
| **Minimal change** | Fix the bug, nothing else. No refactoring, no "improvements" |
| **Root cause** | Fix the cause, not the symptom. Don't add workarounds |
| **Backward compatible** | Existing data and integrations must continue working |
| **No new dependencies** | Avoid adding new libraries/modules for a bug fix |
| **Match codebase style** | Follow surrounding code patterns exactly |

## Step 6: Code Review

### Simple Bug — Built-in Review

Quick self-check (no code-reviewer agent):
- [ ] `php -l` passes
- [ ] No inline CSS/JS added
- [ ] Type casting on `$request->get()` values
- [ ] Prepared statements for any SQL
- [ ] Fix addresses root cause (not symptom)
- [ ] No unrelated changes included

### Medium/Complex Bug — Full Review

Spawn `code-reviewer` agent on all modified files with the Phase 8 checklist:

**Runtime correctness:**
- [ ] Method/function exists (Serena `find_symbol`)
- [ ] Class exists for `extends`/`new`
- [ ] DB columns match schema (`DESCRIBE table` via MCP)

**Security:**
- [ ] `pquery()` with `?` params
- [ ] `.text()` not `.html()` for dynamic JS
- [ ] Type casting on request values
- [ ] `checkPermission()` on controllers

**Performance:**
- [ ] No N+1 queries introduced
- [ ] No DB queries inside loops

**If review fails → fix issues and re-review. Do NOT proceed until review passes.**

## Step 7: Testing & Verification

**NEVER skip** (unless `--skip-test` flag).

### Determine Test Tier

```
Fixed Action/Model/Helper logic?   -> must-test (unit tests via tester agent)
Fixed Report/SQL query?            -> verify-sql (run on real DB)
Fixed DDL/language/config/CSS?     -> verify-only (php -l + review sufficient)
Fixed UI (TPL/JS)?                 -> E2E (Playwright MCP)
```

### Verification Checklist (ALL tiers)

1. **Fix works:** The original bug is resolved
2. **No regressions:** Related functionality still works
3. **Edge cases:** Test with empty data, null values, special characters

### E2E Verification (when UI involved)

Use MCP Playwright tools:
1. `browser_navigate` to the affected page
2. `browser_snapshot` to verify fix renders correctly
3. Reproduce original bug steps → confirm bug no longer occurs
4. `browser_console_messages` to check for JS errors
5. `browser_screenshot` for visual confirmation

### must-test Verification (Action/Model/Helper)

Spawn `tester` agent:
1. Test the fixed code path with valid input
2. Test with the input that originally caused the bug
3. Test edge cases (empty, null, special chars, Vietnamese text)
4. Test permission scenarios if relevant

### verify-sql Verification

1. Run the fixed SQL on real DB via MCP mysql tools
2. Verify correct results with sample data
3. Compare output before/after fix if possible

### Test Failure Recovery

If tests fail:
1. Analyze failure — is it the fix or a pre-existing issue?
2. If fix-related → go back to Step 5, fix, loop through Step 6 → Step 7
3. If pre-existing → document and proceed (don't scope-creep)

## Step 8: Completion

**MANDATORY — every fix ends here.**

1. **Update todolist** `.claude/plans/<ticket-id>-<bug-slug>/todolist.md`:
   - Status: `In progress` -> `Completed`
   - Check off `[x]` all done tasks
   - Fill `## Test Results` with: tier used, pass/fail, details
   - Fill `## Completion Notes` with: root cause summary, fix applied, files changed
2. Summarize to user:
   - Root cause (1 sentence)
   - Fix applied (1 sentence)
   - Files changed (list)
   - Test results (pass/fail)
3. **Migration reminder** if any migrations were created
4. Offer to commit: "Run `/commit #XXXXX` to commit this fix?"

## Step 9: Error Pattern Capture (USER GATE — MANDATORY)

> **NEVER write to `.omp/rules/error-patterns.md` without explicit user approval.**

### When to trigger

Sau khi fix xong, đánh giá xem bug này có đáng ghi `EP-NNN` không. Trigger nếu:

- **Bug do anti-pattern chung** dễ tái phát ở module khác (vd typo, copy-paste sai, race condition pattern)
- **Bug do framework misuse** (vd dùng sai permission method, sai event hook)
- **Bug khó phát hiện qua syntax/lint** (cần human/AI nhìn ngữ nghĩa)
- **Fix có rule rõ ràng** để phòng tránh lần sau (1-2 câu actionable)

Skip nếu: bug đơn lẻ do business logic riêng module, fix typo nhỏ chỉ ảnh hưởng 1 chỗ duy nhất, bug đặc thù không generalize được.

### Steps (2 tầng: auto-draft PENDING — không hỏi · promote CANONICAL — user duyệt)

1. **Auto-draft vào pending (KHÔNG cần hỏi):** trigger khớp → append vào `.omp/rules/error-patterns-pending.md` (tạo nếu chưa có), ID tạm `PEND-<ticket>`:
   ```
   ## PEND-<ticket>: <Tên ngắn>
   **Ticket / MR:** #NNNNN / !NNN
   **Date:** YYYY-MM-DD
   **Module / Layer:** <...>
   **Symptom:** <hiện tượng>
   **Root cause:** <kỹ thuật>
   **Rule:** <1-2 câu phòng tránh>
   **Trigger keywords (review):** <substring máy grep được trong code lỗi — phân cách ' · '>
   ```
   Báo user 1 dòng: "Đã draft PEND-<ticket>". Trigger keywords = chuỗi nguyên văn trong code (hook grep substring), không phải mô tả trừu tượng.

2. **Promote (USER GATE):** cuối flow, show entry pending qua `AskUserQuestion`: `Promote` / `Sửa rồi promote` / `Để ở pending`. Approved → gán `EP-NNN` (max+1), append `.omp/rules/error-patterns.md`, xoá khỏi pending. Từ đó hook `posttooluse-error-patterns.sh` enforce tự động.

3. **KHÔNG ghi thẳng `error-patterns.md` khi chưa approve** — pending thì tự do.

## Resume Mode (`--resume`)

When `--resume` is passed:

1. Search `.claude/plans/` for the ticket number in existing todolists
2. Read the todolist → find first unchecked `[ ]` task
3. Resume from that step
4. Continue normal flow

## Rules

| Rule | Enforced |
|------|----------|
| Ticket or description required | REJECT if neither provided |
| Skills auto-detected | Loaded from bug context |
| TodoList mandatory | ALL bug complexity levels |
| User gate for medium/complex | Confirm root cause + strategy before coding |
| Minimal fix | Fix the bug only — no refactoring, no extras |
| PHP syntax check | `php -l` after every PHP file |
| SQL verification | Test all SQL changes on real DB |
| Code review gate | Must pass before testing |
| Test phase mandatory | Never skip (unless `--skip-test`) |
| Completion mandatory | Update todolist at end |
| Migration reminder | Alert user if migrations created |
| No AI in commits | Never mention Claude in code/commits |
| Backward compatible | Fix must not break existing functionality |
| Error pattern user gate | Step 9 — NEVER write to `.omp/rules/error-patterns.md` without explicit user approval |

## Error Recovery

| Scenario | Action |
|----------|--------|
| Cannot reproduce bug | Ask user for more details, check environment differences |
| Root cause unclear after investigation | Spawn `debugger` agent (escalate to complex flow) |
| Fix breaks other functionality | Revert fix, re-investigate, try alternative approach |
| Code review fails | Fix issues, re-review, do NOT proceed |
| Tests fail | Analyze → fix → loop from Step 6 |
| Subagent fails | Retry once → orchestrator takes over |

## References

- [Fix Checklist](references/fix-checklist.md) — Compact runtime checklist for bug fixes

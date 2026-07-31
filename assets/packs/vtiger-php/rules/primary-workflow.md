# Primary Workflow — Automated Feature Development

**IMPORTANT:** Analyze the skills catalog and activate skills needed for each phase.
**IMPORTANT:** Ensure token efficiency while maintaining high quality.

---

## Phase 1: Receive & Clarify Requirements

- Read the request, analyze scope and complexity
- Classify: `feature` | `bugfix` | `config` | `report` | `integration`
  - `bugfix` → assess bug complexity (simple/medium/complex) → jump to **Debugging Flow** below
- **Assess complexity level:**
  - `simple` (≤ 3 files, single concern) → **Lite mode**: Phase 1 → 3 (Quick Ref only) → 4 (lite todolist) → 7 → 8 → 9 → 10. Skip Phases 2, 5, 6, 7.5.
  - `medium` (≤ 10 files, 2-3 concerns) → Skip Phase 4 Step 1 (planner), Phase 6 if no UI, Phase 7.5 (merge into 8).
  - `complex` (> 10 files, 3+ concerns) → Full workflow with planner + subagents + todolist
- Use `AskUserQuestion` if missing info — checklist: modules, UI needed, API/AJAX, DB changes, language strings, cron jobs
- Propose approach in 1-2 sentences before proceeding

## Phase 1.5: Knowledge Base Lookup (MANDATORY)

1. Đọc `docs/knowledge/INDEX.md` → tra module liên quan
2. Đọc knowledge file → hiểu nghiệp vụ, status transitions, handlers, gotchas
3. Cross-module task → đọc flow file trong `docs/knowledge/flows/`
4. Chưa có knowledge file → tiếp tục, cập nhật sau nếu phát hiện nghiệp vụ mới

**Áp dụng cho TẤT CẢ complexity levels.**

## Phase 2: Load Skills & Project Conventions

Analyze requirements → activate matching skills automatically:

| Requirement | Skill |
|-------------|-------|
| Page / modal / template | `view` |
| AJAX endpoint / JSON | `action` |
| DB queries / tables | `database` |
| Fields / picklists / UITypes | `field` |
| Migration | `migration` |
| Language labels | `language` |
| Event handlers | `handler` |
| Cron / queue | `cron` |
| Config / settings | `config` |
| External API / webhooks | `integration` |
| Reports / charts | `report` |
| Email / SMS / push | `notification` |
| Export CSV / Excel / PDF | `export` |
| Call center / telephony | `callcenter` |
| Module structure / MVC | `module` |
| Buttons / modals / forms | `ui` |
| Error handling / logging | `error-handling` |
| Tests | `testing` |
| Inventory (SO, Invoice) | `inventory` |

**Activate:** Read skill's `SKILL.md` + `references/*.md`.
**Lite mode:** Only Quick Reference (§10) + skill's `SKILL.md`.
**Full mode:** Full `cloudgo-development-rules.md` + all skill references.

## Phase 3: Research & Context Gathering

- Use **Serena** tools: `get_symbols_overview`, `find_symbol`, `find_referencing_symbols`, `search_for_pattern`
- Spawn `researcher` agents for new APIs/libraries or complex architecture decisions
- Use `Explore` agent or `Grep`/`Glob` as fallback
- Skip if task is simple and patterns are known

## Phase 4: Plan & Create TodoList

**MANDATORY for ALL complexity levels.** Plans live in `.claude/plans/<ticket-id>-<feature-name>/`.

### TodoList Template (all levels)

```markdown
# <Feature Name> — TodoList

**Date:** YYYY-MM-DD | **Ticket:** #XXXXX | **Complexity:** simple/medium/complex
**Status:** In progress

## Tasks
- [ ] Task 1 — description
- [ ] Code review
- [ ] Verify/test (tier: verify-only / verify-sql / must-test / E2E)

## Files to create/modify
- path/to/file.php

## Dependencies (medium/complex only)
- Task 2 depends on Task 1

## Test Results
_(filled in Phase 9)_

## Completion Notes
_(filled in Phase 10)_
```

**Simple:** Lite todolist directly, no plan.md, no user review gate.
**Medium:** Create plan.md → user reviews → create todolist. No planner agent.
**Complex:** Spawn `planner` agent → plan.md → user reviews → todolist → `TaskCreate` for each task with `addBlockedBy`.

## Phase 5: Gateway — User Confirmation

- After todolist created, `AskUserQuestion` for final confirmation before coding
- **DO NOT write code until user confirms**

## Phase 6: UI/UX Layout Confirmation (if UI involved)

- Simple/medium: text/ASCII layout description → confirm → code
- Complex: spawn `ui-ux-designer` agent for mockup
- VTiger legacy views: prefer text description over full mockup

## Phase 7: Implementation

- Simple/medium: implement directly. Complex: spawn `fullstack-developer` agents with strict file ownership.
- `TaskUpdate` → `in_progress` when starting, `completed` when done
- File separation per `cloudgo-development-rules.md` File Separation Rules section
- **After each PHP file:** Run `php -l <file>`
- **Method existence verification:** Before calling parent/base class methods, verify with Serena `find_symbol`. Do NOT trust skill references blindly.
- **SQL verification:** Run each SQL query on real DB via MCP tools before code review
- **Subagent error recovery:** retry once → if fails → orchestrator takes over

## Phase 7.5: Code Simplification (complex only)

Spawn `code-simplifier`: remove dead code, extract repeated logic, simplify nesting, split >30-line methods.

## Phase 8: Code Review per Task

Spawn `code-reviewer` per task. If review fails → fix and re-review. DO NOT proceed if review fails.

**Review checklist:**

**Runtime correctness** (CRITICAL — `php -l` cannot catch):
- Method/function exists (verify with Serena `find_symbol`)
- Class exists (every `extends`, `implements`, `new`)
- Config keys exist, DB columns exist, require/include paths exist

**Syntax & structure:** `php -l` clean, no inline CSS/JS, class naming convention, file headers

**Security (OWASP):**
- SQL Injection: ALL queries use `pquery()` with `?` params
- XSS: `.text()` not `.html()` for dynamic content; `vtlib_purify()` for user input
- CSRF: `$request->validateWriteAccess()` on write Actions
- Auth: `checkPermission()` enforced; admin pages check `isAdminUser()`
- Type casting on ALL `$request->get()` values
- No hardcoded secrets; no user-controlled `include`/`require`

**Performance:** no N+1 queries, guard expensive ops, use indexes, LIMIT/pagination, caching

**Code cleanliness:** no dead code, DRY, methods ≤ 30 lines, classes ≤ 200 lines, early returns

## Phase 9: Test Case Generation & Execution

**MANDATORY — NEVER skip.**

| Tier | Applies to | Actions |
|------|-----------|--------|
| **must-test** | Action, Model, Helper logic | `tester` agent → unit + security tests |
| **verify-sql** | Reports, DB queries, migrations with SELECT | Verify SQL on real DB + `php -l` |
| **verify-only** | Migrations (DDL), language strings, config, CSS/JS | `php -l` + code review sufficient |

**must-test:** Load `testing` skill → test cases (TC-01...) → standalone PHP scripts in `test/test-{component}-{module}-{feature}.php` → include security tests (Vietnamese text, XSS, SQL injection).

**Frontend E2E (when UI involved):** Use MCP Playwright → `browser_navigate`, `browser_snapshot`, `browser_screenshot`, `browser_click`/`browser_type`, `browser_console_messages`. Test: page load, form submit, validation, AJAX, permissions.

**Rules:** No mocks. Backend = real DB. E2E = real URLs. No PHPUnit — standalone PHP scripts only.

## Phase 10: Completion & Documentation

1. `TaskUpdate` all tasks → `completed` (if used)
2. **Update todolist** `.claude/plans/<ticket-id>-<feature-name>/todolist.md`: status → Completed, check off tasks, fill Test Results + Completion Notes
3. Summarize to user
4. If docs impact → spawn `docs-manager`
5. **If migrations created:** remind user to run pending migrations
6. **If fails mid-way:** revert partial changes, document in todolist, notify user

## Rollback Strategy

1. Stop immediately on failure
2. Fixable → fix and continue. Fundamental → `git checkout -- <files>` + delete created files
3. Update todolist with failure reason
4. No partial implementations in codebase

---

## Rules Summary

| Rule | Detail |
|------|--------|
| File ownership | No two agents edit same file |
| Test tiers | must-test / verify-sql / verify-only. NEVER skip Phase 9 |
| Skills auto-detect | Analyze requirements → load matching skills |
| Todolist MANDATORY | ALL tasks create + update todolist.md |
| Syntax check | `php -l` after every PHP file |
| SQL verification | Test SQL on real DB before code review |
| File separation | NO inline CSS/JS — per `cloudgo-development-rules.md` |
| User gate | Never code without user confirmation (Phase 5) |
| Knowledge lookup | Read `docs/knowledge/INDEX.md` + module file BEFORE coding |
| Review gate | Code review must pass before next task |
| Rollback | Revert partial changes; no partial code in codebase |

---

## Debugging Flow

**ALL bugs: knowledge lookup → todolist → fix → test → update todolist.**

### Step 0: Knowledge Base Lookup (ALL bugs)
Đọc `docs/knowledge/INDEX.md` → knowledge file → flow file (if cross-module). Purpose: avoid fixing wrong due to business logic misunderstanding.

### Simple bug (single file, obvious cause)
1. Knowledge lookup → 2. Lite todolist → 3. Locate with Serena → 4. Fix + `php -l` → 5. Verify (Phase 9) → 6. Update todolist

### Medium bug (2-5 files, needs investigation)
1. Knowledge lookup → 2. Todolist → 3. Load skills → 4. Trace with Serena → 5. Fix + `php -l` → 6. Code review → 7. Testing → 8. Update todolist

### Complex bug (cross-module, unclear cause)
1. Knowledge lookup (esp. flow files) → 2. Todolist → 3. Spawn `debugger` agent → 4. Load skills → 5. Implement fix → 6. Spawn `tester` → 7. If fails, repeat 6 → 8. Update todolist

---

## Visual Explanations

When topic has 3+ interacting components or user asks "explain"/"visualize":
- `/preview --explain <topic>` | `/preview --diagram <topic>` | `/preview --slides <topic>` | `/preview --ascii <topic>`

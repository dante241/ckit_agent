---
name: cook
disable-model-invocation: true
description: "Code a feature end-to-end following the primary workflow. Handles requirements analysis, skill loading, research, planning, implementation, review, testing, and completion. Use when given a ticket or feature request."
---

# Cook Skill

> Code a feature from requirements to completion, following the primary workflow phases.

## Usage

```
/cook #16873                          -> Cook feature from PMS ticket
/cook #16873 --simple                 -> Force simple complexity (skip planner)
/cook #16873 --complex                -> Force complex complexity (full planner)
/cook #16873 --skip-test              -> Skip Phase 9 testing (not recommended)
/cook #16873 --resume                 -> Resume from last todolist checkpoint
/cook "Add export button to ListView" -> Cook from inline description
```

## Workflow Overview

This skill orchestrates the **Primary Workflow** (`.omp/rules/primary-workflow.md`) automatically:

```
Phase 1: Receive & Classify   ->  Analyze request, assess complexity
Phase 1.5: Knowledge Lookup   ->  Read docs/knowledge/ INDEX + module + flow files
Phase 2: Load Skills          ->  Auto-detect and activate relevant skills
Phase 3: Research             ->  Codebase analysis with Serena + agents
Phase 4: Plan & TodoList      ->  Create plan (medium/complex) + todolist (all)
Phase 5: User Gate            ->  Confirm before coding
Phase 6: UI Confirmation      ->  Layout review (if UI involved)
Phase 6.5: Sync & Branch      ->  checkout master, pull latest, create feature branch
Phase 7: Implementation       ->  Code with file separation rules
Phase 8: Code Review          ->  Security, performance, correctness
Phase 9: Testing              ->  Test tier: must-test / verify-sql / verify-only
Phase 10: Completion          ->  Update todolist, summarize
Phase 10.5: Knowledge Update  ->  Propose updates -> USER GATE -> write to docs/knowledge/
Phase 10.6: Error Pattern     ->  Propose EP-NNN entry -> USER GATE -> append to error-patterns.md
```

## Phase 1: Receive & Classify

### Extract Requirements

1. **From ticket number** (`#NNNNN`):
   - Use `pms_get_ticket` with raw numeric ID (NOT `17x` prefixed)
   - Read ticket title, description, **and also `steps_to_reproduce`, `expected_result`, `actual_result`** (bugfix tickets), **check images** (`imagename` field)
   - If ticket has images, read them for UI reference
   - Extract: module(s), expected behavior, acceptance criteria

2. **From inline description** (quoted string):
   - Parse the description directly
   - Ask clarifying questions if ambiguous

### Classify Request

| Type | Indicators |
|------|-----------|
| `feature` | New functionality, new page, new field |
| `bugfix` | "fix", "error", "broken", "not working" -> jump to **Debugging Flow** |
| `config` | Settings, preferences, toggles |
| `report` | Charts, data aggregation, export |
| `integration` | External API, webhook, sync |

### Assess Complexity

| Level | Criteria | Workflow |
|-------|----------|----------|
| `simple` | ≤ 3 files, single concern | Lite mode: Phase 1 -> 3(quick) -> 4(lite) -> 6.5 -> 7 -> 8 -> 9 -> 10 |
| `medium` | ≤ 10 files, 2-3 concerns | Skip planner agent, skip Phase 6 if no UI |
| `complex` | > 10 files, 3+ concerns | Full workflow with planner + subagents |

**Complexity can be overridden** with `--simple` or `--complex` flags.

### Clarification Checklist

If missing info, ask user (use `AskUserQuestion`):
- Which module(s)?
- Need UI? (View + TPL + CSS + JS)
- Need AJAX endpoint? (Action controller)
- Need new fields or DB changes? (Migration)
- Need language strings? (en_us, vn_vn)
- Need cron/background jobs?

**Output:** 1-2 sentence approach proposal before proceeding.

## Phase 1.5: Knowledge Base Lookup (MANDATORY)

> **Áp dụng cho TẤT CẢ complexity levels.** Mục đích: hiểu nghiệp vụ trước khi code, tránh fix/build sai do hiểu nhầm logic.

### Steps

1. **Read `docs/knowledge/INDEX.md`** -> tra module liên quan tới ticket
2. **Read knowledge file** `docs/knowledge/modules/<Module>.md` (nếu có) -> hiểu:
   - Nghiệp vụ chính
   - Status transitions / state machine
   - Event handlers đang đăng ký
   - Gotchas / edge cases đã document
3. **Cross-module task** -> đọc thêm `docs/knowledge/flows/<flow>.md`
4. **Không có knowledge file** -> tiếp tục, mark trong todolist để xem xét tạo mới ở Phase 10.5

### Output

Tóm tắt 1-3 dòng những điểm nghiệp vụ quan trọng đã đọc, hoặc note "Chưa có knowledge file cho `<Module>`" nếu trống.

## Phase 2: Load Skills

Auto-detect required skills from requirements and load them:

| Requirement | Skill to Load |
|-------------|---------------|
| New page / modal / template | `view` |
| AJAX endpoint / JSON response | `action` |
| DB queries / table changes | `database` |
| New fields / picklists / UITypes | `field` |
| Migration file | `migration` |
| Language labels | `language` |
| Event handlers (aftersave, etc.) | `handler` |
| Cron jobs / queue processing | `cron` |
| Config / settings pages | `config` |
| External API / webhooks | `integration` |
| Reports / charts | `report` |
| Email / SMS / push notifications | `notification` |
| Export CSV / Excel / PDF | `export` |
| Call center / telephony | `callcenter` |
| Module structure / MVC | `module` |
| Buttons / modals / form validation | `ui` |
| Error handling / logging | `error-handling` |
| Test generation | `testing` |
| Inventory modules (SO, Invoice) | `inventory` |

**How to load:**
- Read `SKILL.md` of each detected skill
- **Lite mode (simple):** Only read `SKILL.md`, skip `references/*.md`
- **Full mode (medium/complex):** Read `SKILL.md` + all `references/*.md`

## Phase 3: Research & Context

### Token-Efficient Research (Serena-first)

1. `get_symbols_overview` -> understand file structure
2. `find_symbol` -> locate classes, methods by name path
3. `find_referencing_symbols` -> trace dependencies
4. `search_for_pattern` -> flexible regex search

### When to Spawn Agents

- **New external API/library** -> spawn `researcher` agent
- **Unfamiliar module** -> spawn `Explore` agent
- **Complex architecture decision** -> spawn `researcher` for comparison

### Skip Conditions

- Simple task + patterns already known from loaded skills -> skip research
- Task is a straightforward CRUD addition -> minimal research

## Phase 4: Plan & TodoList

**MANDATORY for ALL complexity levels.**

All plans live in `.claude/plans/<ticket-id>-<feature-name>/` directory (or `.claude/plans/<feature-slug>/` if no ticket). **`.claude/plans/**` is auto-approved for Edit/Write** — create/update `plan.md`, `todolist.md`, and any working notes there freely without prompting the user.

### Feature Name Derivation

Generate directory name from:
- **With ticket** (preferred): `#21027 "Adjust CRM field label"` -> `21027-import-adjust-crm-field-label`
- **Without ticket**: `"Add export button"` -> `add-export-button`
- Format: `<ticket-id>-<slug>`, lowercase, kebab-case, max 50 chars for slug part

### Simple Tasks -> Lite TodoList

Create todolist directly, no plan.md, no user gate:

```markdown
# <Feature Name> -- TodoList

**Date:** YYYY-MM-DD | **Ticket:** #XXXXX | **Complexity:** simple
**Status:** In progress

## Tasks
- [ ] Task 1 -- description
- [ ] Task 2 -- description
- [ ] Code review
- [ ] Verify/test (tier: verify-only / E2E / must-test)

## Files to modify
- path/to/file.php

## Test Results
_(filled in Phase 9)_

## Completion Notes
_(filled in Phase 10)_
```

### Medium Tasks -> Plan + TodoList

1. Create `plan.md` directly (no planner agent)
2. Present plan to user -> **STOP and wait for approval**
3. Create `todolist.md` after approval

### Complex Tasks -> Full Planner

1. Spawn `planner` agent -> outputs `plan.md`
2. Present plan to user -> **STOP and wait for approval**
3. Create `todolist.md` after approval
4. Build runtime tasks with `TaskCreate` (set dependencies)

### Approach Declaration (MANDATORY trong plan.md — medium/complex; simple thì rút gọn 3 dòng đầu vào todolist)

Mọi plan mở đầu bằng khối này — khai CÁCH LÀM trước khi khai VIỆC LÀM, để user bắt sai kiến trúc trong 10 giây thay vì sau khi code xong:

```markdown
## Approach Declaration
- **Loại thay đổi:** [new field / new view / new AJAX endpoint / new report / fix logic / ...]
- **Pattern chọn:** [vd: HandleAjax mode=xxx — KHÔNG tạo Action rời] (đối chiếu bảng View Base Classes + "Prefer HandleAjax")
- **Base class:** [vd: CustomView_Base_View / Vtiger_List_View / VTEventHandler]
- **Schema/field:** [không đụng / BlocksAndFieldsRegister + quick_repair / migration CPMigration]
- **Exemplar đã đọc:** [path file cùng loại trong repo đã mở để bắt chước — BẮT BUỘC trước khi tạo file mới]
- **Files tạo/sửa:** [danh sách path]
```

Quy tắc:
- **Exemplar:** chưa đọc file mẫu cùng loại → chưa được viết code. Skill domain có mục Exemplars; không có thì `codegraph_explore` tìm 2 file cùng loại gần nhất trong repo. CẤM viết theo trí nhớ VTiger open-source.
- Chọn pattern KHÁC convention mặc định (vd Action rời thay vì HandleAjax) → ghi lý do 1 câu.
- Declaration thiếu/sai phát hiện ở Phase 8 review = finding.

## Phase 5: User Gate

- **Simple:** No gate needed (lite todolist is self-approved)
- **Medium/Complex:** Plan was reviewed in Phase 4. Use `AskUserQuestion` for final "Start coding?" confirmation
- **NEVER write code until user confirms** (medium/complex)

## Phase 6: UI Confirmation (if applicable)

- **No UI involved** -> skip entirely
- **Simple/medium UI** -> describe layout in text/ASCII -> confirm with user
- **Complex UI** -> spawn `ui-ux-designer` agent for mockup

## Phase 6.5: Sync & Branch (MANDATORY, before any code write)

> Same rule as the `commit` skill: never write code on a stale local branch.

1. Run `git status` — if there are unrelated uncommitted changes already in the tree, stop and ask the user (do not silently carry them onto master).
2. Determine current branch (`git rev-parse --abbrev-ref HEAD`).
3. **If on `master` or `dev`:**
   - `git checkout master`
   - `git pull origin master`
   - Derive branch name `<type>/#<ticket>-<slug>` using the same slug/type rules as `commit` skill Step 2 (`feature/`, `bug/`, `hotfix/`, `refactor/`)
   - Confirm branch name with user via `AskUserQuestion`
   - `git checkout -b <branch-name>` — created from the freshly-pulled master
4. **If already on a feature/bug branch** (e.g. resumed via `--resume`) -> stay on it, skip sync (do NOT switch to master mid-feature; risks merge conflicts).
5. Proceed to Phase 7 only after the branch is confirmed.

## Phase 7: Implementation

### Execution Strategy

| Complexity | Strategy |
|-----------|---------|
| Simple/Medium | Implement directly, no subagents |
| Complex | Spawn `fullstack-developer` agents with strict file ownership |

### Mandatory Checks (after each file)

1. **`php -l <file>`** on every PHP file
2. **Method existence**: Verify parent/base class methods with Serena `find_symbol`
3. **File separation**: NO inline CSS/JS in PHP/TPL
4. **SQL verification**: Test queries on real DB via MCP mysql tools

### File Location Rules

| File Type | Location |
|-----------|----------|
| CSS | `modules/<Module>/resources/<View>.css` |
| JS (custom views) | `modules/<Module>/resources/<View>.js` |
| JS (core views) | `layouts/v7/modules/<Module>/resources/<View>.js` |
| TPL (custom) | `modules/<Module>/tpls/<View>.tpl` |
| TPL (core) | `layouts/v7/modules/<Module>/<View>.tpl` |

## Phase 8: Code Review

Spawn `code-reviewer` agent on all modified files. Review must pass before proceeding.

### Review Checklist (abbreviated)

- **Runtime correctness:** Method/class exists, config keys valid, DB columns match schema
- **Syntax:** `php -l`, no inline CSS/JS, naming conventions, file headers
- **Security (OWASP):** Prepared statements, XSS prevention, CSRF, type casting, no secrets
- **Performance:** No N+1 queries, guard expensive ops, use indexes, LIMIT/pagination
- **Cleanliness:** No dead code, DRY, early returns, methods ≤ 30 lines

## Phase 9: Testing

**NEVER skip** (unless `--skip-test` flag).

### Determine Test Tier

| Tier | Applies to | Actions |
|------|-----------|---------|
| `must-test` | Action controllers, Model logic, Helpers | Spawn `tester` agent -> unit + security tests |
| `verify-sql` | Reports, DB queries, migrations with SELECT | Verify SQL on real DB + `php -l` |
| `verify-only` | DDL migrations, language strings, config, CSS/JS | `php -l` + code review is sufficient |

### Frontend E2E (when UI involved)

Use MCP **chrome-devtools** tools (auto-approved — `mcp__chrome-devtools__*` whitelisted; reuses the user's logged-in browser session, so no re-login needed):
1. `list_pages` → `select_page` / `navigate_page` to the page URL
2. `take_snapshot` to read the a11y tree + element `uid`s; `take_screenshot` for visual confirmation
3. `click` / `fill` / `fill_form` to drive interactions (use the `uid` from the snapshot)
4. `evaluate_script` to assert state directly (e.g. read JS controller flags, toggle a threshold, spy on `app.helper` notifications) — most reliable for VTiger select2/AJAX widgets where the visible element is hidden
5. Reuse the live session for gated pages (Settings, admin) — the user logs in once, no credentials handled by the agent

If chrome-devtools is unavailable, fall back to MCP Playwright (`browser_navigate`, `browser_snapshot`, `browser_click`, `browser_evaluate`).
4. `browser_console_messages` to check JS errors

## Phase 10: Completion

**MANDATORY -- every cook ends here.**

1. Update runtime tasks -> `completed` (if TaskCreate was used)
2. **Update todolist** `.claude/plans/<ticket-id>-<feature-name>/todolist.md`:
   - Status: `In progress` -> `Completed`
   - Check off `[x]` all done tasks
   - Fill `## Test Results` with tier, pass/fail, details
   - Fill `## Completion Notes` with fixes applied, final file list
3. Summarize to user: what was done, files changed, warnings
4. **Migration reminder** if any migrations were created
5. Offer to commit: "Run `/commit #XXXXX` to commit these changes?"

## Phase 10.5: Knowledge Update (USER GATE — MANDATORY)

> **NEVER write to `docs/knowledge/` without explicit user approval.**

### When to Trigger

Sau khi feature/bugfix hoàn tất, đánh giá xem có nên cập nhật knowledge base không. Trigger nếu:

- **Nghiệp vụ mới** vừa được build (status mới, flow mới, rule mới)
- **Logic cũ thay đổi** (status transition đổi, handler thêm/sửa, validation đổi)
- **Discovery quan trọng** trong quá trình code (gotcha, edge case, hidden coupling)
- **Cross-module flow** mới hoặc sửa
- **Module chưa có knowledge file** mà ticket vừa code đụng vào
- **User đã CHỐT một quyết định cách làm** trong phiên (chọn pattern, từ chối đề xuất, ranh giới scope) → đề xuất entry `D-NNN` vào `agents/DECISIONS.md` (cùng user-gate). Quyết định đã ghi = phiên sau không hỏi lại / không làm ngược.

Skip nếu: chỉ thay label/CSS, fix typo, refactor không đổi nghiệp vụ.

### Steps

1. **Draft proposal** — chuẩn bị danh sách thay đổi cụ thể:

   ```
   File: docs/knowledge/modules/<Module>.md
   Action: CREATE / UPDATE / APPEND
   Sections:
     - <Section>: <tóm tắt nội dung sẽ thêm/sửa>
   Why: <lý do — link tới ticket, commit, hoặc discovery>
   ```

   Lặp cho mỗi file knowledge cần đụng (`INDEX.md`, `modules/*.md`, `flows/*.md`).

2. **Show proposal to user** — dùng `AskUserQuestion`:

   - Question: "Cập nhật knowledge base với các thay đổi sau?"
   - Hiển thị FULL nội dung sẽ ghi (không tóm tắt) cho user review
   - Options: `Approve all` / `Edit before save` / `Skip` / `Approve partial (specify)`

3. **Wait for user response.** **TUYỆT ĐỐI KHÔNG ghi file trước khi user duyệt.**

4. **Apply approved changes:**
   - User chọn `Approve all` -> Write/Edit các file knowledge
   - User chọn `Edit before save` -> hỏi sửa gì -> show lại -> chờ duyệt
   - User chọn `Skip` -> không ghi gì
   - User chọn `Approve partial` -> chỉ ghi những phần được duyệt

5. **Update `INDEX.md`** nếu tạo knowledge file mới hoặc đổi tiêu đề mục.

6. **Confirm to user** sau khi ghi xong, list file đã update.

### Output Template (proposal)

```markdown
## Knowledge update proposal

### File 1: docs/knowledge/modules/CPCampaign.md
**Action:** UPDATE
**Section:** "Status transitions"
**Change:**
  Thêm transition: `Draft -> Auto-approved` khi `auto_approve = 1`
  (Reference: ticket #21766)

### File 2: docs/knowledge/INDEX.md
**Action:** UPDATE
**Change:** Add "Auto-approve" keyword to CPCampaign module entry

---
Approve all? Edit? Skip?
```

## Phase 10.6: Error Pattern Capture (USER GATE — MANDATORY)

> **NEVER write to `.omp/rules/error-patterns.md` without explicit user approval.**

### When to Trigger

Áp dụng chủ yếu cho `bugfix` flow. Feature flow: chỉ trigger nếu trong quá trình code phát hiện anti-pattern đáng ghi. Trigger nếu:

- **Bug/lỗi do anti-pattern chung** dễ tái phát ở module khác (typo, copy-paste sai, race condition pattern)
- **Bug do framework misuse** (sai permission method, sai event hook, sai response shape)
- **Bug khó phát hiện qua syntax/lint** (cần ngữ nghĩa)
- **Fix có rule rõ ràng** để phòng tránh (1-2 câu actionable)

Skip nếu: feature thuần không có bug, fix typo nhỏ chỉ 1 chỗ duy nhất, bug đặc thù business logic không generalize được.

### Steps (2 tầng: auto-draft PENDING — không hỏi · promote CANONICAL — user duyệt)

1. **Auto-draft vào pending (KHÔNG cần hỏi user):** khi trigger khớp, append entry vào `.omp/rules/error-patterns-pending.md` (tạo file nếu chưa có) — format giống EP chuẩn nhưng ID tạm `PEND-<ticket>`:
   ```
   ## PEND-<ticket>: <Tên ngắn>
   **Ticket / MR:** #NNNNN / !NNN
   **Date:** YYYY-MM-DD
   **Module / Layer:** <...>
   **Symptom:** <hiện tượng>
   **Root cause:** <kỹ thuật>
   **Rule:** <1-2 câu phòng tránh>
   **Trigger keywords (review):** <substring máy grep được trong code — phân cách ' · '>
   ```
   Ghi xong báo user 1 dòng: "Đã draft PEND-<ticket> vào error-patterns-pending.md".
   Trigger keywords phải là **chuỗi xuất hiện nguyên văn trong code lỗi** (hook `posttooluse-error-patterns.sh` grep substring) — không viết mô tả trừu tượng.

2. **Promote (USER GATE — chỉ bước này cần duyệt):** cuối flow, nếu pending có entry mới → show qua `AskUserQuestion`: `Promote vào error-patterns.md` / `Sửa rồi promote` / `Để ở pending`. Approved → gán ID thật `EP-NNN` (max+1), append vào `.omp/rules/error-patterns.md`, xoá khỏi pending. Entry đã promote được hook enforce tự động từ đó.

3. **KHÔNG BAO GIỜ ghi thẳng vào `error-patterns.md` khi chưa có approve** — pending thì tự do.

## Resume Mode (`--resume`)

When `--resume` is passed:

1. Search `.claude/plans/` for the ticket number in existing todolists
2. Read the todolist -> find first unchecked `[ ]` task
3. Resume from that phase
4. Continue normal flow from there

## Debugging Flow (Bug Tickets)

When classified as `bugfix`:

| Bug Complexity | Flow |
|---------------|------|
| Simple (single file) | Lite todolist -> Serena locate -> fix -> `php -l` -> verify -> update todolist |
| Medium (2-5 files) | Todolist -> load skills -> Serena trace refs -> fix -> review -> test -> update todolist |
| Complex (cross-module) | Todolist -> spawn `debugger` agent -> load skills from report -> fix -> `tester` agent -> update todolist |

## Rules

| Rule | Enforced |
|------|----------|
| Ticket or description required | REJECT if neither provided |
| Knowledge lookup mandatory | Phase 1.5 — read INDEX + module + flow files BEFORE coding |
| Knowledge update user gate | Phase 10.5 — NEVER write to `docs/knowledge/` without explicit user approval |
| Error pattern user gate | Phase 10.6 — NEVER write to `.omp/rules/error-patterns.md` without explicit user approval |
| Skills auto-detected | Loaded before research phase |
| TodoList mandatory | ALL complexity levels |
| User gate for medium/complex | NEVER code without approval |
| Sync before branch | Phase 6.5 — `checkout master` + `pull origin master` before creating feature branch; never code on stale master |
| File separation | NO inline CSS/JS -- enforced |
| PHP syntax check | `php -l` after every PHP file |
| SQL verification | Test all queries on real DB |
| Code review gate | Must pass before next task |
| Test phase mandatory | Never skip (unless `--skip-test`) |
| Completion mandatory | Update todolist at end |
| Migration reminder | Alert user if migrations created |
| No AI in commits | Never mention Claude in code/commits |

## Error Recovery

| Scenario | Action |
|----------|--------|
| Subagent fails | Retry once with reduced scope -> orchestrator takes over |
| Implementation fails mid-way | Revert partial changes -> document in todolist -> notify user |
| Code review fails | Fix issues -> re-review -> do NOT proceed until pass |
| Tests fail | Fix -> re-run from Phase 8 |
| Missing requirements | `AskUserQuestion` -> pause until answered |

## References

- [Phase Checklist](references/phase-checklist.md) — Compact runtime checklist: phase matrix by complexity, file separation quick check, review essentials, test tier decision guide

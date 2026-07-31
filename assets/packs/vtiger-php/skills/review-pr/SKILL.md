---
name: review-pr
disable-model-invocation: true
description: "Review GitLab Merge Request for bugs, security, logic, and CLAUDE.md compliance. Posts comment and auto-merges if clean."
user-invokable: true
allowed-tools: Bash(glab mr *), Bash(glab api *), Bash(git diff*), Bash(git log*), Bash(git fetch*), Bash(git show*), Bash(git merge-base*), Bash(php -l *), mcp__gitLab__*, mcp__pms__*, Agent, Read, Grep, Glob
---

# Review PR Skill (GitLab)

> Review GitLab Merge Requests for bugs, security, logic errors, and coding standards compliance. Post results as MR comment. Auto-merge if clean.

## Tool Priority (MCP first, CLI fallback)

**Ưu tiên dùng GitLab MCP tools** (`mcp__gitLab__*`) nếu available. Nếu MCP không load hoặc lỗi → fallback sang `glab` CLI.

| Operation | MCP Tool (ưu tiên) | glab CLI (fallback) |
|-----------|-------------------|---------------------|
| View MR | `mcp__gitLab__get_merge_request` | `glab mr view <ID>` |
| Get diff | `mcp__gitLab__get_merge_request_diffs` | `glab mr diff <ID>` |
| List MRs | `mcp__gitLab__list_merge_requests` | `glab mr list` |
| Post comment | `mcp__gitLab__create_merge_request_note` | `glab mr comment <ID>` |
| Merge MR | `mcp__gitLab__accept_merge_request` | `glab mr merge <ID>` |
| List comments | `mcp__gitLab__list_merge_request_notes` | `glab mr view <ID> --comments` |

**Detection logic:** Tại Step 0, thử gọi MCP tool trước. Nếu tool không tồn tại hoặc trả lỗi → set `$USE_CLI=true` và dùng `glab` cho toàn bộ flow.

## Usage

```
/review-pr 218                  -> Review MR #218 (auto post + auto merge if no blocking issues)
/review-pr 218 --no-merge       -> Review only, do NOT merge even if clean
/review-pr 218 --dry-run        -> Review only, do NOT post comment and do NOT merge
/review-pr <URL>                -> Review from GitLab MR URL
```

**Default behavior (no flag):**
- Không có lỗi CRITICAL/HIGH → tự động post comment + tự động merge (squash). Không hỏi user.
- Có lỗi CRITICAL/HIGH → trình kết quả cho user, hỏi ý kiến trước khi post.

## Workflow

### Step 0: Parse Input & Validate

1. **Detect repo** từ git remote (nếu không truyền từ prompt):
   ```bash
   REPO=$(git remote get-url origin | sed 's|.*[:/]\([^/]*/[^/]*\)\.git$|\1|; s|.*[:/]\([^/]*/[^/]*\)$|\1|')
   ```
   Dùng `$REPO` cho tất cả lệnh `glab --repo` trong các step sau.

2. Extract MR number from args:
   - `218` or `#218` → MR number `218`
   - `https://git.example.com/acme/vtiger/-/merge_requests/218` → MR number `218` + `REPO=acme/vtiger`
   - If no MR number → **REJECT**: "MR number required. Usage: `/review-pr 218`"

3. Fetch MR info:
   ```bash
   glab mr view <MR_NUMBER> --repo $REPO
   ```

3. Pre-checks — stop if any is true:
   - MR is `closed` or `merged`
   - MR is `draft` (WIP)
   - MR has a comment from `tung.nguyen` containing "Code Review" (already reviewed by human)
   - **MR `target_branch` ≠ `master`** — skill này CHỈ áp dụng cho MR merge vào `master`. Với target `develop`, `release-*`, `hotfix/*`, v.v. → BLOCK ngay, không review, không merge, không gắn label. Thông báo user: `"MR !<NUMBER> target '$target' is not master — skill /review-pr chỉ xử lý MR vào master. Dừng."`

### Step 0.5 + 0.6: Previous comments & PMS ticket verification (BẮT BUỘC)

Đọc [references/pms-verification.md](references/pms-verification.md) — load comment review cũ (tránh lặp finding đã resolve), verify ticket PMS tồn tại + đúng status trước khi review.

### Step 1: Get Diff

Use `glab mr diff` to get the actual diff (GitLab compares against target branch):

```bash
glab mr diff <MR_NUMBER> --repo $REPO
```

If diff is empty → post comment "No changes to review" and stop.

If diff is too large (>5000 lines) → also fetch via git:
```bash
git fetch origin refs/merge-requests/<MR_NUMBER>/head:mr/<MR_NUMBER>
git diff origin/<TARGET>..mr/<MR_NUMBER> --stat
```

### Step 2: Load Knowledge & Logic Context (BẮT BUỘC)

**Mục đích:** Hiểu nghiệp vụ dự án để review logic, không chỉ review cú pháp.

1. Read `docs/knowledge/INDEX.md` → tra bảng Module → File Lookup
2. Xác định modules nào bị ảnh hưởng bởi MR (từ file paths trong diff)
3. Đọc **tất cả** knowledge files liên quan:
   - Module knowledge: `docs/knowledge/modules/<Module>.md`
   - Cross-module flows: `docs/knowledge/flows/<flow>.md` (nếu MR liên quan nhiều module)
4. Từ knowledge files, xác định:
   - **Status transitions**: Các trạng thái hợp lệ và luồng chuyển đổi
   - **Handlers & side effects**: Các event handler sẽ trigger khi save/update
   - **Relationships**: Quan hệ giữa các module (1-N, N-N)
   - **Gotchas & edge cases**: Các lỗi đã biết, cần tránh
   - **Business rules**: Quy tắc nghiệp vụ đặc thù của module
5. Truyền knowledge context này cho TẤT CẢ 5 agents ở Step 3

### Step 2.5: Business Logic Extraction (BẮT BUỘC)

**Mục tiêu:** Hiểu rõ MR đang làm gì về **nghiệp vụ**, không chỉ về cú pháp. Tránh sót logic mới hoặc review lệch với ý đồ ticket.

**Cách làm:** Spawn 1 subagent (`subagent_type: "code-reviewer"`, opus) với input:
- MR title, description
- Full diff
- Ticket detail từ Step 0.6 (title, description từ PMS)
- Knowledge files từ Step 2

**Yêu cầu agent trả về Business Logic Manifest (YAML):**

```yaml
feature_intent: "<1-2 câu mô tả mục đích nghiệp vụ của MR — dựa trên ticket + diff, KHÔNG paraphrase MR title>"

# BẮT BUỘC: Extract từng yêu cầu cụ thể trong PMS ticket description (bullet/numbered list).
# Mỗi sub-item trong description PMS = 1 requirement. Giữ nguyên ngôn từ ticket (tiếng Việt).
# Đây là input để cross-check sau với code → tạo checklist "Done vs Missing".
ticket_requirements:
  - id: REQ-01
    description: "<Trích nguyên văn từ ticket description, vd: 'Bổ sung hiển thị kênh Facebook khi user thêm trang'>"
    category: "<Nhóm cha trong ticket, vd: '1. Tích hợp kênh đăng bài'>"
    expected_files: [<dự đoán file/module sẽ implement>]
    verification_hint: "<Cách verify, vd: 'Tìm field facebook_tab trong SocialConfig.tpl + JS handler'>"
  - id: REQ-02
    ...

business_rules:
  - id: BR-01
    rule: "<Phát biểu rule theo dạng 'Khi X xảy ra → hệ thống làm Y với điều kiện Z'>"
    trigger: "<Event/entry point: AJAX action, handler, cron, button click, webhook...>"
    files: [<list file:line chứa logic>]
    dependencies: [<modules/classes rule này phụ thuộc>]
    side_effects: [<email, sms, comment, record update trên module khác...>]

status_transitions:
  - "<module>.<field>: <from> → <to> khi <điều kiện>"

new_integrations: [<external APIs được gọi, nếu có>]
new_fields: [<module.field mới>]
new_endpoints: [<Module_Action mode=X, nếu có AJAX mới>]
affected_modules: [<tất cả module bị ảnh hưởng trực tiếp/gián tiếp>]

unknowns: [<câu hỏi nghiệp vụ agent chưa giải đáp được — sẽ flag cho reviewer>]
```

**Quy tắc build manifest:**
- `ticket_requirements` BẮT BUỘC parse từ ticket `description` (Step 0.6) — không skip dù description ngắn. Mỗi gạch đầu dòng / mỗi câu mô tả tính năng = 1 REQ. Giữ nguyên tiếng Việt ticket.
- Rule phải là **phát biểu nghiệp vụ**, không phải "thêm method foo()" — VD: ✗ "Thêm `applyRoundRobin()`" | ✓ "Khi ticket mới + assigned_user_id rỗng → auto-assign user tiếp theo theo round-robin"
- Rule KHÔNG bao gồm thay đổi thuần UI/CSS (trừ khi UI drive logic, VD: toggle switch điều khiển feature)
- `affected_modules` phải include cả module gián tiếp (VD: handler của module khác bị trigger)
- Nếu MR chỉ là refactor/rename không đổi logic → manifest có `feature_intent: "Refactor — no business logic change"`, `business_rules: []`, nhưng `ticket_requirements` vẫn extract nếu ticket có mô tả (để đối chiếu xem refactor có làm thiếu gì không).

**Lưu `$BUSINESS_MANIFEST`** → truyền cho TẤT CẢ 5 agents ở Step 3.

**Nếu manifest trả về `business_rules: []` KHÔNG phải do refactor** → block review, yêu cầu agent chạy lại hoặc user bổ sung context.

### Step 3: Parallel Review (5 agents)

Dùng `subagent_type: "code-reviewer"` cho tất cả agents. Launch 5 agents song song, mỗi agent nhận: MR title, description, full diff, knowledge context, và **`$BUSINESS_MANIFEST` từ Step 2.5**.

**Yêu cầu CHUNG cho mọi agent:** Ngoài list lỗi, mỗi agent PHẢI trả về **Coverage Report** cho cả `business_rules` VÀ `ticket_requirements`:

```yaml
coverage:
  - rule_id: BR-01
    reviewed: true
    consistent_with_knowledge: true       # so với docs/knowledge/modules/*.md
    missing_edge_cases: []                 # VD: ["pool rỗng", "user offline", "race condition"]
    unintended_side_effects: []            # handler khác trigger theo? loop risk?
    issues_found: []                       # reference tới lỗi cụ thể (nếu có)
  - rule_id: BR-02
    reviewed: false
    reason: "Rule nằm trong file ngoài phạm vi agent này"

# BẮT BUỘC: Cross-check từng requirement với code thực tế.
requirement_check:
  - req_id: REQ-01
    status: done           # done | partial | missing | not_verifiable
    evidence:              # file:line chứng minh đã làm (hoặc trống nếu missing)
      - "modules/X/views/Y.php:123 — added field"
      - "layouts/v7/modules/X/Y.tpl:45 — render field"
    gap: ""                # mô tả nếu partial/missing, vd: "Field thêm nhưng chưa wire vào save flow"
    confidence: 90         # 0-100, theo Step 5
  - req_id: REQ-02
    status: missing
    evidence: []
    gap: "Không tìm thấy logic xử lý event X trong diff. Có thể author quên hoặc làm ở MR khác."
    confidence: 85
```

Orchestrator tổng hợp coverage + requirement_check từ 5 agents:
- Nếu có `rule_id` không agent nào `reviewed: true` → **re-dispatch** cho 1 agent chuyên trách.
- Nếu có `req_id` không agent nào verify được (`status: not_verifiable` từ tất cả) → flag `unknowns` trong comment.
- Mỗi `req_id` lấy status "tệ nhất" trong các agent (missing > partial > done) để tránh false-positive done.

**Agent 1: Performance review (opus) — ƯU TIÊN CAO NHẤT**
Đây là agent quan trọng nhất. Review tất cả vấn đề hiệu năng:

- **N+1 queries**: DB queries bên trong vòng lặp → phải batch fetch với `IN (?)`
- **SQL không dùng prepared statements**: Tất cả query PHẢI dùng `pquery()` với `?` params, KHÔNG string concatenation
- **Missing LIMIT/pagination**: Query trả về toàn bộ dữ liệu mà không giới hạn
- **SELECT * thay vì chọn cột cụ thể**: Chỉ SELECT những cột cần thiết
- **Missing index**: WHERE/JOIN trên cột không có index
- **Redundant queries**: Query trùng lặp có thể cache hoặc gộp
- **file_get_contents trên file lớn**: Dùng streaming thay vì load toàn bộ vào memory
- **LIMIT/OFFSET không cast (int)**: Giá trị từ request phải cast `(int)` trước khi đưa vào SQL
- **Expensive operations không có guard**: Kiểm tra feature flag/config TRƯỚC khi query DB hoặc gọi API

**Agent 2: Rules compliance & Security (opus)**
Audit changes against `cloudgo-development-rules.md`:

- **File separation (NON-NEGOTIABLE)**: No inline CSS/JS in PHP/TPL files. No HTML in PHP classes.
  - CSS MUST be in `modules/<Module>/resources/<ViewName>.css`
  - JS core views → `layouts/v7/modules/<Module>/resources/<ViewName>.js`
  - JS custom views → `modules/<Module>/resources/<ViewName>.js`
  - TPL → `layouts/v7/modules/<Module>/<ViewName>.tpl`
- **SQL injection**: All queries use `pquery()` with `?` params, NO string concatenation
- **Security**: `$request->validateWriteAccess()` on write actions, type casting `(int)`/`(string)` on ALL `$request->get()`, `checkPermission()` enforced
- **XSS**: `.text()` not `.html()` for dynamic content; `vtlib_purify()` for user input; jQuery DOM construction not string concatenation
- **CSRF**: `$request->validateWriteAccess()` on ALL write Action controllers
- **PHP headers**: `@author`, `@email`, `@create date` (required on new files)
- **Type declarations**: Return types, parameter types per PHP 7+ conventions
- **Language file directory** (MEDIUM): Language files phải đúng path theo `$developerTeam` trong `config.env.php` (hiện tại `R&D`):
  - R&D → `languages/<locale>/ModuleName.php` (root, KHÔNG được đặt trong `dev/`)
  - DEV → `languages/<locale>/dev/ModuleName.php`
  - Customer → `languages/<locale>/cus/ModuleName.php`
- **Language key prefix** (MEDIUM): Keys trong `$languageStrings` phải có prefix `LBL_`; keys trong `$jsLanguageStrings` phải có prefix `JS_`. Sai prefix → flag MEDIUM.
- **Autoload rule** (MEDIUM): Các class theo convention `<Module>_<Component>_<Type>` được autoload — KHÔNG dùng `require_once`. Chỉ dùng `require_once` cho `include/utils/`, `include/Webservice/`, `modules/Reports/custom/` hoặc non-module paths.
- **Migration naming** (MEDIUM): Migration files phải theo format `YYYY.MM.DD.HH.mm.ss_DescriptiveName.php`.

**Agent 3: Clean Code & Structure (sonet)**
Audit code quality theo `cloudgo-development-rules.md`:

- **Method length**: Mỗi method ≤ 30 dòng. Nếu vượt → flag và gợi ý tách
- **Class/File length**: Mỗi file ≤ 200 dòng code. Nếu vượt → flag và gợi ý modularize
- **Class naming**: `<Module>_<Component>_<Type>` (VD: `Accounts_Record_Model`)
- **Method naming**: camelCase với verb prefix (get/set/is/has/can/create/process)
- **JS controller naming**: `<Module>_<View>_Js` (VD: `Products_Config_Js`); parent class phải đúng (`Vtiger_List_Js`, `Vtiger_Edit_Js`, `Vtiger_Detail_Js`, `CustomView_BaseController_Js`)
- **Early return pattern**: Không deep nesting, dùng guard clauses
- **Dead code**: Không commented-out code, unused variables, unused imports
- **DRY**: Không logic trùng lặp — phải extract thành shared methods
- **Null coalescing**: Dùng `??` thay vì verbose `isset()` checks
- **Modification tracking**: Comment `// Added by` / `// Modified by` với blank line trước
- **File locations**: Kiểm tra CSS/JS/TPL đúng thư mục theo convention table
- **Constants**: Dùng class constants thay vì magic strings/numbers
- **Error handling**: try/catch cho external calls, `error_log()` cho errors
- **BlocksAndFieldsRegister** (HIGH): Field mới PHẢI đăng ký trong `modules/{Module}/BlocksAndFieldsRegister.php` (Quick Repair sẽ tạo DB column). KHÔNG tạo field bằng raw SQL migration. Thiếu register → flag HIGH.
- **JS AJAX pattern** (MEDIUM): Phải dùng `app.request.post()` / `app.request.get()`, không dùng raw `$.ajax()`, `$.post()`, `$.get()`.
- **Class structure order** (MEDIUM): Constants → Singleton `getInstance()` → Public methods → Protected/private methods. Sai thứ tự → flag MEDIUM.

**Agent 4: Bug scan (sonet)**
Scan diff for obvious bugs — only flag issues visible in the diff:
- Code that will fail to compile or parse
- Logic errors that produce wrong results regardless of input
- Undefined variables, wrong method signatures
- Missing return statements, off-by-one errors

**Agent 5: Logic nghiệp vụ & security deep review + Ticket requirement verification (sonet)**
Dùng knowledge context từ Step 2 + `ticket_requirements` từ Step 2.5 để review:

**A. Ticket requirement verification (BẮT BUỘC — ưu tiên đầu tiên):**
- Với MỖI `REQ-XX` trong `ticket_requirements`: tìm trong diff xem có code tương ứng không
- Verify status:
  - `done`: tìm thấy code đầy đủ → ghi `evidence: [file:line, ...]`
  - `partial`: có code nhưng thiếu nhánh / thiếu validate / thiếu wire flow → ghi `gap: "..."`
  - `missing`: không tìm thấy code trong diff → `gap: "Không tìm thấy ..."`
  - `not_verifiable`: yêu cầu thuần UI/visual không verify được qua diff
- Đối với requirement liên quan field DB → check `BlocksAndFieldsRegister` + migration + Edit.tpl + save flow
- Đối với requirement liên quan action UI → check JS event handler + AJAX endpoint
- Đối với requirement liên quan API ngoài → check helper method + call site

**B. Logic dự án:**
- **Status transitions**: Kiểm tra chuyển trạng thái có đúng flow không (VD: `outline_status` chỉ được chuyển `writing` → `written` → `approved`)
- **Handler side effects**: Khi save record, có handler nào bị trigger không mong muốn không? (đọc `HandlersRegister.php`)
- **Relationship integrity**: Thêm/xóa/sửa record có ảnh hưởng đến related records không?
- **Missing validation**: Dữ liệu đầu vào có được validate đủ trước khi lưu DB không?
- **Edge cases từ knowledge**: Knowledge file có liệt kê gotcha nào liên quan đến code đang thay đổi không?
- **Security**: SQL injection, XSS, CSRF, path traversal, mass assignment
- **Race conditions**: Concurrent requests có thể gây data corruption không?

**QUAN TRỌNG: CHỈ review code thay đổi trong diff.**
- CHỈ review các dòng code mới/sửa (có prefix `+` trong diff)
- Code cũ trong file KHÔNG review, trừ khi phát hiện lỗi **CRITICAL** (crash, security vulnerability, data loss)
- Nếu code cũ có vấn đề nhưng không phải CRITICAL → bỏ qua, không flag

**HIGH SIGNAL only.** Do NOT flag:
- Pre-existing issues (not introduced in this MR) — trừ CRITICAL
- Potential issues that depend on specific runtime state
- Issues chỉ linter mới bắt được (missing semicolons, trailing whitespace)
- Pedantic nitpicks không ảnh hưởng chất lượng code

**DO flag** (đây là tiêu chí quan trọng, KHÔNG bỏ qua):
- Vi phạm cấu trúc file (inline CSS/JS, file sai thư mục)
- Method/class quá dài (>30 lines / >200 lines)
- Không tuân thủ naming conventions
- Dead code, commented-out code, duplicated logic
- Missing error handling, missing type declarations

### Step 4: Validate Issues

For each issue from Step 3:
- Launch validation subagent với `subagent_type: "code-reviewer"` (opus cho bugs/performance, sonnet cho rules)
- Subagent nhận: MR context + mô tả lỗi + source code liên quan
- Phải xác nhận với độ tin cậy cao rằng lỗi là thật
- Loại bỏ những issue không được xác nhận

### Step 5: Confidence Scoring

Score each validated issue 0-100:
- **0**: Not confident, likely false positive
- **25**: Somewhat confident
- **50**: Moderately confident
- **75**: Highly confident, real and important
- **100**: Absolutely certain, definitely real

**Threshold: 80** — discard issues below 80.

### Step 5.5: Quyết định post/merge

**Quy tắc tự động (default — không có flag):**

| Tình huống | Hành động |
|-----------|-----------|
| Không có lỗi CRITICAL/HIGH/PERFORMANCE | **Tự động** post comment (Step 6) + gán label "Dev done" (Step 6.5) + **tự động** merge (Step 7). KHÔNG hỏi user. |
| Có lỗi CRITICAL/HIGH/PERFORMANCE | Trình kết quả cho user + dùng `AskUserQuestion` hỏi trước khi post. KHÔNG merge. |
| Có `--dry-run` flag | Chỉ in kết quả ra terminal, KHÔNG post, KHÔNG merge. |
| Có `--no-merge` flag | Post comment + gán label, nhưng KHÔNG merge kể cả khi clean. |

**Lưu ý:** Mọi lỗi do Agent 1 (Performance review) phát hiện — kể cả khi không có severity CRITICAL/HIGH — đều BLOCK merge. Performance là tiêu chí chặn độc lập với severity scale thông thường.

**Khi cần hỏi user (có lỗi CRITICAL/HIGH/PERFORMANCE):**

1. Hiển thị tóm tắt kết quả review ra terminal (tiếng Việt có dấu):
   - Số lỗi phát hiện theo severity
   - Danh sách từng lỗi: file, dòng, mô tả ngắn

2. Dùng `AskUserQuestion`:
   - **"Đồng ý post comment lên GitLab"** → tiếp tục Step 6 (không merge)
   - **"Chỉnh sửa trước khi post"** → user sửa nội dung, rồi post
   - **"Không post, chỉ xem"** → dừng lại, không post

**Khi clean (không có CRITICAL/HIGH/PERFORMANCE):**
- In tóm tắt ngắn ra terminal: "✅ Không có lỗi chặn merge — đang post comment và merge..."
- Tiếp tục Step 6 → 6.5 → 7 mà không cần user confirmation

### Step 6: Post Review Comment

**BẮT BUỘC: Toàn bộ comment PHẢI viết bằng tiếng Việt có dấu.**

Format the review comment:

**Nếu có lỗi:**

```markdown
Comment format chuẩn (2 template: có lỗi / clean): đọc [references/comment-templates.md](references/comment-templates.md) khi đến bước post.

### Step 6.5 → 7.5: Label, merge, update PMS

Đọc [references/merge-and-labels.md](references/merge-and-labels.md) — gán label kết quả (Dev done / có lỗi), merge decision (--squash --remove-source-branch khi PASS), update PMS ticket sau merge.

## Severity Levels

| Level | Description | Blocks merge? |
|-------|-------------|---------------|
| **CRITICAL** | Will crash, security vulnerability, data loss | Yes |
| **HIGH** | Wrong results, logic error, missing validation | Yes |
| **PERFORMANCE** | N+1, missing index, unbounded query, missing pagination, redundant DB calls (do Agent 1 phát hiện) | **Yes** |
| **MEDIUM** | Rules violation, missing type cast, convention break | No (warning) |
| **LOW** | Style, naming, minor improvement | No |

CRITICAL, HIGH, và PERFORMANCE đều block merge. PERFORMANCE là một category độc lập — kể cả khi Agent 1 đánh giá severity là MEDIUM trên thang thông thường, nó vẫn chặn auto-merge.

## False Positive Filters

Do NOT flag these:
- Pre-existing issues not introduced in this MR
- Code that looks buggy but matches established codebase patterns (e.g., `require` vs `require_once` for privilege files)
- `checkPermission() { return true; }` on View controllers that serve AJAX fragments (common VTiger pattern for views — only flag on Action controllers)
- `die()` in webhook handlers (standard pattern)
- Inline `style="display:none"` for conditional show/hide (acceptable)
- Missing type cast on `$current_user->id` (server-controlled, not user input)

## Rules

| Rule | Detail |
|------|--------|
| **Target branch MANDATORY = master** | Skill CHỈ áp dụng cho MR có `target_branch = master`. Target khác (`develop`, `release-*`, `hotfix/*`, `feature/*`, v.v.) → block ngay ở Step 0 Pre-check, không review, không gắn label, không merge. |
| **Ticket ID MANDATORY** | MR title BẮT BUỘC có `#<digits>`. Thiếu → block ngay, KHÔNG review code. Không có ngoại lệ. |
| **PMS verification MANDATORY** | Ticket phải tồn tại trên PMS. Search bằng `mcp__pms__pms_tickets(filters=[{name:"ticket_no",value:"<ID>",operator:"c"}])` trước (stored format là `#<ID>`), lấy record_id → rồi `pms_ticket_detail(id=record_id)`. Không tìm thấy → block. |
| **GitLab only** | Uses `glab` CLI, not `gh` |
| **Diff from GitLab** | Always use `glab mr diff` first (correct target branch) |
| **Knowledge required** | MUST read knowledge files before reviewing logic |
| **Business manifest MANDATORY** | Step 2.5 phải build Business Logic Manifest (YAML) trước khi review. Không có manifest → block. Manifest rỗng mà không phải refactor → yêu cầu chạy lại. |
| **Knowledge cross-check** | Mỗi `business_rule` phải đối chiếu với `docs/knowledge/modules/<Module>.md`. Mâu thuẫn nghiệp vụ → CRITICAL. |
| **Coverage gate** | Comment BẮT BUỘC có section "Phạm vi nghiệp vụ đã review". Merge chỉ cho phép khi 100% `business_rules` có status `reviewed: true` từ ít nhất 1 agent. |
| **Ticket requirement checklist MANDATORY** | Step 2.5 extract `ticket_requirements` từ PMS ticket description; Step 3 mỗi agent verify `requirement_check` (done/partial/missing/not_verifiable); Step 6 BẮT BUỘC có section "Checklist tính năng theo yêu cầu ticket". Thiếu section này → block post comment. |
| **Missing/partial blocks merge** | Bất kỳ `REQ-*` có status `missing` hoặc `partial` → flag HIGH, chặn auto-merge. Author phải confirm bằng comment hoặc bổ sung code (hoặc xác nhận tách sang MR khác → reviewer override). |
| **Edge case coverage** | Mỗi rule agent phải liệt kê `missing_edge_cases`. Không rỗng → HIGH (nếu blocker) / MEDIUM (nếu chỉ cảnh báo). |
| **Side effect audit** | Rule có save/update record phải trace handler chain (via knowledge `handlers` section). Loop/duplicate event → CRITICAL. |
| **High signal** | Confidence threshold 80 — no noise |
| **Validate all** | Every issue gets a validation subagent |
| **Auto post + merge khi clean** | Default: không có CRITICAL/HIGH → tự động post comment + merge, không hỏi user |
| **Ask user khi có blocker** | Có CRITICAL/HIGH → trình kết quả + `AskUserQuestion` trước khi post |
| **Post always** | Always post comment, even if clean (trừ khi `--dry-run`) |
| **Merge convention** | `[Category] #Ticket: Description` format |
| **Squash merge** | Use `--squash --yes` when merging |
| **Remove source branch** | Always add `--remove-source-branch` khi merge — xoá branch nguồn để tránh tồn đọng |
| **No auto-merge on issues** | CRITICAL/HIGH/PERFORMANCE blocks merge |
| **Performance blocks merge** | Mọi lỗi do Agent 1 (Performance review) phát hiện đều block auto-merge, bất kể severity. PERFORMANCE là tiêu chí chặn độc lập với scale CRITICAL/HIGH/MEDIUM/LOW. |
| **Flags override default** | `--no-merge` = post nhưng không merge; `--dry-run` = chỉ in, không làm gì |
| **Label "Dev done"** | Clean MR nhận label "Dev done" sau khi merge. release-check sẽ transition `Dev done → Chờ release` khi PMS ticket đã verify. |
| **PMS status → Testing sau merge** | Step 7.5 BẮT BUỘC update ticket status sang `Testing` qua `mcp__pms__pms_ticket_update(id=$TICKET_RECORD_ID, ...)`. Dùng `$TICKET_RECORD_ID` (numeric record_id), KHÔNG dùng ticket_no. |
| **PMS comment optional** | `pms_comments(action="add")` yêu cầu `assigned_user_id` — nếu không xác định được reviewer user ID thì skip (rating_note đã đủ context). |
| **No subjective downgrade** | Rule là mechanical, KHÔNG được downgrade severity dựa trên "cảm tính" reviewer |

---
description: "Core CloudGO dev rules áp dụng mọi việc — file-separation, brace K&R + attribution comment (PHP/JS/CSS/TPL), naming, error-pattern gate, bảng TTSR. Always-on."
alwaysApply: true
---

# CloudGo Development Rules — Core

## Path-scoped sibling rules

TTSR rules below auto-load on edit and are listed nowhere else; when coding OR reviewing a matching file, `read rule://<name>` yourself. Keep this table in sync with each rule's frontmatter `scope`.

| File | Áp dụng cho (code + review) |
|------|--------------------|
| `security-rules`, `php83` (mã C/F/P/V/E/R/S — review trích mã), `phpstan` (gate trước khi xong/commit) | `**/*.php`, `**/*.inc`, `**/*.service`; phpstan thêm `tools/phpstan/**`, `tools/composer.*` |
| `performance-rules` | như trên — chỉ khi edit có SQL keyword, `pquery(`/`->query(`/`getInstanceById(`/`getQuery(`, hoặc vòng lặp |
| `modern-php` | `*.php` trong `modules\|include\|includes\|domains\|integrations\|api\|cron/**` — chỉ khi edit thêm lời gọi thư viện legacy/bị cấm (curl, HTTP_Request, log4php, PHPExcel, JWT, mysqli, `require_once`…) |
| `domain-glue` | `*.php` trong `modules\|api\|include/Webservice\|include/Webhooks/**` — chỉ khi edit dùng namespace `CloudGo\` hoặc gọi `<Domain>::xxxService(` |
| `domain-architecture` | `domains/**/*.php`, `integrations/**/*.php` |
| `domain-php` | `domains/**/*.php` |
| `domain-service` / `domain-repository` / `domain-gateway` | `domains/**/*Service.php` / `*Repository.php` / `*Gateway.php`; gateway thêm `integrations/**/*.php` |
| `domain-composition-root` | `domains/Social/src/Social.php`, `domains/**/Channel/**/*Channel.php` |
| `domain-tests` | `domains/**/tests/**/*.php` |
| `migration-rules` | `modules/CPMigration/migrations/**` |
| `api-connector-rules` (connector legacy) | `include/Webservice/**/*.php`, `modules/CP*Integration/**/*.php` |
| `api-v1` (nhánh `feature/cloudgo-crm-api-v1`) | `include/Webservice/V1/**/*.php`, `include/Webservice/V1/**/*.md` |
| `css-rules` | `**/*.css`, `**/*.tpl` |
| `language-rules` | `languages/**/*.php` |

**[MANDATORY khi review] Rule code = tiêu chí review.** Review/audit bất kỳ diff (MR, `/review-pr`, `code-reviewer`, validator, subagent) → TRƯỚC khi đánh giá, `read rule://<name>` cho MỌI rule có glob khớp file bị đổi; `scope` chỉ quyết định lúc rule tự nạp — cấm bác finding vì "rule chỉ áp dụng khi sửa code". Vi phạm brace/style/security/performance là finding.

## Error Patterns (MANDATORY check)

- Trước khi code hoặc review PHP: grep diff / code bị chạm theo các dòng `Trigger` trong `rule://error-patterns`, đọc và áp các entry khớp. Reviewer trong `/review-pr` nhận sẵn các entry khớp qua `$EP_HITS`.
- Sau khi fix bug mới: hỏi user có thêm entry vào `error-patterns` không.

## ⚠️ File Separation Rules (MANDATORY — NON-NEGOTIABLE)

No hook enforces this — check it yourself before finishing and when reviewing.

1. **NO INLINE CSS/JS** in PHP or TPL files.
2. **NO HTML IN PHP**: markup lives in `.tpl`; PHP views only set data and call templates.
3. Locations: CSS **always** `modules/<Module>/resources/<ViewName>.css`; JS core views (List/Edit/Detail) `layouts/v7/modules/<Module>/resources/<ViewName>.js`, custom views `modules/<Module>/resources/<ViewName>.js`; TPL core `layouts/v7/modules/<Module>/<ViewName>.tpl`, custom `modules/<Module>/tpls/<ViewName>.tpl`.

## Style & Comments (PHP/JS/CSS/TPL)

### Brace Style

- **K&R, NOT PSR-12 Allman**: `{` on the same line for class, method/function, control blocks. `else`/`elseif`/`catch`/`finally` on their **OWN line** after `}` — never copy legacy cuddled `} else`. One-line guards (`if (x) { a(); } else { b(); }`) may stay inline.
- Tabs; keep the file's existing line endings (some are `\r\n`).
- Subagent output defaults to Allman → before commit grep the diff for lone `{` lines, cuddled `} else|catch|finally`, duplicate `@author`; never trust a subagent's self-report.

### Modification Tracking Comments

**No ticket / plan / phase / finding numbers in comments** — the general code rule (`/feature` included). Exception: `/cook` works on one ticket and uses the ticket-ID comment format `cook.md` defines (`// #NNNNN: Added/Modified by …`) — follow it there. By ownership of the touched class/function:

- New function, class you own → plain 1-line description. Editing a function you wrote → no comment.
- New function in someone else's class → first docblock line `Added by <Name> on <DATE> - <REASON>`; rewriting most of someone else's function → `Modified by <Name> on <DATE> - <REASON>`. Plain text, never an `@` tag, above other description/tags; no docblock → add one; later attributions add a line below; no `End`.
- A few lines inside someone else's function → `// Modified by <Name> on <DATE> - <REASON>` above them, closed by `// End <Name>` (adjacent to a following `}`). CSS `/* … */`, TPL `{* … *}` same form; JS docblock attribution goes in JSDoc.
- Owner = file's `@author` / `Author:` header; no header or multi-author → `git blame` the function, never assume. New file → add header, you own it.
- Comments in **English** (rewrite touched legacy Vietnamese ones), **≤ 2 lines** per block (`@` tags excluded), stating meaning/invariant — never chat decisions or tradeoffs.

## General Principles

- YAGNI / KISS / DRY. **Framework First**: use VTiger patterns; avoid reinventing.
- **File Size**: keep under 200 lines; split into focused modules.
- **Edit, Don't Duplicate**: update existing files; no "enhanced" copies.
- `php -l` after every PHP file; `code-reviewer` agent after every implementation.
- Run the narrowest useful test first, then broaden.
- **Hạn chế sửa core tối đa** — không có giải pháp ngoài sửa core thì phải hỏi Leader chốt phương án trước.

### Pre-commit / Pre-push

- Conventional commit messages, no AI references. Lint before commit, tests before push.
- Commits focused on the actual change — no unrelated debug code (e.g. the `return true;` in `Users::doLogin()`).
- GitLab remote is `git.onlinecrm.vn`; use `gitlab-mr` for MRs.

## Quick Reference

### View Base Classes (CRITICAL — pick correctly BEFORE writing the view)

- Core List/Edit/Detail of an entity module → `Vtiger_Index_View` (or `Vtiger_List/Edit/Detail_View`), TPL `layouts/v7/modules/<Module>/`, custom JS/CSS via `getHeaderScripts()`/`getHeaderCss()`.
- Settings page → `Settings_Vtiger_Index_View` (auto `isAdminUser()`), TPL `layouts/v7/modules/Settings/<Module>/`.
- Anything else (Config, custom page, dashboard, landing, report) → **`CustomView_Base_View`**, TPL `modules/<Module>/tpls/<ViewName>.tpl` via `$viewer->display()`; auto-loads `modules/<Module>/resources/<ViewName>.{js,css}`. Details: `skill://view` `references/custom-view-base.md`.

### Naming Conventions

- PHP class `<Module>_<Component>_<Type>` in file `<Component>.php` (no type suffix); method camelCase + verb (`get*`/`set*`/`is*`/`has*`/`can*`; migration `up`/`down`); variable camelCase; constant UPPER_SNAKE.
- Module: singular English noun, PascalCase (`TourBooking`). Field / DB column: noun or adjective, snake_case (`is_primary`). Picklist field: module prefix + plural noun (`tour_types`); picklist value / array key: lowercase snake_case (`new_customer`).
- JS controller `<Module>_<View>_Js`. CSS/JS/TPL file: PascalCase noun (`EditView.js`). HTML id/class kebab-case, CSS class kebab-case; input `name` = field name. Language key `LBL_UPPER_SNAKE` / `JS_`.

### File Locations (beyond File Separation)

Cron service `cron/modules/<Module>/` (logic usually in the model; `modules/<Module>/crons/` rare) · API connectors `include/Webservice/CloudBotApi/` · entry points `include/EntryPoints/` · webhooks `include/Webhooks/` · event handlers `modules/<Module>/handlers/` · API endpoints `api/` (logic in `include/utils/`, the shared utils dir) · downloaded PHP libs `libraries/` (strip LICENSE/examples/demo) · shared CSS/JS libs `resources/`.

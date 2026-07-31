# CloudGo Development Rules — Core

> **Always loaded.** Contains rules that apply to ALL work. File-type specific rules are in path-scoped sibling files (auto-loaded by Claude Code when matching files are edited).

## Path-scoped sibling rules

| File | Loads when editing |
|------|--------------------|
| `php-conventions.md` | `**/*.php` |
| `security-rules.md` | `**/*.php` |
| `performance-rules.md` | `**/*.php` |
| `migration-rules.md` | `modules/CPMigration/migrations/**` |
| `api-connector-rules.md` | `include/Webservice/**`, `modules/CP*Integration/**` |
| `javascript-rules.md` | `**/*.js` |
| `css-rules.md` | `**/*.css`, `**/*.tpl` |
| `language-rules.md` | `languages/**/*.php` |

## Error Patterns (MANDATORY check)

> **Always loaded.** Catalog các bug đã từng gặp với rule phòng tránh. Khi code → tránh; khi review → check trigger keywords.

📄 [`error-patterns.md`](./error-patterns.md) — Đọc trước khi viết code mới chạm các module/method đã có entry. `code-reviewer` agent BẮT BUỘC scan file này trước khi review.

**[MANDATORY khi review/code PHP]** Trước khi review hoặc sinh code PHP, BẮT BUỘC load: [`php-conventions.md`](./php-conventions.md) (đặc biệt mục **Brace Style** — codebase là K&R, KHÔNG phải PSR-12 Allman), [`security-rules.md`](./security-rules.md), [`performance-rules.md`](./performance-rules.md). Vi phạm brace/security/performance là finding (CRITICAL/HIGH/PERF), không phải bỏ qua. Subagent sinh PHP mặc định PSR-12 Allman → SAI; phải audit (grep dòng `{` đứng riêng, header `@author` trùng) trước khi commit, KHÔNG tin self-report của subagent.

**Sau khi fix bug mới:** `cook` (Phase 10.6) / `fix` (Step 9) skill phải hỏi user có bổ sung entry vào `error-patterns.md` không.

---

## ⚠️ File Separation Rules (MANDATORY — NON-NEGOTIABLE)

Violations break the codebase. Enforced by `pretooluse-no-inline-css-js.sh` hook.

1. **NO INLINE CSS/JS**: CSS and JavaScript MUST be in separate files, NEVER inline in PHP or TPL files
2. **CSS/JS Location**:
   - CSS: **ALWAYS** `modules/<Module>/resources/<ViewName>.css`
   - JS (core views — List, Edit, Detail): `layouts/v7/modules/<Module>/resources/<ViewName>.js`
   - JS (custom views — Config, reports, custom pages): `modules/<Module>/resources/<ViewName>.js`
3. **NO HTML IN PHP**: HTML markup MUST be in `.tpl` template files, NEVER directly in PHP classes
   - Templates (core): `layouts/v7/modules/<Module>/<ViewName>.tpl`
   - Templates (custom): `modules/<Module>/tpls/<ViewName>.tpl`
   - PHP Views only set data and call templates

**Correct Structure (core view — inherits parent controller):**
```
layouts/v7/modules/Accounts/
├── Detail.tpl                                # HTML template
├── resources/
│   └── Detail.js                             # JS controller (core view)
modules/Accounts/
├── resources/
│   └── Detail.css                            # CSS (always here)
```

**Correct Structure (custom view — standalone controller):**
```
modules/Products/
├── views/CheckWarranty.php                   # View controller
├── resources/
│   ├── CheckWarranty.js                      # JS controller (custom view)
│   └── CheckWarranty.css                     # CSS (always here)
layouts/v7/modules/Products/
├── CheckWarranty.tpl                         # HTML template
```

---

## General Principles

- **YAGNI**: Don't implement features before they are needed
- **KISS**: Favor clarity over cleverness
- **DRY**: Extract common logic into reusable functions/helpers
- **Framework First**: Use VTiger patterns; avoid reinventing
- **File Size**: Keep under 200 lines; split into focused modules
- **File Naming**: Use kebab-case with descriptive names for LLM readability
- **Real Code Only**: No mocking or simulating — always implement real code
- **Edit, Don't Duplicate**: Update existing files directly; don't create "enhanced" copies
- **Syntax check**: `php -l` after every PHP file (auto-enforced by `posttooluse-php-syntax.sh`)
- **Code review**: Use `code-reviewer` agent after every implementation
- **No secrets**: Never commit confidential data (.env, API keys, credentials) — enforced by `pretooluse-protect-secrets.sh`
- **Conventional commits**: No AI references in messages — enforced by `pretooluse-block-debug-commit.sh`

### Code Quality

- Prioritize functionality and readability over strict style enforcement
- Ensure no syntax errors (`php -l`) — code must be compilable
- Use try/catch error handling and cover security standards
- Use `code-reviewer` agent after every implementation

### Pre-commit / Pre-push

- Run linting before commit, run tests before push
- Never commit confidential data (.env, API keys, credentials)
- Use conventional commit format; no AI references in messages
- Keep commits focused on actual code changes — no unrelated debug code (e.g., the `return true;` in `Users::doLogin()`)

---

## Quick Reference

### View Base Classes (CRITICAL — pick correctly BEFORE writing the view)

| View type | Extends | TPL location | Notes |
|-----------|---------|--------------|-------|
| Core view (List / Edit / Detail của entity module) | `Vtiger_Index_View` (hoặc subclass `Vtiger_List_View` / `Vtiger_Edit_View` / `Vtiger_Detail_View`) | `layouts/v7/modules/<Module>/<ViewName>.tpl` | Manual `getHeaderScripts()` / `getHeaderCss()` nếu cần custom JS/CSS |
| **Custom view** (Config, custom page, landing, report page…) | **`CustomView_Base_View`** | **`modules/<Module>/tpls/<ViewName>.tpl`** | Auto-load `modules/<Module>/resources/<ViewName>.{js,css}` theo viewName. Gọi TPL bằng `$viewer->display('modules/<Module>/tpls/<ViewName>.tpl')` |
| Settings view | `Settings_Vtiger_Index_View` | `layouts/v7/modules/Settings/<Module>/<ViewName>.tpl` | Auto check `isAdminUser()` |

**Tự kiểm tra trước khi gõ `class ... extends`:**
- View của tôi có phải List/Edit/Detail core? → `Vtiger_Index_View`.
- Là Settings page? → `Settings_Vtiger_Index_View`.
- Còn lại (Config, custom page, dashboard, landing…) → **`CustomView_Base_View`** + TPL ở `modules/<Module>/tpls/`.

Chi tiết auto-load JS/CSS, helper class skeleton: skill `view/references/custom-view-base.md`.

### Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| PHP Class | `<Module>_<Component>_<Type>` | `Accounts_Record_Model` |
| PHP File (autoload) | `<Component>.php` (no type suffix) | `helpers/Logic.php` |
| PHP Method | camelCase + verb | `getAccountName()` |
| PHP Variable | camelCase | `$recordId` |
| PHP Constant | UPPER_SNAKE | `STATUS_ACTIVE` |
| JS Controller | `<Module>_<View>_Js` | `Products_Config_Js` |
| DB Column | snake_case | `account_name` |
| Language Key | `LBL_UPPER_SNAKE` / `JS_` | `LBL_ACCOUNT_NAME` |
| Migration | `YYYY.MM.DD.HH.mm.ss_Name.php` | `2025.04.16.10.30.00_AddSocialAds.php` |
| CSS Class | kebab-case | `last-campaign-link` |

### File Locations

| Type | Location |
|------|----------|
| Model | `modules/<Module>/models/` |
| View | `modules/<Module>/views/` |
| Action | `modules/<Module>/actions/` |
| Helper | `modules/<Module>/helpers/` |
| Cron | `modules/<Module>/crons/` |
| Cron Service | `cron/modules/<Module>/` |
| Template (core) | `layouts/v7/modules/<Module>/` |
| Template (custom) | `modules/<Module>/tpls/` |
| CSS | `modules/<Module>/resources/` |
| JS (core views) | `layouts/v7/modules/<Module>/resources/` |
| JS (custom views) | `modules/<Module>/resources/` |
| Language (R&D) | `languages/<locale>/ModuleName.php` |
| Language (DEV) | `languages/<locale>/dev/ModuleName.php` |
| Shared Utils | `include/utils/` |
| Migrations | `modules/CPMigration/migrations/` |
| API Connectors | `include/Webservice/CloudBotApi/` |
| Entry Points | `include/EntryPoints/` |
| Webhooks | `include/Webhooks/` |
| Event Handler (per-module) | `modules/<Module>/handlers/` |
| API endpoints | `api/` (logic qua util trong `include/utils/`) |
| Thư viện PHP tải về | `libraries/` (xoá file thừa: LICENSE, examples, demo) |
| CSS/JS lib dùng chung | `resources/` (ngang hàng index.php) |

**Hạn chế sửa core tối đa** — không có giải pháp ngoài sửa core thì phải hỏi Leader chốt phương án trước (nguồn: DevKit nội bộ).

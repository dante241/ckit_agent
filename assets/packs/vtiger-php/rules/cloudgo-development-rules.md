---
description: "Core CloudGO dev rules áp dụng mọi việc — file-separation, brace K&R, quy trình, error-pattern gate. Always-on."
alwaysApply: true
---

# CloudGo Development Rules — Core

> **Always loaded** (`alwaysApply`). Contains rules that apply to ALL work. File-type rules in the table below are TTSR: omp auto-injects one only when you edit/write a matching file and does not list them in the prompt — read `rule://<name>` yourself whenever you code OR review a matching file. This table is the only place agents see TTSR rules: keep it in sync with each rule's frontmatter `scope`.

## Path-scoped sibling rules

| File | Áp dụng cho (code + review) |
|------|--------------------|
| `security-rules.md` | `**/*.php` |
| `performance-rules.md` | `**/*.php` |
| `migration-rules.md` | `modules/CPMigration/migrations/**` |
| `api-connector-rules.md` | `include/Webservice/**/*.php`, `modules/CP*Integration/**/*.php` |
| `css-rules.md` | `**/*.css`, `**/*.tpl` |
| `language-rules.md` | `languages/**/*.php` |

Always-on (`alwaysApply`, không phải TTSR — luôn nằm trong prompt): `cloudgo-development-rules.md` (file này), `php-conventions.md` (code + review PHP), `javascript-rules.md` (code + review JS).

**[MANDATORY khi review] Rule code = tiêu chí review.** Review/audit bất kỳ diff hay file nào (MR, `/review-pr`, `code-reviewer`, validator, subagent) → TRƯỚC khi đánh giá, BẮT BUỘC `read rule://<name>` cho MỌI rule trong bảng trên có glob khớp file bị đổi, kèm `error-patterns`. `scope: tool:edit/write` trong frontmatter chỉ quyết định LÚC NÀO rule tự nạp (khi sửa), không quyết định rule có áp dụng khi review hay không — cấm bỏ qua/bác finding với lý do "rule chỉ áp dụng khi sửa code". Rule áp cho loại file nào thì vẫn theo glob ở bảng + chính nội dung rule (vd "in PHP or TPL files").

## Error Patterns (MANDATORY check)

> **Không tự nạp** (domain rule, đọc bằng `read rule://error-patterns`). BẮT BUỘC đọc khi code chạm module/method đã có entry và khi review. Catalog các bug đã từng gặp với rule phòng tránh. Khi code → tránh; khi review → check trigger keywords.

📄 [`error-patterns.md`](./error-patterns.md) — Đọc trước khi viết code mới chạm các module/method đã có entry. `code-reviewer` agent BẮT BUỘC scan file này trước khi review.

**[MANDATORY khi review/code PHP]** Trước khi review hoặc sinh code PHP, BẮT BUỘC load: [`php-conventions.md`](./php-conventions.md) (đặc biệt mục **Brace Style** — codebase là K&R, KHÔNG phải PSR-12 Allman), [`security-rules.md`](./security-rules.md), [`performance-rules.md`](./performance-rules.md). Vi phạm brace/security/performance là finding (CRITICAL/HIGH/PERF), không phải bỏ qua. Subagent sinh PHP mặc định PSR-12 Allman → SAI; phải audit (grep dòng `{` đứng riêng, header `@author` trùng) trước khi commit, KHÔNG tin self-report của subagent.

**Sau khi fix bug mới:** `/cook` (Phase 10.6) / `fix` (Step 9) phải hỏi user có bổ sung entry vào `error-patterns.md` không.

---

## ⚠️ File Separation Rules (MANDATORY — NON-NEGOTIABLE)

Violations break the codebase. No hook enforces this — check it yourself before finishing and when reviewing.

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
modules/Products/tpls/
├── CheckWarranty.tpl                         # HTML template (custom view)
```

---

## General Principles

- **YAGNI**: Don't implement features before they are needed
- **KISS**: Favor clarity over cleverness
- **DRY**: Extract common logic into reusable functions/helpers
- **Framework First**: Use VTiger patterns; avoid reinventing
- **File Size**: Keep under 200 lines; split into focused modules
- **File Naming**: Follow the Naming Conventions table below (PHP autoload `<Component>.php`, PascalCase JS/CSS/TPL filenames)
- **Real Code Only**: No mocking or simulating — always implement real code
- **Edit, Don't Duplicate**: Update existing files directly; don't create "enhanced" copies
- **Syntax check**: `php -l` after every PHP file
- **Code review**: Use `code-reviewer` agent after every implementation
- **No secrets**: Never commit confidential data (.env, API keys, credentials)
- **Conventional commits**: No AI references in messages

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
| Cron logic (hiếm — repo chỉ có 1 file; đa số logic cron nằm trong model) | `modules/<Module>/crons/` |
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

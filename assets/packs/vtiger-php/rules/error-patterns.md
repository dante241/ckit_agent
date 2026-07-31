# Error Patterns — Anti-Patterns / Lessons Learned

> **Always loaded.** Bugs đã từng gặp trong codebase, format chuẩn hóa để Claude Code và human dễ search/áp dụng. Khi code → tránh; khi review → check.

## Format

Mỗi entry:

```markdown
## EP-NNN: <Tên ngắn>

**Ticket / MR:** #NNNNN / !NNN
**Date:** YYYY-MM-DD
**Module / Layer:** <Module hoặc framework layer>

**Symptom:** <Hiện tượng người dùng / dev nhìn thấy>

**Root cause:** <Nguyên nhân kỹ thuật cụ thể>

**Rule:** <Quy tắc 1-2 câu để tránh tái phát>

**Trigger keywords (review):** <từ khóa code-reviewer scan để bắt pattern>
```

ID `EP-NNN` chạy tăng dần. Không xóa entry cũ — fix rồi vẫn giữ làm tham khảo.

---

<!-- Entries appear below this line. Add via Phase 10.6 (cook) / Step 9 (fix) USER GATE. -->

## EP-001: Custom view sai base class / TPL location / Helper filename

**Ticket / MR:** — (Quick Guide POC review 2026-05-13)
**Date:** 2026-05-13
**Module / Layer:** View layer (any custom view module)

**Symptom:** Trang custom view render trắng / CSS-JS không auto-load / autoload class fail / TPL không tìm thấy.

**Root cause:** 3 lỗi liên hoàn khi tạo custom view (không phải core List/Edit/Detail):
1. `extends Vtiger_Index_View` thay vì `CustomView_Base_View` → mất auto-load CSS/JS theo viewName.
2. TPL đặt ở `layouts/v7/modules/<Module>/` (chỗ của core view) thay vì `modules/<Module>/tpls/`.
3. Helper filename `<Name>Helper.php` thay vì `<Name>.php` → VTiger autoload không tìm được class `<Module>_<Name>_Helper`.

**Rule:** Custom view (Config, Report, custom page, landing…) — BẮT BUỘC:
- View `extends CustomView_Base_View` (source: `modules/CustomView/views/Base.php`).
- TPL: `modules/<Module>/tpls/<ViewName>.tpl`, gọi bằng `$viewer->display('modules/<Module>/tpls/<ViewName>.tpl')`.
- Helper file: `modules/<Module>/helpers/<Component>.php` — filename = Component part only, KHÔNG suffix `Helper`/`Model`/`View`.
- Core view (List/Edit/Detail của entity module) thì ngược lại: `extends Vtiger_Index_View` + TPL ở `layouts/v7/modules/<Module>/`.

Đọc đầy đủ ở skill `view/references/custom-view-base.md` TRƯỚC khi tạo file.

**Trigger keywords (review):** `extends Vtiger_Index_View` (trong custom view, không phải List/Edit/Detail) · file `*Helper.php` trong `helpers/` · TPL trong `layouts/v7/modules/<Module>/` mà không phải List/Edit/Detail · `$viewer->view(` với path tương đối.

---

## EP-002: Language key đặt sai array ($languageStrings vs $jsLanguageStrings)

**Ticket / MR:** #22219 / —
**Date:** 2026-06-02
**Module / Layer:** Language files (`languages/<locale>/**`)

**Symptom:** TPL `{vtranslate('LBL_FOO')}` render ra chuỗi raw key `LBL_FOO` thay vì text dịch. Hoặc JS `app.vtranslate('JS_FOO')` trả về `JS_FOO`. Notice/label hiển thị tên key lủng củng.

**Root cause:** File ngôn ngữ có 2 array tách scope: `$languageStrings` (đọc bởi PHP/TPL `vtranslate()`) và `$jsLanguageStrings` (đọc bởi `app.vtranslate()`). Hai scope KHÔNG thấy nhau. Khi thêm key mới mà append mù vào cuối file → rơi vào `$jsLanguageStrings` (vì dấu `);` cuối file đóng array JS). `LBL_` nằm nhầm trong array JS → `vtranslate()` không thấy → render raw key. Lỗi không bị `php -l` bắt (cú pháp vẫn hợp lệ).

**Rule:** `LBL_*` → `$languageStrings`; `JS_*` → `$jsLanguageStrings`. KHÔNG append cuối file mà không xác định array. Label dùng ở CẢ TPL lẫn JS phải có key ở cả 2 array (`LBL_X` + `JS_X`). Sau mỗi sửa, verify cho CẢ 2 locale:
```bash
awk '/languageStrings = array/{a="LBL-array"} /jsLanguageStrings = array/{a="JS-array"} /YOUR_KEY/{print a": "$0}' languages/en_us/<File>.php
```
Kỳ vọng `LBL_*`→`LBL-array`, `JS_*`→`JS-array`. Chi tiết: skill `language` Pitfall #8.

**Trigger keywords (review):** key `LBL_` hoặc `JS_` thêm sát trước dấu `);` cuối file ngôn ngữ · `LBL_` bên trong block `$jsLanguageStrings` · `JS_` bên trong block `$languageStrings` · thêm language string mà không có grep/awk verify array.

# Error Patterns — PENDING (draft tự động, chờ user promote vào error-patterns.md)

> Flow: cook Phase 10.6 / fix Step 9 auto-draft vào đây (KHÔNG cần approve).
> User review định kỳ → promote entry tốt sang `error-patterns.md` (gán EP-NNN) → hook enforce từ đó.

---

## PEND-field-migration: Tạo field entity bằng SQL migration thay vì BFR

**Ticket / MR:** — (user báo lỗi lặp lại nhiều lần, 2026-07-03)
**Date:** 2026-07-03
**Module / Layer:** Field creation (mọi entity module)

**Symptom:** Field mới có cột trong DB nhưng không có metadata `vtiger_field` đúng, hoặc có migration ALTER TABLE thừa song song với BFR; label không hiển thị; layout không thấy field.

**Root cause:** AI viết migration `ALTER TABLE vtiger_x ADD COLUMN` theo thói quen framework khác. Chuẩn CloudGo: field entity đi qua `BlocksAndFieldsRegister.php` + `php cli_tool.php quick_repair` (tự tạo cột DB) + label ở CẢ `languages/en_us/` VÀ `languages/vn_vn/` ($languageStrings).

**Rule:** Field trên entity module = BFR + quick_repair + 2 locale label, KHÔNG BAO GIỜ migration. Migration chỉ cho bảng phi-entity (quan hệ, config, index). Đã enforce bằng tripwire T9.

**Trigger keywords (review):** ALTER TABLE vtiger_ · ADD COLUMN

---

## PEND-field-uitype: Chọn sai uitype khi tạo field

**Ticket / MR:** — (user báo lỗi lặp lại nhiều lần, 2026-07-03)
**Date:** 2026-07-03
**Module / Layer:** Field creation / BlocksAndFieldsRegister

**Symptom:** Field render sai control (datetime thành text, dropdown không có giá trị), typeofdata/columntype lệch chuẩn codebase.

**Root cause:** AI chọn uitype theo tài liệu VTiger open-source thay vì theo codebase: dùng 50 cho datetime (codebase dùng 70 — 236 fields, 0 field dùng 50), dùng 15 cho dropdown thường (chuẩn là 16 — 389 fields), tưởng 19 là varchar (là mediumtext).

**Rule:** Chọn uitype = tra bảng "UITypes Quick Reference" trong skill `field` (build từ BFR thật) hoặc grep 1 field cùng loại trong `modules/*/BlocksAndFieldsRegister.php` và copy entry. Datetime = 70. Dropdown = 16. Verify bước 1 của skill field so uitype trong DB với bảng.

**Trigger keywords (review):** 'uitype' => '50' · 'uitype' => '15'

---

## PEND-brace-copy-old-code: AI bắt chước brace/comment style của code cũ xung quanh thay vì theo php-conventions.md

**Ticket / MR:** #10432 (user báo lỗi 2026-07-03)
**Date:** 2026-07-03
**Module / Layer:** Bất kỳ file PHP core cũ nào (Save.php, SaveAjax.php, và các file legacy khác)

**Symptom:** Code mới AI viết dùng `} else {` cùng dòng (K&R) thay vì `else` xuống dòng riêng (Stroustrup); comment dài nhiều dòng giải thích chi tiết thay vì 1 dòng ngắn; nhét ticket ID (`[Fix #10432]`) vào code comment. Code-reviewer subagent review vẫn báo "PASS" — không bắt được các lỗi này.

**Root cause:** File core cũ (VD `Save.php`, `SaveAjax.php`) được viết theo chuẩn cũ của công ty (else cùng dòng `}`, comment style khác). Khi sửa file này, AI pattern-match theo code cũ *nhìn thấy ngay trong file* (context cục bộ) thay vì tra lại `php-conventions.md` (rule cố định, đã đọc từ đầu phiên nhưng bị "quên" theo ngữ cảnh sau nhiều bước trung gian: tìm ticket, trace root cause...). File core cũ ≠ chuẩn để bắt chước — chuẩn mới của công ty được định nghĩa DUY NHẤT trong `php-conventions.md`.

**Rule:** Khi sửa file PHP bất kỳ (kể cả file core cũ dùng chuẩn cũ), brace/comment style của code MỚI phải theo `php-conventions.md`, KHÔNG bắt chước style code cũ xung quanh trong cùng file. Cụ thể: else/elseif/catch/finally luôn xuống dòng riêng (Stroustrup) dù `if` cùng dòng (K&R); comment 1 dòng ngắn `// Modified by X on DATE to REASON`, không nhét ticket/finding ID (rule `review-audit-self-decision.md` — Stable Code Artifacts). Sau khi code xong, BẮT BUỘC tự grep diff đối chiếu (không tin subagent review một mình):
```bash
git diff -- <file> | grep -n "}\s*else\|}\s*catch\|}\s*finally"   # phải KHÔNG match cho code mới (dòng bắt đầu bằng +)
git diff -- <file> | grep -n "^\+.*#[0-9]\{3,\}"                    # ticket ID lọt vào comment
```

**Trigger keywords (review):** `+\t\t\t} else {` hoặc tương tự trong diff (dòng thêm mới có `} else {`/`} catch (`/`} finally {`) · comment thêm mới dài > 1 dòng · `[Fix #` hoặc `#\d{4,}` trong comment thêm mới

---

## PEND-conditional-var-used-outside-if: Biến chỉ gán trong nhánh `if` nhưng dùng vô điều kiện ngoài `if`

**Ticket / MR:** #17557
**Date:** 2026-07-03
**Module / Layer:** Bất kỳ hàm PHP nào có biến trung gian gán trong `if(...)` rồi dùng lại sau khối `if` mà không có `else`

**Symptom:** Khi điều kiện của `if` trở thành `false` (branch trước giờ luôn `true` trong thực tế, chưa từng bị test qua nhánh `false`) → biến dùng ngoài `if` là undefined → PHP warning, hoặc logic rơi vào giá trị rỗng/sai (SQL `IN ()` rỗng, `$db->num_rows($result)` gọi trên biến chưa gán).

**Root cause:** `HelpDesk_Module_Model::getOpenTickets()`/`getTicketsByStatus()` (`modules/HelpDesk/models/Module.php`) gán `$picklistvaluesmap`/`$result` chỉ bên trong `if(vtws_isRoleBasedPicklist('ticketstatus'))`, rồi dùng biến đó vô điều kiện ngay sau khối `if` (không có `else` khởi tạo giá trị mặc định). Vì `ticketstatus` từ trước tới giờ LUÔN role-based (`true`), nhánh `false` chưa từng chạy qua production — bug tiềm ẩn nằm im cho tới khi 1 thay đổi khác (thêm field vào `$nonRoleBasedPicklists`) làm hàm điều kiện trả `false` lần đầu tiên.

**Rule:** Khi viết `if(cond) { $x = ...; }` rồi dùng `$x` sau khối `if` mà không có nhánh `else`, PHẢI: (1) khởi tạo `$x` với giá trị mặc định AN TOÀN trước `if`, hoặc (2) thêm `else { $x = <default>; }`, hoặc (3) bọc luôn phần dùng `$x` vào trong `if`. Đặc biệt cảnh giác khi sửa 1 hàm helper/config dùng chung (vd `vtws_isRoleBasedPicklist()`) — BẮT BUỘC grep toàn bộ call site (`grep -rn "tên_hàm("`) và kiểm tra từng nơi có giả định ngầm "điều kiện này luôn true/false" hay không, trước khi đổi hành vi hàm đó.

**Trigger keywords (review):** biến gán trong `if(...) { $var = ...; }` không có `else`, được dùng lại (đọc/gọi method) ở dòng ngay sau khối `if` cùng scope · thay đổi return value của 1 hàm helper dùng chung mà không grep toàn bộ call site trước

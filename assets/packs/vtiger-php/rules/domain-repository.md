---
description: "Checklist khi sửa Repository trong domains/ (XxxRepository): SQL bind, ép kiểu, fetchByAssoc raw, assertResult, upsert, scope kênh."
scope: "tool:edit(domains/**/*Repository.php), tool:write(domains/**/*Repository.php)"
condition: ".*"
---

# Domain Repository (`domains/**/*Repository.php`)

Bổ sung cho `domain-architecture.md` §2, §6. `PearDatabase` inject qua constructor — không `PearDatabase::getInstance()`, không `global $adb`, không logic nghiệp vụ.

- `final class XxxRepository implements XxxStore`. JOIN sang bảng domain khác chỉ để đọc + comment lý do.
- **Mọi giá trị đi qua `?`** — kể cả id, `LIMIT`/`OFFSET`, IN-list (`generateQuestionMarks`). Không nội suy biến vào chuỗi SQL. Tên cột/bảng động chỉ lấy từ whitelist hằng trong class.
- **Mọi lần đọc dùng `fetchByAssoc($result, -1, false)`** (raw): mặc định vtiger HTML-encode mọi cột chuỗi → nội dung/tên bị `&amp;`/`&quot;` và encode 2 lần khi xuất. Escape để glue/tpl lo.
- **Ép kiểu từng cột** khi dựng kết quả (`fromRow()`): `(int)`, `(bool)` (`(int) $v === 1`), `(string)`, cột NULL → `null` thật; JSON decode tại đây.
- `pquery` trả `false` → `throw <Area>Exception` (helper `assertResult()`). **Không catch.**
- Danh sách lớn: phân trang (cursor hoặc `LIMIT ?`), một query cho nhiều id (không N+1).
- Ghi trùng/đua: `INSERT … ON DUPLICATE KEY UPDATE` hoặc khoá unique thay vì đọc-rồi-ghi.
- Repository scope theo kênh nhận `string $channel` ở constructor và đưa vào mọi `WHERE`.
- **`<Vendor>XxxRepository`** (lưu qua API vendor, vd `Chathub/ChathubCommentRepository`): checklist SQL ở trên không áp; gọi qua client `integrations/<Vendor>` inject ở constructor; bắt lỗi client (`ChathubException`) → `throw <Area>Exception(…, 0, $e)`; dữ liệu cần ở CRM (vd cờ delivery) đi qua một Store SQL riêng inject vào, không tự viết SQL. Đổi định dạng vendor (ms, mảng đính kèm) sang shape của Store tại đây.

Mẫu: `domains/Social/src/Conversation/ConversationRepository.php` (select/update/assertResult, raw fetch, `LAST_INSERT_ID`), `Comment/CommentDeliveryRepository.php` (`ON DUPLICATE KEY`, `LIMIT ?`), `Chathub/ChathubCommentRepository.php` (vendor).

---
description: "Checklist khi sửa Repository trong domains/ (XxxRepository): vị trí trong luồng gọi, SQL bind, ép kiểu, fetchByAssoc raw, throw không catch."
scope: "tool:edit(domains/**/*Repository.php), tool:write(domains/**/*Repository.php)"
condition: ".*"
---

# Domain Repository (`domains/**/*Repository.php`)

## Luồng gọi (▶ = file đang sửa)

```
Glue ── <Domain>::xxxService() ── XxxService
                                     └─ XxxStore (interface)
                                          └─▶ XxxRepository     nơi DUY NHẤT có SQL
                                                └─ PearDatabase (inject qua constructor)
```

- **Được gọi bởi**: Service, qua interface `XxxStore`. Tạo (`new`) chỉ trong composition root `<Domain>.php`.
- **Được gọi**: `PearDatabase` (`pquery`, `fetchByAssoc`, `getAffectedRowCount`, `getLastInsertID`), VO `fromRow()`, `generateQuestionMarks()`.
- **KHÔNG**: Service, Gateway/vendor, `PearDatabase::getInstance()` tự lấy, `global $adb`, logic nghiệp vụ (rule chọn kênh, quota…).

## Checklist

- `final class XxxRepository implements XxxStore`. Chỉ ghi bảng của domain mình. JOIN sang bảng domain khác chỉ để **đọc** (danh sách/báo cáo) + comment lý do.
- **Mọi giá trị đi qua `?`** — kể cả id, `LIMIT`/`OFFSET`, IN-list (`generateQuestionMarks`). Không nội suy biến vào chuỗi SQL. Tên cột/bảng động chỉ lấy từ whitelist hằng trong class.
- **Mọi lần đọc dùng `fetchByAssoc($result, -1, false)`** (raw): mặc định vtiger HTML-encode mọi cột chuỗi → nội dung/tên bị `&amp;`/`&quot;` và encode 2 lần khi xuất. Escape để glue/tpl lo.
- **Ép kiểu từng cột** khi dựng kết quả: `(int)`, `(bool)` (`(int) $v === 1`), `(string)`, cột NULL → `null` thật; JSON decode tại đây.
- `pquery` trả `false` → `throw <Area>Exception` (helper `assertResult()`). **Không catch.** Không trả `false` cho lỗi DB.
- Không thấy → `null`; rỗng → `[]`; tồn tại/không → `bool`. Danh sách lớn: phân trang (cursor hoặc `LIMIT ?`), một query cho nhiều id (không N+1).
- Ghi trùng/đua: `INSERT … ON DUPLICATE KEY UPDATE` hoặc khoá unique thay vì đọc-rồi-ghi.
- Repository scope theo kênh nhận `string $channel` ở constructor và đưa vào mọi `WHERE`.
- **`<Vendor>XxxRepository`** (lưu qua API vendor, vd `Chathub/ChathubCommentRepository`): checklist SQL ở trên không áp; gọi qua client `integrations/<Vendor>` inject ở constructor; bắt lỗi client (`ChathubException`) → `throw <Area>Exception(…, 0, $e)`; dữ liệu cần ở CRM (vd cờ delivery) đi qua một Store SQL riêng inject vào, không tự viết SQL. Đổi định dạng vendor (ms, mảng đính kèm) sang shape của Store tại đây.

Mẫu: `domains/Social/src/Conversation/ConversationRepository.php` (select/update/assertResult, raw fetch, `LAST_INSERT_ID`), `Comment/CommentDeliveryRepository.php` (`ON DUPLICATE KEY`, `LIMIT ?`), `Chathub/ChathubCommentRepository.php` (vendor). Chi tiết: `domain-architecture.md` §2, §6.

---
description: "Checklist khi sửa Service trong domains/ (XxxService): tham số đã ép kiểu, không gọi vtiger/thời gian trực tiếp, logic quyết định thuần."
scope: "tool:edit(domains/**/*Service.php), tool:write(domains/**/*Service.php)"
condition: ".*"
---

# Domain Service (`domains/**/*Service.php`)

Bổ sung cho bảng Service ở `domain-architecture.md` §2, §6, §7:

- KHÔNG: `include/utils/*Utils`, `vtranslate`, `date()`/`time()`/`new DateTime()` (dùng `ClockInterface`), `<Domain>::` (service không tự gọi composition root).
- Constructor nhận **interface**, gán vào property có kiểu.
- Tham số đã ép kiểu / là VO — không nhận `array $data` thô từ request.
- `try/catch` có fallback nghiệp vụ → comment ngắn nói fallback gì.
- Logic quyết định thuần (tính quota, chọn kênh, rule scope) tách thành method/class thuần để unit test không cần fake.

Mẫu: `domains/Social/src/Ingestion/CommentInboundService.php` (status constants, để gateway lỗi đi lên), `Comment/CommentThreadService.php` `postCard()` (catch có fallback).

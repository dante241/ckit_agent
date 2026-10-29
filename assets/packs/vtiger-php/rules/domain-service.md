---
description: "Checklist khi sửa Service trong domains/ (XxxService): vị trí trong luồng gọi, phụ thuộc interface, return/throw/catch."
scope: "tool:edit(domains/**/*Service.php), tool:write(domains/**/*Service.php)"
condition: ".*"
---

# Domain Service (`domains/**/*Service.php`)

## Luồng gọi (▶ = file đang sửa)

```
Glue (modules action/view/handler/cron · api/* · include/Webservice · include/Webhooks · CPQueue job)
  └─ <Domain>::xxxService()                  composition root <Domain>.php
       └─▶ XxxService                         logic nghiệp vụ
             ├─ XxxStore (interface)    ── XxxRepository          SQL
             ├─ XxxGateway (interface)  ── <Vendor>XxxGateway ── integrations/<Vendor>
             │                          └─ Vtiger…/legacy adapter
             └─ Shared (ClockInterface…) · <OtherDomain>::xxxService() qua Gateway của mình
```

- **Được gọi bởi**: glue (qua `<Domain>::xxxService()`), Service khác cùng domain (inject qua constructor).
- **Được gọi**: interface Store/Gateway, `Psr\Clock\ClockInterface`, VO/exception của domain.
- **KHÔNG**: `new` Repository/adapter, `$adb`/`PearDatabase`, `use` class vtiger (`Vtiger_*`, `CPSocialIntegration_*`, `Settings_*`), `include/utils/*Utils`, `$_REQUEST`/`Vtiger_Request`, `echo`/HTML, `vtranslate`, `date()`/`time()`/`new DateTime()`, `<Domain>::` (service không tự gọi composition root).

## Checklist

- `final class`, KHÔNG có interface (1 bản cài). Constructor nhận **interface**, gán vào property có kiểu.
- Tham số đã ép kiểu / là VO — không nhận `array $data` thô từ request.
- Return: VO / `list<VO>` / array có shape PHPDoc / hằng trạng thái (`@return self::A|self::B`). Không thấy (bình thường) → `null`; rỗng → `[]`.
- Sự cố hoặc luồng không đi tiếp được → `throw <Area>Exception`. Không trả `false`/`['error']`.
- `try/catch` CHỈ khi có fallback nghiệp vụ có chủ đích (comment ngắn nói fallback gì). Không có fallback → để exception đi lên glue.
- Logic quyết định thuần (tính quota, chọn kênh, rule scope) tách thành method/class thuần để unit test không cần fake.

Mẫu: `domains/Social/src/Ingestion/CommentInboundService.php` (status constants, để gateway lỗi đi lên), `Comment/CommentThreadService.php` `postCard()` (catch có fallback). Chi tiết: `domain-architecture.md` §2, §3a, §6, §7.

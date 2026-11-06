---
description: "Checklist khi sửa Gateway/adapter trong domains/ và client vendor trong integrations/: bọc lỗi vendor, token có lock, không log secret."
scope: "tool:edit(domains/**/*Gateway.php), tool:write(domains/**/*Gateway.php), tool:edit(integrations/**/*.php), tool:write(integrations/**/*.php)"
condition: ".*"
---

# Gateway & integrations (`domains/**/*Gateway.php`, `integrations/**`)

Bổ sung cho `domain-architecture.md` §2, §5, §8. Connector legacy ngoài `domains/` theo `api-connector-rules.md`.

- Interface `XxxGateway`: chỉ khai báo method domain cần; docblock ghi shape trả về + `@throws`.
- Adapter legacy (`Vtiger…Gateway`, `FacebookInboxGateway`): `use` từng class vtiger; không chứa SQL riêng nếu đã có helper legacy làm việc đó.
- Không tự `new` SDK trong từng gateway — đi qua client chung.
- Bắt lỗi SDK/HTTP → `throw new <Area>Exception('…', 0, $e)`; cho exception của domain đi qua nguyên vẹn (catch domain exception rethrow **trước** catch `Throwable`).
- Thời gian vendor về `'Y-m-d H:i:s'` theo múi giờ site.
- Token: refresh ở **một chỗ** (provider trong `integrations/<Vendor>`), có lock; `GET_LOCK` trả 0/NULL (timeout) → fail hoặc đọc lại, không refresh khi không giữ lock; `RELEASE_LOCK` chỉ khi đã giữ. Mọi chỗ ghi token đi qua cùng store đó, đọc lại dưới lock và chỉ merge trường của mình.
- Không log token/secret. Không `vtranslate` — map lỗi sang mã/hằng, glue dịch.

Mẫu:
- `integrations/Meta/FacebookGraphClient.php` — 1 client/vendor, bọc lỗi SDK → `FacebookGraphException`, không biết CRM.
- `domains/Social/src/Channel/Facebook/FacebookCommentGateway.php` — `FacebookGraphException` → `CommentException`; `FacebookGraphNodes::toEvent()` dịch Graph → `CommentEvent`.
- `FacebookScopeGateway.php` — gateway thứ 2 dùng chung client; `PageSettingsException` đi qua nguyên vẹn.
- `FacebookConfig.php` — phần biết CRM (đọc `vtiger_config`) ở lại domain.

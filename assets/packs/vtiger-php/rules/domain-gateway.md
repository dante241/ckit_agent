---
description: "Checklist khi sửa Gateway/adapter trong domains/ và client vendor trong integrations/: vị trí trong luồng gọi, bọc lỗi vendor, dịch dữ liệu, token có lock."
scope: "tool:edit(domains/**/*Gateway.php), tool:write(domains/**/*Gateway.php), tool:edit(integrations/**/*.php), tool:write(integrations/**/*.php)"
condition: ".*"
---

# Gateway & integrations (`domains/**/*Gateway.php`, `integrations/**`)

## Luồng gọi (▶ = file đang sửa)

```
Glue ── <Domain>::xxxService() ── XxxService
                                     └─ XxxGateway (interface, port của domain)
                                          ├─▶ <Vendor>XxxGateway (Channel/<Vendor>/)   dịch vendor ⇄ VO
                                          │      └─▶ integrations/<Vendor> client     HTTP + token, không biết CRM
                                          └─▶ Vtiger…Gateway / legacy adapter          gọi code vtiger cũ
```

- **Interface `XxxGateway`**: đặt trong mảng nghiệp vụ của domain DÙNG nó; chỉ khai báo method domain cần; docblock ghi shape trả về + `@throws`.
- **Adapter vendor** — được gọi bởi Service (qua interface); được gọi: client `integrations/<Vendor>` (hoặc client dùng chung trong `Channel/<Vendor>/` khi mới 1 domain dùng), VO của domain. KHÔNG: Repository, bảng CRM, domain khác.
- **`integrations/<Vendor>`** — được gọi bởi adapter vendor của các domain; được gọi: HTTP/SDK, `domains/Shared`. KHÔNG: bất cứ gì trong `domains/`, crmid, bảng vtiger.
- **Adapter legacy** (`Vtiger…Gateway`, `FacebookInboxGateway`): được gọi class vtiger (`use` từng class); không chứa SQL riêng nếu đã có helper legacy làm việc đó.

## Checklist

- 1 client cho mỗi vendor, 1 gateway mỏng cho mỗi khả năng (comment / chat / đăng bài). Không tự `new` SDK trong từng gateway — đi qua client chung.
- Bắt lỗi SDK/HTTP → `throw new <Area>Exception('…', 0, $e)`; cho exception của domain đi qua nguyên vẹn (catch domain exception rethrow trước catch `Throwable`).
- Dịch payload vendor → VO/event của domain ngay trong adapter; không trả JSON thô của vendor ra Service.
- Id vendor (page, OA, user) luôn `string`; thời gian về `'Y-m-d H:i:s'` theo múi giờ site.
- Token: refresh ở **một chỗ** (provider trong `integrations/<Vendor>`), có lock; `GET_LOCK` trả 0/NULL (timeout) → fail hoặc đọc lại, không refresh khi không giữ lock; `RELEASE_LOCK` chỉ khi đã giữ. Mọi chỗ ghi token đi qua cùng store đó, đọc lại dưới lock và chỉ merge trường của mình.
- Không log token/secret. Không `vtranslate` — map lỗi sang mã/hằng, glue dịch.
- Chuyển code cũ: mang **nguyên thân** hàm legacy (retry, map lỗi) vào adapter; static cũ thành facade 1 dòng giữ format trả về cũ.

Mẫu: `integrations/Meta/FacebookGraphClient.php` (1 client/vendor: SDK, credential/token qua callable, bọc lỗi SDK → `FacebookGraphException`, không biết CRM), `domains/Social/src/Channel/Facebook/FacebookCommentGateway.php` (gateway ghép `FacebookCommentWriter` — đổi `FacebookGraphException` → `CommentException` — và `FacebookGraphNodes::toEvent()` dịch dữ liệu Graph → `CommentEvent`), `FacebookScopeGateway.php` (gateway `final` thứ 2 dùng chung client; để `PageSettingsException` của `GrantedScopes` đi qua nguyên vẹn), `FacebookConfig.php` (phần biết CRM — đọc `vtiger_config` — ở lại domain). Connector legacy ngoài `domains/` vẫn theo `api-connector-rules.md`. Chi tiết: `domain-architecture.md` §5.

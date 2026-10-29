---
description: "Checklist khi viết test trong domains/*/tests: test lớp nào thì dựng gì, Unit/Integration/Legacy/Golden, fake theo interface, PHPUnit 9.6 trên PHP 8.3, PHPStan."
scope: "tool:edit(domains/**/tests/**/*.php), tool:write(domains/**/tests/**/*.php)"
condition: ".*"
---

# Domain tests (`domains/<Domain>/tests/**`)

## Luồng gọi & test lớp nào

```
Glue ── <Domain>::xxxService() ── XxxService ──┬─ XxxStore ── XxxRepository ── DB
                                               └─ XxxGateway ── <Vendor>XxxGateway ── vendor
 Legacy/Golden          (Unit: SocialFactoryTest)   Unit          Integration        Unit (HTTP giả)
```

|Lớp|Suite|Dựng thế nào|
|---|---|---|
|Service|`Unit/`|`new XxxService(new FakeXxxStore(), new FakeXxxGateway(), new FixedClock(...))` — KHÔNG qua `<Domain>::` (factory lắp DB thật)|
|Repository|`Integration/`|DB thật qua `DbTestCase`, chạy với `-d auto_prepend_file=domains/<D>/tests/bootstrap-integration.php`. Không mock `PearDatabase`|
|Vendor gateway|`Unit/`|HTTP/SDK giả + fixture payload thật trong `tests/Fixtures/` (không gọi mạng)|
|Composition root|`Unit/`|Override qua `configureOverrides`, nhớ reset `configureOverrides([])` trong `tearDown`|
|Glue / hành vi cũ|`Legacy/`, `Golden/`|Đối chiếu kết quả trước/sau khi chuyển code (oracle = SQL/hàm cũ, replay webhook)|

## Checklist

- Vị trí `domains/<Domain>/tests/` (tracked); KHÔNG `test/` hay `.cache/` (gitignore).
- PHPUnit **9.6** chạy bằng PHP **8.3** (target D-019; suite cũng còn chạy được trên 7.4) (`tools/vendor/bin/phpunit`, `phpunit.xml.dist`); cài trong `tools/`, không `require-dev` ở composer gốc.
- Fake implement **interface** của port, đặt `tests/Fake/` (vd `FakeCommentStore`, `FakeInboxGateway` có `callsTo()`, `FixedClock`). Class `final` không mock được — luôn fake theo interface.
- Mỗi test kiểm hành vi người dùng thấy được: kết quả, trạng thái lưu, lời gọi ra ngoài, nhánh lỗi. Không test wiring/getter/chuỗi nguyên văn.
- Có nhánh `throw`/fallback → có test cho nhánh đó. Có idempotency/đua → test chạy lại 2 lần cho cùng kết quả.
- Integration test tự dọn dữ liệu; không phụ thuộc thứ tự chạy.
- PHPStan 2.x mức 6 chỉ quét `domains/*/src` (tests loại trừ); code mới sạch, không baseline.

Mẫu: `Unit/CommentInboundServiceTest.php`, `Integration/ConversationRepositoryTest.php`, `Legacy/ConversationListTest.php`, `Unit/SocialFactoryTest.php`.

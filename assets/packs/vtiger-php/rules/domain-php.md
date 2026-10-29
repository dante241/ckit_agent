---
description: "PHP trong domains/ (PSR-4, CloudGo\\<Domain>\\): PHP 8.3 target (D-019), typed, final, Hook→Service→Repository, test ở domains/*/tests. Bổ sung php-conventions; đè phần naming/autoload. Auto-load khi sửa domains/**/*.php."
scope: "tool:edit(domains/**/*.php), tool:write(domains/**/*.php)"
condition: ".*"
---

# Domain PHP (`domains/**/*.php`)

> Áp cho code PSR-4 trong `domains/<Domain>/src` + `domains/<Domain>/tests`. Code module vtiger (`modules/**`) vẫn theo `php-conventions.md` nguyên vẹn.
> Nguồn: D-007, D-011..D-014 trong `agents/DECISIONS.md`. Kiến trúc tầng, ai gọi ai, vendor, trả dữ liệu, exception/try-catch: **`domain-architecture.md`** (đọc kèm).

## Runtime target: PHP 8.3 (D-019, 2026-10-26 — thay mục "PHP 7.2" chốt 2026-10-20)

- Release yêu cầu server PHP 8.3; không còn cam kết chạy 7.x (rollback = rollback release). Code `domains/*/src`, `domains/*/config`, `integrations/` được dùng toàn bộ cú pháp ≤ 8.3.
- **Nên dùng** khi viết mới: typed property, constructor promotion, `readonly` cho VO/DTO, `enum` cho tập giá trị cố định mới, `match` thay `switch` khi map giá trị, union type `A|B` khi thật sự hai kiểu, `?->`, `??=`, arrow `fn`, `str_contains`/`str_starts_with`, kiểu `mixed`/`static` khi đúng nghĩa.
- Không đổi chữ ký public (tham số/return) chỉ để "hiện đại hoá" — đổi chữ ký là đổi contract, mọi caller + bản cài interface phải sửa cùng MR.
- **Gate**: `php -l` bằng PHP 8.3 cho mọi file đã sửa, PHPUnit (`domains/*/tests`, chạy được trên 8.3), PHPStan `tools/phpstan/phpstan-domains.neon.dist` (rule `phpstan.md`). Nạp toàn bộ class `CloudGo\*` dưới 8.3 để bắt lỗi chữ ký interface/abstract.

## Giữ nguyên từ `php-conventions.md`

File header `@author`, K&R brace + `else`/`catch`/`finally` dòng riêng, tab, blank line trước method đầu tiên, comment tiếng Anh ≤ 2 dòng, attribution theo ownership, không căn thẳng `=>`, nháy đơn/đôi, SQL keyword HOA + prepared statement, early return.

## Đè lên `php-conventions.md`

|Mục|Trong `domains/`|
|---|---|
|Tên class `<Module>_<Component>_<Type>`|Namespace `CloudGo\<Domain>\<SubArea>`, class PascalCase|
|Tên file|= tên class (`CommentRepository.php`) — PSR-4 bắt buộc; quy ước tên file vtiger (`<Component>.php`, `cloudgo-development-rules.md` § Naming Conventions) KHÔNG áp|
|`require_once`|Cấm — class domain qua Composer PSR-4; class vtiger `Module_Component_Type` qua `Vtiger_Loader` (tự nạp, không require). File không autoload (`include/utils/*Utils.php`) KHÔNG gọi từ domain — cần thì chuyển phần đó vào `integrations/<Vendor>`|
|Dùng class vtiger/global trong namespace|**Mỗi class phải có `use Xxx;` riêng (hoặc viết `\Xxx`)** — thiếu là PHP hiểu thành `CloudGo\...\Xxx` và fatal; class KHÔNG tự lùi về global. Hàm/hằng global (`saveLog`, `vtranslate`, `date`) tự lùi, không cần `use`. Class vtiger chỉ xuất hiện ở composition root, Repository, adapter — Service/VO không `use` class vtiger|

## Thiết kế

1. **Khai báo kiểu đầy đủ** cho tham số, return, property. `array` public phải ghi shape bằng PHPDoc (`@param array<string, mixed> $row`, `@return list<SocialIdentity>`).
2. **`final` mặc định**; code dùng chung giữa các nền tảng/gateway cùng vendor đặt trong client inject qua constructor (`integrations/<Vendor>`), không qua base `abstract` (`abstract` chỉ theo `domain-architecture.md` §4); inject qua constructor; không state static, không singleton tự chế. Chỉ factory `<Domain>.php` (vd `CloudGo\Social\Social`) là **composition root**: nơi DUY NHẤT `new` class thật + lấy DB (`self::db()`).
2a. **Site override = factory callable trả về interface** (D-014): `configureOverrides([ScopeGateway::class => fn(callable $makeDefault, string $channel): ScopeGateway => …])`. Factory nhận builder lười của bản mặc định + channel (`''` với service không theo channel) → decorator (`new X($makeDefault())`) hoặc thay hẳn; channel khác trả `$makeDefault()`. Factory method trả **kiểu interface**, `resolve()` từ chối kết quả không `instanceof` interface. Factory method nhận `string $channel` khi service phụ thuộc nền tảng.
2b. **Thêm nền tảng** (thay `scopeGatewayBuilders()` của D-013): provider `Channel/<Name>/` có class implement contract plugin (`CommentChannel`) + 1 dòng trong `domains/<Domain>/config/channels.php`; `ChannelRegistry` nạp danh sách, service không biết tên kênh.
3. **Luồng: controller (hook vtiger) → Service → Repository.** Controller = action/view/cron/handler trong module (~15 dòng): đọc + ép kiểu input, lấy Service từ factory (`Social::commentReplyService()`), gọi 1 method, bắt lỗi, trả response. Controller KHÔNG `new` Repository, KHÔNG viết SQL. Chỉ Repository viết SQL; dựng DTO bằng `fromRow()` và **ép kiểu tường minh** (`(int)`, `(bool)`, `(string)`, hoặc `null` khi cột NULL) — `pquery`/`fetchByAssoc` trả mọi cột dạng chuỗi.
3a. **Service phụ thuộc interface, không phụ thuộc class thật.** Constructor Service nhận `CommentStore` (interface), không nhận `CommentRepository`; tương tự Graph client và `ClockInterface`. Lý do: class `final` không mock được (PHPUnit 9: `ClassIsFinalException`, đo 2026-10-05), và Service phải unit test được không cần DB. Repository `implements` interface đó.
    **Tên theo vai (user chốt 2026-10-05, sửa 2026-10-07)** — đọc tên là biết file làm gì: `XxxService` = nghiệp vụ (vd `PageSyncService`) · `XxxStore` = interface lưu trữ (KHÔNG bao giờ là class), `XxxRepository` = bản cài SQL, `<Vendor>XxxRepository` = bản cài lưu qua API vendor (vd `Chathub/ChathubCommentRepository`) · `XxxGateway` = interface hệ thống ngoài, `<Platform>XxxGateway` = bản `final` của từng nền tảng, `<Vendor>GraphClient` / `<Vendor>Client` = client dùng chung của vendor trong `integrations/<Vendor>` (vd `ScopeGateway` ← `FacebookScopeGateway` dùng `CloudGo\Integration\Meta\FacebookGraphClient`) · DTO / quy tắc / exception không hậu tố vai. Thư mục con trong `src/` = mảng nghiệp vụ (`PageSettings/`, `Comment/`) hoặc thư mục vendor gom adapter của vendor đó (`Channel/Facebook/`, `Chathub/`), KHÔNG chia theo tầng (`Port/`, `Infrastructure/`).
3b. **`declare(strict_types=1);`** ở mọi file mới trong `domains/` (chốt 2026-10-05), đặt theo thứ tự PER-CS: `<?php` → header docblock `@author` → `declare(strict_types=1);` → `namespace`. Đi kèm ép kiểu tường minh ở Repository (Thiết kế 3) — strict chỉ áp cho lời gọi phát ra từ file khai báo, code legacy gọi vào không bị ảnh hưởng.
3c. **Lỗi + trả dữ liệu**: theo `domain-architecture.md` §6–§7 — sự cố thì `throw` exception riêng của domain (vd `CommentException`), không trả `false` âm thầm; không thấy → `null`, rỗng → `[]`; glue bắt `Throwable`, ghi log, trả lỗi cho UI.
4. API mới trả VO / array có shape / `null` khi miss. Shape `[]`-khi-miss chỉ còn trong facade legacy.
5. **Tập giá trị cố định**: code mới dùng `enum` (PHP 8.1+, backed `enum Direction: string { case Outbound = 'outbound'; }`) khi giá trị không cần đọc từ code legacy; hằng class cũ (`final class Direction { public const OUTBOUND = 'outbound'; }`) giữ nguyên, không đổi hàng loạt (đổi = đổi contract với caller).
6. **Phụ thuộc ngoài sau interface**: Graph/HTTP client (interface riêng), thời gian qua `Psr\Clock\ClockInterface` (`psr/clock` ^1.0). Logic nghiệp vụ không gọi thẳng `FacebookUtils::`, `date()`, `time()`, `new DateTime()`.
7. **Domain không I/O web**: không `echo`, HTML, `exit`/`die`, `$_REQUEST`/`$_GET`/`Vtiger_Request`, `header()`. Hook đọc request, ép kiểu, truyền tham số.

## Test

Xem **`domain-tests.md`** (tự nạp khi sửa `domains/**/tests/**`): suite theo lớp, fake theo interface, PHPUnit 9.6 (chạy trên PHP 8.3), PHPStan mức 6 trên `domains/*/src`.

## Để sau (đã chốt hoãn)

- PHP-CS-Fixer. Khi làm: `@PER-CS` + `braces_position` `same_line` + `control_structure_continuation_position: next_line` + tắt `no_blank_lines_after_class_opening` + `trailing_comma_in_multiline: [arrays, arguments]` + line ending `\r\n`; chỉ chạy trên `domains/`.

---
description: "PHP trong domains/ (PSR-4, CloudGo\\<Domain>\\): PHP 8.3 target (D-019), typed, final, Hook→Service→Repository, test ở domains/*/tests. Bổ sung php-conventions; đè phần naming/autoload. Auto-load khi sửa domains/**/*.php."
scope: "tool:edit(domains/**/*.php), tool:write(domains/**/*.php)"
condition: ".*"
---

# Domain PHP (`domains/**/*.php`)

> Code PSR-4 trong `domains/<Domain>/src` + `tests` (nguồn D-007, D-011..D-014, D-019). `php-conventions.md` + `cloudgo-development-rules.md` § Style & Comments áp nguyên, trừ bảng dưới; kiến trúc tầng ở `domain-architecture.md`.

- Không đổi chữ ký public (tham số/return) chỉ để "hiện đại hoá" — đổi chữ ký là đổi contract, mọi caller + bản cài interface phải sửa cùng MR. PHPUnit `domains/*/tests` chạy trên PHP 8.3.

## Đè lên `php-conventions.md`

|Mục|Trong `domains/`|
|---|---|
|Tên class `<Module>_<Component>_<Type>`|Namespace `CloudGo\<Domain>\<SubArea>`, class PascalCase|
|Tên file|= tên class (`CommentRepository.php`) — PSR-4 bắt buộc; quy ước tên file vtiger (`<Component>.php`, `cloudgo-development-rules.md` § Naming Conventions) KHÔNG áp|
|`require_once`|Cấm — class domain qua Composer PSR-4; class vtiger `Module_Component_Type` qua `Vtiger_Loader` (tự nạp, không require). File không autoload (`include/utils/*Utils.php`) KHÔNG gọi từ domain — cần thì chuyển phần đó vào `integrations/<Vendor>`|
|Dùng class vtiger/global trong namespace|**Mỗi class phải có `use Xxx;` riêng (hoặc viết `\Xxx`)** — thiếu là PHP hiểu thành `CloudGo\...\Xxx` và fatal; class KHÔNG tự lùi về global. Hàm/hằng global (`saveLog`, `vtranslate`, `date`) tự lùi, không cần `use`. Class vtiger chỉ xuất hiện ở composition root, Repository, adapter — Service/VO không `use` class vtiger|

## Thiết kế

1. **Khai báo kiểu đầy đủ** cho tham số, return, property. `array` public phải ghi shape bằng PHPDoc (`@param array<string, mixed> $row`, `@return list<SocialIdentity>`).
2. **`final` mặc định**; code dùng chung giữa các nền tảng/gateway cùng vendor đặt trong client inject qua constructor (`integrations/<Vendor>`), không qua base `abstract`; không state static, không singleton tự chế.
3. **Controller** (action/view/cron/handler, ~15 dòng): đọc + ép kiểu input, lấy Service từ factory (`Social::commentReplyService()`), gọi 1 method, bắt lỗi, trả response. Không `new` Repository, không SQL.
4. **`declare(strict_types=1);`** ở mọi file mới, thứ tự: `<?php` → header docblock `@author` → `declare(strict_types=1);` → `namespace`.
5. **Tập giá trị cố định**: code mới dùng backed `enum`; hằng class cũ giữ nguyên, không đổi hàng loạt (= đổi contract).
6. **Phụ thuộc ngoài sau interface**: Graph/HTTP client (interface riêng), thời gian qua `Psr\Clock\ClockInterface` (`psr/clock` ^1.0). Logic nghiệp vụ không gọi thẳng `FacebookUtils::`, `date()`, `time()`, `new DateTime()`.
7. **Domain không I/O web**: không `echo`, HTML, `exit`/`die`, `$_REQUEST`/`$_GET`/`Vtiger_Request`, `header()`. Hook đọc request, ép kiểu, truyền tham số.

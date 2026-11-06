---
description: "PHPStan gate — chạy tools/vendor/bin/phpstan (2.2.17) trên file PHP đã sửa trước khi báo xong/commit; cấm ignore/ép kiểu để im lỗi/sinh lại baseline; docblock kiểu thật. Auto-load (TTSR) khi sửa file khớp; cũng là checklist review."
scope: "tool:edit(**/*.php), tool:write(**/*.php), tool:edit(**/*.inc), tool:write(**/*.inc), tool:edit(**/*.service), tool:write(**/*.service), tool:edit(tools/phpstan/**), tool:write(tools/phpstan/**), tool:edit(tools/composer.*), tool:write(tools/composer.*)"
condition: ".*"
---

# PHPStan — gate cho mọi thay đổi PHP

Tài liệu team: `docs/phpstan.md` (cơ chế, phạm vi, baseline). Quy tắc runtime 8.3: `rule://php83`.

## Gate (MUST, trước khi báo xong hoặc commit code PHP)

```bash
# modules api cron/modules include/{EntryPoints,utils,Webhooks,Webservice,WebserviceTraits}
php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/phpstan.neon.dist <các file đã sửa>
# domains/
php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/phpstan-domains.neon.dist <các file đã sửa>
```
- Phải ra `[OK] No errors` (`[ERROR]` → sửa, chạy lại); báo kết quả gate trong tổng kết. MR thường: diff `phpstan-baseline.php` rỗng.
- PHPStan chỉ báo lỗi trong file được truyền vào. Đổi chữ ký hàm/method → MUST chạy kèm mọi file gọi (`lsp references` / `git grep`) hoặc chạy toàn bộ; chỉ chạy file chứa hàm = `[OK]` giả.
- `phpstan-domains.neon.dist` có sẵn lỗi cũ trong `domains/Social` (không do code mới): chỉ cần không phát sinh thêm lỗi.
- Ngoài gate `phpstan.neon.dist` (chỉ scan symbol, không báo lỗi): phần còn lại của `include/`, `includes/`, `vtlib/`, `libraries/`, `data/`, `modules/com_vtiger_workflow`, `modules/Webforms`, `modules/WSAPP`, `include/Webservice/*/tests/*`, `modules/MailManager/models/Message.php`. `include/Webservice/CloudWorkApi/v1/*` loại hẳn; `integrations/` chỉ được `phpstan-domains.neon.dist` scan symbol. Sửa file ở đó → gate không bảo vệ; tự review + chạy thử.

## Môi trường (MUST)

| # | Quy tắc |
|---|---|
| E1 | Chỉ dùng `tools/vendor/bin/phpstan`, ghim **2.2.17** (`tools/composer.json` + lock; `--version` phải ra 2.2.17). KHÔNG dùng phar/bản khác: baseline khớp câu chữ của đúng bản này. |
| E2 | Cài: `composer install -d tools`. KHÔNG `composer update` trong `tools/` trừ khi chủ động nâng PHPStan. |
| E3 | Cần `include/utils/utils.php8.php` (untracked, bản giải mã ionCube; KHÔNG commit). Thiếu → ~2.400 lỗi "function not found" = lỗi môi trường, không phải lỗi code. |
| E4 | Chạy từ gốc repo bằng PHP 8.3 (`php -v`; PHPStan lấy built-in từ chính bản PHP chạy nó), `-d memory_limit=4G`. |
| E5 | Mỗi lúc chỉ MỘT tiến trình PHPStan (3–4GB RAM mỗi tiến trình). Subagent song song phải xếp hàng (lock file). |

## Viết code để PHPStan hiểu (MUST)

| # | Quy tắc |
|---|---|
| W1 | Docblock = kiểu PHP thật (`@return Vtiger_Record_Model\|null`), có cả nhánh `false`/`null`. KHÔNG `@return <Array>`, `@param type $x`, `@return The ...`. |
| W2 | Tên class trong docblock đúng chính tả (`Users_Privileges_Model`, `EmailTemplates_Record_Model`). |
| W4 | = php83 F1/F5/F8 (kiểu param + return + shape mảng cho hàm/tham số mới; không tự thêm kiểu vào hàm cũ). Gate báo `missingType.parameter`/`missingType.return`/`missingType.iterableValue`; KHÔNG lách bằng `array<mixed>`. |
| W5 | KHÔNG tạo hai class cùng tên trong vùng quét (PHPStan chọn ngẫu nhiên → kết quả lệch giữa các máy). |
| W7 | `method.notFound` trên kiểu cha (vd `Vtiger_Record_Model::getProducts()`): nếu object thật là class con có method → sửa `@var`/`@return` cho đúng; KHÔNG thêm `method_exists()` chỉ để im lỗi. Gọi động: php83 P16. |

## Cấm (MUST NOT)

- Thêm `@phpstan-ignore` / `@phpstan-ignore-next-line` / mục `ignoreErrors` mới để lấp lỗi code mới.
- Ép kiểu, nới kiểu, `assert()`, `@var` inline chỉ để cảnh báo biến mất.
- Sinh lại `tools/phpstan/phpstan-baseline.php` để lấp lỗi mới. Baseline = nợ cũ, chỉ được **giảm** (sửa xong lỗi cũ thì xoá mục); nó bỏ qua từng lỗi, không cả file: code mới trong file cũ vẫn bị kiểm.
- Thêm `excludePaths` / `ignoreErrors` vì class/file cần không tồn tại (`class.notFound`, `vtiger.includeNotFound`, `vtiger.classNotLoaded`): sửa, hoặc nêu finding/báo user. `excludePaths` chỉ cho test, code chết, bản giải mã ionCube. Exclude tạm duy nhất: `modules/MailManager/models/Message.php` (chờ team thêm `MailRecordIMAP.php`).
- Thêm mục vào `tools/phpstan/baseline-loadcheck.php` (nợ của 2 rule vtiger lúc thêm rule): chỉ được xoá mục khi đã sửa; chạy toàn bộ để thấy `ignore.unmatched`.

Chỉ được sinh lại baseline khi (a) user chấp nhận một lỗi cũ, hoặc (b) nâng PHPStan (sửa `tools/composer.json`, sinh lại bằng chính bản mới, cùng MR); diff baseline phải được review. Lệnh: `docs/phpstan.md` (`--generate-baseline`), sau đó MUST chạy `php tools/phpstan/baseline-eol.php` (không thì checkout CRLF/LF khác làm gate báo lỗi oan).

## Giới hạn

`[OK]` ≠ an toàn: level 8 KHÔNG kiểm giá trị `mixed` (`$request->get()`, DB, JSON API) → vẫn theo php83 R2/P14; không kiểm SQL, bảo mật, nghiệp vụ, `.tpl`, JS → `rule://security-rules`, test, review.

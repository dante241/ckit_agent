---
description: "PHPStan gate — chạy tools/vendor/bin/phpstan (2.2.17) trên file PHP đã sửa trước khi báo xong/commit; cấm ignore/ép kiểu để im lỗi/sinh lại baseline; docblock kiểu thật. Auto-load (TTSR) khi sửa file khớp; cũng là checklist review."
scope: "tool:edit(**/*.php), tool:write(**/*.php), tool:edit(**/*.inc), tool:write(**/*.inc), tool:edit(**/*.service), tool:write(**/*.service), tool:edit(tools/phpstan/**), tool:write(tools/phpstan/**), tool:edit(tools/composer.*), tool:write(tools/composer.*)"
condition: ".*"
---

# PHPStan — gate cho mọi thay đổi PHP

> Tài liệu team: `docs/phpstan.md` (cơ chế, phạm vi, baseline). Quy tắc runtime 8.3: `php83.md`.
> Auto-load khi sửa `*.php`/`*.inc`/`*.service`, config PHPStan, `tools/composer.*`; đồng thời là checklist khi review.

## Gate (MUST, trước khi báo xong hoặc commit code PHP)

```bash
# modules api cron/modules include/{EntryPoints,utils,Webhooks,Webservice,WebserviceTraits}
php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/phpstan.neon.dist <các file đã sửa>
# domains/
php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/phpstan-domains.neon.dist <các file đã sửa>
```
- Kết quả phải là `[OK] No errors`. `[ERROR]` → sửa code, chạy lại. Báo kết quả gate trong phần tổng kết.
- PHPStan chỉ báo lỗi trong file được truyền vào. Đổi chữ ký hàm/method (tham số, kiểu, return, tên) → MUST chạy kèm mọi file gọi hàm đó (`lsp references` / `git grep`) hoặc chạy toàn bộ (không truyền file); chỉ chạy file chứa hàm = `[OK]` giả.
- `tools/phpstan/phpstan-domains.neon.dist` hiện có 10 lỗi có sẵn trong `domains/Social` (không do code mới): chỉ cần không phát sinh thêm lỗi.
- Ngoài phạm vi gate của `phpstan.neon.dist` (chỉ scan symbol, KHÔNG báo lỗi): phần còn lại của `include/`, `includes/`, `vtlib/`, `libraries/`, `data/`, `modules/com_vtiger_workflow`, `modules/Webforms`, `modules/WSAPP`, `include/Webservice/*/tests/*`, `modules/MailManager/models/Message.php`. `include/Webservice/CloudWorkApi/v1/*` bị loại hẳn (không scan cả symbol). `integrations/` chỉ được `phpstan-domains.neon.dist` scan symbol. Sửa file ở đó → gate không bảo vệ; tự review + chạy thử.

## Môi trường (MUST)

| # | Quy tắc |
|---|---|
| E1 | Chỉ dùng `tools/vendor/bin/phpstan`, ghim **2.2.17** (`tools/composer.json` + lock). KHÔNG dùng phar/bản khác: baseline khớp câu chữ của đúng bản này (bản 2.3.2 từng báo 75 lỗi oan). |
| E2 | Cài: `composer install -d tools`. KHÔNG `composer update` trong `tools/` trừ khi chủ động nâng PHPStan. |
| E3 | Cần `include/utils/utils.php8.php` (untracked, machine-local, bản giải mã ionCube; KHÔNG commit, `.gitignore` chặn `*.php8.php`). Thiếu → ~2.400 lỗi "function not found": đó là lỗi môi trường, không phải lỗi code. |
| E4 | Chạy từ gốc repo, `-d memory_limit=4G`. PHP 7.4 hay 8.3 đều cho cùng kết quả. |
| E5 | Mỗi lúc chỉ MỘT tiến trình PHPStan (3–4GB RAM; máy 16GB đã từng sập khi chạy 3 cái song song). Subagent song song phải xếp hàng (lock file). |

## Viết code để PHPStan hiểu (MUST)

| # | Quy tắc |
|---|---|
| W1 | Docblock = kiểu PHP thật: `@return array\|false`, `@param int $recordId`, `@return Vtiger_Record_Model\|null`. KHÔNG `@return <Array>`, `@param type $x`, `@return The ...`. |
| W2 | Tên class trong docblock đúng chính tả (`Users_Privileges_Model`, `EmailTemplates_Record_Model`). |
| W3 | Hàm có nhánh trả `false`/`null` → ghi vào `@return` để PHPStan bắt người gọi kiểm tra. |
| W4 | Hàm/method MỚI, tham số MỚI, hàm đổi tên: MUST khai báo kiểu param + return (`function getName(int $id): string`) và nội dung mảng trong docblock (`@param list<int> $ids`, `@return array{id: int, name: string}`) — gate báo `missingType.parameter`/`missingType.return`/`missingType.iterableValue`. KHÔNG lách bằng `array<mixed>`. Hàm cũ chưa có kiểu: KHÔNG tự thêm kiểu PHP khi chưa rà nơi gọi + lớp con (thêm return type ở lớp cha khi lớp con override chưa có kiểu = fatal; thêm param type ở lớp con khi cha chưa có = fatal). |
| W5 | KHÔNG tạo hai class cùng tên trong vùng quét (PHPStan chọn ngẫu nhiên → kết quả lệch giữa các máy). |
| W6 | Hạn chế gọi động (`new $cls`, `$obj->$m()`, `call_user_func`); bắt buộc thì kiểm `class_exists`/`method_exists`. |
| W7 | `method.notFound` trên kiểu cha (vd `Vtiger_Record_Model::getProducts()`): nếu object thật là class con có method → sửa `@var`/`@return` cho đúng; KHÔNG thêm `method_exists()` chỉ để im lỗi. |

## Cấm (MUST NOT)

- Thêm `@phpstan-ignore` / `@phpstan-ignore-next-line` / mục `ignoreErrors` mới để lấp lỗi code mới.
- Ép kiểu, nới kiểu, `assert()`, `@var` inline chỉ để cảnh báo biến mất.
- Sinh lại `tools/phpstan/phpstan-baseline.php` để lấp lỗi mới. Baseline = nợ cũ đã kiểm kê, chỉ được **giảm** (sửa xong lỗi cũ thì xoá mục). Baseline bỏ qua từng lỗi cụ thể, KHÔNG bỏ qua cả file: code mới trong file cũ vẫn bị kiểm.
- Thêm `excludePaths` / `ignoreErrors` cho file vì class hay file nó cần không tồn tại (`class.notFound`, `vtiger.includeNotFound`, `vtiger.classNotLoaded`): phải sửa, hoặc nêu thành finding trong review/báo user. `excludePaths` chỉ cho test, code không còn chạy, bản giải mã ionCube. Exclude tạm duy nhất: `modules/MailManager/models/Message.php` (chờ team thêm `MailRecordIMAP.php`).
- Thêm mục vào `tools/phpstan/baseline-loadcheck.php` (nợ của 2 rule vtiger lúc thêm rule): chỉ được xoá mục khi đã sửa; chạy toàn bộ để thấy `ignore.unmatched`.

Chỉ được sinh lại baseline khi (a) user chấp nhận một lỗi cũ, hoặc (b) nâng PHPStan (sửa `tools/composer.json`, sinh lại bằng chính bản mới, cùng MR). Diff baseline phải được review:
```bash
php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/phpstan.neon.dist --generate-baseline tools/phpstan/phpstan-baseline.php
php tools/phpstan/baseline-eol.php   # MUST: line breaks/offsets in messages -> \R / \d+, else CRLF/LF checkouts fail the gate
```

## Giới hạn (đừng coi `[OK]` là đủ)

- Level 8 KHÔNG kiểm giá trị `mixed` (`$request->get()`, DB, JSON API): `count($request->get('ids') ?: false)` không bị báo → vẫn phải theo `php83.md` R2 / P14.
- Không kiểm SQL, bảo mật, nghiệp vụ, `.tpl`, JS → `security-rules.md`, test, review.

## Self-check

```bash
php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/phpstan.neon.dist <file>   # → [OK] No errors
git diff --stat tools/phpstan/phpstan-baseline.php                                                      # phải rỗng trong MR thường
grep -n "@phpstan-ignore" <file>                                                                 # phải rỗng
tools/vendor/bin/phpstan --version                                                               # 2.2.17
```

---
description: "Modern-code principle cho source 2005: code MỚI dùng stack composer có sẵn (Guzzle/Monolog/Carbon/JWT v6...), cấm mở rộng legacy bundle (log4php/NuSOAP/PHPExcel/Zend_Json/ADOdb raw); target duy nhất PHP 8.3 (D-019, 2026-10-26 — thay D-016 floor 7.2 + D-017 dual-run)."
scope: "tool:edit(modules/**/*.php), tool:write(modules/**/*.php), tool:edit(include/**/*.php), tool:write(include/**/*.php), tool:edit(includes/**/*.php), tool:write(includes/**/*.php), tool:edit(domains/**/*.php), tool:write(domains/**/*.php), tool:edit(integrations/**/*.php), tool:write(integrations/**/*.php), tool:edit(api/**/*.php), tool:write(api/**/*.php), tool:edit(cron/**/*.php), tool:write(cron/**/*.php)"
condition: ".*"
---

# Modern PHP — nguyên tắc cho code MỚI (source gốc 2005)

> Nguyên tắc một câu: **code cũ được đông lạnh, code mới phải hiện đại.** Không restyle code cũ hàng loạt (diff nhiễu, rủi ro hồi quy — bài học MR PHP 8.3: 3.209 file); mọi ranh giới dưới đây áp cho dòng code VIẾT MỚI.

## Trần cú pháp (theo D-019: target 8.3 — trần cũ D-016 đã gỡ 2026-10-26)
- Target runtime = PHP 8.3. Code MỚI được dùng toàn bộ cú pháp ≤ 8.3: typed property, arrow fn, `??=`, spread, match, nullsafe `?->`, enum, promotion, readonly, typed const. Boot 7.4 KHÔNG còn được cam kết (D-019): release yêu cầu server 8.3; rollback = rollback release. Cú pháp 8.3 dùng được ở MỌI code mới, kể cả domains/ load runtime.
- ĐƯỢC (và NÊN) dùng: type declaration cho param/return (`: bool`, `: ?int`), `declare(strict_types=1)` trong file MỚI hoàn toàn, `??`, const visibility, closure thay `create_function`.
- Kỷ luật runtime 8.3 (count trên non-array, static-call, dynamic property, chia 0, ép kiểu input…): rule `php83.md`. Không needle rỗng.
- Gate PHPStan bắt buộc trước khi báo xong/commit: rule `phpstan.md`.

## Dependency — một nguồn duy nhất
- Thư viện MỚI: chỉ qua **composer** (pin version, package còn maintain). CẤM chép thư viện mới vào `libraries/`.
- CẤM sửa tay file trong `vendor/` (3 patch GdImage/CurlHandle hiện tại là ngoại lệ được D-017/D-019 ghi nhận, sẽ gỡ bằng composer-patches/upgrade ở MR follow-up).
- CẤM thêm code mới NẠP các bundle đông lạnh. Bản thay thế đã nằm sẵn trong vendor:

| Việc | CẤM dùng (legacy, chỉ code cũ) | DÙNG (có sẵn trong vendor) |
|---|---|---|
| HTTP client | `CURLClient`, `HTTP_Request`, `Vtiger_Net_Client`, curl tay | `guzzlehttp/guzzle` 7 |
| Logging (code mới trong `domains/`, integrations) | `log4php.debug`, `$log->debug` kiểu cũ | `monolog/monolog` (hoặc `saveLog` wrapper hiện có) |
| Ngày giờ phức tạp | tính tay `strtotime` chuỗi nối | `nesbot/carbon` (đã có qua deps) |
| JWT | `libraries/PHP-JWT` (v5 — trùng class với v6, bug load-order) | `firebase/php-jwt` v6 |
| JSON | `Zend_Json::encode/decode` | `json_encode`/`json_decode` thuần |
| SOAP mới | NuSOAP | không viết SOAP mới — REST (`api/`) |
| Excel mới | PHPExcel | PhpSpreadsheet chưa có trong vendor (MR riêng, xem Lộ trình gỡ đông lạnh); tạm thời dùng export helper hiện có |
| XSS filter | tự regex | `voku/anti-xss` (đã dùng ở CommonUtils) / `vtlib_purify` |
| IMAP | code imap_* tay | `webklex/php-imap` |
| Ảnh | GD tay | `intervention/image` |

## Kiến trúc code mới
- Business logic mới: theo tầng `domains/` (D-013/D-014 — interface → abstract base → final leaf, repository/service/gateway) hoặc module `CP*` đúng `vtiger-module-layering`. KHÔNG thêm hàm global mới vào `include/utils/CommonUtils.php` trừ polyfill/compat có lý do.
- KHÔNG `require_once` cho class theo convention `<Module>_<Component>_<Type>` (autoload lo). `require_once` chỉ cho `include/utils/**`, webservice handler, file ngoài convention.
- DB: 100% prepared statement qua `pquery` (security-rules); CẤM nối chuỗi SQL, CẤM mysqli trực tiếp.
- CẤM `@` nuốt lỗi trong code mới (trừ `@unlink` dọn file); lỗi ngoài ý muốn phải lên log.
- Logic mới trong `domains/` PHẢI có PHPUnit test (`domains/*/tests`).
- Style giữ nguyên house-style K&R theo `php-conventions.md` — hiện đại hoá ngôn ngữ, KHÔNG đổi brace/format của repo.

## Lộ trình gỡ đông lạnh (đã xếp MR, không tự ý làm trong MR khác)
Smarty core 3→4, ADOdb, HTML Purifier, TCPDF, HTTP_Request2, PhpSpreadsheet, google-apiclient Calendar qua composer, Mobile HTTP_Session→Session2, PHP-JWT dedupe (36 call site), inline-fork 2 package vendor đã patch. Mỗi mục một MR, có crawl/simulate gate như PHP 8.3.

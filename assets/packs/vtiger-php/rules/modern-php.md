---
description: "Modern-code principle cho source 2005: code MỚI dùng stack composer có sẵn (Guzzle/Monolog/Carbon/JWT v6...), cấm mở rộng legacy bundle (log4php/NuSOAP/PHPExcel/Zend_Json/ADOdb raw); runtime duy nhất PHP 8.3 (D-019). Auto-load khi edit thêm lời gọi thư viện legacy/require_once/mysqli."
scope: "tool:edit(modules/**/*.php), tool:write(modules/**/*.php), tool:edit(include/**/*.php), tool:write(include/**/*.php), tool:edit(includes/**/*.php), tool:write(includes/**/*.php), tool:edit(domains/**/*.php), tool:write(domains/**/*.php), tool:edit(integrations/**/*.php), tool:write(integrations/**/*.php), tool:edit(api/**/*.php), tool:write(api/**/*.php), tool:edit(cron/**/*.php), tool:write(cron/**/*.php)"
condition: ["CURLClient|HTTP_Request|Vtiger_Net_Client|curl_init|curl_setopt|log4php|Zend_Json|PHPExcel|nusoap|soapclient|PHP-JWT|\\bJWT::|\\bimap_\\w+\\(|\\bimage(create|copy)\\w*\\(|libraries/|mysqli_|new mysqli|require_once|include_once"]
---

# Modern PHP — nguyên tắc cho code MỚI (source gốc 2005)

**Code cũ được đông lạnh, code mới phải hiện đại.** Không restyle code cũ hàng loạt; mọi ranh giới dưới đây áp cho dòng code VIẾT MỚI. Cú pháp + runtime 8.3 (gồm `strict_types` chỉ trong `domains/`, R12): `rule://php83`. Không truyền needle rỗng (`str_contains`/`strpos`/`explode`).

## Dependency — một nguồn duy nhất
- Thư viện MỚI: chỉ qua **composer** (pin version, package còn maintain). CẤM chép thư viện mới vào `libraries/`.
- CẤM sửa tay file trong `vendor/` (ngoại lệ duy nhất: các patch đã ghi trong D-019).
- CẤM thêm code mới NẠP các bundle đông lạnh. Bản thay thế đã nằm sẵn trong vendor:

| Việc | CẤM dùng (legacy, chỉ code cũ) | DÙNG (có sẵn trong vendor) |
|---|---|---|
| HTTP client | `CURLClient`, `HTTP_Request`, `Vtiger_Net_Client`, curl tay | `guzzlehttp/guzzle` 7 |
| Logging (code mới trong `domains/`, integrations) | `log4php.debug`, `$log->debug` kiểu cũ | `monolog/monolog` (hoặc `saveLog` wrapper hiện có) |
| Ngày giờ phức tạp | tính tay `strtotime` chuỗi nối | `nesbot/carbon` |
| JWT | `libraries/PHP-JWT` (v5 — trùng class với v6, bug load-order) | `firebase/php-jwt` v6 |
| JSON | `Zend_Json::encode/decode` | `json_encode`/`json_decode` thuần |
| SOAP mới | NuSOAP | không viết SOAP mới — REST (`api/`) |
| Excel mới | PHPExcel | PhpSpreadsheet chưa có trong vendor; tạm dùng export helper hiện có |
| XSS filter | tự regex | `voku/anti-xss` / `vtlib_purify` |
| IMAP | code imap_* tay | `webklex/php-imap` |
| Ảnh | GD tay | `intervention/image` |

## Kiến trúc code mới
- Business logic mới: theo tầng `domains/` (D-013/D-014, `rule://domain-architecture`) hoặc module `CP*` đúng `vtiger-module-layering`; hàm global mới: php83 C9.
- `require_once` chỉ cho `include/utils/**`, webservice handler, file ngoài convention (class `<Module>_<Component>_<Type>` dùng autoload).
- DB: CẤM mysqli trực tiếp (chỉ `pquery`).
- KHÔNG tự migrate bundle đông lạnh (Smarty, ADOdb, TCPDF, PHP-JWT…) trong MR không liên quan — mỗi mục một MR riêng theo lộ trình ở `agents/DECISIONS.md` (D-019).

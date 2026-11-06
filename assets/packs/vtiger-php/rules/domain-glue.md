---
description: "Glue vtiger → domains/: modules/api/include/Webservice/Webhooks gọi domain chỉ qua <Domain>::xxxService()/port, giữ format response, catch Throwable, strangler. Auto-load khi edit dùng CloudGo\\ hoặc gọi <Domain>::xxxService(."
scope: "tool:edit(modules/**/*.php), tool:write(modules/**/*.php), tool:edit(api/**/*.php), tool:write(api/**/*.php), tool:edit(include/Webservice/**/*.php), tool:write(include/Webservice/**/*.php), tool:edit(include/Webhooks/**/*.php), tool:write(include/Webhooks/**/*.php)"
condition: ["CloudGo\\\\", "\\b[A-Z]\\w+::\\w+Service\\("]
---

# Domain glue (code vtiger gọi vào `domains/`)

Glue = `modules/*` action/view/handler/cron, `api/*`, `include/Webservice`, `include/Webhooks`, CPQueue job. Code đã chuyển vào `domains/` đi glue → `<Domain>::xxxService()`; module legacy chưa đụng tới vẫn theo D-002 (Action → Logic → Data); connector legacy (`include/Webservice`, `modules/CP*Integration`) vẫn theo `rule://api-connector-rules`. Kiến trúc phía domain: `rule://domain-architecture`.

## Được gọi gì

|Ai|Được gọi|KHÔNG được|
|---|---|---|
|Glue|`<Domain>::xxxService()`, `<Domain>::xxx()` trả port (vd `Social::feedParser($channel)`), VO/exception/hằng public|Repository, adapter, SQL trên bảng của domain, logic nghiệp vụ|

API vào của mình (`api/*`, `include/Webservice/*Handler`, webhook) là **glue**, không phải integration.

## Trả dữ liệu

|Tầng|Trả|Khi sự cố|
|---|---|---|
|Glue|`Vtiger_Response` / định dạng hiện có của từng họ API|catch, log, trả lỗi đã dịch|
|Facade legacy (static cũ)|**Giữ nguyên định dạng cũ** (`['success' => …]`)|giữ nguyên|

- Domain trả VO/array shape; `vtranslate`, HTML, `toArray()` → JSON là việc của glue/tpl. API mobile/portal/đối tác: giữ nguyên format response hiện có khi đổi bên trong.

## Exception

- **Glue: luôn** — `catch (Throwable $e)`, `saveLog`, trả lỗi đã dịch; không đưa `$e->getMessage()` cho người dùng. Vòng lặp cron/webhook/queue: catch **từng phần tử** để 1 lỗi không dừng cả lô.
- Phân biệt loại lỗi bằng `isX()` của `<Area>Exception` (`CommentException::isInvalidCursor()`), không bằng message.

## Chuyển code legacy (strangler)

- Chỉ chuyển phần đang có ticket chạm tới; không viết lại thứ đang chạy.
- Chuyển **thân hàm cũ** vào Service/Gateway (giữ nguyên hành vi, kể cả retry/map lỗi); static cũ thành facade 1 dòng, giữ format trả về cũ. Xoá facade khi `lsp references` = 0.
- Dữ liệu cũ giữ bảng cũ, đặt sau Store; không tạo bảng song song.
- Trước khi chuyển: có test đối chiếu (`tests/Legacy`, `tests/Golden`, replay webhook); sau khi chuyển kết quả phải giống.

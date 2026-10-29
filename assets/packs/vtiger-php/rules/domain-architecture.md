---
description: "Kiến trúc domains/: 4 tầng glue→domain→integrations, ai gọi ai, Service/Store/Repository/Gateway, interface/final/abstract, vendor, trả dữ liệu, exception + try/catch, chuyển legacy. Auto-load khi sửa domains/, integrations/, glue vtiger."
scope: "tool:edit(domains/**/*.php), tool:write(domains/**/*.php), tool:edit(integrations/**/*.php), tool:write(integrations/**/*.php), tool:edit(modules/**/*.php), tool:write(modules/**/*.php), tool:edit(api/**/*.php), tool:write(api/**/*.php), tool:edit(include/Webservice/**/*.php), tool:write(include/Webservice/**/*.php), tool:edit(include/Webhooks/**/*.php), tool:write(include/Webhooks/**/*.php)"
condition: ".*"
---

# Domain architecture

> Nguồn: D-007, D-011..D-014 (`agents/DECISIONS.md`). Quy ước cú pháp PHP trong `domains/` ở `domain-php.md`. Mẫu tham chiếu: `domains/Social`.
> Phạm vi luồng: code đã chuyển vào `domains/` đi glue → `<Domain>::xxxService()`; module legacy chưa đụng tới vẫn theo D-002 (Action → Logic → Data). Connector legacy (`include/Webservice`, `modules/CP*Integration`) vẫn theo `api-connector-rules.md`; mục 5 dưới đây áp thêm cho `integrations/`.
> Checklist theo từng vai (tự nạp theo tên file, mỗi file có sơ đồ luồng gọi): `domain-service.md`, `domain-repository.md`, `domain-gateway.md`, `domain-composition-root.md`, `domain-tests.md`.

## 1. Bốn tầng

```
Glue (modules/* action/view/handler/cron, api/*, include/Webservice, include/Webhooks, CPQueue job)
  -> <Domain>::xxxService() | <Domain>::xxx() (port)   domains/<Domain>/src  (nghiệp vụ)
       -> Store / Gateway (interface)
            -> XxxRepository (SQL) | <Vendor>XxxRepository / <Vendor>XxxGateway -> integrations/<Vendor> (client vendor)
  domains/Shared  (Clock, Config, Logger, Queue, Lock... — chỉ thứ ≥ 2 domain dùng; không nghiệp vụ)
```

## 2. Ai được gọi ai

|Ai|Được gọi|KHÔNG được|
|---|---|---|
|Glue|`<Domain>::xxxService()`, `<Domain>::xxx()` trả port (vd `Social::feedParser($channel)`), VO/exception/hằng public|Repository, adapter, SQL trên bảng của domain, logic nghiệp vụ|
|Service|Interface (Store/Gateway), Service khác cùng domain, `Shared`, VO, exception|`new` class thật, `$adb`, class vtiger, `$_REQUEST`, `Vtiger_Request`, `echo`|
|`XxxRepository`|`PearDatabase`|Service, vendor|
|`<Vendor>XxxRepository`|Client trong `integrations/<Vendor>`, Repository SQL phụ (qua Store), VO của domain|Service, domain khác|
|`<Vendor>XxxGateway`|Client trong `integrations/<Vendor>` hoặc SDK, VO của domain|Repository, domain khác|
|`integrations/<Vendor>`|HTTP/SDK, `Shared`|Mọi thứ trong `domains/`, bảng/record CRM|
|Domain A → B|Chỉ `B::xxxService()` + VO/exception public của B|Bảng, Repository, class nội bộ của B|

- Mỗi bảng có **một** domain chủ; SQL trên bảng đó chỉ nằm trong Repository của domain chủ.
- Ngoại lệ đọc: query danh sách/báo cáo cần JOIN sang bảng domain khác (vd tên khách cho danh sách chat) được JOIN **chỉ đọc** trong Repository, ghi comment lý do, cho tới khi domain chủ có read model. Không JOIN để ghi.

## 3. Vai trò file

|Vai|Là gì|Tên|
|---|---|---|
|Service|Logic nghiệp vụ|`XxxService` — `final`, KHÔNG có interface|
|Store|Interface lưu trữ của chính domain (port)|`XxxStore`|
|Repository|Bản cài Store. Không tiền tố = SQL trên bảng CRM (nơi DUY NHẤT có SQL); tiền tố vendor = lưu qua API của vendor|`XxxRepository` / `<Vendor>XxxRepository` trong thư mục vendor (`Chathub/ChathubCommentRepository`)|
|Gateway|Interface ra hệ thống khác: vendor, domain khác, code legacy (port)|`XxxGateway`|
|Adapter vendor|Bản cài Gateway gọi vendor|`<Vendor>XxxGateway` trong `Channel/<Vendor>/`|
|Adapter legacy / domain chưa tách|Bản cài Gateway gọi code vtiger cũ|`Vtiger<Xxx>Gateway` hoặc `<Vendor>InboxGateway`… trong mảng nghiệp vụ dùng nó|
|VO|Dữ liệu bất biến|Danh từ, không hậu tố (`Conversation`, `PageConfig`)|
|Exception|Lỗi của một mảng nghiệp vụ — **1 class / mảng**; loại lỗi glue phải phân biệt = named constructor + `isX()` (`CommentException::invalidCursor()` / `isInvalidCursor()`), không class con|`<Area>Exception extends RuntimeException`, 1 file riêng (PSR-4: gom nhiều class vào 1 file thì `throw` fatal, `catch` im lặng không bắt)|
|Composition root|Nơi DUY NHẤT `new` class thật + lấy DB|`<Domain>.php` (`Social::xxxService()`)|

- **Port thuộc bên dùng**: domain cần gì từ bên ngoài thì tự khai báo interface với đúng các method nó cần, đặt trong mảng nghiệp vụ của nó.
- **Đọc tên là biết vai**: `…Store` và `…Gateway` không tiền tố vendor = interface; class cài = `…Repository` hoặc `<Vendor>…Gateway`. Kiểm: `git grep -nE "^final class \w+Store\b" -- 'domains/*/src'` phải rỗng.
- **Service phải có logic**: hàm chỉ gọi lại 1 port thì bỏ Service, `<Domain>::xxx()` trả thẳng port. Service cùng vai (cùng màn, cùng port) gộp làm một thay vì tách Service 1 hàm.

### 3a. Thư mục trong `src/` = capability (user chốt 2026-10-09; chưa có mục riêng trong `agents/DECISIONS.md`)

- Cây `domains/<D>/src` đọc ra danh sách tính năng: mỗi thư mục con = 1 capability (vd Social: `Ingestion/` `Reply/` `PrivateReply/` `Reaction/` `Moderation/`) hoặc thư mục vendor (`Channel/<Vendor>/`, `Chathub/`). Lõi chung nhỏ (`Comment/`): port dùng chung (Store/Gateway), VO/event, exception, permission, service đọc.
- **Tiêu chí tách capability**: có bảng + UC + entry riêng; nói chuyện với capability khác qua port của lõi chung. Port nhiều capability cùng dùng (vd `BlockListStore` cho Reply + PrivateReply) đặt ở lõi để khỏi tạo vòng.
- **Tối đa 1 tầng** thư mục; capability phình thì nâng thành thư mục ngang hàng, không lồng.
- **Ranh giới enforce bằng test** (`tests/Unit/CapabilityBoundariesTest.php`): bản đồ phụ thuộc cho phép phải khớp code, không vòng giữa các thư mục; thêm import chéo mới = sửa bản đồ có lý do, không lách.

## 4. `interface` / `final` / `abstract`

- `interface`: chỉ cho port (Store/Gateway) và contract plugin (`CommentChannel`). Không hậu tố `Interface`.
- `final`: **mặc định** cho mọi class — Service, Repository, adapter, VO, exception, composition root.
- `abstract`: chỉ khi **đã có ≥ 2 bản cài thật dùng chung code thật** của cùng một API vendor **và** phần chung không tách được thành client inject qua constructor. Ưu tiên composition: các gateway cùng vendor dùng chung 1 client (`integrations/Meta/FacebookGraphClient`), mỗi gateway `final`. Không tạo trước "để sau dùng". Hiện chưa có mẫu (`GraphScopeGateway` đã gộp vào `FacebookScopeGateway` 2026-10-07 vì chỉ có 1 bản cài).
- Đổi hành vi class `final`: không kế thừa — viết class khác implement cùng interface (decorator) và cắm qua `<Domain>::configureOverrides()` (D-014).

## 5. Vendor

1. Chỉ 1 domain dùng vendor → để trong `domains/<D>/src/Channel/<Vendor>/`.
2. ≥ 2 domain dùng (Zalo: Social + Messaging; Meta: Social + Content) → phần **gọi HTTP + token** ra `integrations/<Vendor>/` (`CloudGo\Integration\<Vendor>`).
3. **1 client cho mỗi vendor, 1 gateway mỏng cho mỗi khả năng** (comment, chat, đăng bài = 3 gateway, chung 1 client). Không gom mọi khả năng vào 1 class gateway; không để mỗi gateway tự dựng SDK.
4. Dịch dữ liệu vendor → VO/event của domain nằm ở gateway trong domain, không ở `integrations/`.
5. `integrations/` không biết CRM: không crmid, không bảng vtiger, không import `domains/`.
6. Token vendor: refresh ở **một chỗ duy nhất**, có lock (xử lý cả `GET_LOCK` timeout); mọi chỗ ghi token đi qua cùng store đó.
7. API vào của mình (`api/*`, `include/Webservice/*Handler`, webhook) là **glue**, không phải integration.

## 6. Trả dữ liệu

|Tầng|Trả|Khi sự cố|
|---|---|---|
|Repository / Gateway|VO, `list<VO>`, array có shape, id, `bool`|`throw <Area>Exception` (gateway bọc lỗi SDK/HTTP)|
|Service|VO, `list<VO>`, array có shape, hằng trạng thái (`self::DELIVERED`)|`throw <Area>Exception`|
|Glue|`Vtiger_Response` / định dạng hiện có của từng họ API|catch, log, trả lỗi đã dịch|
|Facade legacy (static cũ)|**Giữ nguyên định dạng cũ** (`['success' => …]`)|giữ nguyên|

- Không thấy một bản ghi (bình thường) → `null`. Danh sách rỗng → `[]`. Có/không → `bool`. Chưa biết/có/không → `?bool`. Nhiều nhánh nghiệp vụ → hằng + `@return self::A|self::B`. **Sự cố → exception, không `false`/`null`/`['error']`.**
- Array trả ra phải có shape PHPDoc (`@return array{post_id: string, …}`); không `array` trơn cho dữ liệu đã biết cấu trúc. VO khi có hành vi/quy tắc hoặc dùng ở nhiều nơi; array shape khi chỉ để hiển thị.
- Kiểu: PK bảng riêng của domain `int`; id bản ghi CRM hiện là `string` trong Social (`InboxGateway`) — đổi sang `int` là đổi chữ ký; id vendor (page, OA, user Zalo/FB) **luôn `string`**; ngày giờ `'Y-m-d H:i:s'` theo múi giờ site; tiền không dùng `float`; cột NULL → `null` thật; JSON decode trong Repository.
- Domain KHÔNG trả: chuỗi đã dịch (`vtranslate` ở glue/tpl), HTML, object vtiger (`Vtiger_Record_Model`, `Vtiger_Response`), JSON thô của vendor. VO có `toArray()` cho glue.
- API mobile/portal/đối tác: giữ nguyên format response hiện có khi đổi bên trong.

## 7. Exception và try/catch

- `return` cho kết quả bình thường (kể cả không thấy, bị bỏ qua); `throw` chỉ khi sự cố. Thiếu dữ liệu mà luồng không thể đi tiếp (vd gửi tin vào conversation không tồn tại) → throw.
- Catch ở đâu:
  - **Glue: luôn** — `catch (Throwable $e)`, `saveLog`, trả lỗi đã dịch; không đưa `$e->getMessage()` cho người dùng. Vòng lặp cron/webhook/queue: catch **từng phần tử** để 1 lỗi không dừng cả lô.
  - **Gateway**: catch lỗi SDK/HTTP → throw lại exception của domain.
  - **Service**: chỉ khi có fallback nghiệp vụ có chủ đích (vd `CommentThreadService` bỏ trích dẫn khi đọc comment cha lỗi); không có fallback thì để lỗi đi lên.
  - **Repository, VO: không catch.**
- Cấm: nuốt lỗi (`catch {}` rỗng), trả `false` thay exception, catch rồi throw lại y nguyên (trừ pass-through trước một catch rộng hơn), throw cho kết quả bình thường.

## 8. Chuyển code legacy (strangler)

- Chỉ chuyển phần đang có ticket chạm tới; không viết lại thứ đang chạy.
- Chuyển **thân hàm cũ** vào Service/Gateway (giữ nguyên hành vi, kể cả retry/map lỗi); static cũ thành facade 1 dòng, giữ format trả về cũ. Xoá facade khi `lsp references` = 0.
- Dữ liệu cũ giữ bảng cũ, đặt sau Store; không tạo bảng song song.
- Trước khi chuyển: có test đối chiếu (`tests/Legacy`, `tests/Golden`, replay webhook); sau khi chuyển kết quả phải giống.

## 9. Bản đồ domain định hướng

Social (hội thoại 2 chiều, comment, mapping social↔crm) · Messaging (gửi 1 chiều: email/SMS/ZNS/push/in-app) · Customer (Accounts/Contacts/Leads/CPTarget, convert) · Content (đăng bài) · Event (CPEvent*) · OrderToCash · Catalog · Ecommerce · Support (HelpDesk) · Requests (CPRequest) · Sales/Marketing · Work · Goals · HR · Recognition · BotAI · Ads · CallCenter. Calendar/Events (vtiger core), Users, Org (Department/Branch/Company) là kernel dùng chung, không gộp vào domain Event. Tách domain khi có ticket chạm tới, không tách trước.

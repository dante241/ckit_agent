---
description: "Anti-pattern/lessons đã gặp trong codebase (EP-NNN) — tránh khi code, check khi review."
---

# Error Patterns — Anti-Patterns / Lessons Learned

> **Không tự nạp** (domain rule — `read rule://error-patterns`; BẮT BUỘC đọc khi code chạm module/method đã có entry và khi review). Bugs đã từng gặp trong codebase, format chuẩn hóa để agent và human dễ search/áp dụng. Khi code → tránh; khi review → check.

## Format

Mỗi entry:

```markdown
## EP-NNN: <Tên ngắn>

**Ticket / MR:** #NNNNN / !NNN
**Date:** YYYY-MM-DD
**Module / Layer:** <Module hoặc framework layer>

**Symptom:** <Hiện tượng người dùng / dev nhìn thấy>

**Root cause:** <Nguyên nhân kỹ thuật cụ thể>

**Rule:** <Quy tắc 1-2 câu để tránh tái phát>

**Trigger keywords (review):** <từ khóa code-reviewer scan để bắt pattern>
```

ID `EP-NNN` chạy tăng dần. Không xóa entry cũ — fix rồi vẫn giữ làm tham khảo.

---

<!-- Entries appear below this line. Add via Phase 10.6 (cook) / Step 9 (fix) USER GATE. -->

## EP-001: Custom view sai base class / TPL location / Helper filename

**Ticket / MR:** — (Quick Guide POC review 2026-05-13)
**Date:** 2026-05-13
**Module / Layer:** View layer (any custom view module)

**Symptom:** Trang custom view render trắng / CSS-JS không auto-load / autoload class fail / TPL không tìm thấy.

**Root cause:** 3 lỗi liên hoàn khi tạo custom view (không phải core List/Edit/Detail):
1. `extends Vtiger_Index_View` thay vì `CustomView_Base_View` → mất auto-load CSS/JS theo viewName.
2. TPL đặt ở `layouts/v7/modules/<Module>/` (chỗ của core view) thay vì `modules/<Module>/tpls/`.
3. Helper filename `<Name>Helper.php` thay vì `<Name>.php` → VTiger autoload không tìm được class `<Module>_<Name>_Helper`.

**Rule:** Custom view (Config, Report, custom page, landing…) — BẮT BUỘC:
- View `extends CustomView_Base_View` (source: `modules/CustomView/views/Base.php`).
- TPL: `modules/<Module>/tpls/<ViewName>.tpl`, gọi bằng `$viewer->display('modules/<Module>/tpls/<ViewName>.tpl')`.
- Helper file: `modules/<Module>/helpers/<Component>.php` — filename = Component part only, KHÔNG suffix `Helper`/`Model`/`View`.
- Core view (List/Edit/Detail của entity module) thì ngược lại: `extends Vtiger_Index_View` + TPL ở `layouts/v7/modules/<Module>/`.

Đọc đầy đủ ở skill `view/references/custom-view-base.md` TRƯỚC khi tạo file.

**Trigger keywords (review):** `extends Vtiger_Index_View` (trong custom view, không phải List/Edit/Detail) · file `*Helper.php` trong `helpers/` · TPL trong `layouts/v7/modules/<Module>/` mà không phải List/Edit/Detail · `$viewer->view(` với path tương đối.

---

## EP-002: Language key đặt sai array ($languageStrings vs $jsLanguageStrings)

**Ticket / MR:** #22219 / —
**Date:** 2026-06-02
**Module / Layer:** Language files (`languages/<locale>/**`)

**Symptom:** TPL `{vtranslate('LBL_FOO')}` render ra chuỗi raw key `LBL_FOO` thay vì text dịch. Hoặc JS `app.vtranslate('JS_FOO')` trả về `JS_FOO`. Notice/label hiển thị tên key lủng củng.

**Root cause:** File ngôn ngữ có 2 array tách scope: `$languageStrings` (đọc bởi PHP/TPL `vtranslate()`) và `$jsLanguageStrings` (đọc bởi `app.vtranslate()`). Hai scope KHÔNG thấy nhau. Khi thêm key mới mà append mù vào cuối file → rơi vào `$jsLanguageStrings` (vì dấu `);` cuối file đóng array JS). `LBL_` nằm nhầm trong array JS → `vtranslate()` không thấy → render raw key. Lỗi không bị `php -l` bắt (cú pháp vẫn hợp lệ).

**Rule:** `LBL_*` → `$languageStrings`; `JS_*` → `$jsLanguageStrings`. KHÔNG append cuối file mà không xác định array. Label dùng ở CẢ TPL lẫn JS phải có key ở cả 2 array (`LBL_X` + `JS_X`). Sau mỗi sửa, verify cho CẢ 2 locale:
```bash
awk '/languageStrings = array/{a="LBL-array"} /jsLanguageStrings = array/{a="JS-array"} /YOUR_KEY/{print a": "$0}' languages/en_us/<File>.php
```
Kỳ vọng `LBL_*`→`LBL-array`, `JS_*`→`JS-array`. Chi tiết: skill `language` Pitfall #8.

**Trigger keywords (review):** key `LBL_` hoặc `JS_` thêm sát trước dấu `);` cuối file ngôn ngữ · `LBL_` bên trong block `$jsLanguageStrings` · `JS_` bên trong block `$languageStrings` · thêm language string mà không có grep/awk verify array.

---

## EP-003: Thêm nhánh event vào handler nhưng chưa đăng ký row cho event đó

**Ticket / MR:** #23696 / —
**Date:** 2026-09-07
**Module / Layer:** VTEventHandler (mọi `*Handler.php` xử lý `vtiger.entity.beforesave/aftersave/...`)

**Symptom:** Code trong nhánh `if ($eventName == 'vtiger.entity.beforesave')` (hoặc event khác) không bao giờ chạy dù logic đúng; `php -l` sạch; unit-test đọc/render pass; nhưng side-effect (snapshot/log/hook) không xảy ra ở luồng thật.

**Root cause:** VTEventsManager dispatch handler theo ROW trong `vtiger_eventhandlers` (mỗi cặp `(handler_class, event_name)` một row), KHÔNG theo các nhánh `if` trong `handleEvent()`. Ví dụ `ModCommentsHandler` chỉ có row cho `vtiger.entity.aftersave`; thêm nhánh `beforesave` vào code không tự tạo row → beforesave không bao giờ gọi handler. Giả định "handler đã xử lý aftersave nên cũng nhận beforesave" là SAI.

**Rule:** Khi thêm nhánh xử lý một event MỚI vào một VTEventHandler có sẵn, BẮT BUỘC kiểm tra registration và đăng ký row cho event đó qua migration (idempotent-check trước, `down()` xoá row):
```php
$check = $adb->pquery("SELECT 1 FROM vtiger_eventhandlers WHERE handler_class = ? AND event_name = ?", array('XHandler', 'vtiger.entity.beforesave'));
if ($adb->num_rows($check) == 0) {
    $em = new VTEventsManager($adb);
    $em->registerHandler('vtiger.entity.beforesave', 'modules/X/XHandler.php', 'XHandler');
}
```
Verify bằng luồng save THẬT (không chỉ `php -l`/read-test) rằng side-effect thực sự chạy.

**Trigger keywords (review):** thêm `if ($eventName == 'vtiger.entity.` nhánh mới vào `*Handler.php` mà không có `registerHandler`/INSERT vào `vtiger_eventhandlers` · giả định handler nhận mọi entity event

## EP-004: Nhánh field-đặc-thù mới bị nhánh generic (theo comparator) ĐỨNG TRƯỚC nuốt mất

**Ticket / MR:** #10091 / —
**Date:** 2026-09-11
**Module / Layer:** `modules/Reports/ReportRun.php` chuỗi advanced-filter; tổng quát: mọi if/elseif dài dispatch theo comparator rồi mới tới field

**Symptom:** Thêm toán tử mới (y/ny cho field `tags`) ở nhánh cuối chain nhưng runtime rơi vào nhánh generic phía trên (`$comparator == 'ny'`) → build SQL sai (`X.tags IS NOT NULL` với `tags` là virtual field không cột thật) → SQL error/sai kết quả. Nhánh mới thành dead code; nửa scope (report) hỏng dù listview đúng. `php -l` pass, chỉ lộ khi chạy SQL thật.

**Root cause:** if/elseif khớp theo THỨ TỰ. Nhánh generic theo comparator (`== 'ny'`) đặt TRƯỚC nhánh đặc thù theo field (`$selectedfields[1] == 'tags'`) → điều kiện field không bao giờ tới. Thêm nhánh field-specific ở cuối mà quên comparator đã bị nhánh generic phía trên khớp.

**Rule:** Khi thêm nhánh xử lý field-đặc-thù vào chuỗi if/elseif dispatch-theo-comparator: (1) đọc MỌI nhánh generic phía trên khớp cùng comparator, thêm guard loại field (`&& $selectedfields[1] != '<field>'`) hoặc đẩy nhánh field-specific lên trước; (2) verify TỪNG toán tử bằng SQL thật trên DB, không chỉ `php -l`; (3) chạy review gate trên diff.

**Trigger keywords (review):** thêm nhánh `$selectedfields[1] == '<field>'` sau nhánh `$comparator == 'y'`/`'ny'` trong ReportRun · toán tử filter mới verify chỉ bằng `php -l`

## EP-005: Điều kiện lọc qua INNER JOIN không biểu diễn được "chưa có / trống" — phải correlated subquery

**Ticket / MR:** #10091 / —
**Date:** 2026-09-11
**Module / Layer:** `include/QueryGenerator/*QueryGenerator.php` tag filter; tổng quát: mọi field lọc-only qua bảng nối many-to-many

**Symptom:** Lọc "record CHƯA gắn tag" (hoặc chưa có quan hệ N-N) trả 0 record đúng dù thêm toán tử `IS NULL`/`y` — INNER JOIN đã loại sạch record không có dòng nối trước khi WHERE chạy.

**Root cause:** Điều kiện build bằng `INNER JOIN vtiger_freetagged_objects ... INNER JOIN vtiger_freetags` rồi WHERE `vtiger_freetags.id`. INNER JOIN = record không tag biến mất khỏi tập kết quả ⇒ không WHERE nào lấy lại "record không tag".

**Rule:** Field lọc-only qua bảng N-N: biểu diễn bằng correlated subquery `id_col IN/NOT IN (SELECT object_id FROM bảng_nối WHERE ...)`, KHÔNG INNER JOIN vào FROM. contains=`IN(... AND tag IN(ids))` · not-contains=`NOT IN(... AND tag IN(ids))` · trống=`NOT IN(...any)` · có=`IN(...any)`. Gỡ join khỏi FROM an toàn khi field là filter-only (không select/order). Verify: total = có + chưa trên DB thật.

**Trigger keywords (review):** `INNER JOIN vtiger_freetagged_objects` trong getFromClause · WHERE `vtiger_freetags.id` cho toán tử empty/`y` · lọc "chưa/không có" trên field qua INNER JOIN

## EP-006: Thêm `LIMIT` vào scan hàng đợi mà không phải mọi row scanned đều rời pool → starvation

**Ticket / MR:** #24616 / —
**Date:** 2026-09-11
**Module / Layer:** `CPPipelineManagement_Notification_Helper::getRecordOverdueNotificationsToSend()` (`vtiger_stage_overdue_notification_queue`); tổng quát: mọi cron `SELECT ... WHERE status=0 ORDER BY id LIMIT N` xử lý hàng đợi

**Symptom:** Sau khi thêm `ORDER BY queue_id LIMIT 200` để batch, notification cho record thực sự overdue có `queue_id` cao KHÔNG BAO GIỜ được gửi. Chạy im lặng, không lỗi. Site nhiều deal đã đóng / stage `time=0` càng nặng.

**Root cause:** Batch giả định "mỗi run flip status → run sau tiến tới phần còn lại". SAI: chỉ row THỰC SỰ overdue mới được `markOverdueNotificationAsSent()` (status=1). Row scanned-nhưng-không-overdue (end-stage / không cấu hình SLA / chưa tới hạn) giữ `status=0` VĨNH VIỄN và có `queue_id` thấp nhất. Khi số sticky row ≥ LIMIT, `ORDER BY queue_id LIMIT N` trả cùng N row cũ mỗi run → row overdue queue_id cao bị đói. Scan full (không LIMIT) ban đầu không có starvation.

**Rule:** Trước khi thêm `LIMIT` vào scan hàng đợi, chứng minh MỌI row được scan sẽ rời pool điều kiện `WHERE` (bị flip status / xóa / con trỏ vượt qua). Nếu có row "sticky" ở lại pool vô thời hạn thì `ORDER BY <pk> LIMIT` = starvation cho row mới. Batch đúng cần: (a) flip status cho row non-applicable ổn định, hoặc (b) persisted cursor `WHERE id > ? ... LIMIT N` cuộn vòng qua toàn bộ, hoặc (c) không LIMIT. Đừng viết comment khẳng định invariant "status flips drain queue" mà chưa trace hết nhánh `continue`/không-mark.

**Trigger keywords (review):** `ORDER BY queue_id` / `ORDER BY .*_id LIMIT` thêm mới vào scan `WHERE status = 0` mà trong loop có nhánh `continue` không kèm mark-status · comment "status flips" / "successive runs progress" cạnh một `LIMIT`

## EP-007: Inject HTML cross-origin qua `innerHTML` không sanitize → DOM-XSS (inline on* handler chạy)

**Ticket / MR:** #25943 / !411
**Date:** 2026-09-15
**Module / Layer:** Client loader nạp nội dung cross-domain — `resources/InternalCommunicationRemote.js`; tổng quát mọi JS `el.innerHTML = <nội dung fetch từ origin khác>` rồi appendChild vào DOM sống

**Symptom:** Nội dung nguồn (instance PMS) chứa `<img onerror=...>` / `<svg onload=...>` / `href="javascript:..."` → khi node vào DOM sống của CRM khách, handler chạy với session đã đăng nhập = DOM-XSS thực thi JS tuỳ ý vượt trust boundary.

**Root cause:** `<script>` không chạy qua innerHTML nên dễ tưởng an toàn, nhưng inline event handler (`on*`) và URL scheme `javascript:`/`data:` VẪN kích hoạt khi element trở thành node sống. Fetch cross-origin = nội dung ngoài tầm kiểm soát, không được tin.

**Rule:** Nội dung HTML từ origin/nguồn ngoài PHẢI sanitize client-side trước khi vào DOM sống, KHÔNG dựa vào nguồn: parse inert bằng `new DOMParser().parseFromString(html,'text/html')`, strip `script/iframe/object/embed/link/meta/base`, xoá mọi attribute `on*`, và bỏ `javascript:`/`data:` trên `src`/`href`/`xlink:href`; chỉ import subtree đã làm sạch (hoặc `DOMPurify.sanitize` nếu có bản vetted). KHÔNG gán thẳng `innerHTML = fetchedHtml`. Verify browser: payload `<img onerror>` → counter phải = 0 sau inject.

**Trigger keywords (review):** `.innerHTML =` với biến là body của `fetch(` khác origin · gán innerHTML nội dung remote mà không có DOMParser/DOMPurify/strip on*

## EP-008: "Cache" table không có khoá — INSERT mỗi lần refresh, đọc không ORDER BY → phình vô hạn + đọc row cũ

**Ticket / MR:** #26070 / —
**Date:** 2026-09-21
**Module / Layer:** `include/utils/FCMHelper.php` `getAccessToken` + `firebase_token_cache`; tổng quát mọi bảng dùng như single-value cache (token, config snapshot)

**Symptom:** Bảng cache phình tới hàng trăm nghìn row (475k), disk/backup bloat; cache-hit đọc `LIMIT 1` không `ORDER BY` → có thể trả token cũ đã hết hạn → refresh sai nhịp, gọi OAuth thừa.

**Root cause:** Bảng tạo KHÔNG có PRIMARY KEY; writer `INSERT` một row MỚI mỗi lần refresh (mỗi ~50 phút) thay vì update tại chỗ; đọc `SELECT ... LIMIT 1` không `ORDER BY last_fetch_time DESC` → MySQL trả row bất kỳ (thường cũ nhất theo thứ tự vật lý).

**Rule:** Bảng cache single-value = 1 row cố định: PK `id TINYINT UNSIGNED` (hoặc key cố định), ghi bằng `INSERT ... ON DUPLICATE KEY UPDATE` với `id=1`, đọc `ORDER BY <freshness_col> DESC LIMIT 1`. KHÔNG BAO GIỜ INSERT-append vào bảng cache. Thấy bảng cache phình row → kiểm tra ngay writer có upsert-by-key hay append.

**Trigger keywords (review):** `CREATE TABLE *cache*` không có PRIMARY KEY · `INSERT INTO *cache*` trong hàm refresh mà không `ON DUPLICATE KEY UPDATE` · `SELECT ... FROM *cache* LIMIT 1` không `ORDER BY`

## EP-009: Gọi HTTP ngoài chậm (FCM/OAuth/3rd-party) INLINE trong request/webhook path → nghẽn php-fpm

**Ticket / MR:** #26070 / —
**Date:** 2026-09-21
**Module / Layer:** `include/utils/NotificationHelper.php` + mọi caller `FCMHelper::sendNotification`; tổng quát mọi push/email/SMS/3rd-party HTTP gọi đồng bộ trong luồng request/webhook

**Symptom:** Webhook/request chậm 8-16s tuỳ lúc, HTTP 499, php-fpm cạn worker (cần 60 worker lúc flood). Từng request tự nó nhanh nhưng nhân theo N token/N user thì tắc pool.

**Root cause:** Funnel `sendNotification` lưu in-app notification (nhanh) RỒI gọi thẳng `FCMHelper::sendNotification` (Google HTTP v1, 0.5-0.9s/batch × N chunk) NGAY trong luồng php-fpm. Return value của push KHÔNG được caller dùng → hoàn toàn có thể defer. Bug ẩn cho tới khi tải cao.

**Rule:** Tách phần user-visible instant (ghi DB, publish realtime) khỏi I/O ngoài chậm: giữ ghi DB đồng bộ, ĐẨY push/email/SMS sang queue worker (`CPQueue_Server_Model::getCleanInstance()->addTask("SendNotification", [...])`, worker `deliver_fcm_push`). Nếu return value của lời gọi HTTP không ai dùng và nó nằm trên request path → defer. Job worker gọi helper CẤP THẤP (FCMHelper), KHÔNG gọi lại funnel (NotificationHelper) để tránh re-enqueue loop.

**Trigger keywords (review):** `FCMHelper::sendNotification` / `curl_exec` / `file_get_contents('http` gọi đồng bộ trong action/handler/webhook mà return không dùng · gửi push/email/SMS inline không qua queue

## EP-010: Lấy kiểu tệp đính kèm từ field `type` do client gửi

**Ticket / MR:** #26296 / !420
**Date:** 2026-09-28
**Module / Layer:** `modules/CPAICopilot/helpers/Sse.php` (lọc attachment); tổng quát mọi chỗ nhận lại tham chiếu tới tệp đã upload

**Symptom:** Gửi `[{url: ".../copilot_x.pdf"}]` (không có hoặc giả `type`) thì PDF được lưu và gửi cho AI dạng `{type:'image'}`; hub `msg_type = image`, lịch sử hiện thẻ `<img>` hỏng.

**Root cause:** Hàm lọc đã kiểm tra URL là tệp của server, nhưng lại rẽ nhánh theo `$item['type']` của request thay vì đuôi của tệp mà chính server đã lưu.

**Rule:** Sau khi kiểm tra một tham chiếu tới tệp đã upload, xác định loại tệp (ảnh/tệp...) từ tên hoặc nội dung tệp server đã lưu; KHÔNG dùng field type/mime do client gửi.

**Trigger keywords (review):** `$item['type']` · `($item['type'] ?? '') ===` · `['type'] ?? '') === 'file'`

## EP-011: Kiểm tra bảng tồn tại bằng `SHOW TABLES` trên mỗi request

**Ticket / MR:** — / commit e629714ef3, f84f7a5d8d
**Date:** 2026-09-28
**Module / Layer:** `Vtiger_Utils::CheckTable`; helper tự viết `SHOW TABLES LIKE` (`Campaigns_Data_Model`, `CPPipelineManagement_Logic_Helper`)

**Symptom:** Leads List AJAX chạy 20 `SHOW TABLES` mỗi render; bỏ đi thì server time giảm 25-41%.

**Root cause:** Code phòng bảng chưa được migration tạo, hỏi lại MySQL mỗi lần gọi, thậm chí gọi trong vòng lặp. Kết quả "có bảng" gần như không bao giờ đổi.

**Rule:** Dùng `Vtiger_Utils::CheckTable()` (memo request + Redis, chỉ cache kết quả "có"), không tự viết `SHOW TABLES`. Code DROP/RENAME bảng phải gọi `Vtiger_Utils::forgetTable()`.

**Trigger keywords (review):** `SHOW TABLES` · `information_schema.tables` trong code chạy theo request · `CheckTable(` trong vòng lặp

## EP-012: Template gọi model/DB/Redis trong vòng lặp

**Ticket / MR:** — / commit 166588e97b, 2b03b11564
**Date:** 2026-09-28
**Module / Layer:** Smarty `.tpl` (list/detail), `Vtiger_Field_Model::getFieldInfo`, màu picklist theo giá trị

**Symptom:** Trang Detail có 167 query, trong đó 33 query nằm trong template; mỗi ô picklist hỏi DB lấy màu, mỗi field hỏi lại danh sách picklist.

**Root cause:** Gọi `{Class::method(...)}` hoặc hàm model bên trong `{foreach}`, nên mỗi dòng/ô chạy 1 query, ẩn khỏi controller.

**Rule:** Template chỉ hiển thị dữ liệu. Controller/model lấy dữ liệu một lần theo lô (hoặc cache theo request) rồi `assign`; không gọi model/DB/Redis trong `{foreach}`.

**Trigger keywords (review):** `{foreach` chứa `_Model::` · `getInstance(` · `->pquery(` · `getPicklistColor` · `RedisUtils` trong `.tpl`

## EP-013: Gọi model/query cho từng dòng trên trang danh sách (N+1)

**Ticket / MR:** — / commit 9c62bf327b, ea7c72593a
**Date:** 2026-09-28
**Module / Layer:** List view (`Accounts_ListView_Model`, `getRecordName`, group-for-user, overdue info)

**Symptom:** Accounts List AJAX 653 query và 252 lệnh Redis cho một trang; Leads 249 query.

**Root cause:** Mỗi dòng tự gọi `getInstanceById`/`getRecordName`/check flag/Redis `get` riêng, nên chi phí nhân theo page size.

**Rule:** Gom id của cả trang, lấy bằng 1 query (hoặc Redis `MGET`), map lại theo id. Quy tắc chi tiết: `performance-rules.md` § Avoid N+1. Đo số query/Redis call mỗi request (harness `tests/Perf/baseline.php --budget` chỉ có trên các commit perf-harness d4bbf067ef / 9db3273910, không có trên nhánh này).

**Trigger keywords (review):** `foreach ($listViewEntries` / `foreach ($rows` chứa `getInstanceById(` · `getRecordName(` · `->pquery(` · `RedisUtils::` `->get(`

## EP-014: Redis `KEYS pattern` trên đường chạy thật

**Ticket / MR:** — / commit aa8e0c020e
**Date:** 2026-09-29
**Module / Layer:** `Vtiger_Cache_Connector_Redis::flushByPattern` và mọi chỗ xoá cache theo pattern

**Symptom:** Mỗi lần flush cache theo pattern, Redis chặn mọi client (session, cache, queue) trong thời gian quét toàn bộ keyspace.

**Root cause:** `KEYS` là O(N) và chạy đơn luồng trên server Redis; keyspace dùng chung session + cache nên càng lớn càng chậm.

**Rule:** Dùng `SCAN` theo lô (`COUNT 1000`) rồi `DEL`/`UNLINK` từng lô; không gọi `KEYS` ngoài script CLI chạy tay.

**Trigger keywords (review):** `->keys(` · `KEYS ` · `'keys'` trong code PHP chạy theo request/cron

## EP-015: Đổi cờ/biến global rồi không trả lại

**Ticket / MR:** — / commit 5a6e83f19e
**Date:** 2026-09-28
**Module / Layer:** `include/ListView/ListViewSession.php` `getListViewNavigation` (`Zend_Json::$useBuiltinEncoderDecoder`)

**Symptom:** Sau khi trang List chạy hàm điều hướng, mọi `Zend_Json::encode` còn lại trong request (Footer, JS language) dùng encoder PHP thuần chậm hơn nhiều thay vì `json_encode`.

**Root cause:** Hàm đặt `Zend_Json::$useBuiltinEncoderDecoder = true` để xử lý riêng mình nhưng không đặt lại, nên ảnh hưởng tới code chạy sau trong cùng request.

**Rule:** Đổi static/global dùng chung thì lưu giá trị cũ và khôi phục trong `finally`. Ưu tiên truyền tham số thay vì bật cờ toàn cục.

**Trigger keywords (review):** `Zend_Json::$useBuiltinEncoderDecoder =` · gán `static::$` / `$GLOBALS[` trong hàm mà không có `finally` khôi phục

## EP-016: Index/column added by editing an already-applied migration never reaches existing databases

**Ticket / MR:** #26929 / !440
**Date:** 2026-10-19
**Module / Layer:** `modules/CPMigration/migrations/*` · `modules/CPMigration/models/Migrator.php` (skips every file with a `success = 1` history row)

**Symptom:** A performance fix adds `KEY ...` / `addIndexIfMissing` inside an existing migration (`CreateSocialCommentOutbox`, `CreateSocialCommentPrivateReply`, `CreateSocialConversation`); fresh installs get the index, but every dev/QA/prod DB that already ran the migration keeps the full scan.

**Root cause:** The migration runner never re-runs an applied migration, so "add if missing" branches in an old file are dead code on existing databases.

**Rule:** Schema changes to a table created by an applied migration go into a NEW migration file (idempotent: `addIndexIfMissing` / `addColumnIfMissing`, skip a missing table). Editing the creating migration is only for fresh installs and never replaces the new file.

**Trigger keywords (review):** `addIndexIfMissing(` · `addColumnIfMissing(` · `ALTER TABLE` in a diff hunk of a migration file not added by the same MR

## EP-017: Platform write succeeds, then an unguarded local follow-up throws and the user sees failure

**Ticket / MR:** #26929 / !440
**Date:** 2026-10-19
**Module / Layer:** `domains/Social/src/{Moderation,Reaction,Reply}/*Service.php` (FB first, Hub/CRM after)

**Symptom:** Hide/delete/like/reply/block are applied on Facebook, but the staff UI shows `hide_failed` / `like_failed` / `send_failed`; a retry takes the "already done" branch and never repairs the CRM/Hub state (conversation stuck "unreplied", block label missing).

**Root cause:** After the irreversible external call, Hub writes, inbox delivery and thread notifications ran without try/catch, and the retry path skipped them.

**Rule:** After an irreversible external write succeeds, every local follow-up (store, inbox, notify, summaries) runs in its own logged try/catch and the response reports the external result. The idempotent retry branch must re-apply the local follow-ups, not just return.

**Trigger keywords (review):** `->hideComment(` · `->deleteComment(` · `->postReply(` · `->likeComment(` · `->blockPerson(` followed by `->notifyThreadChanged(` / `->deliverOutbound(` / `->setHidden(` outside `try {`

## EP-018: `count()` on an ArrayAccess object that is not Countable (webklex `Attribute`)

**Ticket:** #27228
**Date:** 2026-10-29
**Module / Layer:** `modules/Settings/MailConverter/handlers/MailRecordMessage.php::parseHeader` (any code reading `Webklex\PHPIMAP\Header::get()`)

**Symptom:** On PHP 8.3 the Mail Converter converted no email: "Quét hộp thư" showed "Chuyển đổi Email đang chạy" and the MailScanner cron failed, while PHP 7 servers worked.

**Root cause:** `Header::get('date')` returns `Webklex\PHPIMAP\Attribute|null`; `Attribute` implements `ArrayAccess` and has a `count()` method but not `Countable`. PHP 7 `count($attr)` warned and returned 1; the php7Count retirement turned it into `count($x ?? [])`, a `TypeError` on every mail (`?? []` only covers null). `performScanNow()` catches every Throwable, so the UI only showed a generic message.

**Rule:** Before replacing `php7Count()`/`count()` on a value, check what it can hold: an object needs its own API (`$x->count()`, `$x->toArray()`) or `instanceof Countable`; `?? []` does not make a non-null object countable. Read the class of library return values (webklex, Carbon, collections) instead of assuming arrays.

**Trigger keywords (review):** `count($dateHeader` · `count($header->get(` · `->get('date')` · `?? []) > 0 &&` · `Webklex\PHPIMAP\Attribute`

## EP-019: `mb_convert_encoding()` with a charset name taken from mail data throws ValueError on PHP 8

**Ticket:** #27228
**Date:** 2026-10-29
**Module / Layer:** `modules/Settings/MailConverter/handlers/MailRecord.php::__convert_encoding` (shared by MailConverter and MailManager `__mime_decode`)

**Symptom:** Headers like `Re: =?utf-8?...?=` or a charset mbstring does not know (`windows-1258`, `unknown-8bit`) killed the whole request on PHP 8.3; PHP 7.4 only warned.

**Root cause:** `imap_mime_header_decode()` marks unencoded parts with charset `default`, and mail charsets are free text. PHP 8 `mb_convert_encoding()` throws `ValueError` for any name it does not know; PHP 7.4 warned and converted from the internal encoding (byte-identical to `mb_convert_encoding($s, $to, mb_internal_encoding())` on 8.3).

**Rule:** A charset name that comes from outside (mail headers/parts, imap, HTTP, files) must not reach `mb_convert_encoding()`/`mb_check_encoding()` unguarded: catch `ValueError` and fall back to the internal encoding (PHP 7 behaviour) or validate the name first. Never fall back to returning raw bytes when the result goes through `Normalizer::normalize()` (invalid UTF-8 → `false` → empty text).

**Trigger keywords (review):** `mb_convert_encoding($input, $to, $from)` · `imap_mime_header_decode(` · `$prop->charset` · `->charset->first()` · `mb_convert_encoding($data, `

## EP-020: native `strpos`/`strstr` with a needle that can be empty or null matches everything on PHP 8

**Ticket:** #27245 (MR !450)
**Date:** 2026-10-29
**Module / Layer:** any `strpos/stripos/strstr/str_contains` left by the php7Strstr/php7Strpos retirement (11200e8861): `ComposeEmail::emailReplyAll`, Reports `ReportContents.tpl` / `CustomReportDetail.tpl` / `ReportRun`, `AbenlaConnector`

**Symptom:** On PHP 8.3 reply-all of a MailScanner email lost the To address; reports grouped by 1–2 fields rendered every non-group column as an empty summary cell.

**Root cause:** PHP 8 `strpos($h, '')` = 0 and `strstr($h, '')` = `$h` (PHP 7: false). The needle came from optional data (`mailboxemail` absent → null, unused group level never assigned → null/''). A guard `=== ''` misses null, and Smarty `strstr($NAME, $X)` with unassigned `$X` gets null.

**Rule:** Before a native substring search on a needle that may be absent, guard with `empty($needle)` / `$needle !== '' && $needle !== null` (Smarty: `$X neq '' && strstr(...)`), and initialise optional needles to '' at assignment.

**Trigger keywords (review):** `=== '' ? false : strpos(` · `strstr($NAME, $` · `strstr($fieldName, $` · `strpos($email, $mailBoxEMail)`

## EP-021: two aftersave handlers that `save()` each other's record loop forever

**Ticket:** #27245 (MR !450)
**Date:** 2026-10-29
**Module / Layer:** `CPReceipt_Logic_Helper::updateRelatedSponsorDetail` ↔ `CPSponsorDetailHandler::processRelatedReceipt`; any handler that syncs a status to a related record via `Vtiger_Record_Model::save()`

**Symptom:** Saving a sponsor detail (with `process_related_receipt=1`) or its receipt never returned: 65k nested saves, then a fatal.

**Root cause:** A refactor replaced a direct UPDATE with a record-model `save()` so the related record's handlers fire; that handler saved the first record back, and the request flag (static `getRequest()`) kept the chain alive.

**Rule:** A handler that saves a related record must save only when the value actually changes (compare as strings), or write with a direct UPDATE when the related handlers are not needed.

**Trigger keywords (review):** `$sponsorDetailRecord->save()` · `updateRecord(reset($receiptIds)` · `->set('mode', 'edit');` inside `handlers/` or `helpers/Logic.php` called from aftersave

## EP-022: `array_key_exists()` on an unserialized object

**Ticket:** — (MR !450, commit 233847c2f4)
**Date:** 2026-10-29
**Module / Layer:** `modules/Settings/Picklist/handlers/PickListHandler.php` (picklist delete/rename → workflow tasks); any code that inspects `unserialize()`d / decoded objects

**Symptom:** Settings > Picklist: deleting or renaming a Potentials value on PHP 8.3 threw `array_key_exists(): Argument #2 ($array) must be of type array, VTEmailTask given`.

**Root cause:** PHP 7 accepted an object in `array_key_exists()` (deprecated in 7.4) and checked its properties. PHP 8 throws a TypeError. The earlier sweep fixed only the `field_value_mapping` line and missed the same call two branches down.

**Rule:** On objects use `is_object($o) && property_exists($o, $name)`, which gives the same result for public and dynamic properties. When fixing one call, grep the whole file for the same pattern.

**Trigger keywords (review):** `array_key_exists($pickListFieldName, $unserializeTask)` · `array_key_exists(` + `unserialize(`

## EP-023: static call to a non-autoloaded `include/utils` class without `require_once`

**Ticket:** — (MR !450, commit 233847c2f4)
**Date:** 2026-10-29
**Module / Layer:** `modules/CPSocialIntegration/actions/IndividualZaloAjax.php`, `ZaloOAAjax.php` (remove account → Social Hub)

**Symptom:** Social Config, disconnecting a personal Zalo account: `Class "SocialChatUtils" not found` (only when `$socialHub['enable']`, so it went unseen).

**Root cause:** `include/utils/*Utils` classes are not covered by `Vtiger_Loader` (class name does not follow `Module_Component_Type`). The action called `SocialChatUtils::` and relied on some other file having loaded it.

**Rule:** Every file that calls an `include/utils` class statically must `require_once` that class file itself (sibling actions `TelegramAjax` / `FacebookAppAjax` already do). Review: grep `ClassName::` against `ClassName.php` in the same file.

**Trigger keywords (review):** `SocialChatUtils::callSocialIntegrationHubApi` · `SocialChatUtils::`

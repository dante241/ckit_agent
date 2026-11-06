---
description: "EP-NNN catalog (Trigger/Rule/Why). Before coding/reviewing PHP: grep the diff against the Trigger lines; /review-pr passes matched entries to reviewers as $EP_HITS."
---

# Error Patterns (EP-NNN)

> Mỗi entry: `Trigger:` (chuỗi trong backtick = needle grep -F trên dòng `+` và đường dẫn file của diff; chữ ngoài backtick = gợi ý ngữ nghĩa) · `Rule:` · `Why:`.
> ID chỉ tăng, không xoá entry; thêm mới qua fix Step 9 / cook 10.6 (USER GATE). Ticket/symptom đầy đủ ở PMS / git log.

## EP-001 Custom view sai base class / TPL location / Helper filename
Trigger: `extends Vtiger_Index_View` (view không phải List/Edit/Detail) · `Helper.php` · `$viewer->view(` · TPL custom đặt trong `layouts/v7/modules/`
Rule: custom view → `extends CustomView_Base_View`, TPL `modules/<Module>/tpls/<ViewName>.tpl`, helper filename = Component (rule://cloudgo-development-rules § View Base Classes; skill `view/references/custom-view-base.md`).
Why: sai base class mất auto-load CSS/JS theo viewName; `<Name>Helper.php` làm autoload `<Module>_<Name>_Helper` fail.

## EP-002 Language key đặt sai array ($languageStrings vs $jsLanguageStrings)
Trigger: `languages/` · `$jsLanguageStrings` · key `LBL_`/`JS_` thêm sát `);` cuối file
Rule: `LBL_*` → `$languageStrings`, `JS_*` → `$jsLanguageStrings`; label dùng cả TPL lẫn JS cần key ở cả 2 array; không append mù cuối file; verify array cho cả 2 locale (skill `language` Pitfall #8).
Why: 2 array không thấy nhau, `);` cuối file đóng array JS → TPL render raw key; `php -l` không bắt.

## EP-003 Thêm nhánh event vào handler nhưng chưa đăng ký row cho event đó
Trigger: `if ($eventName == 'vtiger.entity.` nhánh mới trong Handler.php mà diff không có registerHandler / INSERT vtiger_eventhandlers
Rule: nhánh event MỚI → migration idempotent (check `vtiger_eventhandlers` theo handler_class + event_name rồi `VTEventsManager::registerHandler`), `down()` xoá row; verify bằng luồng save thật.
Why: VTEventsManager dispatch theo row `(handler_class, event_name)`, không theo nhánh `if` (ModCommentsHandler chỉ có row aftersave → nhánh beforesave chết).

## EP-004 Nhánh field-đặc-thù bị nhánh generic theo comparator đứng trước nuốt mất
Trigger: `$selectedfields[1] ==` sau nhánh comparator 'y'/'ny' trong ReportRun · toán tử filter mới chỉ verify bằng `php -l`
Rule: thêm nhánh field-specific vào chuỗi if/elseif theo comparator → guard loại field ở mọi nhánh generic phía trên (`&& $selectedfields[1] != '<field>'`) hoặc đưa nhánh field lên trước; verify từng toán tử bằng SQL thật.
Why: if/elseif khớp theo thứ tự → nhánh mới thành dead code, SQL `X.tags IS NOT NULL` trên virtual field lỗi.

## EP-005 Lọc "chưa có / trống" qua INNER JOIN bảng N-N
Trigger: `INNER JOIN vtiger_freetagged_objects` · `vtiger_freetags.id`
Rule: field lọc-only qua bảng N-N → correlated subquery `id IN/NOT IN (SELECT object_id FROM <bảng nối> WHERE ...)`, không JOIN vào FROM; verify total = có + chưa trên DB thật.
Why: INNER JOIN loại record không có dòng nối trước WHERE → "chưa gắn tag" luôn 0 (QueryGenerator tag filter).

## EP-006 LIMIT trên scan hàng đợi mà không phải row nào cũng rời pool → starvation
Trigger: `ORDER BY queue_id` · `status flips` · `successive runs progress` · LIMIT mới trên scan WHERE status = 0 trong loop có continue không mark status
Rule: trước khi thêm LIMIT, chứng minh mọi row scan đều rời pool WHERE; không thì flip status row non-applicable, dùng cursor `id > ?` lưu lại, hoặc bỏ LIMIT.
Why: row scan-nhưng-không-xử-lý giữ status=0 với id thấp nhất → mỗi run lấy lại cùng N row (CPPipelineManagement overdue notification queue).

## EP-007 innerHTML với HTML cross-origin không sanitize → DOM-XSS
Trigger: `.innerHTML =` · `innerHTML=` với body của `fetch(` khác origin
Rule: parse inert bằng `DOMParser`, strip `script/iframe/object/embed/link/meta/base`, attribute `on*`, `javascript:`/`data:` URL (hoặc `DOMPurify.sanitize`) rồi mới import; verify payload `<img onerror>` không chạy.
Why: `<script>` không chạy qua innerHTML nhưng `on*` và `javascript:` vẫn chạy với session CRM (InternalCommunicationRemote.js).

## EP-008 Bảng cache không khoá, INSERT mỗi lần refresh
Trigger: `_cache` · `CREATE TABLE` cache không PRIMARY KEY · `INSERT INTO` cache không `ON DUPLICATE KEY UPDATE` · `LIMIT 1` không `ORDER BY`
Rule: cache single-value = 1 row cố định (PK `id`), ghi `INSERT ... ON DUPLICATE KEY UPDATE` với `id=1`, đọc `ORDER BY <freshness> DESC LIMIT 1`; không INSERT-append.
Why: `firebase_token_cache` phình 475k row, `LIMIT 1` không ORDER BY trả token cũ.

## EP-009 HTTP ngoài chậm gọi inline trong request/webhook
Trigger: `FCMHelper::sendNotification` · `curl_exec(` · `file_get_contents('http` · push/email/SMS gửi inline không qua queue
Rule: ghi DB/realtime đồng bộ, đẩy push/email/SMS sang queue (`CPQueue_Server_Model::...->addTask(...)`); worker gọi helper cấp thấp (FCMHelper), không gọi lại funnel (re-enqueue loop).
Why: N chunk × 0.5–0.9s ngay trong php-fpm → request 8–16s, 499, cạn worker (NotificationHelper).

## EP-010 Lấy kiểu tệp đính kèm từ field `type` client gửi
Trigger: `$item['type']` · `['type'] ?? '') ===`
Rule: sau khi kiểm tra tham chiếu tới tệp đã upload, xác định loại tệp từ tên/nội dung tệp server lưu, không dùng type/mime client gửi.
Why: PDF giả `type` được gửi AI dạng image, lịch sử hiện `<img>` hỏng (CPAICopilot Sse).

## EP-011 `SHOW TABLES` trên mỗi request
Trigger: `SHOW TABLES` · `information_schema.tables` · `CheckTable(` trong vòng lặp
Rule: dùng `Vtiger_Utils::CheckTable()` (memo request + Redis, chỉ cache "có"); code DROP/RENAME bảng gọi `Vtiger_Utils::forgetTable()`.
Why: List AJAX chạy 20 `SHOW TABLES`/render; bỏ đi giảm 25–41% server time.

## EP-012 Template gọi model/DB/Redis trong `{foreach}`
Trigger: `getPicklistColor` · RedisUtils / _Model:: / getInstance( / pquery bên trong {foreach của file .tpl
Rule: template chỉ hiển thị; controller/model lấy dữ liệu theo lô (hoặc cache theo request) rồi `assign`.
Why: Detail 167 query, 33 nằm trong template (màu picklist, getFieldInfo mỗi ô).

## EP-013 Query/model cho từng dòng trên trang danh sách (N+1)
Trigger: `foreach ($listViewEntries` · `getRecordName(` · getInstanceById( / pquery / RedisUtils get trong loop dòng
Rule: gom id cả trang, lấy bằng 1 query (hoặc Redis `MGET`), map theo id (rule://performance-rules § Avoid N+1).
Why: Accounts List AJAX 653 query + 252 lệnh Redis mỗi trang.

## EP-014 Redis `KEYS pattern` trên đường chạy thật
Trigger: `->keys(` · `'keys'` · lệnh KEYS trong code request/cron
Rule: `SCAN` theo lô (`COUNT 1000`) rồi `DEL`/`UNLINK`; `KEYS` chỉ trong script CLI chạy tay.
Why: `KEYS` O(N) đơn luồng chặn mọi client session/cache/queue (`flushByPattern`).

## EP-015 Đổi cờ/biến global rồi không trả lại
Trigger: `Zend_Json::$useBuiltinEncoderDecoder =` · gán static::$ / $GLOBALS[ trong hàm không có finally khôi phục
Rule: đổi static/global dùng chung → lưu giá trị cũ, khôi phục trong `finally`; ưu tiên truyền tham số.
Why: `getListViewNavigation` bật encoder PHP thuần và để nguyên → mọi `Zend_Json::encode` sau đó trong request chậm.

## EP-016 Index/column thêm bằng cách sửa migration đã chạy
Trigger: `addIndexIfMissing(` · `addColumnIfMissing(` · `ALTER TABLE` trong hunk của migration file không do MR này thêm
Rule: đổi schema bảng do migration đã chạy tạo → migration MỚI, idempotent, bỏ qua bảng chưa có; sửa migration gốc chỉ cho fresh install.
Why: Migrator bỏ qua file đã có history `success = 1` → DB hiện có không bao giờ nhận index.

## EP-017 Platform write thành công, follow-up local không guard throw → user thấy lỗi
Trigger: `->hideComment(` · `->deleteComment(` · `->postReply(` · `->likeComment(` · `->blockPerson(` rồi `->notifyThreadChanged(` / `->deliverOutbound(` / `->setHidden(` ngoài `try {`
Rule: mỗi follow-up local (store, inbox, notify) sau external write không đảo ngược chạy trong try/catch có log riêng, response báo kết quả external; nhánh retry idempotent chạy lại follow-up local.
Why: FB đã hide/reply nhưng UI báo `*_failed`, retry không sửa Hub/CRM (domains/Social *Service).

## EP-018 `count()` trên object ArrayAccess không Countable
Trigger: `count($dateHeader` · `count($header->get(` · `->get('date')` · `?? []) > 0 &&` · `Webklex\PHPIMAP`
Rule: object dùng API riêng (`->count()`, `->toArray()`) hoặc `instanceof Countable`; `?? []` không làm object countable; đọc class trả về của thư viện.
Why: webklex `Attribute` có `count()` nhưng không Countable → PHP 8.3 TypeError mỗi mail, MailScanner không chuyển email nào.

## EP-019 `mb_convert_encoding()` với charset lấy từ dữ liệu mail
Trigger: `mb_convert_encoding(` · `mb_check_encoding(` · `imap_mime_header_decode(` · `->charset`
Rule: charset từ ngoài không vào `mb_*` trần: catch `ValueError` rồi fallback `mb_internal_encoding()`, hoặc validate trước; không trả raw bytes nếu sau đó qua `Normalizer::normalize()`.
Why: charset `default`/`windows-1258`/`unknown-8bit` → PHP 8.3 ValueError giết cả request (MailRecord `__convert_encoding`).

## EP-020 `strpos`/`strstr` với needle rỗng/null khớp mọi thứ
Trigger: `=== '' ? false : strpos(` · `strstr($NAME, $` · `strstr($fieldName, $` · `strpos($email, $mailBoxEMail)` · strpos/stripos/strstr/str_contains với needle từ dữ liệu optional
Rule: guard `empty($needle)` / `$needle !== '' && $needle !== null` (Smarty: `$X neq '' && strstr(...)`), khởi tạo needle optional = '' lúc gán.
Why: PHP 8.3 `strpos($h, '')` = 0, `strstr($h, '')` = `$h` → needle null/'' khớp mọi thứ (reply-all mất To, ô summary report rỗng); guard `=== ''` sót null.

## EP-021 Hai aftersave handler `save()` record của nhau → vòng lặp vô hạn
Trigger: `$sponsorDetailRecord->save()` · `updateRecord(reset($receiptIds)` · ->set('mode', 'edit') + ->save() trong handlers/ hoặc helpers/Logic.php gọi từ aftersave
Rule: handler save record liên quan chỉ save khi giá trị thực sự đổi (so sánh dạng string), hoặc UPDATE trực tiếp khi không cần handler phía kia.
Why: refactor UPDATE → `save()` làm handler bên kia save ngược lại → 65k save lồng nhau rồi fatal (CPReceipt ↔ CPSponsorDetail).

## EP-022 `array_key_exists()` trên object unserialize
Trigger: `array_key_exists(` cùng `unserialize(` · `$unserializeTask`
Rule: object → `is_object($o) && property_exists($o, $name)`; sửa một chỗ thì grep cả file cùng pattern.
Why: PHP 8.3 TypeError với `VTEmailTask` khi xoá/đổi tên picklist (PickListHandler; sweep cũ sót nhánh thứ hai).

## EP-023 Static call tới class `include/utils` không autoload mà thiếu `require_once`
Trigger: `SocialChatUtils::` · static call <Name>Utils:: tới class include/utils mà file không require_once
Rule: mỗi file gọi static class `include/utils` phải tự `require_once` file class đó; review: grep `ClassName::` đối chiếu `ClassName.php` trong cùng file.
Why: `Vtiger_Loader` không phủ `include/utils/*Utils` → `Class "SocialChatUtils" not found` chỉ khi Social Hub bật.

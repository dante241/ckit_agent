# Error Patterns — PENDING (draft tự động, chờ user promote vào error-patterns.md)

> Flow: cook Phase 10.6 / fix Step 9 auto-draft vào đây (KHÔNG cần approve).
> User review định kỳ → promote entry tốt sang `error-patterns.md` (gán EP-NNN) → từ đó code/review check theo catalog (không có hook tự động).

---

## PEND-field-migration: Tạo field entity bằng SQL migration thay vì BFR

**Ticket / MR:** — (user báo lỗi lặp lại nhiều lần, 2026-07-03)
**Date:** 2026-07-03
**Module / Layer:** Field creation (mọi entity module)

**Symptom:** Field mới có cột trong DB nhưng không có metadata `vtiger_field` đúng, hoặc có migration ALTER TABLE thừa song song với BFR; label không hiển thị; layout không thấy field.

**Root cause:** AI viết migration `ALTER TABLE vtiger_x ADD COLUMN` theo thói quen framework khác. Chuẩn CloudGo: field entity đi qua `BlocksAndFieldsRegister.php` + `php cli_tool.php -c quick_repair` (tự tạo cột DB) + label ở CẢ `languages/en_us/` VÀ `languages/vn_vn/` ($languageStrings).

**Rule:** Field trên entity module = BFR + quick_repair + 2 locale label, KHÔNG BAO GIỜ migration. Migration chỉ cho bảng phi-entity (quan hệ, config, index). Đã enforce bằng tripwire T9.

**Trigger keywords (review):** ALTER TABLE vtiger_ · ADD COLUMN

---

## PEND-field-uitype: Chọn sai uitype khi tạo field

**Ticket / MR:** — (user báo lỗi lặp lại nhiều lần, 2026-07-03)
**Date:** 2026-07-03
**Module / Layer:** Field creation / BlocksAndFieldsRegister

**Symptom:** Field render sai control (datetime thành text, dropdown không có giá trị), typeofdata/columntype lệch chuẩn codebase.

**Root cause:** AI chọn uitype theo tài liệu VTiger open-source thay vì theo codebase: dùng 50 cho datetime (codebase dùng 70 — 236 fields, 0 field dùng 50), dùng 15 cho dropdown thường (chuẩn là 16 — 389 fields), tưởng 19 là varchar (là mediumtext).

**Rule:** Chọn uitype = tra bảng "UITypes Quick Reference" trong skill `field` (build từ BFR thật) hoặc grep 1 field cùng loại trong `modules/*/BlocksAndFieldsRegister.php` và copy entry. Datetime = 70. Dropdown = 16. Verify bước 1 của skill field so uitype trong DB với bảng.

**Trigger keywords (review):** 'uitype' => '50' · 'uitype' => '15'

---

## PEND-brace-copy-old-code: AI bắt chước brace/comment style của code cũ xung quanh thay vì theo php-conventions.md

**Ticket / MR:** #10432 (user báo lỗi 2026-07-03)
**Date:** 2026-07-03
**Module / Layer:** Bất kỳ file PHP core cũ nào (Save.php, SaveAjax.php, và các file legacy khác)

**Symptom:** Code mới AI viết dùng `} else {` cùng dòng (K&R) thay vì `else` xuống dòng riêng (Stroustrup); comment dài nhiều dòng giải thích chi tiết thay vì 1 dòng ngắn; nhét ticket ID (`[Fix #10432]`) vào code comment. Code-reviewer subagent review vẫn báo "PASS" — không bắt được các lỗi này.

**Root cause:** File core cũ (VD `Save.php`, `SaveAjax.php`) được viết theo chuẩn cũ của công ty (else cùng dòng `}`, comment style khác). Khi sửa file này, AI pattern-match theo code cũ *nhìn thấy ngay trong file* (context cục bộ) thay vì tra lại `php-conventions.md` (rule cố định, đã đọc từ đầu phiên nhưng bị "quên" theo ngữ cảnh sau nhiều bước trung gian: tìm ticket, trace root cause...). File core cũ ≠ chuẩn để bắt chước — chuẩn mới của công ty được định nghĩa DUY NHẤT trong `php-conventions.md`.

**Rule:** Khi sửa file PHP bất kỳ (kể cả file core cũ dùng chuẩn cũ), brace/comment style của code MỚI phải theo `php-conventions.md`, KHÔNG bắt chước style code cũ xung quanh trong cùng file. Cụ thể: else/elseif/catch/finally luôn xuống dòng riêng (Stroustrup) dù `if` cùng dòng (K&R); comment 1 dòng ngắn `// Modified by X on DATE to REASON`, không nhét ticket/finding ID (rule `review-audit-self-decision.md` — Stable Code Artifacts). Sau khi code xong, BẮT BUỘC tự grep diff đối chiếu (không tin subagent review một mình):
```bash
git diff -- <file> | grep -n "}\s*else\|}\s*catch\|}\s*finally"   # phải KHÔNG match cho code mới (dòng bắt đầu bằng +)
git diff -- <file> | grep -n "^\+.*#[0-9]\{3,\}"                    # ticket ID lọt vào comment
```

**Trigger keywords (review):** `+\t\t\t} else {` hoặc tương tự trong diff (dòng thêm mới có `} else {`/`} catch (`/`} finally {`) · comment thêm mới dài > 1 dòng · `[Fix #` hoặc `#\d{4,}` trong comment thêm mới

---

## PEND-conditional-var-used-outside-if: Biến chỉ gán trong nhánh `if` nhưng dùng vô điều kiện ngoài `if`

**Ticket / MR:** #17557
**Date:** 2026-07-03
**Module / Layer:** Bất kỳ hàm PHP nào có biến trung gian gán trong `if(...)` rồi dùng lại sau khối `if` mà không có `else`

**Symptom:** Khi điều kiện của `if` trở thành `false` (branch trước giờ luôn `true` trong thực tế, chưa từng bị test qua nhánh `false`) → biến dùng ngoài `if` là undefined → PHP warning, hoặc logic rơi vào giá trị rỗng/sai (SQL `IN ()` rỗng, `$db->num_rows($result)` gọi trên biến chưa gán).

**Root cause:** `HelpDesk_Module_Model::getOpenTickets()`/`getTicketsByStatus()` (`modules/HelpDesk/models/Module.php`) gán `$picklistvaluesmap`/`$result` chỉ bên trong `if(vtws_isRoleBasedPicklist('ticketstatus'))`, rồi dùng biến đó vô điều kiện ngay sau khối `if` (không có `else` khởi tạo giá trị mặc định). Vì `ticketstatus` từ trước tới giờ LUÔN role-based (`true`), nhánh `false` chưa từng chạy qua production — bug tiềm ẩn nằm im cho tới khi 1 thay đổi khác (thêm field vào `$nonRoleBasedPicklists`) làm hàm điều kiện trả `false` lần đầu tiên.

**Rule:** Khi viết `if(cond) { $x = ...; }` rồi dùng `$x` sau khối `if` mà không có nhánh `else`, PHẢI: (1) khởi tạo `$x` với giá trị mặc định AN TOÀN trước `if`, hoặc (2) thêm `else { $x = <default>; }`, hoặc (3) bọc luôn phần dùng `$x` vào trong `if`. Đặc biệt cảnh giác khi sửa 1 hàm helper/config dùng chung (vd `vtws_isRoleBasedPicklist()`) — BẮT BUỘC grep toàn bộ call site (`grep -rn "tên_hàm("`) và kiểm tra từng nơi có giả định ngầm "điều kiện này luôn true/false" hay không, trước khi đổi hành vi hàm đó.

**Trigger keywords (review):** biến gán trong `if(...) { $var = ...; }` không có `else`, được dùng lại (đọc/gọi method) ở dòng ngay sau khối `if` cùng scope · thay đổi return value của 1 hàm helper dùng chung mà không grep toàn bộ call site trước

---

## PEND-migration-dml-autocommit-off: Migration runner nuốt DML — chỉ DDL sống sót, "success" giả

**Ticket / MR:** #23494
**Date:** 2026-08-28
**Module / Layer:** CPMigration (`modules/CPMigration/models/Migrator.php`, `Base.php`) — mọi migration thuần DML (INSERT/UPDATE/DELETE, vd `createSystemConfig`, seed row)

**Symptom:** `php modules/CPMigration/Main.php up` in "Migration success" nhưng row KHÔNG có trong DB khi query bằng connection khác (MCP/CLI mới). Migration DDL (CREATE/ALTER) thì persist bình thường. Cùng câu SQL chạy standalone lại persist.

**Root cause:** Một sibling migration chạy trước dùng `multipquery(useTransaction=true)`/`startTransaction()` rồi throw giữa chừng (vd "column already exists" khi re-run) → ADOdb smart-transaction để mở, autocommit OFF cho cả process. Mọi DML sau đó (kể cả ghi `vtiger_migration_history`) roll back lúc disconnect; chỉ DDL sống nhờ MySQL implicit commit. `runMirations()` vẫn báo success vì `up()` trả `UP_SUCCESS` bất kể pquery có commit hay không, và `completeTransaction()` sẽ ROLL BACK (không commit) khi `HasFailedTrans()` sticky = true.

**Rule:** Migration thuần DML phải tự đảm bảo commit: `$this->db->query('SET autocommit = 1');` ở đầu `up()` (đừng dựa `startTransaction/completeTransaction` — nó rollback do cờ fail sticky). Verify persistence bằng connection KHÁC (MCP/CLI mới), KHÔNG tin dòng "Migration success" của runner. Với settings menu: KHÔNG dùng `createSystemConfig()` mù — nó lấy fieldid = `MAX(id)+1 FROM vtiger_settings_field_seq`, seq này drift sau `MAX(fieldid)` thật → INSERT trùng PK; tự tính `MAX(fieldid)+1` từ bảng thật rồi resync seq.

**Trigger keywords (review):** migration `up()` có INSERT/UPDATE/DELETE mà không `SET autocommit` · `createSystemConfig(` · verify migration chỉ bằng log "Migration success" (không query lại DB)

---

## PEND-vtiger-read-layer-double-encode: toSafeHTML lần 2 trên giá trị đã bị read-layer encode → double-escape

**Ticket / MR:** #23494
**Date:** 2026-08-28
**Module / Layer:** Bất kỳ Settings/list Record model nào tự `getDisplayValue()` rồi output ra TPL

**Symptom:** Cell hiển thị `&lt;img&gt;` / `Tom &amp; Jerry` (entity lộ ra màn hình) thay vì `<img>` text sạch. DB lưu RAW (`SELECT ... HEX` = `3C`=`<`) nhưng giá trị tới record model đã là `&lt;img&gt;`.

**Root cause:** PearDatabase read-path (query_result/fetch → Base_Model) tự HTML-encode string columns lúc đọc. Nếu `getDisplayValue()` gọi thêm `Vtiger_Util_Helper::toSafeHTML()` → encode lần 2 (`&amp;lt;`). Suy luận "DB lưu raw nên phải tự encode" là SAI vì bỏ qua read-layer.

**Rule:** Trong `getDisplayValue()` của list/Settings model: hoặc trả thẳng giá trị (đã được read-layer encode — pattern LoginHistory), hoặc chuẩn hoá `toSafeHTML(html_entity_decode($value))` để encode ĐÚNG 1 lần (an toàn + không double-escape). Verify bằng browser: `td.textContent` phải ra chuỗi payload literal + `td.children.length === 0` (0 node inject).

**Trigger keywords (review):** `getDisplayValue` trả `Vtiger_Util_Helper::toSafeHTML($value)` không kèm `html_entity_decode` · giả định "DB lưu raw nên phải encode" mà chưa trace giá trị thật ở model read

---

## PEND-18172: Copy-paste permission block re-queries users already fetched in the same iteration
**Ticket / MR:** #18172 / !393
**Date:** 2026-09-09
**Module / Layer:** Sharing recalc — `modules/Users/CreateUserPrivilegeFile.php` `getUserModuleSharingObjects()`
**Symptom:** `recalculateSharingRules` chậm khi có nhiều advanced sharing rule; mỗi role/group share sinh thừa 1+ query lấy user (role) hoặc re-expand đệ quy group/subgroup.
**Root cause:** Block `delete` (permission=2) được copy từ block `write` nhưng gọi lại `getRoleUserIds($share_roleid)` / `new GetGroupUsers()->getAllUsersInGroup($share_grpid)` — chính data mà block write vừa fetch trong cùng vòng lặp (write luôn chạy trước vì delete⊂write, permission>=1).
**Rule:** Khi thêm block quyền mới cạnh block đã fetch user/group cùng key trong cùng iteration, REUSE kết quả (`$role_write_per[$share_roleid]`, hoặc object `$focusGrpUsers` khi group vừa expand — guard cờ freshness cho case key đã tồn tại từ iteration trước) thay vì query lại. Verify golden-master: regenerate file quyền OLD vs NEW phải md5 giống hệt.
**Trigger keywords (review):** `_delete_per[$share_roleid]=getRoleUserIds($share_roleid)` · `getAllUsersInGroup` trong block delete ngay sau block write cùng `$share_grpid`

---

## PEND-25746: Thêm key mới vào JSON blob dùng chung mà không enumerate MỌI writer
**Ticket / MR:** #25746
**Date:** 2026-09-10
**Module / Layer:** Reports — `vtiger_reporttype.data` (2 writer: `actions/ChartSave.php` + `views/ChartSaveAjax.php`); tổng quát: mọi JSON/serialized blob được rebuild-toàn-bộ khi save
**Symptom:** Setting mới (vd `colorscale`) lưu OK qua flow chính, nhưng bị reset ngầm về default khi user save qua flow phụ (ChartDetail re-save) — vì writer phụ rebuild blob từ request và không biết key mới.
**Root cause:** JSON blob được `Zend_Json::encode(array(...))` lại TOÀN BỘ ở mỗi writer. Plan chỉ tìm thấy 1 writer (grep theo action chính); writer thứ 2 (`ChartSaveAjax`) chỉ lộ ra khi grep `reporttype.*chart` lúc E2E. Nếu không patch, key mới sống sót lint + smoke + E2E flow chính — chỉ chết ở flow phụ.
**Rule:** Trước khi thêm key vào blob dùng chung, grep TẤT CẢ nơi encode/rebuild blob đó (vd `grep -rn "reporttypedata"`) và patch từng writer: writer có UI gửi key → whitelist + lưu; writer không có UI → đọc giá trị đã lưu và preserve. Verify bằng cách save qua TỪNG flow rồi query lại DB.
**Trigger keywords (review):** `Zend_Json::encode(array(` rebuild blob trong save path · thêm key vào `reporttypedata`/blob config mà diff chỉ đụng 1 writer

---

## PEND-25746b: So sánh cột DATE/DATETIME với chuỗi rỗng `= ''` — MySQL 8+ error 1525, pquery fail im lặng
**Ticket / MR:** #25746
**Date:** 2026-09-11
**Module / Layer:** `include/QueryGenerator/QueryGenerator.php` operator `empty`/`y`/`ny`; tổng quát: mọi SQL so sánh cột DATE với `''`
**Symptom:** Filter "trống"/"không trống" trên cột date ở list view (hoặc search_params `empty`) trả 0 kết quả dù DB có row `IS NULL`. Không có error hiển thị — pquery trả false im lặng (EP pquery-silent-false), list rỗng như thể "không có dữ liệu".
**Root cause:** SQL sinh ra `col IS NULL OR col = ''` / `col != ''`. MySQL 8+ (mọi sql_mode) reject so sánh DATE với chuỗi rỗng: `ERROR 1525 Incorrect DATE value: ''`. Query fail toàn bộ → 0 rows. Pattern viết cho MySQL 5.x (nơi `''` so sánh được) sống sót đến khi upgrade DB.
**Rule:** Cột DATE/DATETIME: check trống chỉ dùng `IS NULL` (không `OR = ''`), check có giá trị chỉ `IS NOT NULL`. Khi filter date trả 0 kết quả bất thường → chạy thẳng câu SQL trên mysql CLI để lộ error 1525 (đừng tin count=0). Đã fix tại QueryGenerator dòng ~1143 (gate theo `getFieldDataType()`).
**Trigger keywords (review):** `= ''` hoặc `!= ''` trên cột date/datetime trong SQL/QueryGenerator · `IS NULL OR %s = ''` cho field date

---

## PEND-26349: "Stale" check dựa trên cột thời gian mà đường claim không ghi — đóng nhầm task consumer khác đang chạy
**Ticket / MR:** #26349 / !423
**Date:** 2026-09-30
**Module / Layer:** `modules/CPQueue/models/Server.php::completeProcessingTask`; tổng quát: mọi cleanup "treo > N phút" trên queue/lock/job table
**Symptom:** Nhiều consumer cùng queue: task backlog (tạo > 1 giờ trước) bị consumer khác đánh DONE ngay khi đang chạy; progress session sai, task có thể bị purge sớm.
**Root cause:** Nhánh stale dùng `COALESCE(start_date, create_date) < NOW() - INTERVAL 1 HOUR`, nhưng vendor claim (`SqlPdoDriver::select_task`) chỉ UPDATE `status`, không ghi `start_date` (TaskPerformer chỉ set trong bộ nhớ, `modify_task` lưu sau khi job xong). Task đang chạy luôn có `start_date` NULL → fallback `create_date` = tuổi backlog, không phải thời gian chạy.
**Rule:** Cột dùng để kết luận "run đã chết" phải được ghi NGAY lúc claim (cùng connection với claim); điều kiện stale chỉ dựa trên cột đó, không fallback sang `create_date`/cột không phản ánh thời điểm bắt đầu chạy. Trước khi viết cleanup, đọc code claim của vendor để xác nhận cột thật sự được ghi khi nào.
**Trigger keywords (review):** `COALESCE(start_date, create_date)` · `INTERVAL 1 HOUR` trong UPDATE status queue · `completeProcessingTask`

---

## PEND-26349b: Vòng `while ($row = $adb->fetch_array($result))` không chặn pquery trả false — lỗi DB thành fatal
**Ticket / MR:** #26349 / !423
**Date:** 2026-09-30
**Module / Layer:** `vtlib/Vtiger/Block.php::getModuleBlockRows`, `vtlib/Vtiger/Field.php::getModuleFieldRows`; tổng quát: mọi chỗ đổi vòng `num_rows()`/`query_result` sang `while (fetch_array)`
**Symptom:** Deadlock / lock-wait timeout / "gone away" khi đọc metadata làm trang chết vì fatal error (`FetchRow()` on bool). Code cũ trả rỗng và trang vẫn chạy.
**Root cause:** `dieOnError` mặc định là false, nên `pquery` trả `false` khi lỗi. `num_rows(false)` xử lý được giá trị này, nhưng `fetch_array(false)` gọi `$result->FetchRow()` trên bool, nên PHP 7.4 báo Error. Code refactor sang `while (fetch_array)` bỏ mất lớp bảo vệ ngầm đó. Nếu kết quả còn được cache, lỗi tạm thời có thể bị lưu thành "rỗng".
**Rule:** Sau `pquery` mà kết quả đi thẳng vào `fetch_array`/`fetchByAssoc` trong `while`, phải có `if ($result === false) return …;`. Không ghi cache khi query lỗi.
**Trigger keywords (review):** `while ($row = $adb->fetch_array($result))` · `while ($row = $db->fetchByAssoc($result))` ngay sau `pquery(` không có `=== false`

---

## PEND-26349c: Danh sách id từ request nối thẳng vào SQL rồi `pquery($sql, [])`
**Ticket / MR:** #26349 / !424
**Date:** 2026-10-01
**Module / Layer:** `include/utils/CommonUtils.php::countActivitiesByRecord/countActivitiesByAccount`, `Accounts_Module_Model::getCalendarActivities`; nguồn: `CloudGoAppApiHandler::countActivities` (`Data.ids`)
**Symptom:** User app đã đăng nhập gửi `Data.ids = ["1998 OR SLEEP(0.3)"]` → query Accounts chạy 1,4 s (time-based SQLi); payload `"2377) OR SLEEP(0.3) OR (1"` ở Contacts/Leads trả số activity của mọi record khác.
**Root cause:** Hàm helper dùng chung nhận mảng id "tin là số" rồi `implode(',', $ids)` / `= $id` vào SQL và gọi `pquery($sql, [])`. Ép `(int)` ở một nhánh (UNION mới) nhưng nhánh WHERE cũ vẫn dùng biến thô; caller API không ép kiểu.
**Rule:** Hàm nhận id từ ngoài phải lọc ngay đầu hàm (`filter_var(..., FILTER_VALIDATE_INT, min_range 1)`, bỏ phần tử không hợp lệ) và bind mọi id bằng `generateQuestionMarks($ids)` + params — kể cả id lấy từ DB. Khi sửa một nhánh của query, rà cả các nhánh khác dùng cùng biến.
**Trigger keywords (review):** `IN (". implode(",",` · `implode(',', $entityIds)` · `pquery($sql, [])` · `pquery($query, [])` · `= $id ` / `= $recordId ` trong chuỗi SQL

---

## PEND-26368: Cache Redis output đã dịch nhưng key thiếu ngôn ngữ

**Ticket / MR:** #26368
**Date:** 2026-10-01
**Module / Layer:** Settings/MenuEditor (`getDisplayStructure`) · `includes/runtime/cache/Connectors.php`

**Symptom:** User đổi ngôn ngữ sang tiếng Anh, menu trái vẫn hiện tiếng Việt (hoặc ngược lại) cho tới khi admin lưu MenuEditor / Redis hết TTL.

**Root cause:** Menu đã dịch (`name_vn`/`name_en`, `vtranslate(..., $lang)`) được cache với key `user_data:{$userName}:menu_structure` — chỉ có user, không có `$current_user->language`. Ngôn ngữ nào warm cache trước thì mọi request sau dùng lại.

**Rule:** Cache chứa chuỗi đã dịch/đã chọn theo ngôn ngữ thì key PHẢI có ngôn ngữ (`…:{$language}:…`), hoặc cache dữ liệu thô rồi dịch lúc render. Review mọi `set*`/`hset` Redis mà value được build từ `vtranslate` / `name_vn` / `name_en` / `$current_user->language`.

**Trigger keywords (review):** `user_data:{$userName}:menu_structure` · `$current_user->language == 'vn_vn'` · `setMenuStructure(`

---

## PEND-24840: Init plugin delegated theo selector trên container từ payload event — container rỗng → bind selector chết

**Ticket / MR:** #24840
**Date:** 2026-10-01
**Module / Layer:** `layouts/v7/modules/Vtiger/resources/Detail.js::registerEventsForRelatedList` (handler `post.relatedListLoad.click`) · Swipebox v1.4.4

**Symptom:** Từ tab Chi tiết chuyển AJAX sang tab Bình luận, click ảnh đính kèm comment không mở xem nhanh; F5 ngay tại tab Bình luận thì chạy.

**Root cause:** Event `post.relatedListLoad.click` truyền `jQuery('.relatedContainer').find('.searchRow')` — tab ModComments không có `.searchRow`. `container.find('.comment-image-swipebox').initSwipbox()` không lỗi, nhưng swipebox bind `$(document).on('click', jq.selector)` với selector `.relatedContainer .searchRow .comment-image-swipebox` (không bao giờ match), còn handler `preventDefault` delegated vẫn nuốt click.

**Rule:** Không dựa vào container của payload event dùng chung (`post.relatedListLoad.click`) để tìm phần tử của một tab cụ thể — payload phụ thuộc trigger (`.searchRow`, `container`, ...). Dùng holder ổn định (`this.getContentHolder()`) hoặc selector toàn cục. Plugin bind delegated theo `.selector` (swipebox) phải được kiểm tra selector thực bind: `jQuery._data(document,'events').click.map(h => h.selector)`.

**Trigger keywords (review):** `container.find('.comment-image-swipebox')` · `app.event.on("post.relatedListLoad.click",function(event, container)` · `container.find(".searchRow")` · `.initSwipbox()`

## PEND-26411: pjax handler on document settles the wrong (newer) request
**Ticket / MR:** #26411 / !427, !428
**Date:** 2026-10-01
**Module / Layer:** `layouts/v7/resources/application.js::_requestPjax` · jquery.pjax (defunkt) · jQuery 1.11

**Symptom:** List: page 2 then page 3 quickly shows an "Error" toast and keeps the old rows. Detail/Project: tab A then tab B while A loads marks B selected but B's content never renders.

**Root cause:** `_requestPjax` bound `pjax:success`/`pjax:error` on `document` per call, without filtering by its own xhr and without unbinding. `jQuery.pjax(B)` aborts the in-flight A synchronously, A's `pjax:error` (abort) resolved B's deferred with `VtError`, and B's real response came after B was already resolved. The bug stayed hidden while the full-page `showProgress()` overlay blocked a second click.

**Rule:** A global event (document-level `pjax:*`, `ajaxComplete`, etc.) used to settle one request's deferred must check that the event belongs to that request (`jqXHR === ownXhr`) and must unbind once settled. Removing a blocking overlay means auditing every loader underneath it for overlapping requests.

**Trigger keywords (review):** `jQuery(document).on('pjax:success'` · `jQuery(document).on('pjax:error'` · `jQuery.pjax(params);` · `app.helper.showContentLoading(`

## PEND-26809: AJAX submit handler without in-flight guard — Enter re-submits under the loading overlay
**Ticket:** #26809
**Date:** 2026-10-18
**Module / Layer:** `layouts/v7/modules/Settings/SharingAccess/resources/SharingAccess.js::saveCustomRule` · `app.helper.showProgress`

**Symptom:** Click Lưu in the "Thêm quyền" popup, press Enter 3 times while loading → 4 identical sharing rules + 4 success toasts. Double click is fine.

**Root cause:** `form.on('submit')` calls the AJAX save with `e.preventDefault()` but no "saving" flag and no disabled submit button. `showProgress()` overlay blocks the mouse only; focus stays in the form, so every Enter is a new implicit submission → new POST → new INSERT (create endpoint, no record id).

**Rule:** Every modal/form AJAX save must set an in-flight flag (e.g. `form.data('isSaving')`) and disable `[name="saveButton"]` before the request, and release both on the error path. Never rely on the loading overlay to block re-submits.

**Trigger keywords (review):** `form.on('submit', function(e) {` · `e.preventDefault();` · `app.helper.showProgress();` · `app.request.post({'data' : data}).then(`

## PEND-26035: server error page loaded into a static-backdrop overlay freezes the UI
**Ticket:** #26035
**Date:** 2026-10-19
**Module / Layer:** `layouts/v7/modules/Vtiger/resources/Vtiger.js::_showQuickPreviewForId` · `app.helper.loadHelpPageOverlay` · `OperationNotPermitted.tpl` · `Vtiger_RecordQuickPreview_View::process`

**Symptom:** Clicking a link to a deleted record opens the quick-preview side panel with no close button; clicking outside empties the panel but the grey backdrop stays — only F5 recovers.

**Root cause:** An exception thrown inside an AJAX view's `process()` (here `getInstanceById` → `LBL_RECORD_DELETE`) is rendered by the encoded WebUI as the full-page `OperationNotPermitted.tpl` with HTTP 200, so `app.request` reports success (`err` null). The caller loads it into an overlay opened with `backdrop: 'static', keyboard: false`; the error page has no close button and its inline script calls `app.helper.hideModal()`, which closes `.myModal`, not the overlay.

**Rule:** An AJAX view whose HTML goes into a modal/overlay must not rely on exceptions for expected states (deleted / not found) — render a dedicated closable template instead. Callers loading HTML into `loadHelpPageOverlay` / `showModal` with a static backdrop must treat an `.app-exception-container` response as an error (notification, no overlay).

**Trigger keywords (review):** `app.helper.loadHelpPageOverlay(response` · `app.request.get({data: params}).then(function (err, response) {` · `Vtiger_DetailView_Model::getInstance($moduleName, $recordId)` · `backdrop: 'static'`

## PEND-26881: module CSS redefines a global utility class (`.hide`) and overrides core `.hide` + `.show` toggles
**Ticket:** #26881
**Date:** 2026-10-19
**Module / Layer:** `modules/CPAICopilot/resources/CopilotPanel.css` (loaded from `Footer.tpl` via `CopilotPanel.tpl`) · `layouts/v7/modules/Vtiger/resources/List.js::showSelectAllMsgDiv`

**Symptom:** List view: ticking the header checkbox no longer shows "Đã chọn Tất cả của trang…, Click vào đây để chọn tất cả (N)"; the element has class `show` and text, but computed `display:none`. Only when Copilot is mounted (enabled, or admin).

**Root cause:** Core shows elements by adding `show` while `hide` stays (`div.hide.messageContainer.show`). It works only because `app.css` `.show{display:block!important}` is declared after the header bootstrap `.hide{...!important}`. A widget stylesheet loaded later (Footer) redeclared unscoped `.hide{display:none!important}` and won the cascade on every page.

**Rule:** Module/widget CSS must never redeclare global utility classes (`.hide`, `.show`, `.hidden`, `.d-none`…) unscoped. Scope them under the widget root (`#copilot-panel .hide`) or use a module-prefixed class. Footer-loaded CSS wins every tie with header CSS.

**Trigger keywords (review):** `.hide {` · `display: none !important;` · `{include file="modules/` in `Footer.tpl` · `<link type="text/css" rel="stylesheet" href="{vresource_url('modules/`

## PEND-26651: write button hidden in TPL but the write endpoint does not check the right; a "preview" button reuses the save mode
**Ticket:** #26651
**Date:** 2026-10-19
**Module / Layer:** `modules/Reports/views/SaveAjax.php` / `ChartSaveAjax.php::checkPermission` · `layouts/v7/modules/Reports/resources/Detail.js::registerSaveOrGenerateReportEvent`

**Symptom:** On a custom-handler report, "Tạo báo cáo ngay" overwrote the saved filter conditions for everyone, even for a user who had no Save button.

**Root cause:** The JS forced `mode = 'save'` for every button so the page could reload and let the handler render, and the server only checked module access. The TPL hid the Save button with `isEditableBySharing()`, but no controller enforced it.

**Rule:** Any UI gate on a write action (`{if $X->isEditableBySharing()}`, `isEditable`) needs the same check in the controller's `checkPermission()` for that mode, and it must reject a non-existent record first (`isEditableBySharing()` returns true for an empty id). Never turn a read/preview action into a write just to reuse a render path; pass the temporary state in the request.

**Trigger keywords (review):** `currentMode = 'save'` · `if ($mode === 'save')` · `{if $REPORT_MODEL->isEditableBySharing()}` · `saveAdvancedFilters()` · `checkPermission` without `isEditableBySharing`

## PEND-26929-B: "Dead comment" predicate checks is_deleted but not post_deleted
**Ticket / MR:** #26929 / !443
**Date:** 2026-10-20
**Module / Layer:** domains/Social Reply (CommentRemovalCloseService)
**Symptom:** After a post removal, removing the customer's last live comment left the conversation stuck in "Chưa phản hồi" when its last message was a comment on an earlier-removed post.
**Root cause:** A comment is gone when `is_deleted` OR `post_deleted` (spec §4.9, `liveUnrepliedCommenters`); the close predicate fetched `post_deleted` from `findMany` but only tested `is_deleted` / the post being removed now.
**Rule:** Any check for "comment no longer live" must test both `is_deleted` and `post_deleted` (plus `is_hidden` where hidden counts); a field read into the shape but unused in the predicate is a red flag.
**Trigger keywords (review):** `['is_deleted'] ||` · `$comment['is_deleted']` · `findMany(`

## PEND-27248: `count($x ?? [])` on a variable that can hold a plain string
**Ticket:** #27248
**Date:** 2026-10-29
**Module / Layer:** `modules/Import/actions/Data.php::transformForImport` (reference fields); any `count($x ?? [])` left by the php7Count retirement (11200e8861)
**Symptom:** On PHP 8.3, importing Leads with a reference column holding a plain label (no `Module::::` prefix) crashed the import (500, `count(): Argument #1 must be of type Countable|array, string given`); PHP 7.4 imported it.
**Root cause:** One branch kept the raw string (`$fieldValueDetails = $fieldValue`) while the others `explode()`; PHP 7 `count('label')` returned 1 and the label-lookup branch ran. `?? []` only replaces null, so the string reached `count()`.
**Rule:** When retiring php7Count, trace every assignment of the variable: if any branch leaves a scalar, make that branch produce an array (`[$value]`) at the assignment, not `?? []` at the use site. Same family as EP-018 (objects): `?? []` guards null only.
**Trigger keywords (review):** `$fieldValueDetails = $fieldValue;` · `count($fieldValueDetails` · `?? []) > 1` · `?? []) == 2`

## PEND-27223: `:hover` rule that changes an inline item's size makes it wrap away from the pointer and flicker
**Ticket:** #27223
**Date:** 2026-10-29
**Module / Layer:** `layouts/v7/resources/custom.css` `.tag:hover .tag-actions-wrapper`; any wrapping chip/pill/inline list
**Symptom:** A tag at the end of a sidebar row blinks while hovered and needs several clicks.
**Root cause:** Hover expanded the tag (`max-width: 0` → `40px`), so it wrapped to the next row and left the pointer. Losing `:hover` shrank it back under the pointer, and the loop repeated about every 4 frames.
**Rule:** Hover/focus state of an item inside a wrapping (inline/flex-wrap) container must not change its box size: reveal extras with opacity/visibility on an absolutely positioned overlay, or reserve the space at rest.
**Trigger keywords (review):** `:hover .tag-actions-wrapper` · `max-width: 0;` · `transition: max-width` · `:hover` + `max-width:`

## PEND-26296: SSE relay checks the upstream `error` flag on only one termination branch
**Ticket / MR:** #26296 / !420
**Date:** 2026-10-29
**Module / Layer:** `modules/CPAICopilot/helpers/Logic.php::streamMessage` + `Sse.php::stream`; any server that relays an upstream SSE stream and then emits its own terminal frame
**Symptom:** Backend sends `token… → error → close`; the client then also gets `done`, the error bubble is overwritten by the partial text, and the partial is stored as a normal reply. With `error` and no text, the client gets a second, generic error.
**Root cause:** `$sawError` was evaluated only inside the cancel/cURL branch (`$endedMidStream`), so a clean close after an error frame took the success path; the transport then mapped success to `done` and every failure to an extra `error`.
**Rule:** Decide the stream outcome in one place and check "upstream already sent error" first, before any success/partial-reply branch; the transport must not emit a second terminal frame (`done` or generic `error`) after a relayed upstream error.
**Trigger keywords (review):** `$sawError` · `&& !$sawError` · `$emit(['type' => 'done'])` · `$endedMidStream`

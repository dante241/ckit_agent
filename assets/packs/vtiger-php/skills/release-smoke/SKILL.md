---
name: release-smoke
description: "Test toàn bộ branch release (release-core-MM.YYYY / pick/*) trên PHP 8.3 ở máy local trước khi push/merge vào release: dựng site + DB clone, migration + QuickRepair, PHPStan so master, crawl 6.480 URL, ghi record, export/queue, API, gom lỗi log."
---

# release-smoke — test branch release trên PHP 8.3 ở local

Chạy cùng bộ kiểm đã dùng để test master lên PHP 8.3, nhưng trên **code của branch release**, rồi đọc log PHP/nginx và ghi mọi lỗi vào một file. Dùng sau `/release-bundle` (mọi `pick/*` đã merge vào branch tháng, `verify-resolves`/`order-check`/`audit-month` đã đọc) và trước khi push / tạo MR vào `release`.

```bash
S="$HOME/.config/8sync/packs/vtiger-php/skills/release-smoke/scripts"   # chạy từ checkout vtiger
bash $S/run.sh release-core-10.2026                    # toàn bộ, ~10 phút
bash $S/run.sh release-core-10.2026 --keep-db          # giữ DB clone đang có (không clone + migration lại)
bash $S/run.sh release-core-10.2026 --skip phpstan,api # bỏ bước
```

Kết quả: `agents/release-queue/smoke-<ngày>-<sha>/report.md` (lần chạy có `--skip`/`--keep-db` ghi `report-partial-<giờ>.md`, không đè report đầy đủ) (lỗi PHP/nginx gom theo nhóm + URL gây lỗi + tóm tắt từng bước) và `*.json` từng bước. Site để bấm tay: `http://vtiger74.localhost:8085`, `admin` / `Relwalk#2026` (mật khẩu chỉ đặt trong DB clone).

## Nguồn DB: `local` (mặc định) hay `beta` (khuyên dùng trước khi merge)

| | `local` | `SMOKE_SOURCE=beta` |
|---|---|---|
| DB | clone DB dev master (`vtiger` → `vtiger_relsmoke`, MySQL local) | dump `rnd_beta` của beta.cloudpro.vn trên vm02 (host `vm02` trong `.omp/ssh.json`, chỉ đọc: `mariadb-dump --single-transaction`), nạp NGUYÊN VĂN vào container `mariadb:10.5.25` (`relsmoke-mariadb`, cổng 3307) cùng `sql_mode`/charset/`innodb_strict_mode` đọc từ server |
| Schema | schema master: migration master đã ghi → bỏ qua; `ADD COLUMN` trần báo trùng cột | schema release thật: migration của đợt chạy như trên production |
| Dữ liệu | dữ liệu dev, ít cấu hình (không có tax region…) | dữ liệu kiểu tenant: lỗi phụ thuộc cấu hình (tax region, charge, webform…) lộ ra |
| Kết quả | `smoke-<ngày>-<sha>/` | `smoke-<ngày>-<sha>-beta/` |

```bash
SMOKE_SOURCE=beta bash $S/run.sh release-core-10.2026          # dump được giữ trong .cache/beta/ cả ngày
SMOKE_SOURCE=beta bash $S/beta.sh --refetch                     # lấy dump mới
```

- `db.sh` chặn gửi ra ngoài trên DB clone (cả `local` lẫn `beta`): SMTP trong `vtiger_systems` → `127.0.0.1:1025`, tắt workflow có task email/SMS/Zalo/ZNS/webhook/notification. Worker CPQueue (gửi FCM, sync) không chạy local. Test tay qua UI trên DB beta chỉ sau bước này.
- Docker Desktop phải chạy (script tự `open -a Docker`). Container có `--restart unless-stopped`; Docker Desktop từng tự dừng giữa chừng → `mysqli_query(): Argument #1 ... false given` hàng loạt = DB chết, KHÔNG phải lỗi code: `docker start relsmoke-mariadb` rồi chạy lại bước đó (`--keep-db --skip …`).
- MySQL 9 local không nạp được dump MariaDB (dòng sandbox, `DEFAULT curdate()`, TEXT có default) và client MySQL 9 không đăng nhập được MariaDB (`mysql_native_password`) → mọi lệnh SQL đi qua `docker exec … mysql` (biến `MYSQL_CMD`). KHÔNG sửa dump bằng sed để nạp vào MySQL: schema sẽ khác server.
- Cổng nằm trong `db_server` (`127.0.0.1:3307`, `db_port` rỗng): `CPQueue_Server_Model` dựng PDO DSN chỉ từ `db_server`, tách cổng ra thì PDO nối nhầm MySQL local (`Access denied for user 'root'@'localhost'` khi lưu record).
- URL crawl lấy từ demo master → record id của beta khác: nhiều trang "record không có" là bình thường; walk tự tìm record trong DB.

## Các bước (`run.sh`)

| Bước | Script | Làm gì | Đạt khi |
|---|---|---|---|
| setup | `setup.sh <ref>` | Worktree detached `../vtiger-relsmoke` tại `<ref>`; copy `config.env.php` (đổi `root_directory`, `db_name`), `license.lic`, `user_privileges/*`; Redis cache db 5; `skip-worktree` 2 file config; vhost nginx `:8085` + snippet `php8.3-fpm-relsmoke.conf` | Trang login 200 |
| db | `db.sh` | Clone DB `vtiger` → `vtiger_relsmoke`; tắt event handler trỏ file release không có, ẩn module release không có; đặt mật khẩu admin; `CPMigration up` (đếm dòng `vtiger_relatedlists`/`vtiger_field` bị xoá); QuickRepair qua `quick_repair.php` (in dòng đồng bộ kiểu cột) | Danh sách `--- migration đã chạy` (đa số chạy TRONG QuickRepair, không phải `CPMigration up`) không có `FAIL`; QuickRepair `OK`; đọc list relatedlist/field bị xoá |
| phpstan | `phpstan.sh` | `release-bundle/phpstan-check.sh origin/release..<ref>` rồi phân lỗi mới: y hệt `origin/master` / chỉ release có / gọi thiếu tham số / **có cả trên master nhưng gọi hàm-method-hằng không tồn tại** (baseline master giấu, vd `getAddressByLongitude()`). Smoke chính `release` sau khi đã merge đợt: `<ref>` = `origin/release` thì vùng so rỗng ("bỏ qua") → chạy lại `SMOKE_PHPSTAN_BASE=<commit release trước đợt> bash $S/phpstan.sh origin/release <OUT tuyệt đối>` | Đọc hết 3 nhóm sau; hàm có thể nằm trong file encode ionCube (`include/utils/utils.php`) — kiểm trước khi kết luận |
| crawl | `crawl.py` | `data/pages.txt` 284 trang (session seed, không XHR; record id DB clone không có → thay bằng record sống mới nhất của module đó) + `data/crawl-urls.json` 6.480 URL của crawl demo master (đăng nhập thật, ajax giữ XHR) | 0 lỗi mới so với demo, hoặc lỗi giải thích được |
| walk | `walk.py` | Mở Detail mọi report; tạo/sửa/xoá Accounts→Contacts→HelpDesk, modal tình trạng, sửa nhanh, bình luận; Settings MailConverter/Webforms; 4 case ghi của gate master (SponsorshipCategory, CPRequest, CPLeaving, CPReportTask — SKIP nếu release không có module) | 0 FAIL |
| export | `export.py` | Export Accounts trực tiếp; hạ ngưỡng queue trên clone → 2 lần gửi chỉ 1 dòng NEW → chạy cron `ProcessExportQueue` (tạm bỏ `run_by=Supervisor`) → Document xlsx đủ dòng; Report GetXLS | Số dòng xlsx = số record + 1 |
| api | `api.py` | Login `WebsiteApi` (200 đúng mật khẩu / 401 sai); gọi mọi action ĐỌC của `api/*` không tham số | Lỗi = do thiếu tham số / dữ liệu, code giống master |
| report | `report_log.py` | Gom dòng Fatal của **riêng worktree smoke** trong `php83-vtiger-error.log` + `vtiger-release-error.log` từ lúc bắt đầu chạy | Mọi nhóm có dòng "Phân loại" |

## Đọc kết quả — lỗi đã gặp KHÔNG phải lỗi release (đợt 10.2026)

Ghi vào report mục "Phân loại" từng nhóm: lỗi release / lỗi cũ (có cả trên `origin/release` hoặc master) / do môi trường. Đối chiếu nhanh:

| Hiện tượng | Nguyên nhân | Cách nhận ra |
|---|---|---|
| `db.txt`: `Migration failed ... ALTER TABLE ... ADD COLUMN` (Duplicate column) | DB clone lấy từ DB dev master, cột đã có sẵn; migration dùng `ADD COLUMN` trần thay vì `addColumnIfMissing` | `information_schema.columns` của DB nguồn có cột đó + file migration đã có trên `origin/release` (không phải migration của đợt). Clone master KHÔNG chứng minh migration chạy được trên DB production: migration master đã ghi `vtiger_migration_history` thì clone bỏ qua → phải chạy thử trên server/DB clone từ production |
| `Sorry! Attempt to access restricted file - modules/<X>/<X>.php` | DB clone (từ master) còn quan hệ/report tới module release không có | Script tự xếp "DB clone" CHỈ khi file cũng không có trên `origin/release` (hoặc `modules//.php` mà record không có / đã xoá trong clone). File `origin/release` có mà branch mất → vẫn tính lỗi |
| Lưu record 500 `Failed opening required modules/<X>/handlers/...` | Handler của module release không có — `db.sh` đã tắt; gặp trên DB clone khác (vd `release.localhost`) thì tắt tương tự | Như trên |
| `Incorrect DATE value: ''` (1525), alias `rank` (1064) | MySQL 8/9 local khắt khe hơn MariaDB server | `SELECT VERSION()` local; câu SQL giống master |
| Fatal `... on null/false` khi gọi kèm XHR (`preProcess` bị bỏ) | Harness gửi `X-Requested-With` cho full page | Gọi lại không XHR → 200 |
| Record id không tồn tại (Webforms 6, CPTodoList 1523771…) | URL lấy từ DB demo | `SELECT` record trong `vtiger_relsmoke` |
| API "Record not found" / `FROM vtiger_customview AND` / `LIMIT , 20` | Gọi action không truyền tham số | File handler y hệt master |
| PHPStan "chỉ release có": `SocketIO::setProtocole/emit/SSL_PROTOCOLE`, `NotificationAPI::getInstance()` | Trùng tên class: release có 2 `class SocketIO` (`libraries/SocketIO/SocketIO.php` có đủ method, `socket.io.php` thì không) và 2 `NotificationAPI` (v1 không có `getInstance`, v2 có); PHPStan chọn nhầm bản | Xem `require_once` ở caller (PBXManager/BaseConnector, Chatbox, CPAICameraIntegration → `SocketIO.php`; CloudWorkApi v2 → `v2/NotificationApi.php`) và method có trong file đó |
| `Vtiger_Record_Model::getAttachmentDetails()` không có (CPTicketCommunicationLog Data.php) | `getInstanceById(..., 'CPTicketCommunicationLog')` khai báo trả `Vtiger_Record_Model`, runtime trả lớp con có method | `CPTicketCommunicationLog_Record_Model::getAttachmentDetails()` tồn tại |
| `invalid return type imap_search` (MailConverter MailBox) | Docblock `@return imap_search` sai kiểu | Chỉ docblock |
| `DivisionByZeroError`, `array_key_exists(): ... null given`, `... must be of type array` | **Lỗi thật chỉ có trên PHP 8** (PHP 7 chỉ cảnh báo) — release đợt này lên 8.3 nên phải sửa | Sửa trên master qua MR (ticket nâng runtime), pick `-x -m 1` vào branch tháng, chạy lại smoke |

Lỗi chỉ có trên PHP 8 mà code giống master **vẫn là lỗi của đợt release** khi đợt đó chuyển production lên 8.3: sửa trên master rồi pick (đợt 10.2026: MR !484 — chia tổng 0 trong `Chart.php`, `getSelectedFieldsList()` trả null, `array + null` trong Webforms).

## Không làm được ở local (ghi rõ trong báo cáo cho người test server)

- Migration mà DB master đã chạy (đánh dấu trong `vtiger_migration_history`) không chạy lại trên clone; migration xoá dữ liệu xoá gì trên production chưa biết → dump DB đầy đủ trước QuickRepair trên server.
- `ext-imap` (MailScanner, MailManager), Supervisor (`ProcessExportQueue` có `run_by=Supervisor`), gửi mail/SMS/Zalo, webhook ngoài.
- Module bị license local chặn (~90 trang "Không có quyền" trong crawl 284) — không phải trang đã kiểm.

## Dọn sau release

```bash
git -C <vtiger> worktree remove --force ../vtiger-relsmoke
rm /opt/homebrew/etc/nginx/domains/vtiger-release-local.conf /opt/homebrew/etc/nginx/snippets/php8.3-fpm-relsmoke.conf && nginx -s reload
mysql -e 'DROP DATABASE vtiger_relsmoke'; redis-cli -n 5 flushdb
```

## Sai sót đã gặp

| Sai | Hậu quả | Làm đúng |
|---|---|---|
| Sửa `config.env.php` trong worktree tháng `vtiger_release` | File tracked → lọt vào commit release | Chỉ dùng worktree smoke riêng; config đánh `skip-worktree` |
| Dùng chung Redis db 0 với master | Cache key theo `SERVER_NAME` trùng → site release đọc metadata của DB master | Redis db riêng (`SMOKE_REDIS_DB`, mặc định 5) |
| Gửi XHR cho mọi GET | 500 giả ở Report Detail, MailConverter Edit | `Client.req(..., xhr=False)` cho trang đầy đủ |
| Đọc fatal trong log chung theo offset | Gán nhầm lỗi của site khác (8083) cho request đang chạy | `fatals_since()` chỉ lấy dòng có đường dẫn worktree smoke |
| Script test copy từ master trỏ DB `vtiger` / ROOT master | Kiểm và **xoá dữ liệu** trên DB dev của master | Mọi script đọc `WT`/`DB` từ `lib.sh`; `db.sh` dừng nếu config không trỏ DB clone |
| Trả lời "lỗi giống master nên không sao" | Lỗi chỉ có trên PHP 8 lọt production khi đợt nâng runtime | Phân loại theo PHP 7 vs 8 (bảng trên) |
| Gọi tay `phpstan.sh` với `<out_dir>` tương đối | `phpstan-check.sh` đổi thư mục → không ghi được `phpstan.json`, kết quả in "0 lỗi" giả | Luôn truyền đường dẫn tuyệt đối; kiểm `jq '.errors|length' <out>/phpstan.json` trước khi báo |
| Lỗi `mysqli_query(): ... false given` ×1 tại `/index.php` trong report | `setup.sh` gọi trang login trước khi `db.sh` dựng DB beta (`setup.txt`: login 500) | Bỏ qua nếu `setup.txt` có "DB … chưa có" |

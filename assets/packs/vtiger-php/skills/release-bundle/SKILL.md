---
name: release-bundle
disable-model-invocation: true
description: "Cherry-pick các ticket ready (từ /release-check) lên branch release-core-MM.YYYY tạo từ branch `release` của onlinecrm/vtiger (layout single — 1 repo), theo nhóm ticket (cả ticket hoặc không gì). Tự xử lý conflict ở file không phát hành, so sánh code master↔release, PHPStan kiểm tra thiếu function/method/class/sai số tham số. Ticket conflict: người release resolve theo hunk bằng resolve.sh (chỉ mang dòng MR đổi, T1/T2/T3), hoặc bàn giao dev (--handoff). Gắn `Đã release` + milestone. Không push branch release."
allowed-tools: Bash(bash .omp/skills/release-bundle/scripts/*), Bash(git *), Bash(jq*), Bash(cat *), Bash(glab *), Read, Write, Ask
---

# Release Bundle Skill

> Tiếp nối `/release-check`. Cherry-pick ticket ready lên `release-core-MM.YYYY` (tạo từ branch code đã phát hành). Giữ từng commit MR, không squash. Quy trình team: `docs/release-process.md`.

## Usage

```bash
S=.omp/skills/release-bundle/scripts; [ -d "$S" ] || S="$HOME/.omp/skills/release-bundle/scripts"   # project hoặc pack cài --global; luôn chạy từ repo vtiger

bash $S/run.sh --month 10.2026 --preview     # XEM TRƯỚC trên worktree tạm — không đổi branch, không đổi GitLab
bash $S/run.sh --month 10.2026               # chạy thật: pick → PHPStan → label/milestone → report
bash $S/run.sh --month 10.2026 --handoff     # + ticket conflict: push pick/<ticket> + MR assign tác giả
bash $S/run.sh skip 235,242                  # bỏ ticket của các MR này (vi: "bo qua MR 235")
bash $S/run.sh --no-gitlab                   # pick local, chưa đổi label (chạy ./gitlab-update.sh sau)
bash $S/run.sh --no-phpstan                  # bỏ bước PHPStan
bash $S/run.sh --pause-on-conflict           # legacy: dừng ở conflict, resolve tay rồi ./resume.sh
bash $S/resolve.sh                           # liệt kê ticket conflict theo thứ tự pick + trạng thái
bash $S/resolve.sh 26403                     # người release resolve ticket theo hunk trên pick/26403
bash $S/resolve.sh 26403 --check             # sau khi sửa tay trên pick/26403: PHPStan + report lại
bash $S/resolve.sh 26403 --merge             # merge --no-ff vào branch tháng + label Đã release
bash $S/verify-resolves.sh              # SAU khi mọi MR pick/* merge: từng pick trên branch tháng
                                        # so patch-vs-patch với commit master gốc (qua marker -x);
                                        # lệch = code mất khi resolve tay → exit 1, liệt kê dòng MẤT/THÊM
bash $S/audit-month.sh                  # rồi kiểm "mất code" cả branch theo từng file (3 chiều, mục dưới)
bash $S/order-check.sh                  # file có MR pick ngược thứ tự merge: dựng lại theo merged_at, so với branch
```

- `--month MM.YYYY`: tháng release (branch + milestone). **Luôn truyền khi release qua đầu tháng mới**; mặc định = tháng hiện tại.
- Classified: tự lấy file `classified-*.json` mới nhất — không cần chạy cùng ngày `/release-check` (cảnh báo nếu > 3 ngày).
- Flag lạ bắt đầu bằng `--` → abort (không bao giờ rơi thành chạy thật).
- Thử trên bản clone: `RELEASE_TARGET=/tmp/vr-test RELEASE_STATE_DIR=/tmp/state bash $S/run.sh ...` (xem `lib.sh`).

## Layout repo (`RELEASE_LAYOUT`, mặc định trong `lib.sh`)

| | `single` (mặc định từ 2026-11-02) | `split` (legacy 2 repo) |
|---|---|---|
| Code đã phát hành | `onlinecrm/vtiger` branch `release` | `onlinecrm/vtiger_release` master |
| Nơi pick | worktree `../vtiger_release` của repo vtiger (cùng thư mục cha với repo vtiger; checkout mirror onlinecrm/vtiger_release nằm ở `../vtiger_release_mirror`), tự tạo lần đầu | checkout `vtiger_release` (remote `vtiger`) |
| MR tháng / `pick/*` | vào repo vtiger, đích `release` / branch tháng | vào repo vtiger_release, đích `master` / branch tháng |
| Sau khi MR tháng merge | tag version trên `release`, `scripts/sync-release.sh <tag>` → 1 commit `Release core version ...` trên `vtiger_release` master (mirror) | — |

`sync-release.sh` chỉ fetch đúng ref cần (tag của vtiger_release có tên trùng tag vtiger), dừng khi mirror có thay đổi chưa commit / lệch origin / file chưa track trùng đường dẫn; đã giống tag thì không làm gì.

## Pipeline (`run.sh`)

| Phase | Script | Việc |
|---|---|---|
| 1 | `pick.sh` | Precondition, checkout branch, pick theo ticket (chi tiết `references/pick-logic.md`) |
| 2 | `phpstan-check.sh` | PHPStan trước/sau trên worktree tạm, chỉ lỗi thiếu function/method/class **mới**. Chạy lần lượt bản trước rồi bản sau, mỗi lần ≤ `RELEASE_PHPSTAN_PROCS` (mặc định 4) worker: hai lần song song trên ticket 1.400+ file đã làm treo máy 16 GB |
| 2b | `lint-runtime.sh` | Parse mọi file PHP đổi bằng PHP của server release: **8.3 từ đợt 10.2026** (#27245; mặc định `php:8.3-cli` qua docker, đổi bằng `RELEASE_PHP_LINT_IMAGE`, hoặc binary local `RELEASE_PHP_LINT_BIN`, vd `/opt/homebrew/opt/php@8.3/bin/php`, khi docker không chạy). Trước đó server 7.2/7.3 (typed property, `fn() =>`, `??=` là parse error) — branch cho server cũ đặt `RELEASE_PHP_LINT_IMAGE=php:7.2-cli`. Có trong report run và `resolve.sh --check`; `--merge` chặn khi còn lỗi (không `--force` được). Docker lỗi → `SKIPPED`, cần `--force` |
| 3 | `gitlab-update.sh` | Milestone, label `Đã release` / `Conflict cherry-pick` (`references/gitlab-update.md`) |
| 4 | `report.sh` | `bundle-report-<today>.md` + danh sách test trên release |
| Sau run | `resolve.sh <ticket>` | Người release resolve từng ticket conflict theo hunk (mục dưới) |
| Trước deploy test | `verify-resolves.sh`, `audit-month.sh`, `order-check.sh` | So patch từng pick; kiểm mất code cả branch; kiểm file có MR pick ngược thứ tự merge |

### Resolve theo hunk (`resolve.sh`, `resolve_hunks.py`)

Chỉ mang **dòng MR đổi** (`git diff <sha>^1 <sha>`) sang release. Làm từng ticket theo thứ tự `resolve.sh` liệt kê, trên `pick/<ticket>` (worktree `<cha>/vtiger-pick-<ticket>`, tạo từ branch tháng hoặc `--base <ref>`). MR pick sạch thì giữ nguyên. File conflict: lấy lại bản release rồi áp từng hunk của MR:

| Tầng | Điều kiện | Xử lý |
|---|---|---|
| T1 | Áp được với đủ ngữ cảnh **và** nằm dưới cùng hàm / phần tử mảng (`'field' => array(`) như trên master | Giữ |
| T2 | Chỉ áp được với `-C1`, cùng header `@@` **và** cùng hàm / phần tử mảng như trên master | Giữ, report liệt kê để đọc lại |
| T3 | Còn lại (không khớp; `-C1` rơi vào scope khác; khớp đủ ngữ cảnh nhưng ở hàm/phần tử khác — file lặp cấu trúc như `BlocksAndFieldsRegister.php`), file release không có, `composer.lock`/`vendor/`, file nhị phân, MR xoá file | Không áp; hunk lưu `.rej` + lý do (lệch ngữ cảnh / chỉ thêm mất chỗ neo / sửa code release chưa có / một phần / khớp ở chỗ khác) |

Mỗi MR luôn có commit mang marker `-x` (kể cả khi chỉ có T3): sửa tay T3 bằng `git commit --fixup <commit>` rồi autosquash. Sau đó PHPStan trước/sau trên `pick/<ticket>`, report gom lỗi theo symbol, chỉ ra symbol đã có trên branch (suy sai kiểu / autoload) hay commit master đã thêm nó (code thiếu cần mang sang — thuộc ticket chưa release hoặc không mang được thì hỏi). `--merge` chặn khi branch đổi sau lần `--check` cuối hoặc còn lỗi PHPStan mới (trừ `--force`). Report: `resolve-<ticket>/report.md`.

### Tình huống khi resolve (đã gặp ở đợt 10.2026 — làm đúng ngay từ đầu)

| Tình huống | Làm | Sai sót đã gặp |
|---|---|---|
| PHPStan/T3 cho thấy code thiếu do **helper thuần** (hàm tiện ích, không logic nghiệp vụ) | Branch `fix/<ticket>-…` tạo từ **branch tháng**, copy nguyên văn master, `merge --no-ff` (hoặc MR) vào **branch tháng**; resolve tiếp. `release` chỉ nhận đúng 1 MR mỗi đợt: `release-core-MM.YYYY → release` | #27458: MR !483 nhắm thẳng `release` (c839d94305) → sai quy tắc "Không MR thẳng vào release" (`docs/release-process.md` §4.7) |
| Code thiếu là **logic của ticket chưa release** (vd #21103 Ticket V3) | Ticket **phụ thuộc** → gỡ `Chờ release`/`Conflict cherry-pick`, gắn `Dev done` cho mọi MR của ticket, ghi vào report. `/release-check` lần sau sẽ promote lại (classify không biết phụ thuộc) → chạy bundle với `skip <iid>`. Cú pháp PHP 8.3 KHÔNG còn là lý do phụ thuộc từ đợt 10.2026 (release chạy 8.3) | #27290/#27293 (!454 !479); #27390 (!477 !482) lúc đầu bị hạ vì PHP 8.3, sau khi #27245 vào thì chỉ còn thiếu hàng đợi export |
| MR sửa tính năng/file **release chưa từng có** (CPEcommerceIntegration, file của ticket chưa release) | Bỏ đúng hunk/file đó, giữ phần còn lại; ghi lý do trong report. `audit-month.sh` liệt kê `FILE-BỎ`/`THIẾU` để đối chiếu | — |
| T3 composer/`vendor/` | Sửa tay: copy package từ master vào `vendor/`, thêm prefix vào `autoload_psr4.php` **và** `autoload_static.php` (`prefixLengthsPsr4` + `prefixDirsPsr4`), `composer.json`, entry trong `composer.lock` (content-hash để cũ — ghi rõ). KHÔNG `composer require`: vendor đã commit ≠ lock → bump cả vendor. Kiểm load class trên `php:7.2-cli` | — (#26929 psr/clock) |
| Lint PHP của release báo lỗi | Sửa dòng đó bằng `git commit --fixup <commit marker của MR>` trên `pick/<ticket>` **trước** `--merge` | Sửa thẳng trên branch tháng (#26103 `9396040b73`, viết lại cho 7.2) → commit không marker, `verify-resolves` báo ✗ UNVERIFIED mãi |
| Tìm commit để `--fixup` | `git log --format='%H %B' <base>..HEAD \| awk '/^[0-9a-f]{40} /{h=$1} /cherry picked from commit <sha12>/{print h}'`; sau autosquash `git log <base>..HEAD` không còn `fixup!` | Lấy SHA theo vị trí → fixup không gộp được |
| Áp tay hunk T3 "lệch ngữ cảnh" lớn | So vùng pre-image master với vùng release (difflib) trước; dòng chỉ release có phải giữ hoặc là đúng dòng MR xoá; thay vùng bằng `+` của hunk chỉ khi khớp duy nhất | — |
| Hunk chỉ thêm (log4php appender, config) | Kiểm số/ khoá chưa bị dùng trên release (`grep appender.A46`), file cuối không xuống dòng thì thêm newline | — |
| `verify-resolves` báo `NGHI XOÁ` / lệch nhiều | Kiểm trước khi kết luận: `git diff --ignore-cr-at-eol` (đổi LF↔CRLF), `-w` (thụt lề do `-Xignore-all-space`); số dòng còn khớp patch master = báo nhầm | !440 FacebookConnector (CRLF), !460 (thụt lề) |
| MR `Chờ release` mới xuất hiện sau lần chạy bundle | Trước khi chốt đợt: `/release-check --month MM.YYYY --dry-run --no-preview` (không đổi label) rồi `run.sh ... skip <MR phụ thuộc>` | #27390, #1537342 sót tới cuối đợt |
| MR ghi **ID record PMS** thay số ticket (`#1537342`) → `ticket_not_found` | `pms_retrieve HelpDesk <id>` → `ticket_no` + trạng thái. Closed thì pick tay đúng kỷ luật: `git worktree add -b pick/<ticket> ... <branch tháng>`, `cherry-pick -x -m 1` theo thứ tự merge, so patch, lint 7.2, `merge --no-ff`, gắn `Đã release` + milestone | !414 !448 = #26166 |
| Kết luận "không mất code" | Chạy `audit-month.sh` và đọc từng dòng; chỉ coi dòng là "đã bị thay" khi MR **sau, cùng đợt, cùng file** xoá nó; dòng chung (`}`, `return;`) là bằng chứng yếu — nói rõ giới hạn khi báo user | Đếm "master cũng không còn" là thay thế → sai |
| Ticket nâng runtime PHP (#27245 lên 8.3) | Code release lệch master (~2.800 file) chưa từng được sửa cho PHP 8 → kiểm **cả cây**, không chỉ diff: (1) `php -l` 8.3 mọi file phát hành gồm `vendor/`; (2) PHPStan cả cây bằng config master (baseline master), ≤ 2 worker, rồi đọc lỗi ngoài baseline: hằng chữ trần `$a[key]` (Error trên 8), override sai chữ ký (fatal khi nạp class), nơi gọi chỉ có trên release của hàm MR đổi chữ ký; (3) mỗi T3 bỏ phải có lý do khác "release không có code" — tìm bản tương đương trên release và sửa cùng kiểu (`html_entity_decode` cờ, `count` null, `fopen` false, `mb_convert_encoding` ValueError…) | #27245: `MailScanner`/`MailBox` hằng trần, `SalesAndRevenueByProductReportHandler` override, `IntegrationApiUtils::_setAuthSession` thiếu tham số |
| Sửa code **chỉ release có** (ngoài dòng MR đổi) trong ticket nâng runtime PHP | **Được phép** (user quyết 2026-11-05), là ngoại lệ duy nhất của quy tắc "chỉ mang dòng MR đổi": code release phải chạy được trên runtime mới. Chỉ sửa đúng loại MR đang sửa ở master (hằng chữ trần, chữ ký override, nơi gọi hàm MR đổi chữ ký, cờ `html_entity_decode`, `ValueError`/`TypeError` của builtin, `false` từ `fopen`/`glob`…); mỗi chỗ có comment attribution, đi bằng `--fixup` vào commit của MR; liệt kê đủ trong báo cáo cho user và danh sách test lại. Ticket không nâng runtime: vẫn chỉ mang dòng MR đổi, ngoài ra phải hỏi | #27245: 18 hằng chữ trần, 2 chữ ký override, 1 nơi gọi `_setAuthSession`, 9 cờ `html_entity_decode`, `mb_convert_encoding`, `fopen`, `glob`, `searchEmails`, `GeneralUtils` |
| Sửa file bằng script | Giữ nguyên xuống dòng: đọc/ghi `open(newline='')`, KHÔNG `Path.read_text()` (đổi CRLF→LF cả file). Trước commit `git diff --numstat` không được có file đổi toàn bộ dòng | Script tạm đổi CRLF 2 file (3.685 dòng) ở #27245, phải khôi phục |
| **Thứ tự pick** | Theo `merged_at` trên master (KHÔNG theo số MR — số MR là thứ tự tạo): patch `<sha>^1..<sha>` giả định mọi MR merge trước đã có. Trong các MR release ở đợt, pick lần lượt theo `merged_at`; ticket conflict dừng resolve tại chỗ (`--pause-on-conflict`), không dồn ra cuối. Ticket bị hoãn coi như vắng mặt — MR sau resolve trên nền release như thường; khi ticket hoãn được release ở đợt sau, nó là MR "lệch thứ tự". Sau mọi `pick/*` merge: `order-check.sh` dựng lại mỗi file có cặp pick ngược thứ tự (áp patch các MR của đợt theo `merged_at` bằng `git merge-file` từ `origin/release`) và so với branch: `SAME` xong; `CONFLICT`/`DIFF` đọc từng dòng — dòng MR xoá/đổi tên là đúng, dòng MR thêm mà branch thiếu là mất code | Đợt 10.2026: 31 cặp ngược thứ tự (ticket conflict/hoãn resolve sau, gom trọn ticket); mất guard `empty($_FILES)` của !450 trong `WebformsModel::createDocuments` vì !415 pick trước — `order-check.sh` bắt được trên branch cũ |

### Kết quả từng ticket

| Kết quả | Điều kiện | Branch | GitLab |
|---|---|---|---|
| PICKED | Mọi MR của ticket pick được | Thêm commit | `Đã release` + milestone |
| PICKED (noise) | Conflict chỉ ở `CHANGELOG.md`, `agents/`, `docs/`, `.gitignore`, `tools/`, `.omp/`, `.claude/` | Giữ bản release cho các file đó | như trên |
| DROPPED | Patch đã có trên release | Không đổi | như trên |
| CONFLICT | 1 MR conflict thật | **Bỏ cả ticket** (reset về đầu ticket) | `Conflict cherry-pick` cho mọi MR của ticket, giữ `Chờ release` |
| PENDING | Đang có MR mở `pick/<ticket>` | Không pick | Không đổi |
| USER_SKIPPED | MR trong skip list | Bỏ cả ticket | Không đổi |

## Rules

| Rule | Detail |
|---|---|
| Precondition | Có `classified-*.json` (`/release-check`) |
| Cả ticket hoặc không gì | Không bao giờ để nửa ticket trên branch release |
| Không MR/merge thẳng vào `release` | Mọi thay đổi (helper, hotfix, fix chỉ-release) vào **branch tháng**; `release` chỉ nhận MR `release-core-MM.YYYY → release` sau khi test |
| Không squash, không push branch release | Push branch release + tạo MR tổng là việc người release làm tay sau khi test |
| `--handoff` | Ngoại lệ duy nhất có push: chỉ push `pick/<ticket>` lên vtiger_release |
| Idempotent | Marker `-x`; chạy lại cùng tháng chỉ pick MR mới |
| Preview | Không bao giờ ghi branch thật, GitLab hay paused-state |
| Kiểm tra code | So sánh vtiger↔release (`compare-*/<iid>.diff`) + PHPStan + lint PHP release khi pick; sau khi `pick/*` merge → `verify-resolves.sh` (patch-vs-patch mọi pick trên branch, bắt resolve ẩu xoá code người khác) và `audit-month.sh` (A: dòng MR thêm có lên branch; B: dòng MR xoá còn sót; C: dòng release bị xoá mà không MR nào xoá); KHÔNG thay test trên môi trường release |
| Không `rerere` mặc định | rerere ghi cả cách resolve sai và chỉ lưu trên máy resolve |

## Trình bày cho user

Đọc `bundle-report-<today>.md` (hoặc `preview-<today>.md`), báo: số ticket pick/conflict/chờ dev, lỗi PHPStan mới, ticket bắt buộc test lại, ticket conflict + người cần resolve. Không tự push.

## State files (`STATE_DIR` = `vtiger/agents/release-queue/`)

| File | Tạo bởi |
|---|---|
| `classified-YYYY-MM-DD.json`, `report-*.md` | `/release-check` |
| `preview-YYYY-MM-DD.{json,md}`, `compare-preview-*/` | `--preview` (và Step 6 của `/release-check`) |
| `bundle-result-*.json`, `bundle-report-*.md`, `compare-*/`, `phpstan-*.json` | chạy thật |
| `paused-state-*.json` | chỉ `--pause-on-conflict` |

## Rollback (cùng ngày với run)

```bash
STATE_DIR="$(git rev-parse --show-toplevel)/agents/release-queue"   # chạy từ repo vtiger
RESULT="$STATE_DIR/bundle-result-$(date +%Y-%m-%d).json"
cd "$(jq -r .target_repo "$RESULT")" || exit 1     # worktree vtiger_release (single) hoặc repo vtiger_release cũ (split)
git cherry-pick --abort 2>/dev/null
git checkout "$(jq -r .release_branch "$RESULT")" \
  && git log --oneline "$(jq -r .base_sha "$RESULT")"..HEAD \
  && git reset --hard "$(jq -r .base_sha "$RESULT")"    # chỉ reset khi đã đứng đúng branch tháng

jq -r '.picked[].iid' "$RESULT" | while read -r IID; do
    glab api -X PUT "projects/onlinecrm%2Fvtiger/merge_requests/$IID" \
        -f "add_labels=Chờ release" -f "remove_labels=Đã release" -f "milestone_id=" >/dev/null
done
jq -r '.conflicts_manual[]? | (.iids // [.iid])[]' "$RESULT" | while read -r IID; do
    glab api -X PUT "projects/onlinecrm%2Fvtiger/merge_requests/$IID" -f "remove_labels=Conflict cherry-pick" >/dev/null
done
```

MR `pick/<ticket>` đã tạo bằng `--handoff` thì đóng tay trên GitLab.

## References

- `references/pick-logic.md` — vòng lặp ticket, file không phát hành, so sánh vtiger↔release, handoff, PHPStan, schema kết quả
- `references/gitlab-update.md` — milestone, label, cập nhật MR

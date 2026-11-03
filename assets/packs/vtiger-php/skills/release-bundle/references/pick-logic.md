# Pick Logic

Spec của `scripts/pick.sh` (Phase 0–1), `lib.sh` (`pick_one`, `compare_pick`) và `phpstan-check.sh`. Script là nguồn thực thi — file này giải thích hành vi, không lặp lại code.

## §1. Đầu vào và cấu hình (`lib.sh`)

| Biến | Mặc định | Override |
|---|---|---|
| Tháng release → `BRANCH=release-core-MM.YYYY`, milestone `Release core tháng MM/YYYY` | tháng hiện tại | `--month MM.YYYY` (env `RELEASE_MONTH`) |
| `CLASSIFIED` | file `classified-*.json` **mới nhất** trong `STATE_DIR` | env `RELEASE_CLASSIFIED` |
| Layout | `split`: release = `vtiger_release` master, remote nguồn `vtiger`. `single`: release = branch `release` của vtiger, nguồn `origin` | env `RELEASE_LAYOUT` (hoặc sửa mặc định trong `lib.sh`) |
| `TARGET` (nơi pick) | single: worktree `<thư mục cha của vtiger>/vtiger_release` của repo vtiger, tự tạo từ `origin/release` (guard: phải cùng git-common-dir với `VTIGER_ROOT`, tránh nhầm repo mirror); split: checkout mirror `<cha>/vtiger_release_mirror` | env `RELEASE_TARGET` (trỏ bản clone tạm để thử) |
| `RELEASE_BASE` | `origin/master` (split) / `origin/release` (single): điểm tạo branch tháng, mốc kiểm idempotent | — |
| `STATE_DIR` | `<vtiger>/agents/release-queue` | env `RELEASE_STATE_DIR` |
| `VTIGER_ROOT` | repo vtiger mà lệnh được chạy từ đó (worktree → về checkout chính); abort nếu origin không phải onlinecrm/vtiger | env `VTIGER_ROOT` |

File classified cũ hơn 3 ngày → cảnh báo nên chạy lại `/release-check` (không chặn).

## §2. Precondition (chạy thật, không áp dụng cho `--preview`)

Abort khi: không có classified, `TARGET` không phải git repo (worktree cũng được) / split thiếu remote `vtiger`, có thay đổi chưa commit (file tracked), có cherry-pick dở (`CHERRY_PICK_HEAD` trong git dir của target), `glab` chưa auth. `paused-state-<today>.json` tồn tại mà không có `CHERRY_PICK_HEAD` → stale, đổi tên `.stale-HHMMSS`.

## §3. Branch

`git fetch origin master` + `git fetch vtiger master` + `git fetch origin release-core-MM.YYYY` (MR bàn giao đã merge trên GitLab làm origin đi trước). Đối chiếu local với origin: local chứa origin → dùng local; local thiếu commit của origin → fast-forward theo origin; hai bên rẽ nhánh → **abort** (xử lý tay). Không có local → `origin/release-core-MM.YYYY` → `origin/master` (tạo branch mới).

`--preview`: `git worktree add --detach /tmp/release-preview.XXXX <điểm bắt đầu>`, chạy cùng vòng lặp trong worktree, xoá worktree khi thoát (trap). Branch thật, GitLab và paused-state không đổi. Kết quả: `preview-<today>.json` + `preview-<today>.md`.

## §4. Vòng lặp theo ticket

`ready[]` nhóm theo `ticket_id`, các nhóm xếp theo `merged_at` nhỏ nhất, MR trong nhóm xếp theo `merged_at`.

| Trường hợp | Hành vi |
|---|---|
| Một MR của ticket nằm trong skip list | Bỏ cả ticket → `skipped_by_user[]` |
| Ticket đang có MR mở `pick/<ticket>` trên vtiger_release | Không pick lại → `pending_handoff[]` |
| MR đã có marker `(cherry picked from commit <sha>)` trong `origin/master..HEAD` | Bỏ qua MR đó (idempotent), pick tiếp MR còn lại |
| Mọi MR pick được | Ghi vào `picked[]` |
| Một MR conflict thật / SHA không có / lỗi lạ | `git cherry-pick --abort`, `git reset --hard` về đầu ticket → **bỏ cả ticket**, ghi `conflicts_manual[]` kèm hướng dẫn bàn giao |

SHA của MR: `merge_commit_sha` > `squash_commit_sha` > `sha`. Cherry-pick: `-x -m 1 --empty=drop -Xignore-all-space`, commit thường (không phải merge) thì bỏ `-m 1`.

### `pick_one` — conflict chỉ ở file không phát hành

`NOISE_RE` = `CHANGELOG.md`, `.gitignore`, `agents/`, `docs/`, `tools/`, `.omp/`, `.claude/`. Conflict chỉ nằm ở các path này → giữ bản release (`git checkout HEAD -- <file>`, file không có trên release thì `git rm`) rồi `cherry-pick --continue`; nếu không còn gì thay đổi → `--skip`, trạng thái `dropped`. Ghi `noise_resolved: true`.

### `compare_pick` — so sánh vtiger ↔ release

Sau mỗi MR `picked`: các dòng MR thêm/xoá trên vtiger (`git diff <sha>^1 <sha>`) so với các dòng commit pick thêm/xoá trên release (`git diff HEAD^ HEAD`), bỏ khoảng trắng, bỏ path không phát hành. Lệch → `mismatch: N`, chi tiết ở `compare-<today>/<iid>.diff` (`<` chỉ có trên vtiger, `>` chỉ có trên release). Pick sạch gần như luôn 0; lệch nghĩa là code trên release khác code đã test → ticket bắt buộc test lại.

### Bàn giao ticket conflict

Mặc định: chỉ ghi hướng dẫn vào `conflicts_manual[].instructions` và report: base `origin/release-core-MM.YYYY` (branch release phải được push trước), lệnh cherry-pick **mọi MR chưa có trên release của ticket** theo đúng thứ tự, cách xử lý file không phát hành.

`--handoff` (chạy thật, cần `origin/release-core-MM.YYYY` đã push; chưa có thì chỉ ghi hướng dẫn): tạo `pick/<ticket>` **từ `origin/release-core-MM.YYYY`** (không từ HEAD local, vì HEAD có ticket khác vừa pick trong cùng run, chưa push), pick lại các MR sạch của ticket đứng trước MR conflict (pick lại lỗi → để trống, hướng dẫn liệt kê đủ ticket), `git push -f origin pick/<ticket>`, `glab mr create` vào branch release (label `Conflict cherry-pick`, assign tác giả MR đầu của ticket, mô tả = hướng dẫn với base `origin/pick/<ticket>` và chỉ các MR còn thiếu), rồi checkout lại branch release. Dev resolve ở máy local (cherry-pick dở không push được). Lần chạy sau bỏ qua ticket khi MR còn mở; MR merge xong → lần chạy sau fast-forward theo origin, marker `-x` làm các MR thành idempotent.

### `--pause` (legacy `--pause-on-conflict`)

Dừng ở conflict thật đầu tiên, giữ các MR đã pick của ticket trên branch, lưu `paused-state-<today>.json` (`current_conflict`, `remaining[]`), exit 1. User resolve + `git cherry-pick --continue|--skip` rồi `resume.sh` (resume pick từng MR, không nhóm theo ticket).

## §5. `bundle-result-<today>.json` / `preview-<today>.json`

```json
{
  "today": "2026-11-02", "preview": false, "classified": "classified-2026-11-02.json",
  "release_branch": "release-core-10.2026", "target_repo": "...", "base_sha": "<HEAD trước run>", "head_sha": "<HEAD sau run>",
  "compare_dir": ".../compare-2026-11-02",
  "picked": [{"iid": 422, "ticket": "26309", "sha": "...", "new_sha": "...", "status": "picked|dropped", "noise_resolved": false, "mismatch": 0}],
  "conflicts_manual": [{"ticket": "26929", "iids": [440, 441], "iid": 440, "sha": "...", "files": ["composer.json"], "author": "...", "handoff_mr": "", "instructions": "..."}],
  "skipped_by_user": [{"ticket": "...", "iids": [235], "reason": "user_request"}],
  "pending_handoff": [{"ticket": "...", "iids": [...], "handoff_mr": "https://..."}]
}
```

## §6. PHPStan trước/sau (`phpstan-check.sh <base> <head> <out> [result]`)

`vtiger_release` không có `tools/` và baseline của vtiger không áp được lên release. Script tạo 2 worktree tạm (`base_sha`, `head_sha`), mượn `vtiger/tools/phpstan` + `tools/vendor` + file ionCube đã decode (`include/utils/*.php8.php`), dùng config vtiger **bỏ baseline**, phân tích các file PHP đổi trong vùng PHPStan của vtiger. Giữ các lỗi `*.notFound` (function/method/staticMethod/class/classConstant/constant/property/staticProperty), `arguments.count` và `phpstan.parse` xuất hiện ở bản sau mà bản trước không có (khoá = file + message, không theo số dòng) → lỗi có sẵn trên release tự triệt tiêu. Lỗi cú pháp làm kết quả PHPStan không đầy đủ nên luôn được báo (trước đây bị nuốt thành 0 lỗi); lỗi chung của PHPStan ghi `phpstan.internal`. Thiếu `tools/vendor` hay PHPStan lỗi → ghi `skipped`, không chặn run.

Không thấy lời gọi động kiểu vtiger (`getInstance($name)`, `$obj->$method()`, tên action/view) và không kiểm được số tham số khi PHPStan không suy ra kiểu biến (vd `Vtiger_Cache_Connector_Redis::getInstance()->flushPicklistData(...)`) — test trên môi trường release vẫn bắt buộc.

## §7. Report (`report.sh`)

`bundle-report-<today>.md` (hoặc `preview-<today>.md`): ticket pick được (ghi chú lệch/file không phát hành), ticket conflict + hướng dẫn, ticket chờ dev, **danh sách test trên release**: bắt buộc test lại (ticket có `mismatch > 0`), sửa trước khi test (lỗi PHPStan mới, kèm MR gây ra), smoke test (phần còn lại).

## §8. Resolve theo hunk (`resolve.sh`, `resolve_hunks.py`)

Dành cho người release xử lý ticket conflict, mỗi lần một ticket, theo thứ tự `resolve.sh` (không đối số) liệt kê từ `bundle-result-*.json` mới nhất.

1. `resolve.sh <ticket>` tạo `pick/<ticket>` từ branch tháng (hoặc `--base <ref>`, vd `pick/<ticket trước>` chưa merge) trong worktree `<cha của vtiger>/vtiger-pick-<ticket>`, bật `merge.conflictStyle diff3`.
2. MR của ticket lấy từ `ready[]` của classified, xếp theo `merged_at`. MR đã có marker trong `origin/release..HEAD` → bỏ qua. Mỗi MR: `cherry-pick -x --no-commit -Xignore-all-space` (`-m 1` nếu là merge commit). File không conflict giữ kết quả merge 3 bên của git.
3. File conflict:
   - path không phát hành → bản release;
   - `DU` (release không có file) → bỏ, T3 kèm patch cả file;
   - `UD` (MR xoá file release còn) → giữ release, T3;
   - `composer.lock` / `vendor/` → giữ release, T3 "chạy lại composer";
   - còn lại: lấy bản release, áp từng hunk của `git diff <sha>^1 <sha> -- <file>`: T1 `git apply` đủ ngữ cảnh; T2 `-C1` và header `@@` của hunk vừa áp (diff worktree↔index) chứa đúng header của hunk master; T3 phần còn lại → hoàn file, ghi `resolve-<ticket>/<iid>/<file>.rej` + lý do theo dòng `-` của hunk so với file release (`lệch ngữ cảnh` = mọi dòng `-` có trên release; `sửa code release chưa có` = không dòng nào có; `chỉ thêm, mất chỗ neo` = hunk không có dòng `-`; `một phần`).
4. Mỗi MR được commit với message gốc + `(cherry picked from commit <sha>)`, kể cả khi chỉ có T3 (commit rỗng) → sửa tay đi vào `git commit --fixup` rồi `rebase -i --autosquash`, nên `verify-resolves.sh` so được patch của MR.
5. PHPStan trước/sau (§6) trên `base_sha..HEAD` của `pick/<ticket>`; report gom lỗi theo symbol: định nghĩa đã có trên branch (class → kiểm autoload; method → PHPStan suy sai kiểu), hoặc commit master đầu tiên chứa định nghĩa (`git log -S`) — nếu commit đó thuộc chính một MR của ticket thì code nằm trong hunk T3 chưa áp.
6. `--check` chạy lại bước 5 (worktree phải sạch). `--merge`: chặn khi `pick/<ticket>` đổi sau lần kiểm tra cuối, hoặc còn lỗi PHPStan mới (trừ `--force`); `merge --no-ff` vào branch tháng (TARGET phải đứng ở branch tháng, sạch), gắn `Đã release` + milestone, gỡ `Chờ release` / `Conflict cherry-pick` cho mọi MR của ticket (`--no-gitlab` để bỏ), gỡ worktree, giữ branch.

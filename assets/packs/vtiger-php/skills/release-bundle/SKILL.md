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
| 2 | `phpstan-check.sh` | PHPStan trước/sau trên worktree tạm, chỉ lỗi thiếu function/method/class **mới** |
| 3 | `gitlab-update.sh` | Milestone, label `Đã release` / `Conflict cherry-pick` (`references/gitlab-update.md`) |
| 4 | `report.sh` | `bundle-report-<today>.md` + danh sách test trên release |
| Sau run | `resolve.sh <ticket>` | Người release resolve từng ticket conflict theo hunk (mục dưới) |

### Resolve theo hunk (`resolve.sh`, `resolve_hunks.py`)

Chỉ mang **dòng MR đổi** (`git diff <sha>^1 <sha>`) sang release. Làm từng ticket theo thứ tự `resolve.sh` liệt kê, trên `pick/<ticket>` (worktree `<cha>/vtiger-pick-<ticket>`, tạo từ branch tháng hoặc `--base <ref>`). MR pick sạch thì giữ nguyên. File conflict: lấy lại bản release rồi áp từng hunk của MR:

| Tầng | Điều kiện | Xử lý |
|---|---|---|
| T1 | Áp được với đủ ngữ cảnh | Giữ |
| T2 | Chỉ áp được với `-C1` và rơi vào **cùng scope** (header `@@` function/selector/mảng) như trên master | Giữ, report liệt kê để đọc lại |
| T3 | Còn lại (không khớp, hoặc `-C1` rơi vào scope khác), file release không có, `composer.lock`/`vendor/`, file nhị phân, MR xoá file | Không áp; hunk lưu `.rej` + lý do (lệch ngữ cảnh / chỉ thêm mất chỗ neo / sửa code release chưa có / một phần) |

Mỗi MR luôn có commit mang marker `-x` (kể cả khi chỉ có T3): sửa tay T3 bằng `git commit --fixup <commit>` rồi autosquash. Sau đó PHPStan trước/sau trên `pick/<ticket>`, report gom lỗi theo symbol, chỉ ra symbol đã có trên branch (suy sai kiểu / autoload) hay commit master đã thêm nó (code thiếu cần mang sang — thuộc ticket chưa release hoặc không mang được thì hỏi). `--merge` chặn khi branch đổi sau lần `--check` cuối hoặc còn lỗi PHPStan mới (trừ `--force`). Report: `resolve-<ticket>/report.md`.


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
| Không squash, không push branch release | Push branch release + tạo MR tổng là việc người release làm tay sau khi test |
| `--handoff` | Ngoại lệ duy nhất có push: chỉ push `pick/<ticket>` lên vtiger_release |
| Idempotent | Marker `-x`; chạy lại cùng tháng chỉ pick MR mới |
| Preview | Không bao giờ ghi branch thật, GitLab hay paused-state |
| Kiểm tra code | So sánh vtiger↔release (`compare-*/<iid>.diff`) + PHPStan khi pick; sau khi `pick/*` merge → `verify-resolves.sh` (patch-vs-patch mọi pick trên branch, bắt resolve ẩu xoá code người khác); KHÔNG thay test trên môi trường release |
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

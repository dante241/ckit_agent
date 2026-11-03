#!/usr/bin/env bash
# resolve.sh — the release person resolves one conflicted ticket hunk by hunk on pick/<ticket>.
#
#   resolve.sh                         list the conflicted tickets of the last run, in pick order
#   resolve.sh <ticket> [--base <ref>] build pick/<ticket> from the month branch (or <ref>) in worktree
#                                      <parent>/vtiger-pick-<ticket>, then PHPStan + report
#   resolve.sh <ticket> --redo         drop that worktree/branch and build again
#   resolve.sh <ticket> --check        PHPStan + report again after hand edits on pick/<ticket>
#   resolve.sh <ticket> --merge [--no-gitlab] [--force]
#                                      merge --no-ff into the month branch, label the ticket's MRs
#                                      "Đã release" + milestone, remove "Conflict cherry-pick"
#   (all accept --month MM.YYYY)
#
# Only the MR's own changed lines are carried: see resolve_hunks.py for the T1/T2/T3 tiers.
# Report: $STATE_DIR/resolve-<ticket>/report.md (rejected hunks as .rej next to it).

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

TICKET="" MODE=build BASE_REF="" GITLAB=1 FORCE=0
while [ $# -gt 0 ]; do
    case "$1" in
        --month) export RELEASE_MONTH="${2:-}"; shift 2 ;;
        --month=*) export RELEASE_MONTH="${1#--month=}"; shift ;;
        --base) BASE_REF="${2:-}"; shift 2 ;;
        --redo) MODE=redo; shift ;;
        --check) MODE=check; shift ;;
        --merge) MODE=merge; shift ;;
        --no-gitlab) GITLAB=0; shift ;;
        --force) FORCE=1; shift ;;
        --*) echo "ABORT: flag lạ '$1'" >&2; exit 1 ;;
        *) TICKET="${1#\#}"; shift ;;
    esac
done

source ./lib.sh
export CLASSIFIED RELEASE_BASE NOISE_RE
PY="$PWD/resolve_hunks.py"

if [ -z "$TICKET" ]; then
    LAST=$(ls -1 "$STATE_DIR"/bundle-result-*.json 2>/dev/null | sort | tail -1)
    [ -n "$LAST" ] || abort "chưa có bundle-result-*.json (chạy run.sh trước)"
    echo "Ticket conflict theo thứ tự pick ($(basename "$LAST")):"
    python3 "$PY" list "$LAST" | while IFS=$'\t' read -r t iids author; do
        id="${t#\#}"
        if git -C "$TARGET" merge-base --is-ancestor "pick/$id" "$BRANCH" 2>/dev/null; then st="đã merge vào $BRANCH"
        elif git -C "$TARGET" rev-parse -q --verify "pick/$id" >/dev/null; then st="đang resolve (pick/$id)"
        else st="chưa làm"; fi
        printf '  %-8s %-26s %-14s %s\n' "$t" "$iids" "$author" "$st"
    done
    exit 0
fi
[[ "$TICKET" =~ ^[0-9]+$ ]] || abort "ticket phải là số, nhận '$TICKET'"

PB="pick/$TICKET"
WT="$SOURCE_PARENT/vtiger-pick-$TICKET"
OUT="$STATE_DIR/resolve-$TICKET"
REPORT="$OUT/report.md"
git -C "$TARGET" rev-parse -q --verify "$BRANCH" >/dev/null || abort "chưa có branch $BRANCH trong $TARGET (chạy run.sh trước)"

check() {
    local base head
    base=$(jq -r .base_sha "$OUT/result.json")
    head=$(git -C "$WT" rev-parse HEAD)
    git -C "$WT" diff --quiet && git -C "$WT" diff --cached --quiet || abort "$WT còn thay đổi chưa commit — commit trước khi kiểm tra"
    jq --arg h "$head" '.head_sha = $h' "$OUT/result.json" > "$OUT/result.json.tmp" && mv "$OUT/result.json.tmp" "$OUT/result.json"
    echo "→ PHPStan $base..$head"
    ./phpstan-check.sh "$base" "$head" "$OUT/phpstan.json"
    python3 "$PY" report "$OUT" "$OUT/phpstan.json" > "$REPORT" || abort "lỗi khi viết report"
    echo "Report: $REPORT"
    jq -r '"T2 cần đọc: \(.T2) · T3 chưa áp: \(.T3) · PHPStan lỗi mới: \(.phpstan)\(if .phpstan_skipped then " (PHPStan chưa chạy)" else "" end)"' "$OUT/check.json"
}

case "$MODE" in
    redo|build)
        if [ "$MODE" = redo ]; then
            [ -d "$WT" ] && git -C "$TARGET" worktree remove --force "$WT"
            git -C "$TARGET" branch -D "$PB" >/dev/null 2>&1
            rm -rf "$OUT"
        fi
        [ -e "$WT" ] && abort "$WT đã có — dùng --check, hoặc --redo để làm lại từ đầu"
        git -C "$TARGET" rev-parse -q --verify "$PB" >/dev/null && abort "branch $PB đã có — dùng --redo để làm lại"
        START="${BASE_REF:-$BRANCH}"
        git -C "$TARGET" rev-parse -q --verify "$START" >/dev/null || abort "không thấy ref '$START'"
        git -C "$TARGET" worktree add -q -b "$PB" "$WT" "$START" || abort "không tạo được worktree $WT"
        git -C "$WT" config merge.conflictStyle diff3
        echo "=== #$TICKET → $PB (từ $START) @ $WT ==="
        python3 "$PY" pick "$TICKET" "$WT" "$OUT" || abort "pick lỗi"
        check
        ;;
    check)
        [ -d "$WT" ] && [ -f "$OUT/result.json" ] || abort "chưa có $WT — chạy: resolve.sh $TICKET"
        check
        ;;
    merge)
        [ -f "$OUT/check.json" ] || abort "chưa kiểm tra — chạy: resolve.sh $TICKET --check"
        TIP=$(git -C "$TARGET" rev-parse "$PB") || abort "không thấy $PB"
        [ "$(jq -r .head_sha "$OUT/check.json")" = "$TIP" ] || abort "$PB đã đổi sau lần kiểm tra cuối — chạy: resolve.sh $TICKET --check"
        T3=$(jq .T3 "$OUT/check.json"); NPS=$(jq .phpstan "$OUT/check.json")
        if [ "$NPS" -gt 0 ] && [ "$FORCE" -eq 0 ]; then
            abort "còn $NPS lỗi PHPStan mới ($REPORT) — sửa trên $PB hoặc --force nếu đã xác nhận là dương tính giả"
        fi
        [ "$T3" -gt 0 ] && echo "⚠ Report ghi $T3 hunk T3 lúc build — merge nghĩa là anh đã xử lý/chấp nhận bỏ từng hunk đó."
        [ "$(git -C "$TARGET" rev-parse --abbrev-ref HEAD)" = "$BRANCH" ] || abort "$TARGET không đứng ở $BRANCH"
        git -C "$TARGET" diff --quiet && git -C "$TARGET" diff --cached --quiet || abort "$TARGET có thay đổi chưa commit"
        IIDS=$(jq -r '[.mrs[] | select(.status != "already") | "!\(.iid)"] | join(" ")' "$OUT/result.json")
        git -C "$TARGET" merge -q --no-ff "$PB" -m "Merge $PB into $BRANCH (#$TICKET $IIDS)" || abort "merge lỗi — git -C $TARGET merge --abort"
        echo "✓ Merged $PB → $BRANCH ($(git -C "$TARGET" rev-parse --short HEAD))"
        if [ "$GITLAB" -eq 1 ]; then
            glab auth status >/dev/null 2>&1 || abort "glab chưa auth — branch đã merge, gắn label tay"
            TITLE="Release core tháng $MONTH/$YEAR"
            MID=$(glab api "projects/$PROJECT_PATH/milestones?search=$(printf %s "$TITLE" | jq -sRr @uri)&state=active" \
                | jq -r --arg t "$TITLE" '.[] | select(.title == $t) | .id' | head -1)
            [ -n "$MID" ] || abort "không thấy milestone '$TITLE' — branch đã merge, gắn label tay"
            for iid in $(jq -r '.mrs[].iid' "$OUT/result.json"); do
                glab api -X PUT "projects/$PROJECT_PATH/merge_requests/$iid" -f "add_labels=Đã release" \
                    -f "remove_labels=Chờ release,Conflict cherry-pick" -f "milestone_id=$MID" >/dev/null \
                    && echo "  ✓ !$iid Đã release | $TITLE" || echo "  ✗ !$iid gắn label lỗi"
            done
        fi
        git -C "$TARGET" worktree remove "$WT" && echo "Đã gỡ worktree $WT (giữ branch $PB)"
        echo "Ticket #$TICKET resolve tay → test lại đầy đủ trên môi trường release. Sau cùng: verify-resolves.sh"
        ;;
esac

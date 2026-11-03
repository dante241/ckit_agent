#!/usr/bin/env bash
# pick.sh — Phase 0 (precondition) + Phase 1 (cherry-pick) of /release-bundle.
# See references/pick-logic.md for the spec this implements.
#
# Usage: pick.sh [--preview] [--handoff] [--pause] [--skip CSV]
#   --preview   simulate on a throwaway worktree of the release branch; writes preview-<today>.{json,md},
#               never touches the release branch, GitLab or paused state
#   --handoff   for a ticket that conflicts: push pick/<ticket> to the release repo and open an MR
#               assigned to the ticket's author (otherwise only the instructions are written)
#   --pause     legacy --pause-on-conflict: stop at the first real conflict, resume with resume.sh
#   --skip CSV  MR iids to leave out; the whole ticket of each iid is left out
#
# Tickets are picked whole: every ready MR of a ticket in merge order, or none of them.
# Exit codes: 0 = done; 1 = aborted (missing precondition) or paused on conflict (--pause)

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
SCRIPT_DIR=$(pwd)
source ./lib.sh

PREVIEW=0
HANDOFF=0
MODE="auto-skip"
SKIP_CSV=""
while [ $# -gt 0 ]; do
    case "$1" in
        --preview) PREVIEW=1; shift ;;
        --handoff) HANDOFF=1; shift ;;
        --pause) MODE="pause"; shift ;;
        --skip) SKIP_CSV="${2:-}"; shift 2 ;;
        *) abort "pick.sh: tham số không hợp lệ '$1'" ;;
    esac
done
SKIP_IIDS=()
[ -n "$SKIP_CSV" ] && IFS=',' read -ra SKIP_IIDS <<< "$SKIP_CSV"

# ---------- Phase 0: precondition ----------

[ -n "$CLASSIFIED" ] && [ -f "$CLASSIFIED" ] || abort "Không tìm thấy classified-*.json trong $STATE_DIR. Chạy /release-check trước."
AGE=$(classified_age_days)
echo "→ Classified: $(basename "$CLASSIFIED") ($AGE ngày trước) | branch: $BRANCH"
[ "$AGE" -gt 3 ] && echo "⚠️  File classified đã $AGE ngày — trạng thái ticket có thể đã đổi, nên chạy lại /release-check."

READY_COUNT=$(jq '.ready | length' "$CLASSIFIED")
if [ "$READY_COUNT" -eq 0 ]; then
    echo "0 MR ready để release. Exit."
    exit 0
fi

ensure_target
git -C "$TARGET" rev-parse --git-dir >/dev/null 2>&1 || abort "$TARGET không phải git repo"
if [ "$SOURCE_REMOTE" != "origin" ]; then
    git -C "$TARGET" remote get-url "$SOURCE_REMOTE" >/dev/null 2>&1 || abort "$TARGET thiếu remote '$SOURCE_REMOTE'. Setup: cd $TARGET && git remote add $SOURCE_REMOTE http://git.onlinecrm.vn/onlinecrm/vtiger.git"
fi
CHERRY_PICK_HEAD_FILE=$(target_git_path CHERRY_PICK_HEAD)

if [ "$PREVIEW" -eq 0 ]; then
    if [ -f "$PAUSED" ]; then
        if [ -f "$CHERRY_PICK_HEAD_FILE" ]; then
            abort "Cherry-pick đang dở (active paused-state). Resolve thủ công hoặc dùng resume.sh."
        fi
        STAMP=$(date +%H%M%S)
        echo "⚠️  STALE paused-state phát hiện (không có CHERRY_PICK_HEAD) — backup → $PAUSED.stale-$STAMP"
        mv "$PAUSED" "$PAUSED.stale-$STAMP"
    fi
    [ -f "$CHERRY_PICK_HEAD_FILE" ] && abort "$TARGET đang có cherry-pick dở. git cherry-pick --abort hoặc resolve trước."
    (cd "$TARGET" && git diff --quiet && git diff --cached --quiet) || {
        echo "ABORT: $TARGET có uncommitted changes (commit hoặc stash trước):" >&2
        (cd "$TARGET" && git status --short --untracked-files=no) >&2
        exit 1
    }
    glab auth status >/dev/null 2>&1 || abort "glab chưa auth. Chạy: glab auth login"
fi

# ---------- Phase 1.1: branch setup ----------

cd "$TARGET"
echo "→ Fetch origin/$RELEASE_MAIN + $SOURCE_REMOTE/master..."
git fetch -q origin "$RELEASE_MAIN" || abort "git fetch origin $RELEASE_MAIN lỗi"
git fetch -q "$SOURCE_REMOTE" master || abort "git fetch $SOURCE_REMOTE master lỗi"

# The release branch may have moved on GitLab (hand-off MRs merged): never pick onto a stale local copy
git fetch -q origin "$BRANCH" 2>/dev/null
HAS_LOCAL=0
HAS_ORIGIN=0
git show-ref --verify --quiet "refs/heads/$BRANCH" && HAS_LOCAL=1
git show-ref --verify --quiet "refs/remotes/origin/$BRANCH" && HAS_ORIGIN=1
FF_LOCAL=0
if [ "$HAS_LOCAL" -eq 1 ] && [ "$HAS_ORIGIN" -eq 1 ]; then
    if git merge-base --is-ancestor "origin/$BRANCH" "$BRANCH"; then
        :
    elif git merge-base --is-ancestor "$BRANCH" "origin/$BRANCH"; then
        FF_LOCAL=1
    else
        abort "$BRANCH local và origin/$BRANCH đã rẽ nhánh — đối chiếu (git log $BRANCH...origin/$BRANCH) rồi xử lý tay trước"
    fi
fi

if [ "$HAS_LOCAL" -eq 1 ] && [ "$FF_LOCAL" -eq 0 ]; then
    START_REF="$BRANCH"
elif [ "$HAS_ORIGIN" -eq 1 ]; then
    START_REF="origin/$BRANCH"
else
    START_REF="$RELEASE_BASE"
fi

WT=""
if [ "$PREVIEW" -eq 1 ]; then
    WT=$(mktemp -d /tmp/release-preview.XXXXXX)
    trap 'cd / && git -C "$TARGET" worktree remove --force "$WT" >/dev/null 2>&1; rm -rf "$WT"' EXIT
    git worktree add -q --detach "$WT" "$START_REF" || abort "Không tạo được worktree preview"
    cd "$WT"
    echo "→ Preview trên worktree tạm $WT (từ $START_REF) — branch thật không đổi"
else
    if [ "$FF_LOCAL" -eq 1 ]; then
        echo "→ $BRANCH local cũ hơn origin — fast-forward theo origin/$BRANCH"
        git checkout -q "$BRANCH" && git merge -q --ff-only "origin/$BRANCH" || abort "fast-forward $BRANCH lỗi"
    else
        case "$START_REF" in
            "$BRANCH") echo "→ Checkout local branch $BRANCH (đã có sẵn — append run mới)"; git checkout -q "$BRANCH" ;;
            "origin/$BRANCH") echo "→ Checkout từ origin/$BRANCH"; git checkout -q -b "$BRANCH" "origin/$BRANCH" ;;
            *) echo "→ Tạo branch mới $BRANCH từ $RELEASE_BASE"; git checkout -q -b "$BRANCH" "$RELEASE_BASE" ;;
        esac
    fi
fi
RUN_BASE=$(git rev-parse HEAD)

# Tickets already handed off (open MR pick/<ticket> on the release repo) are not picked again
OPEN_HANDOFF=$(glab api "projects/${RELEASE_PROJECT/\//%2F}/merge_requests?state=opened&per_page=100" 2>/dev/null \
    | jq -r '.[]? | select(.source_branch | startswith("pick/")) | "\(.source_branch | ltrimstr("pick/"))\t\(.web_url)"' 2>/dev/null)

# ---------- Phase 1.2: pick loop (one ticket at a time) ----------

PICKED_JSON="[]"
CONFLICTS_JSON="[]"
SKIPPED_JSON="[]"
PENDING_JSON="[]"
PROCESSED_IIDS=""
COMPARE_DIR="$STATE_DIR/compare-$TODAY"
[ "$PREVIEW" -eq 1 ] && COMPARE_DIR="$STATE_DIR/compare-preview-$TODAY"
rm -rf "$COMPARE_DIR"

is_user_skip() {
    local x
    for x in "${SKIP_IIDS[@]:-}"; do
        [ "$x" = "$1" ] && return 0
    done
    return 1
}

# cherry-pick command for a source commit (-m 1 only for merge commits)
pick_cmd() {
    if [ "$(git rev-list --parents -n 1 "$1" | wc -w)" -gt 2 ]; then
        echo "git cherry-pick -x -m 1 $1"
    else
        echo "git cherry-pick -x $1"
    fi
}

# handoff_text <ticket> <author> <failed_iid> <files_csv> <base> <remaining_tsv iid\tsha>
handoff_text() {
    local ticket="$1" author="$2" failed="$3" files="$4" base="$5" remaining="$6" iid sha
    echo "Ticket #$ticket (@$author) bị conflict khi cherry-pick MR !$failed sang \`$BRANCH\`."
    echo ""
    echo "File conflict: \`${files//,/\`, \`}\`"
    echo ""
    if [ "$LAYOUT" = "single" ]; then
        echo "Làm ở máy local, repo vtiger (nên dùng worktree riêng: \`git worktree add ../vtiger-pick origin/$RELEASE_MAIN\`):"
    else
        echo "Làm ở máy local, repo vtiger_release (cần remote \`vtiger\` = onlinecrm/vtiger):"
    fi
    echo ""
    echo '```bash'
    echo "git fetch origin$([ "$SOURCE_REMOTE" != "origin" ] && echo " && git fetch $SOURCE_REMOTE master")"
    echo "git checkout -B pick/$ticket $base$([ "$base" = "origin/$BRANCH" ] && echo "   # cần $BRANCH đã push lên origin")"
    echo "git config merge.conflictStyle diff3   # TRƯỚC khi pick: marker 3 khối, thấy base → biết MR đổi dòng nào"
    while IFS=$'\t' read -r iid sha; do
        [ -n "$iid" ] && echo "$(pick_cmd "$sha")   # MR !$iid"
    done <<< "$remaining"
    echo "# conflict → patch chuẩn của MR (first-parent) = thẩm quyền resolve:"
    while IFS=$'\t' read -r iid sha; do
        [ -n "$iid" ] && echo "#   git show ${sha:0:12} -m --first-parent -- <file>   # MR !$iid đổi dòng nào trên master"
    done <<< "$remaining"
    echo "# chỉ áp ĐÚNG các dòng đó lên code release (khối <<<<<<< HEAD); phần còn lại của release GIỮ NGUYÊN."
    echo "# KHÔNG git checkout --ours/--theirs cả FILE CODE (thay cả file = xoá lặng lẽ code ticket khác đã pick)."
    echo "# Lỡ pick khi chưa bật diff3 (marker chỉ 2 khối)? git checkout --conflict=diff3 -- <file> để sinh lại."
    echo "# → git add <file> → git cherry-pick --continue"
    echo "# CHANGELOG.md, agents/, docs/, .gitignore không phát hành: giữ bản release (git checkout --ours <file>; file không có trên release: git rm <file>)"
    echo "git push -u origin pick/$ticket"
    echo '```'
    echo ""
    echo "- Chỉ resolve phần conflict; không merge master vào nhánh này, không thêm code ngoài ticket."
    echo "- Phải pick đủ các MR theo đúng thứ tự trên (cả ticket hoặc không gì)."
    echo "- Xong thì ticket bắt buộc được test lại trên môi trường release."
}

while IFS= read -r group; do
    TICKET=$(jq -r '.ticket' <<< "$group")
    IIDS=$(jq -r '[.mrs[].iid] | map("!\(.)") | join(" ")' <<< "$group")
    IIDS_JSON=$(jq -c '[.mrs[].iid]' <<< "$group")
    AUTHOR=$(jq -r '.mrs[0].author' <<< "$group")
    echo ""
    echo "=== Ticket #$TICKET ($IIDS) ==="

    SKIP_HIT=""
    for iid in $(jq -r '.mrs[].iid' <<< "$group"); do
        is_user_skip "$iid" && SKIP_HIT="$iid"
    done
    if [ -n "$SKIP_HIT" ]; then
        echo "  USER_SKIPPED (MR !$SKIP_HIT trong danh sách skip — bỏ cả ticket)"
        SKIPPED_JSON=$(jq --arg t "$TICKET" --argjson iids "$IIDS_JSON" '. + [{ticket: $t, iids: $iids, reason: "user_request"}]' <<< "$SKIPPED_JSON")
        continue
    fi

    HANDOFF_URL=$(awk -F'\t' -v t="$TICKET" '$1 == t {print $2}' <<< "$OPEN_HANDOFF")
    if [ -n "$HANDOFF_URL" ]; then
        echo "  PENDING — đang chờ dev resolve ở $HANDOFF_URL"
        PENDING_JSON=$(jq --arg t "$TICKET" --argjson iids "$IIDS_JSON" --arg url "$HANDOFF_URL" '. + [{ticket: $t, iids: $iids, handoff_mr: $url}]' <<< "$PENDING_JSON")
        continue
    fi

    TICKET_START=$(git rev-parse HEAD)
    GROUP_PICKED="[]"
    FAIL_IID=""
    FAIL_FILES=""
    FAIL_SHA=""
    REMAINING=""
    ALL_PENDING=""

    while IFS= read -r mr; do
        IID=$(jq -r '.iid' <<< "$mr")
        SHA=$(mr_sha "$mr")
        PROCESSED_IIDS="${PROCESSED_IIDS}${IID},"

        if [ -n "$FAIL_IID" ]; then
            REMAINING+="$IID"$'\t'"$SHA"$'\n'
            ALL_PENDING+="$IID"$'\t'"$SHA"$'\n'
            continue
        fi

        if [ -z "$SHA" ] || [ "$SHA" = "null" ] || ! git cat-file -t "$SHA" >/dev/null 2>&1; then
            echo "  !! MR !$IID: SHA không có trong repo"
            FAIL_IID="$IID"; FAIL_SHA="$SHA"; FAIL_FILES="SHA_NOT_FOUND"
            continue
        fi

        if already_cherry_picked "$SHA"; then
            echo "  MR !$IID: đã có trên branch (idempotent)"
            continue
        fi

        ALL_PENDING+="$IID"$'\t'"$SHA"$'\n'
        pick_one "$SHA"
        case "$PICK_STATUS" in
            conflict)
                echo "  !! MR !$IID CONFLICT: $PICK_FILES"
                if [ "$MODE" = "pause" ] && [ "$PREVIEW" -eq 0 ]; then
                    PICKED_JSON=$(jq --argjson g "$GROUP_PICKED" '. + $g' <<< "$PICKED_JSON")
                    save_pause_state "$mr" "$PICKED_JSON" "$PICK_FILES" "$PROCESSED_IIDS"
                    exit 1
                fi
                git cherry-pick --abort >/dev/null 2>&1
                FAIL_IID="$IID"; FAIL_SHA="$SHA"; FAIL_FILES="$PICK_FILES"
                REMAINING+="$IID"$'\t'"$SHA"$'\n'
                ;;
            failed)
                FAIL_IID="$IID"; FAIL_SHA="$SHA"; FAIL_FILES="$PICK_FILES"
                REMAINING+="$IID"$'\t'"$SHA"$'\n'
                ;;
            dropped)
                echo "  MR !$IID: DROPPED (đã có sẵn trên release)"
                GROUP_PICKED=$(jq --arg iid "$IID" --arg sha "$SHA" --arg t "$TICKET" '. + [{iid: ($iid|tonumber), ticket: $t, sha: $sha, new_sha: "", status: "dropped", noise_resolved: false, mismatch: 0}]' <<< "$GROUP_PICKED")
                ;;
            picked)
                NEW_SHA=$(git rev-parse HEAD)
                MISMATCH=$(compare_pick "$SHA")
                MCOUNT=$(grep -c . <<< "$MISMATCH")
                if [ "$MCOUNT" -gt 0 ]; then
                    mkdir -p "$COMPARE_DIR"
                    printf '%s\n' "# MR !$IID — '<' chỉ có trên vtiger, '>' chỉ có trên release" "$MISMATCH" > "$COMPARE_DIR/$IID.diff"
                fi
                echo "  MR !$IID: picked ${NEW_SHA:0:10}$([ "$PICK_NOISE" -eq 1 ] && echo ' (conflict file không phát hành → giữ bản release)')$([ "$MCOUNT" -gt 0 ] && echo " ⚠ lệch $MCOUNT dòng so với vtiger")"
                GROUP_PICKED=$(jq --arg iid "$IID" --arg sha "$SHA" --arg new "$NEW_SHA" --arg t "$TICKET" --argjson noise "$([ "$PICK_NOISE" -eq 1 ] && echo true || echo false)" --argjson m "$MCOUNT" \
                    '. + [{iid: ($iid|tonumber), ticket: $t, sha: $sha, new_sha: $new, status: "picked", noise_resolved: $noise, mismatch: $m}]' <<< "$GROUP_PICKED")
                ;;
        esac
    done < <(jq -c '.mrs[]' <<< "$group")

    if [ -z "$FAIL_IID" ]; then
        PICKED_JSON=$(jq --argjson g "$GROUP_PICKED" '. + $g' <<< "$PICKED_JSON")
        continue
    fi

    # Whole ticket or nothing: drop this ticket's picks from the branch
    git reset -q --hard "$TICKET_START"
    HANDOFF_MR=""
    BASE_TEXT="origin/$BRANCH"
    LIST="$ALL_PENDING"
    if [ "$HANDOFF" -eq 1 ] && [ "$PREVIEW" -eq 0 ] && [ "$FAIL_FILES" != "SHA_NOT_FOUND" ]; then
        if git ls-remote --exit-code --heads origin "$BRANCH" >/dev/null 2>&1; then
            # pick/<ticket> starts from the PUSHED release branch, so its MR carries this ticket only
            HB="pick/$TICKET"
            git fetch -q origin "$BRANCH"
            git checkout -q -B "$HB" "origin/$BRANCH"
            PRE_OK=1
            while IFS=$'\t' read -r iid sha; do
                [ -n "$sha" ] || continue
                pick_one "$sha"
                case "$PICK_STATUS" in
                    picked|dropped) ;;
                    *) git cherry-pick --abort >/dev/null 2>&1; PRE_OK=0; break ;;
                esac
            done < <(jq -r '.[] | "\(.iid)\t\(.sha)"' <<< "$GROUP_PICKED")
            if [ "$PRE_OK" -eq 1 ]; then
                LIST="$REMAINING"
            else
                git reset -q --hard "origin/$BRANCH"
            fi
            BASE_TEXT="origin/$HB"
            TEXT=$(handoff_text "$TICKET" "$AUTHOR" "$FAIL_IID" "$FAIL_FILES" "$BASE_TEXT" "$LIST")
            if git push -q -f -u origin "$HB" 2>/dev/null; then
                HANDOFF_MR=$(glab mr create --repo "$RELEASE_PROJECT" --source-branch "$HB" --target-branch "$BRANCH" \
                    --title "[Release core $MONTH/$YEAR] Pick ticket #$TICKET ($IIDS)" --description "$TEXT" \
                    --label "Conflict cherry-pick" --assignee "$AUTHOR" --yes 2>&1 | grep -oE 'https?://[^ ]+/merge_requests/[0-9]+' | head -1)
                echo "  → Handoff: ${HANDOFF_MR:-tạo MR lỗi, xem glab}"
            else
                echo "  → Handoff: push $HB lỗi — chỉ ghi hướng dẫn"
                BASE_TEXT="origin/$BRANCH"
                LIST="$ALL_PENDING"
            fi
            git checkout -q "$BRANCH"
        else
            echo "  → Handoff: origin/$BRANCH chưa push — push branch release trước, chỉ ghi hướng dẫn"
        fi
    fi
    TEXT=$(handoff_text "$TICKET" "$AUTHOR" "$FAIL_IID" "$FAIL_FILES" "$BASE_TEXT" "$LIST")
    echo "  → Bỏ cả ticket #$TICKET khỏi branch (cần xử lý tay)"
    CONFLICTS_JSON=$(jq --arg t "$TICKET" --argjson iids "$IIDS_JSON" --arg iid "$FAIL_IID" --arg sha "$FAIL_SHA" --arg files "$FAIL_FILES" \
        --arg author "$AUTHOR" --arg mr "$HANDOFF_MR" --arg text "$TEXT" \
        '. + [{ticket: $t, iids: $iids, iid: ($iid|tonumber), sha: $sha, files: ($files|split(",")), author: $author, handoff_mr: $mr, instructions: $text}]' <<< "$CONFLICTS_JSON")

done < <(jq -c '.ready | group_by(.ticket_id) | map({ticket: .[0].ticket_id, first: (map(.merged_at) | min), mrs: sort_by(.merged_at)}) | sort_by(.first) | .[]' "$CLASSIFIED")

# ---------- Phase 1.3: persist result ----------

OUT="$BUNDLE_RESULT"
[ "$PREVIEW" -eq 1 ] && OUT="$PREVIEW_RESULT"
jq -n \
    --arg today "$TODAY" --arg branch "$BRANCH" --arg target "$TARGET" --arg base "$RUN_BASE" --arg head "$(git rev-parse HEAD)" \
    --arg classified "$(basename "$CLASSIFIED")" --argjson preview "$([ "$PREVIEW" -eq 1 ] && echo true || echo false)" \
    --arg compare_dir "$COMPARE_DIR" \
    --argjson picked "$PICKED_JSON" --argjson conflicts "$CONFLICTS_JSON" --argjson skipped "$SKIPPED_JSON" --argjson pending "$PENDING_JSON" \
    '{today: $today, preview: $preview, classified: $classified, release_branch: $branch, target_repo: $target, base_sha: $base, head_sha: $head,
      compare_dir: $compare_dir, picked: $picked, conflicts_manual: $conflicts, skipped_by_user: $skipped, pending_handoff: $pending}' > "$OUT"

echo ""
echo "=== SUMMARY $([ "$PREVIEW" -eq 1 ] && echo '(PREVIEW — branch không đổi)') ==="
echo "Tickets pick được: $(jq '[.picked[].ticket] | unique | length' "$OUT") ($(jq '.picked | length' "$OUT") MR)"
echo "Tickets conflict:  $(jq '.conflicts_manual | length' "$OUT")"
echo "Tickets chờ dev:   $(jq '.pending_handoff | length' "$OUT")"
echo "User-skipped:      $(jq '.skipped_by_user | length' "$OUT")"
echo "Result: $OUT"

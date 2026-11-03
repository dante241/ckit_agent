#!/usr/bin/env bash
# resume.sh — resume a /release-bundle run after --pause-on-conflict paused it.
# See references/pick-logic.md §7-§9 for the spec this implements.
#
# Precondition: user has resolved the conflict markers, `git add`-ed the
# resolved files, and run either `git cherry-pick --continue` or `--skip`.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./lib.sh

[ -f "$PAUSED" ] || abort "--resume được gọi nhưng không có paused state ($PAUSED)."

cd "$TARGET"

RELEASE_BRANCH=$(jq -r '.release_branch' "$PAUSED")
CURRENT_BRANCH=$(git branch --show-current)
[ "$CURRENT_BRANCH" = "$RELEASE_BRANCH" ] || abort "không ở branch $RELEASE_BRANCH (đang ở $CURRENT_BRANCH). Switch: git checkout $RELEASE_BRANCH"

UNMERGED=$(git diff --name-only --diff-filter=U)
if [ -n "$UNMERGED" ]; then
    echo "ABORT: vẫn còn unmerged files:" >&2
    echo "$UNMERGED" >&2
    exit 1
fi

if [ -d "$(git rev-parse --git-path sequencer)" ] || [ -f "$(git rev-parse --git-path CHERRY_PICK_HEAD)" ]; then
    abort "cherry-pick đang dở. Chạy: git cherry-pick --continue (hoặc --skip)"
fi

PICKED_COUNT_IN_STATE=$(jq '.picked | length' "$PAUSED")
COMMIT_COUNT_IN_BRANCH=$(git log "$RELEASE_BASE"..HEAD --oneline | wc -l | tr -d ' ')

if [ "$COMMIT_COUNT_IN_BRANCH" -lt "$PICKED_COUNT_IN_STATE" ]; then
    abort "commit count ($COMMIT_COUNT_IN_BRANCH) < picked trong state ($PICKED_COUNT_IN_STATE). Có lẽ đã cherry-pick --abort. Cleanup thủ công + start lại."
fi

# Determine whether the conflicted MR got a new commit (user resolved + continue)
# or not (user ran --skip). Either way, fold it into the running picked/conflicts
# lists and continue with `remaining[]`.
CONFLICT_IID=$(jq -r '.current_conflict.iid' "$PAUSED")
CONFLICT_SHA=$(jq -r '.current_conflict.sha' "$PAUSED")
CONFLICT_FILES=$(jq -c '.current_conflict.files' "$PAUSED")

PICKED_JSON=$(jq -c '.picked' "$PAUSED")
CONFLICTS_JSON="[]"

if [ "$COMMIT_COUNT_IN_BRANCH" -gt "$PICKED_COUNT_IN_STATE" ]; then
    NEW_SHA=$(git rev-parse HEAD)
    echo "→ MR !$CONFLICT_IID đã resolve + continue. new_sha=$NEW_SHA"
    PICKED_JSON=$(echo "$PICKED_JSON" | jq --arg iid "$CONFLICT_IID" --arg sha "$CONFLICT_SHA" --arg new_sha "$NEW_SHA" \
        '. + [{iid: ($iid|tonumber), sha: $sha, new_sha: $new_sha, status: "picked"}]')
else
    echo "→ MR !$CONFLICT_IID đã bị --skip (không có commit mới)."
    CONFLICTS_JSON=$(jq -n --arg iid "$CONFLICT_IID" --arg sha "$CONFLICT_SHA" --argjson files "$CONFLICT_FILES" \
        '[{iid: ($iid|tonumber), sha: $sha, files: $files}]')
fi

REMAINING=$(jq -c '.remaining' "$PAUSED")
REMAINING_COUNT=$(echo "$REMAINING" | jq 'length')

echo "→ Continuing pick với $REMAINING_COUNT MR còn lại..."

while IFS= read -r mr; do
    IID=$(echo "$mr" | jq -r '.iid')
    SHA=$(mr_sha "$mr")

    echo "=== MR !$IID ($SHA) ==="

    if ! git cat-file -t "$SHA" >/dev/null 2>&1; then
        echo "  !! SHA not found — treating as conflict-for-review"
        CONFLICTS_JSON=$(echo "$CONFLICTS_JSON" | jq --arg iid "$IID" --arg sha "$SHA" '. + [{iid: ($iid|tonumber), sha: $sha, files: ["SHA_NOT_FOUND"]}]')
        continue
    fi

    if already_cherry_picked "$SHA"; then
        echo "  SKIP (đã có trên branch — idempotent)"
        continue
    fi

    HEAD_BEFORE=$(git rev-parse HEAD)
    OUT=$(git cherry-pick -x -m 1 --empty=drop -Xignore-all-space "$SHA" 2>&1)
    RC=$?

    if [ $RC -ne 0 ] && [[ "$OUT" == *"not a merge"* ]]; then
        git cherry-pick --abort >/dev/null 2>&1
        OUT=$(git cherry-pick -x --empty=drop -Xignore-all-space "$SHA" 2>&1)
        RC=$?
    fi

    if [ $RC -ne 0 ] && [ -n "$(git diff --name-only --diff-filter=U 2>/dev/null)" ]; then
        FILES=$(git diff --name-only --diff-filter=U | tr '\n' ',' | sed 's/,$//')
        echo "  !! CONFLICT again — pausing again"
        save_pause_state "$mr" "$PICKED_JSON" "$FILES" ""
        # persist what we have so far before exiting
        jq -n --arg today "$TODAY" --arg branch "$BRANCH" --arg target "$TARGET" \
            --argjson picked "$PICKED_JSON" --argjson conflicts "$CONFLICTS_JSON" --argjson skipped "[]" \
            '{today:$today, release_branch:$branch, target_repo:$target, picked:$picked, conflicts_manual:$conflicts, skipped_by_user:$skipped}' \
            > "$BUNDLE_RESULT"
        exit 1
    fi

    if [ $RC -ne 0 ]; then
        echo "  !! UNKNOWN FAILURE: $OUT"
        git cherry-pick --abort >/dev/null 2>&1
        CONFLICTS_JSON=$(echo "$CONFLICTS_JSON" | jq --arg iid "$IID" --arg sha "$SHA" '. + [{iid: ($iid|tonumber), sha: $sha, files: ["UNKNOWN_FAILURE"]}]')
        continue
    fi

    HEAD_AFTER=$(git rev-parse HEAD)
    if [ "$HEAD_BEFORE" = "$HEAD_AFTER" ]; then
        echo "  DROPPED (patch already upstream)"
        PICKED_JSON=$(echo "$PICKED_JSON" | jq --arg iid "$IID" --arg sha "$SHA" '. + [{iid: ($iid|tonumber), sha: $sha, new_sha: "", status: "dropped"}]')
    else
        NEW_SHA=$(git rev-parse HEAD)
        echo "  -> picked ($NEW_SHA)"
        PICKED_JSON=$(echo "$PICKED_JSON" | jq --arg iid "$IID" --arg sha "$SHA" --arg new_sha "$NEW_SHA" '. + [{iid: ($iid|tonumber), sha: $sha, new_sha: $new_sha, status: "picked"}]')
    fi

done < <(echo "$REMAINING" | jq -c '.[]')

rm -f "$PAUSED"

jq -n --arg today "$TODAY" --arg branch "$BRANCH" --arg target "$TARGET" \
    --argjson picked "$PICKED_JSON" --argjson conflicts "$CONFLICTS_JSON" --argjson skipped "[]" \
    '{today:$today, release_branch:$branch, target_repo:$target, picked:$picked, conflicts_manual:$conflicts, skipped_by_user:$skipped}' \
    > "$BUNDLE_RESULT"

echo ""
echo "=== Resume complete ==="
echo "Picked: $(echo "$PICKED_JSON" | jq 'length')"
echo "Conflicts: $(echo "$CONFLICTS_JSON" | jq 'length')"
echo ""
echo "Chạy tiếp Phase 2: ./gitlab-update.sh"

#!/usr/bin/env bash
# gitlab-update.sh — Phase 2 of /release-bundle: milestone lookup/create,
# label auto-create, and per-MR label+milestone updates from bundle-result.
# See references/gitlab-update.md for the spec this implements.
#
# Usage: gitlab-update.sh
# Reads: $BUNDLE_RESULT (bundle-result-<today>.json, written by pick.sh)

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./lib.sh

[ -f "$BUNDLE_RESULT" ] || abort "$BUNDLE_RESULT không tồn tại. Chạy pick.sh trước."
[ "$(jq -r '.preview // false' "$BUNDLE_RESULT")" = "true" ] && abort "$BUNDLE_RESULT là kết quả preview — không cập nhật GitLab."

if [ "$(jq '(.picked | length) + (.conflicts_manual | length)' "$BUNDLE_RESULT")" -eq 0 ]; then
    echo "Không có MR mới pick hay conflict — GitLab giữ nguyên."
    exit 0
fi

glab auth status >/dev/null 2>&1 || abort "glab chưa auth. Chạy: glab auth login"

# ---------- §2: milestone lookup or create ----------

MILESTONE_TITLE="Release core tháng $MONTH/$YEAR"
TITLE_ENC=$(printf %s "$MILESTONE_TITLE" | jq -sRr @uri)

MILESTONE_ID=$(glab api "projects/$PROJECT_PATH/milestones?search=$TITLE_ENC&state=active" \
    | jq -r --arg t "$MILESTONE_TITLE" '.[] | select(.title == $t) | .id' | head -1)

if [ -z "$MILESTONE_ID" ] || [ "$MILESTONE_ID" = "null" ]; then
    DUE_DATE=$(date -v1d -v+1m -v-1d +%Y-%m-%d 2>/dev/null || date -d "$(date +%Y-%m-01) +1 month -1 day" +%Y-%m-%d)
    START_DATE="${YEAR}-${MONTH}-01"

    MILESTONE_JSON=$(glab api -X POST "projects/$PROJECT_PATH/milestones" \
        -f title="$MILESTONE_TITLE" \
        -f description="Auto-created bởi /release-bundle" \
        -f start_date="$START_DATE" \
        -f due_date="$DUE_DATE" 2>&1)

    MILESTONE_ID=$(echo "$MILESTONE_JSON" | jq -r '.id // empty')
    if [ -z "$MILESTONE_ID" ]; then
        echo "ABORT: không tạo được milestone. Response:" >&2
        echo "$MILESTONE_JSON" >&2
        echo "Không đủ quyền tạo milestone? Nhờ maintainer tạo tay \"$MILESTONE_TITLE\" rồi chạy lại." >&2
        exit 1
    fi
    echo "✓ Tạo milestone: $MILESTONE_TITLE (id=$MILESTONE_ID, due=$DUE_DATE)"
else
    echo "✓ Milestone có sẵn: $MILESTONE_TITLE (id=$MILESTONE_ID)"
fi

# ---------- §3: auto-create "Conflict cherry-pick" label ----------

LABEL_NAME="Conflict cherry-pick"
LABEL_EXISTS=$(glab api "projects/$PROJECT_PATH/labels?search=$(printf %s "$LABEL_NAME" | jq -sRr @uri)" \
    | jq -r --arg n "$LABEL_NAME" '.[] | select(.name == $n) | .name' | head -1)

if [ -z "$LABEL_EXISTS" ]; then
    glab api -X POST "projects/$PROJECT_PATH/labels" \
        -f "name=$LABEL_NAME" \
        -f "color=#d9534f" \
        -f "description=MR có conflict khi cherry-pick lên release branch — cần resolve thủ công" \
        >/dev/null
    echo "✓ Tạo label: $LABEL_NAME"
fi

# ---------- §4: per-MR updates ----------

COUNT=0

PICKED_IIDS=$(jq -r '.picked[].iid' "$BUNDLE_RESULT")
for iid in $PICKED_IIDS; do
    COUNT=$((COUNT + 1))
    [ "$COUNT" -gt 10 ] && sleep 1

    echo "→ MR !$iid (released)..."
    RESPONSE=$(glab api -X PUT "projects/$PROJECT_PATH/merge_requests/$iid" \
        -f "add_labels=Đã release" \
        -f "remove_labels=Chờ release" \
        -f "milestone_id=$MILESTONE_ID" 2>&1)

    if echo "$RESPONSE" | grep -q "404 Not Found"; then
        echo "  ⚠ MR !$iid không tồn tại, skip" >&2
    elif echo "$RESPONSE" | jq -e .iid >/dev/null 2>&1; then
        echo "  ✓ Đã release | Milestone: $MILESTONE_TITLE"
    else
        echo "  ✗ FAILED: $RESPONSE" >&2
    fi
done

# A ticket in conflict is left out whole: every MR of it gets the label
CONFLICT_IIDS=$(jq -r '.conflicts_manual[]? | (.iids // [.iid])[]' "$BUNDLE_RESULT")
for iid in $CONFLICT_IIDS; do
    COUNT=$((COUNT + 1))
    [ "$COUNT" -gt 10 ] && sleep 1

    echo "→ MR !$iid (conflict — manual)..."
    glab api -X PUT "projects/$PROJECT_PATH/merge_requests/$iid" \
        -f "add_labels=$LABEL_NAME" >/dev/null
    echo "  ✓ +$LABEL_NAME (giữ Chờ release)"
done

SKIPPED_IIDS=$(jq -r '.skipped_by_user[]? | (.iids // [.iid])[]' "$BUNDLE_RESULT")
for iid in $SKIPPED_IIDS; do
    echo "→ MR !$iid (user-skipped — giữ nguyên label)"
done

echo ""
echo "=== Phase 2 done ==="
echo "Milestone: $MILESTONE_TITLE (id=$MILESTONE_ID)"
echo "Released: $(echo "$PICKED_IIDS" | grep -c . || true)"
echo "Conflicts labeled: $(echo "$CONFLICT_IIDS" | grep -c . || true)"

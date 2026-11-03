#!/usr/bin/env bash
# verify-resolves.sh [branch] — review aid for hand-resolved pick/* merges. NOT a pass/fail gate.
#
# Every commit on the month branch is checked:
#   - no "(cherry picked from commit <sha>)" marker  -> ✗ UNVERIFIED (code outside the pick flow)
#   - marker present -> its first-parent patch is compared line-by-line (compare_pick
#     normalization: -U0, whitespace stripped, non-shipping paths excluded) with the
#     ORIGINAL master commit's patch.
#
# Reading the result:
#   - Picks the TOOL made (first-parent commits): any mismatch is a hard alarm —
#     compare_pick guaranteed 0 at pick time.
#   - Picks arriving via a pick/* MERGE (hand-resolved conflicts): mismatch is EXPECTED —
#     the removed-line side differs whenever the conflicted region had drifted. The value
#     is the focused line list a human reads instead of a 300-file MR diff:
#       MẤT  (< only in master patch)   = part of the ticket's own change never landed
#       THÊM (> only in branch commit)  = the resolve did something master never did
#   - "NGHI XOÁ CODE CỦA !X": a THÊM line that REMOVES text another pick on this branch
#     ADDED -> the exact "resolver kept their side and wiped my MR" case. Mechanical.
#
# Exit 0 = all picks match byte-exact. Exit 1 = anything to read (expected when the
# batch had hand resolves). Decide per ticket: fix the pick/* or accept + full retest.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./lib.sh

BR="${1:-$BRANCH}"
if git -C "$TARGET" rev-parse -q --verify "$BR" >/dev/null; then REF="$BR";
elif git -C "$TARGET" rev-parse -q --verify "origin/$BR" >/dev/null; then REF="origin/$BR";
else echo "ABORT: không thấy branch '$BR' (local hay origin) trong $TARGET" >&2; exit 2; fi

SLIM=$(ls -t "$STATE_DIR"/mrs-slim-*.json 2>/dev/null | head -1)
iid_of() {
    [ -n "$SLIM" ] || { echo "?"; return; }
    jq -r --arg s "$1" '.[] | select((.merge_commit_sha==$s) or (.squash_commit_sha==$s) or (.sha==$s)) | .iid' "$SLIM" | head -1 | grep . || echo "?"
}

patch_norm() { # normalized first-parent ±lines of a commit (compare_pick rules)
    git -C "$TARGET" diff -U0 "$1^1" "$1" -- . "${NOISE_PATHSPEC[@]}" 2>/dev/null \
        | grep -vE '^(\+\+\+|---) ' | grep -E '^[+-]' | sed 's/[[:space:]]//g' | grep -vE '^[+-]$' | LC_ALL=C sort
}

TMP=$(mktemp -d /tmp/verify-resolves.XXXXXX); trap 'rm -rf "$TMP"' EXIT
RANGE="origin/$RELEASE_MAIN..$REF"
git -C "$TARGET" rev-list --first-parent --no-merges "$RANGE" > "$TMP/fp"

# Pass 1: pairs (C S iid via) + index of added lines per pick (for wipe suspicion)
PAIRS_F="$TMP/pairs"; : > "$PAIRS_F"; : > "$TMP/added_idx"
while IFS= read -r C; do
    S=$(git -C "$TARGET" log -1 --format=%B "$C" \
        | grep -oE '\(cherry picked from commit [0-9a-f]{40}\)' | grep -oE '[0-9a-f]{40}' | head -1)
    if [ -z "$S" ]; then echo "$C - ? nomarker" >> "$PAIRS_F"; continue; fi
    VIA=merged; grep -qx "$C" "$TMP/fp" && VIA=tool
    IID=$(iid_of "$S")
    echo "$C $S $IID $VIA" >> "$PAIRS_F"
    # index: added lines (≥12 chars sau dấu) của pick này — để dò "xoá code người khác"
    patch_norm "$S" | grep '^+' | awk -v iid="$IID" 'length($0) >= 13 {print substr($0,2) "\t!" iid}' >> "$TMP/added_idx"
done < <(git -C "$TARGET" rev-list --no-merges "$RANGE")

OK=0 HARD=0 REVIEW=0 UNVER=0 SUSPECT=0
while read -r C S IID VIA; do
    if [ "$VIA" = "nomarker" ]; then
        UNVER=$((UNVER+1))
        echo "✗ UNVERIFIED ${C:0:10} \"$(git -C "$TARGET" log -1 --format=%s "$C")\" — không có marker -x: code ngoài quy trình pick, kiểm tay"
        continue
    fi
    if ! git -C "$TARGET" rev-parse -q --verify "$S^{commit}" >/dev/null; then
        UNVER=$((UNVER+1)); echo "✗ UNVERIFIED ${C:0:10} — marker trỏ commit không tồn tại ($S)"; continue
    fi
    DIFFLINES=$(diff <(patch_norm "$S") <(patch_norm "$C") | grep -E '^[<>]' || true)
    if [ -z "$DIFFLINES" ]; then OK=$((OK+1)); continue; fi

    N=$(echo "$DIFFLINES" | grep -c .)
    if [ "$VIA" = "tool" ]; then
        HARD=$((HARD+1)); TAG="✗ BẤT THƯỜNG (tool pick phải khớp 100%)"
    else
        REVIEW=$((REVIEW+1)); TAG="● CẦN ĐỌC (resolve tay — lệch là bình thường, đọc từng dòng)"
    fi
    echo "$TAG MR !$IID — ${C:0:10} vs patch gốc ${S:0:10}: $N dòng lệch"
    while IFS= read -r L; do
        BODY="${L:2}"
        case "$L" in
            "<"*) echo "    MẤT  (master có, release không): $BODY" ;;
            ">"*) echo "    THÊM (release làm, master không): $BODY" ;;
        esac
    done < <(echo "$DIFFLINES" | head -8)
    [ "$N" -gt 8 ] && echo "    … ($((N-8)) dòng nữa: diff <(git diff -U0 $S^1 $S) <(git diff -U0 $C^1 $C))"

    # Wipe suspicion: scan the FULL diff (not just the displayed head) for branch-side
    # removals ("> -<line>") whose text is a "+" line of ANOTHER pick on this branch.
    SUS=$(echo "$DIFFLINES" | grep '^> -' | cut -c4- | awk 'length($0) >= 12' \
        | grep -F -f /dev/stdin "$TMP/added_idx" 2>/dev/null | grep -v "	!$IID\$" | sort -u || true)
    if [ -n "$SUS" ]; then
        NS=$(echo "$SUS" | grep -c .)
        SUSPECT=$((SUSPECT+NS))
        echo "    ⚠ NGHI XOÁ CODE CỦA PICK KHÁC ($NS dòng — resolve này gỡ dòng mà pick khác đã thêm):"
        echo "$SUS" | head -5 | awk -F'\t' '{printf "      %s  ← của %s\n", $1, $2}'
        [ "$NS" -gt 5 ] && echo "      … ($((NS-5)) dòng nữa)"
    fi
done < "$PAIRS_F"

TOTAL=$((OK+HARD+REVIEW+UNVER))
[ "$TOTAL" -eq 0 ] && { echo "Branch $REF chưa có commit nào ngoài base — không có gì để kiểm."; exit 0; }
echo ""
echo "Tổng: $TOTAL commit — ✓ khớp: $OK · ● cần đọc (resolve tay): $REVIEW · ✗ bất thường: $HARD · ✗ unverified: $UNVER · ⚠ nghi xoá code pick khác: $SUSPECT"
if [ $((HARD+REVIEW+UNVER)) -eq 0 ]; then
    echo "✓ Mọi pick khớp patch gốc từng dòng."
    exit 0
fi
echo "● = đọc dòng MẤT/THÊM rồi quyết: sửa pick/* hoặc chấp nhận + ticket test lại đầy đủ. ⚠ NGHI XOÁ = ưu tiên xem trước."
exit 1

#!/usr/bin/env bash
# Read-only forecast: will this MR cherry-pick cleanly onto branch `release` once its ticket is Closed?
# Builds the would-be master merge commit (origin/master + MR head) in memory, then replays exactly
# that first-parent change onto origin/release with `git merge-tree` (no worktree, no branch, no checkout).
# Usage (from the vtiger repo root): bash <skill dir>/scripts/release-pick-preview.sh <MR_IID> [TARGET_BRANCH=master]
# Output: summary + last line "PICK: CLEAN" | "PICK: NOISE-ONLY" | "PICK: CONFLICT (<n> files)" | "PICK: SKIP (<why>)".
# Advisory only — never blocks the merge into master. Exit 0 always except setup errors (2).
set -uo pipefail

MR="${1:?usage: release-pick-preview.sh <MR_IID> [TARGET_BRANCH]}"
TARGET="${2:-master}"
REPO_ROOT="$(git rev-parse --show-toplevel)" || exit 2
cd "$REPO_ROOT" || exit 2

MR_REF="refs/remotes/mr/${MR}"
git fetch -q origin "$TARGET" release "+refs/merge-requests/${MR}/head:${MR_REF}" 2>/dev/null \
    || { echo "PICK: SKIP (git fetch failed — branch release chưa có trên origin?)"; exit 0; }

# 1) the commit master would get after merging the MR (in-memory)
MTREE=$(git merge-tree --write-tree --no-messages "origin/$TARGET" "$MR_REF" 2>/dev/null | head -1)
git merge-tree --write-tree --no-messages "origin/$TARGET" "$MR_REF" >/dev/null 2>&1 \
    || { echo "PICK: SKIP (MR còn conflict với $TARGET — rebase trước, gate cũng FAIL)"; exit 0; }
MCOMMIT=$(git -c user.name=preview -c user.email=preview@local commit-tree "$MTREE" -p "origin/$TARGET" -m "preview !$MR")

# 2) replay that first-parent change onto release (what /release-bundle will do with -m 1)
OUT=$(git merge-tree --write-tree --name-only --no-messages --merge-base="origin/$TARGET" origin/release "$MCOMMIT" 2>/dev/null)
RC=$?
if [ "$RC" -eq 0 ]; then
    echo "Release pickability !$MR: pick sạch lên origin/release."
    echo "PICK: CLEAN"; exit 0
fi
[ "$RC" -eq 1 ] || { echo "PICK: SKIP (merge-tree lỗi $RC)"; exit 0; }

NOISE='^(CHANGELOG\.md|\.gitignore|agents/|docs/|tools/|\.omp/|\.claude/)'
FILES=$(echo "$OUT" | tail -n +2 | sed '/^$/d' | sort -u)
REAL=$(echo "$FILES" | grep -vE "$NOISE" || true)
if [ -z "$REAL" ]; then
    echo "Release pickability !$MR: chỉ conflict file không phát hành ($(echo "$FILES" | tr '\n' ' ')) — tool tự giữ bản release."
    echo "PICK: NOISE-ONLY"; exit 0
fi

N=$(echo "$REAL" | grep -c .)
echo "Release pickability !$MR: $N file sẽ conflict khi cherry-pick lên release (cảnh báo sớm, KHÔNG chặn merge vào $TARGET):"
while IFS= read -r f; do
    if ! git cat-file -e "origin/release:$f" 2>/dev/null; then
        WHY="release chưa có file này — thường do ticket tạo ra nó chưa release"
    else
        WHY="vùng code đã đổi trên $TARGET bởi thay đổi chưa release"
    fi
    echo "  - $f — $WHY"
    git log --first-parent -3 --format='      gần nhất trên '"$TARGET"': %h %s' "origin/$TARGET" -- "$f" 2>/dev/null
done <<< "$REAL"
echo "Gợi ý cho author: ghi 'Phụ thuộc: #<ticket>' trong mô tả MR nếu dựa vào ticket ở trên; tách helper dùng chung thành MR riêng (docs/release-process.md §5)."
echo "PICK: CONFLICT ($N files)"
exit 0

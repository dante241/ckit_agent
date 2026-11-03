#!/usr/bin/env bash
# sync-release.sh <tag|branch> [version] — single layout: make vtiger_release master identical to <tag|branch>
# of onlinecrm/vtiger (normally a version tag on branch `release`), as ONE commit
# "Release core version <version>". Derived projects and auto_update keep reading vtiger_release.
# No push. Version defaults to the tag name without a leading "v".
#
# Env: RELEASE_MIRROR=<path> vtiger_release checkout (default <parent of vtiger>/vtiger_release_mirror, see lib.sh)

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./lib.sh

REF="${1:-}"
[ -n "$REF" ] || abort "Usage: sync-release.sh <tag|branch> [version]"
VERSION="${2:-${REF#v}}"
MIRROR="${RELEASE_MIRROR:-$MIRROR_REPO}"

cd "$MIRROR" || abort "Không vào được $MIRROR"
[ -z "$(git status --porcelain --untracked-files=no)" ] || { git status --short --untracked-files=no >&2; abort "$MIRROR có thay đổi chưa commit — commit hoặc stash trước"; }
[ -f "$(git rev-parse --git-path CHERRY_PICK_HEAD)" ] && abort "$MIRROR đang có cherry-pick dở"
git remote get-url vtiger >/dev/null 2>&1 || abort "$MIRROR thiếu remote 'vtiger' (onlinecrm/vtiger)"
git remote get-url origin 2>/dev/null | grep -q 'vtiger_release\.git' || abort "$MIRROR origin không phải onlinecrm/vtiger_release — đang trỏ nhầm worktree/repo khác? (RELEASE_MIRROR)"

# vtiger_release has its own tags (some share names with vtiger's): fetch only <ref>, into a private namespace
git fetch -q origin master || abort "git fetch origin master lỗi"
if git fetch -q vtiger "+refs/tags/$REF:refs/vtiger-sync/$REF" 2>/dev/null \
    || git fetch -q vtiger "+refs/heads/$REF:refs/vtiger-sync/$REF" 2>/dev/null; then
    SRC="refs/vtiger-sync/$REF"
else
    abort "Không tìm thấy tag hay branch '$REF' trên onlinecrm/vtiger"
fi
git rev-parse -q --verify "$SRC^{commit}" >/dev/null || abort "'$REF' không trỏ tới commit"

git checkout -q master || abort "checkout master lỗi"
if git diff --quiet "$SRC" HEAD; then
    echo "vtiger_release master đã giống $REF — không có gì để commit"
    exit 0
fi
if ! git merge-base --is-ancestor origin/master master; then
    abort "master local thiếu commit của origin/master — git pull trước"
fi
if ! git merge-base --is-ancestor master origin/master; then
    abort "master local có commit chưa push — xử lý trước khi đồng bộ"
fi

# read-tree --reset overwrites untracked files in the way without asking: refuse instead
CLASH=$(comm -12 <(git ls-files --others --exclude-standard | LC_ALL=C sort) <(git ls-tree -r --name-only "$SRC" | LC_ALL=C sort))
[ -n "$CLASH" ] && { echo "$CLASH" >&2; abort "File chưa track trùng đường dẫn với $REF — xoá/di chuyển trước"; }

git read-tree -u --reset "$SRC" || abort "read-tree $SRC lỗi"
if git diff --cached --quiet; then
    echo "vtiger_release master đã giống $REF — không có gì để commit"
    exit 0
fi
FILES=$(git diff --cached --name-only | wc -l | tr -d ' ')
git commit -q -m "Release core version $VERSION" -m "Synced from onlinecrm/vtiger $REF ($(git rev-parse --short "$SRC"))" || abort "commit lỗi"

if git diff --quiet "$SRC" HEAD; then
    echo "✓ master = $REF ($FILES file thay đổi) — commit $(git rev-parse --short HEAD) \"Release core version $VERSION\""
    echo "Next: cd $MIRROR && git push origin master"
else
    abort "Sau commit vẫn khác $REF — kiểm tra lại (git diff $SRC HEAD --stat)"
fi

#!/usr/bin/env bash
# lib.sh — shared vars/helpers for release-bundle scripts. Sourced, not executed directly.
#
# Layout (RELEASE_LAYOUT):
#   single (default) released code lives in branch `release` of onlinecrm/vtiger itself (baseline commit
#          42c4e290f9 without parent = vtiger_release content 7.1.0.20260915.1800, pushed 2026-11-02).
#          Picks happen in a dedicated worktree of the vtiger repo; vtiger_release is a mirror refreshed
#          by sync-release.sh.
#   split  (legacy) released code lives in repo onlinecrm/vtiger_release, branch master; MR commits come
#          from remote `vtiger`. Monthly branch release-core-MM.YYYY is picked in the vtiger_release checkout.
#
# Overrides (env):
#   RELEASE_LAYOUT=split|single
#   RELEASE_MONTH=MM.YYYY      release month (default: current month) → branch + milestone
#   RELEASE_CLASSIFIED=<path>  classified JSON (default: newest classified-*.json in STATE_DIR)
#   RELEASE_TARGET=<path>      checkout where picks happen (default per layout; point at a scratch clone to test)
#   RELEASE_STATE_DIR=<path>   state dir (default: <vtiger>/agents/release-queue)
#   VTIGER_ROOT=<path>         vtiger checkout (default: the repo you run the command from; tools/phpstan,
#                              worktree source in single layout)
#   RELEASE_MIRROR=<path>      onlinecrm/vtiger_release checkout (default: <parent of vtiger>/vtiger_release_mirror)

# _repo_root <dir>: main checkout of the repo containing <dir> (worktrees resolve to the main one)
_repo_root() {
    local d
    d=$(git -C "$1" rev-parse --path-format=absolute --git-common-dir 2>/dev/null) || return 1
    echo "${d%/.git}"
}
# Scripts cd into their own dir before sourcing this file: the caller's cwd is $OLDPWD.
# Project-local install (.omp/ inside the repo) also resolves from $PWD. Exported for child scripts.
if [ -z "${VTIGER_ROOT:-}" ]; then
    VTIGER_ROOT=$(_repo_root "${OLDPWD:-$PWD}") || VTIGER_ROOT=$(_repo_root "$PWD") || VTIGER_ROOT=""
fi
if [ -z "$VTIGER_ROOT" ] || ! git -C "$VTIGER_ROOT" remote get-url origin 2>/dev/null | grep -qE '[/:]onlinecrm/vtiger(\.git)?$'; then
    echo "ABORT: chạy lệnh từ trong repo onlinecrm/vtiger, hoặc set VTIGER_ROOT=<đường dẫn repo vtiger>" >&2
    exit 1
fi
export VTIGER_ROOT
# Scripts run from their own dir, which may be a shared pack store outside any git repo: there glab
# falls back to every configured host (gitlab.com included). Pin the host of the vtiger remote.
if [ -z "${GITLAB_HOST:-}" ]; then
    GITLAB_HOST=$(git -C "$VTIGER_ROOT" remote get-url origin | sed -E 's#^[a-z+]+://([^@/]*@)?##; s#^[^@]*@##; s#[:/].*$##')
fi
export GITLAB_HOST
SOURCE_PARENT=$(dirname "$VTIGER_ROOT")
STATE_DIR="${RELEASE_STATE_DIR:-$VTIGER_ROOT/agents/release-queue}"
LAYOUT="${RELEASE_LAYOUT:-single}"
PROJECT_PATH="onlinecrm%2Fvtiger"
# Mirror checkout of onlinecrm/vtiger_release (sync-release.sh target; also split-layout TARGET).
# Since the one-repo switch, <parent>/vtiger_release is the pick WORKTREE of the vtiger repo, not the mirror.
MIRROR_REPO="${RELEASE_MIRROR:-$SOURCE_PARENT/vtiger_release_mirror}"

case "$LAYOUT" in
    split)
        TARGET="${RELEASE_TARGET:-$MIRROR_REPO}"
        SOURCE_REMOTE="vtiger"
        RELEASE_MAIN="master"
        RELEASE_PROJECT="onlinecrm/vtiger_release"
        ;;
    single)
        TARGET="${RELEASE_TARGET:-$SOURCE_PARENT/vtiger_release}"
        SOURCE_REMOTE="origin"
        RELEASE_MAIN="release"
        RELEASE_PROJECT="onlinecrm/vtiger"
        ;;
    *) echo "ABORT: RELEASE_LAYOUT phải là split hoặc single, nhận '$LAYOUT'" >&2; exit 1 ;;
esac
# Released code the monthly branch starts from and is compared against
RELEASE_BASE="origin/$RELEASE_MAIN"

# target_git_path <name> — path of a file inside the target's git dir (works for worktrees: .git is a file there)
target_git_path() {
    git -C "$TARGET" rev-parse --path-format=absolute --git-path "$1"
}

# ensure_target — single layout: create the pick worktree of the vtiger repo on first use
ensure_target() {
    [ "$LAYOUT" = "single" ] || return 0
    if git -C "$TARGET" rev-parse --git-dir >/dev/null 2>&1; then
        # TARGET must be a worktree of VTIGER_ROOT, not a foreign repo (e.g. an old vtiger_release mirror)
        local want have
        want=$(git -C "$VTIGER_ROOT" rev-parse --path-format=absolute --git-common-dir)
        have=$(git -C "$TARGET" rev-parse --path-format=absolute --git-common-dir)
        [ "$have" = "$want" ] || { echo "ABORT: $TARGET là repo khác (git dir $have), không phải worktree của $VTIGER_ROOT — dọn/đổi RELEASE_TARGET" >&2; exit 1; }
        return 0
    fi
    git -C "$VTIGER_ROOT" fetch -q origin "$RELEASE_MAIN" || { echo "ABORT: không fetch được origin/$RELEASE_MAIN — branch release đã push chưa?" >&2; exit 1; }
    echo "→ Tạo worktree pick release: $TARGET (repo $VTIGER_ROOT)"
    git -C "$VTIGER_ROOT" worktree add -q --detach "$TARGET" "origin/$RELEASE_MAIN" || { echo "ABORT: không tạo được worktree $TARGET" >&2; exit 1; }
}

TODAY=$(date +%Y-%m-%d)

if [ -n "${RELEASE_MONTH:-}" ]; then
    [[ "$RELEASE_MONTH" =~ ^(0[1-9]|1[0-2])\.20[0-9]{2}$ ]] || { echo "ABORT: --month phải dạng MM.YYYY (vd 10.2026), nhận '$RELEASE_MONTH'" >&2; exit 1; }
    MONTH="${RELEASE_MONTH%%.*}"
    YEAR="${RELEASE_MONTH##*.}"
else
    MONTH=$(date +%m)
    YEAR=$(date +%Y)
fi
BRANCH="release-core-$MONTH.$YEAR"

# Newest classified file, so /release-bundle does not have to run on the same day as /release-check
if [ -n "${RELEASE_CLASSIFIED:-}" ]; then
    CLASSIFIED="$RELEASE_CLASSIFIED"
else
    CLASSIFIED=$(ls -1 "$STATE_DIR"/classified-*.json 2>/dev/null | sort | tail -1)
fi
PAUSED="$STATE_DIR/paused-state-$TODAY.json"
BUNDLE_RESULT="$STATE_DIR/bundle-result-$TODAY.json"
BUNDLE_REPORT="$STATE_DIR/bundle-report-$TODAY.md"
PREVIEW_RESULT="$STATE_DIR/preview-$TODAY.json"
PREVIEW_REPORT="$STATE_DIR/preview-$TODAY.md"

# Paths that never ship to projects: a conflict limited to them keeps the release side
NOISE_RE='^(CHANGELOG\.md|\.gitignore|agents/|docs/|tools/|\.omp/|\.claude/)'
NOISE_PATHSPEC=(':(exclude)CHANGELOG.md' ':(exclude).gitignore' ':(exclude)agents' ':(exclude)docs' ':(exclude)tools' ':(exclude).omp' ':(exclude).claude')

abort() {
    echo "ABORT: $1" >&2
    exit 1
}

# Days between the classified scan and today (0 = same day)
classified_age_days() {
    local scan
    scan=$(jq -r '.scan_date // empty' "$CLASSIFIED")
    [ -n "$scan" ] || { echo 0; return; }
    echo $(( ($(date -j -f %Y-%m-%d "$TODAY" +%s) - $(date -j -f %Y-%m-%d "$scan" +%s)) / 86400 ))
}

# already_cherry_picked <sha> — true if $sha's "(cherry picked from commit <sha>)"
# marker is already present in $RELEASE_BASE..HEAD.
#
# NOTE: do NOT write this as `git log ... | grep -qF ...`. With `set -o pipefail`,
# `grep -q` exits as soon as it finds a match, closing the pipe early; `git log`'s
# still-writing side then gets SIGPIPE, and pipefail surfaces THAT non-zero exit
# code for the whole pipeline — masking the fact that grep actually matched. This
# makes the check flip nondeterministically run to run. Capture the log into a
# variable first (no pipe to prematurely close) and test with bash's own [[ == *substr* ]].
already_cherry_picked() {
    local sha="$1"
    local log
    log=$(git log "$RELEASE_BASE"..HEAD --format='%H %B' 2>/dev/null)
    [[ "$log" == *"cherry picked from commit $sha"* ]]
}

# SHA priority: merge_commit_sha > squash_commit_sha (non-empty) > sha
mr_sha() {
    local mr="$1"
    echo "$mr" | jq -r 'if (.merge_commit_sha // "" | length > 0) then .merge_commit_sha elif (.squash_commit_sha // "" | length > 0) then .squash_commit_sha else (.sha // "") end'
}

# pick_one <sha> — cherry-pick one MR commit onto HEAD.
# Sets PICK_STATUS = picked | dropped | conflict | failed, PICK_FILES (csv of real conflict
# files), PICK_NOISE=1 when a conflict limited to non-shipping paths was resolved with the
# release side. On "conflict" the cherry-pick is left in progress for the caller.
pick_one() {
    local sha="$1" out rc before unmerged real p
    PICK_STATUS=""
    PICK_FILES=""
    PICK_NOISE=0
    before=$(git rev-parse HEAD)

    out=$(git cherry-pick -x -m 1 --empty=drop -Xignore-all-space "$sha" 2>&1)
    rc=$?
    if [ $rc -ne 0 ] && [[ "$out" == *"not a merge"* ]]; then
        git cherry-pick --abort >/dev/null 2>&1
        out=$(git cherry-pick -x --empty=drop -Xignore-all-space "$sha" 2>&1)
        rc=$?
    fi

    if [ $rc -ne 0 ]; then
        unmerged=$(git diff --name-only --diff-filter=U 2>/dev/null)
        if [ -z "$unmerged" ]; then
            echo "  !! UNKNOWN FAILURE: $out"
            git cherry-pick --abort >/dev/null 2>&1
            PICK_STATUS="failed"
            PICK_FILES="UNKNOWN_FAILURE"
            return
        fi

        real=$(grep -vE "$NOISE_RE" <<< "$unmerged")
        if [ -n "$real" ]; then
            PICK_STATUS="conflict"
            PICK_FILES=$(tr '\n' ',' <<< "$real" | sed 's/,$//')
            return
        fi

        while IFS= read -r p; do
            if git cat-file -e "HEAD:$p" 2>/dev/null; then
                git checkout HEAD -- "$p"
            else
                git rm -q -f -- "$p" >/dev/null 2>&1
            fi
        done <<< "$unmerged"
        PICK_NOISE=1

        if git diff --cached --quiet; then
            git cherry-pick --skip >/dev/null 2>&1
            PICK_STATUS="dropped"
            return
        fi
        if ! git -c core.editor=true cherry-pick --continue >/dev/null 2>&1; then
            git cherry-pick --abort >/dev/null 2>&1
            PICK_STATUS="failed"
            PICK_FILES="UNKNOWN_FAILURE"
            return
        fi
    fi

    if [ "$(git rev-parse HEAD)" = "$before" ]; then
        PICK_STATUS="dropped"
    else
        PICK_STATUS="picked"
    fi
}

# compare_pick <source_sha> — lines the MR added/removed on vtiger vs lines the HEAD commit
# added/removed on the release branch (whitespace-insensitive, non-shipping paths excluded).
# Prints the lines that differ ("<" only on vtiger, ">" only on release); empty = identical.
compare_pick() {
    local src="$1" base
    base=$(git rev-parse "$src^1")
    diff <(git diff -U0 "$base" "$src" -- . "${NOISE_PATHSPEC[@]}" | _norm_diff) \
         <(git diff -U0 HEAD^ HEAD -- . "${NOISE_PATHSPEC[@]}" | _norm_diff) | grep -E '^[<>]'
}

_norm_diff() {
    grep -vE '^(\+\+\+|---) ' | grep -E '^[+-]' | LC_ALL=C sed 's/[[:space:]]//g' | grep -vE '^[+-]$' | LC_ALL=C sort
}

# save_pause_state <current_mr_json> <picked_json_array> <conflict_files_csv> <processed_iids_csv>
# Only used by pick.sh in "pause" mode (legacy --pause-on-conflict). Writes $PAUSED.
save_pause_state() {
    local current_mr="$1" picked_json="$2" conflict_files_csv="$3" processed_csv="$4"
    local conflict_iid conflict_sha conflict_files_json remaining_json

    conflict_iid=$(echo "$current_mr" | jq -r '.iid')
    conflict_sha=$(mr_sha "$current_mr")
    conflict_files_json=$(echo "$conflict_files_csv" | jq -R -c 'split(",") | map(select(length > 0))')

    # remaining = ready[] minus everything already processed (picked iids + current conflict iid)
    remaining_json=$(jq -c --argjson processed "$(echo "$processed_csv" | jq -R -c 'split(",") | map(select(length>0) | tonumber)')" \
        --arg conflict_iid "$conflict_iid" \
        '.ready | sort_by(.merged_at) | map(select(([.iid] - $processed - [($conflict_iid|tonumber)]) | length > 0))' \
        "$CLASSIFIED")

    jq -n \
        --arg paused_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg release_branch "$BRANCH" \
        --arg target_repo "$TARGET" \
        --arg classified_path "$CLASSIFIED" \
        --argjson picked "$picked_json" \
        --arg conflict_iid "$conflict_iid" \
        --arg conflict_sha "$conflict_sha" \
        --argjson conflict_files "$conflict_files_json" \
        --argjson remaining "$remaining_json" \
        '{
            paused_at: $paused_at,
            release_branch: $release_branch,
            target_repo: $target_repo,
            classified_path: $classified_path,
            picked: $picked,
            current_conflict: {iid: ($conflict_iid | tonumber), sha: $conflict_sha, files: $conflict_files},
            remaining: $remaining
        }' > "$PAUSED"

    echo ""
    echo "⚠️  CONFLICT on MR !$conflict_iid (SHA ${conflict_sha:0:8})"
    echo "Files cần resolve:"
    echo "$conflict_files_json" | jq -r '.[] | "  - \(.)"'
    echo ""
    echo "Steps để tiếp tục:"
    echo "  1. cd $TARGET"
    echo "  2. Resolve conflict markers (<<<<<<< / ======= / >>>>>>>)"
    echo "  3. git add <resolved files>"
    echo "  4. git cherry-pick --continue    (hoặc --skip để bỏ MR !$conflict_iid)"
    echo "  5. resume.sh"
    echo ""
    echo "State lưu: $PAUSED"
}

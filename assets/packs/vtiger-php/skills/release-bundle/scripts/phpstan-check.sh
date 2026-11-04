#!/usr/bin/env bash
# phpstan-check.sh <base> <head> <out.json> [result.json]
#
# Missing-code check for a pick run on vtiger_release: runs PHPStan (vtiger's config, WITHOUT its
# baselines) on the PHP files changed between <base> and <head>, once on each side, and reports the
# errors of $IDS (symbol not found, wrong argument count, parse error) that exist after the picks but
# not before. Errors that already exist on the release branch cancel out, so vtiger's baselines are
# not needed. A parse error makes PHPStan's result incomplete, so it is always reported, never hidden.
#
# vtiger_release has no tools/: both sides run in throwaway worktrees that borrow vtiger's
# tools/phpstan + tools/vendor and the decoded ionCube sources (include/utils/*.php8.php).
# [result.json] (bundle-result) maps each error file to the MR that changed it.
#
# Limits: only direct calls are seen; vtiger's string-based dispatch (getInstance($name),
# $obj->$method(), action/view names) is not — the release-environment test still covers that.

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./lib.sh

BASE="$1"
HEAD_REV="$2"
OUT="$3"
RESULT="${4:-}"
VT="$VTIGER_ROOT"
IDS='^((function|method|staticMethod|class|classConstant|constant|property|staticProperty)\.notFound|arguments\.count|phpstan\.parse)$'

skip() {
    jq -n --arg why "$1" '{errors: [], skipped: $why}' > "$OUT"
    echo "PHPStan: bỏ qua — $1"
    exit 0
}

[ -x "$VT/tools/vendor/bin/phpstan" ] || skip "thiếu $VT/tools/vendor (chạy: composer install -d $VT/tools)"
[ -f "$VT/tools/phpstan/phpstan.neon.dist" ] || skip "thiếu $VT/tools/phpstan/phpstan.neon.dist"

FILES=$(git -C "$TARGET" diff --name-only --diff-filter=AM "$BASE" "$HEAD_REV" -- '*.php' '*.inc' '*.service' \
    | grep -E '^(modules|api|cron/modules|include/(EntryPoints|utils|Webhooks|Webservice|WebserviceTraits))/' \
    | grep -vE '\.php[78]\.php$|/tests/|^modules/(com_vtiger_workflow|Webforms|WSAPP)/')
[ -n "$FILES" ] || skip "không có file PHP thay đổi trong vùng PHPStan phân tích"
echo "PHPStan: $(grep -c . <<< "$FILES") file thay đổi, chạy trước/sau..."

WTS=()
cleanup() {
    local w
    for w in "${WTS[@]:-}"; do
        [ -n "$w" ] && git -C "$TARGET" worktree remove --force "$w" >/dev/null 2>&1
        rm -rf "$w"
    done
}
trap cleanup EXIT

# prepare_side <rev>: throwaway worktree of <rev> with vtiger's PHPStan setup; echoes its path
prepare_side() {
    local rev="$1" wt f
    wt=$(mktemp -d /tmp/release-phpstan.XXXXXX)
    git -C "$TARGET" worktree add -q --detach "$wt" "$rev" || { rm -rf "$wt"; return 1; }

    mkdir -p "$wt/tools"
    cp -R "$VT/tools/phpstan" "$wt/tools/phpstan"
    ln -s "$VT/tools/vendor" "$wt/tools/vendor"
    # vtiger's config minus its baselines, with a private result cache per side
    sed '/^includes:/,/^[[:space:]]*$/d' "$VT/tools/phpstan/phpstan.neon.dist" \
        | awk -v tmp="$wt/.phpstan-tmp" -v procs="${RELEASE_PHPSTAN_PROCS:-4}" \
            '{print} /^parameters:/{print "    tmpDir: " tmp; print "    parallel:"; print "        maximumNumberOfProcesses: " procs}' \
        > "$wt/tools/phpstan/release-check.neon"
    for f in "$VT"/include/utils/*.php[78].php "$VT"/include/EntryPoints/*.php[78].php; do
        [ -f "$f" ] && [ ! -e "$wt/${f#"$VT"/}" ] && cp "$f" "$wt/${f#"$VT"/}"
    done
    echo "$wt"
}

# analyse_side <worktree> <json_out>: analyse the changed files that exist in that worktree
analyse_side() {
    local wt="$1" json="$2" f list=()
    while IFS= read -r f; do
        [ -f "$wt/$f" ] && list+=("$f")
    done <<< "$FILES"
    [ "${#list[@]}" -gt 0 ] || { echo '{"files":{}}' > "$json"; return 0; }

    (cd "$wt" && php -d memory_limit=4G tools/vendor/bin/phpstan analyse -c tools/phpstan/release-check.neon \
        --error-format=json --no-progress "${list[@]}" > "$json" 2> "$json.err")
    jq -e '.files' "$json" >/dev/null 2>&1
}

WT_BASE=$(prepare_side "$BASE") || skip "không tạo được worktree cho $BASE"
WTS+=("$WT_BASE")
WT_HEAD=$(prepare_side "$HEAD_REV") || skip "không tạo được worktree cho $HEAD_REV"
WTS+=("$WT_HEAD")

# One side at a time with capped workers: two parallel runs over a large ticket (1,400+ files) exhausted RAM
analyse_side "$WT_BASE" /tmp/release-phpstan-base.json \
    || skip "PHPStan lỗi khi chạy bản trước ($(head -c 300 /tmp/release-phpstan-base.json.err 2>/dev/null))"
analyse_side "$WT_HEAD" /tmp/release-phpstan-head.json \
    || skip "PHPStan lỗi khi chạy bản sau ($(head -c 300 /tmp/release-phpstan-head.json.err 2>/dev/null))"

# file → MR map from the pick result (files each picked MR changed on vtiger)
MAP="[]"
if [ -n "$RESULT" ] && [ -f "$RESULT" ]; then
    while IFS=$'\t' read -r iid sha; do
        [ -n "$sha" ] || continue
        MAP=$(jq --arg iid "$iid" --arg files "$(git -C "$TARGET" diff --name-only "$sha^1" "$sha" | tr '\n' ',')" \
            '. + [{iid: ($iid|tonumber), files: ($files | split(",") | map(select(length > 0)))}]' <<< "$MAP")
    done < <(jq -r '.picked[] | select(.status == "picked") | "\(.iid)\t\(.sha)"' "$RESULT")
fi

# Relative path + message identify an error (line numbers shift between the two sides)
jq -n --slurpfile b /tmp/release-phpstan-base.json --slurpfile h /tmp/release-phpstan-head.json \
    --argjson map "$MAP" --arg ids "$IDS" '
def errs(doc): [doc.files | to_entries[] | (.key | sub("^.*?/release-phpstan\\.[^/]+/"; "")) as $f
    | .value.messages[] | select((.identifier // "") | test($ids)) | {file: $f, line, message, identifier}];
(errs($b[0]) | map("\(.file)|\(.message)")) as $before
| {errors: ([errs($h[0])[] | select(("\(.file)|\(.message)") as $k | $before | index($k) | not)]
        + [($h[0].errors // [])[] | {file: "(phpstan)", line: 0, message: ., identifier: "phpstan.internal"}])
    | map(. + {mr: (.file as $f | ([$map[] | select(.files | index($f)) | .iid] | last))})}' > "$OUT"

N=$(jq '.errors | length' "$OUT")
echo "PHPStan: $N lỗi mới sau khi pick (thiếu symbol / sai số tham số / lỗi cú pháp) → $OUT"

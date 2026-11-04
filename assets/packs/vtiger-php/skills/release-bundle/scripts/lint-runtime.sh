#!/usr/bin/env bash
# lint-runtime.sh <checkout> <base> <head> <out.txt> — parse every runtime PHP file changed in <base>..<head>
# with the PHP version release core runs on (default image php:8.3-cli, override RELEASE_PHP_LINT_IMAGE or
# RELEASE_PHP_LINT_BIN). Release core runs PHP 8.3 from the 10.2026 release (#27245); a branch for an older
# release server lints with php:7.2-cli. <out.txt>: one "file: error" line per failure; first line
# "SKIPPED: <why>" when no runtime is available. Exit 0 always; callers read the file.

set -uo pipefail
DIR="$(cd "$1" && pwd)" BASE="$2" HEAD_REV="$3" OUT="$4"
IMAGE="${RELEASE_PHP_LINT_IMAGE:-php:8.3-cli}"

FILES=$(git -C "$DIR" diff --name-only --diff-filter=AM "$BASE" "$HEAD_REV" -- '*.php' '*.inc' '*.service' \
    | grep -vE '^(agents|docs|tools|\.omp|\.claude)/|/tests/' || true)
: > "$OUT"
[ -n "$FILES" ] || exit 0
LINT_LOOP='while IFS= read -r f; do out=$("$PHP" -n -d display_errors=stderr -l "$f" 2>&1) || echo "$f: $(echo "$out" | grep -m1 -E "error" | sed "s/ in [^ ]* on line / line /")"; done; exit 0'

# A local binary of the release PHP version (RELEASE_PHP_LINT_BIN, e.g. /opt/homebrew/opt/php@8.3/bin/php) skips docker
if [ -n "${RELEASE_PHP_LINT_BIN:-}" ]; then
    [ -x "$RELEASE_PHP_LINT_BIN" ] || { echo "SKIPPED: RELEASE_PHP_LINT_BIN=$RELEASE_PHP_LINT_BIN không chạy được" > "$OUT"; exit 0; }
    RESULT=$(cd "$DIR" && printf '%s\n' "$FILES" | PHP="$RELEASE_PHP_LINT_BIN" sh -c "$LINT_LOOP" 2>&1)
    [ -z "$RESULT" ] || printf '%s\n' "$RESULT" >> "$OUT"
    exit 0
fi
if ! command -v docker >/dev/null 2>&1 || ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "SKIPPED: không có docker image $IMAGE (docker pull $IMAGE, hoặc RELEASE_PHP_LINT_BIN)" > "$OUT"
    exit 0
fi

# The checkout must be at <head>: files are read from the working tree
# The loop always exits 0, so a non-zero status means docker itself failed: record it as SKIPPED
RESULT=$(printf '%s\n' "$FILES" | docker run --rm -i -e PHP=php -v "$DIR":/app:ro -w /app "$IMAGE" sh -c "$LINT_LOOP" 2>&1)
RC=$?
if [ "$RC" -ne 0 ]; then
    printf 'SKIPPED: docker run lỗi (rc=%s): %s\n' "$RC" "$(printf '%s' "$RESULT" | head -1)" > "$OUT"
    exit 0
fi
[ -z "$RESULT" ] || printf '%s\n' "$RESULT" >> "$OUT"

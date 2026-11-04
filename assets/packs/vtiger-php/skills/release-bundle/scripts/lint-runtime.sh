#!/usr/bin/env bash
# lint-runtime.sh <checkout> <base> <head> <out.txt> — parse every runtime PHP file changed in <base>..<head>
# with the PHP version release core runs on (default image php:7.2-cli, override RELEASE_PHP_LINT_IMAGE).
# Master moved to PHP 8.3 syntax; release servers still run 7.2/7.3, where typed properties, `fn() =>`,
# `??=` and array spread are parse errors. <out.txt>: one "file: error" line per failure; first line
# "SKIPPED: <why>" when no runtime is available. Exit 0 always; callers read the file.

set -uo pipefail
DIR="$(cd "$1" && pwd)" BASE="$2" HEAD_REV="$3" OUT="$4"
IMAGE="${RELEASE_PHP_LINT_IMAGE:-php:7.2-cli}"

FILES=$(git -C "$DIR" diff --name-only --diff-filter=AM "$BASE" "$HEAD_REV" -- '*.php' '*.inc' '*.service' \
    | grep -vE '^(agents|docs|tools|\.omp|\.claude)/|/tests/' || true)
: > "$OUT"
[ -n "$FILES" ] || exit 0
if ! command -v docker >/dev/null 2>&1 || ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo "SKIPPED: không có docker image $IMAGE (docker pull $IMAGE)" > "$OUT"
    exit 0
fi

# The checkout must be at <head>: files are read from the working tree
printf '%s\n' "$FILES" | docker run --rm -i -v "$DIR":/app:ro -w /app "$IMAGE" sh -c \
    'while IFS= read -r f; do out=$(php -d display_errors=stderr -l "$f" 2>&1) || echo "$f: $(echo "$out" | grep -m1 -E "error" | sed "s/ in [^ ]* on line / line /")"; done' \
    >> "$OUT"

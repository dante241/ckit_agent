#!/usr/bin/env bash
# run.sh — Orchestrator for /release-bundle: pick (pick.sh) → PHPStan missing-symbol check
# (phpstan-check.sh) → GitLab labels/milestone (gitlab-update.sh) → report (report.sh).
#
# Usage:
#   run.sh                          # pick every ready ticket onto release-core-<this month>
#   run.sh --month 10.2026          # release month (branch release-core-10.2026, milestone tháng 10/2026)
#   run.sh --preview                # simulate on a throwaway worktree: no branch/GitLab change
#   run.sh --handoff                # ticket in conflict → push pick/<ticket> + MR assigned to its author
#   run.sh bo qua MR 235            # skip the ticket of MR 235 (Vietnamese)
#   run.sh skip 235,242             # skip the tickets of several MRs
#   run.sh --no-phpstan             # skip the PHPStan check
#   run.sh --no-gitlab              # pick locally but leave GitLab labels/milestone untouched
#   run.sh --pause-on-conflict      # legacy: stop at the first conflict, then resume.sh
#
# Env overrides: see lib.sh (RELEASE_TARGET, RELEASE_STATE_DIR, RELEASE_CLASSIFIED).

set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

ARGS="$*"
PICK_ARGS=()
PREVIEW=0
PHPSTAN=1
GITLAB=1

while [ $# -gt 0 ]; do
    case "$1" in
        --month) export RELEASE_MONTH="${2:-}"; shift 2 ;;
        --month=*) export RELEASE_MONTH="${1#--month=}"; shift ;;
        --preview|--dry-run) PREVIEW=1; PICK_ARGS+=(--preview); shift ;;
        --handoff) PICK_ARGS+=(--handoff); shift ;;
        --no-phpstan) PHPSTAN=0; shift ;;
        --no-gitlab) GITLAB=0; shift ;;
        --pause-on-conflict) PICK_ARGS+=(--pause); shift ;;
        --skip) shift 2 ;;
        --*) echo "ABORT: flag không hợp lệ '$1'" >&2; exit 2 ;;
        *) shift ;;
    esac
done

if echo "$ARGS" | grep -qiE "(bo qua MR|--skip|skip)[[:space:]]+[0-9]"; then
    SKIP_CSV=$(echo "$ARGS" | sed -E 's/.*((bo qua MR|--skip|skip)[[:space:]]+)//I' | grep -oE '[0-9]+' | tr '\n' ',' | sed 's/,$//')
    echo "→ User skip IIDs: $SKIP_CSV"
    PICK_ARGS+=(--skip "$SKIP_CSV")
fi

source ./lib.sh

echo "=== Phase 1: cherry-pick theo ticket$([ "$PREVIEW" -eq 1 ] && echo ' (PREVIEW)') ==="
./pick.sh "${PICK_ARGS[@]+"${PICK_ARGS[@]}"}"
PICK_RC=$?
if [ $PICK_RC -ne 0 ]; then
    [[ " ${PICK_ARGS[*]-} " == *" --pause "* ]] && echo "" && echo "Paused on conflict — resolve thủ công, rồi chạy resume.sh"
    exit $PICK_RC
fi

if [ "$PREVIEW" -eq 1 ]; then
    [ -f "$PREVIEW_RESULT" ] || exit 0
    ./report.sh "$PREVIEW_RESULT" > "$PREVIEW_REPORT"
    echo ""
    echo "Preview report: $PREVIEW_REPORT"
    exit 0
fi

[ -f "$BUNDLE_RESULT" ] || exit 0

PHPSTAN_OUT="$STATE_DIR/phpstan-$TODAY.json"
echo ""
echo "=== Phase 2: PHPStan — function/method/class thiếu sau khi pick ==="
if [ "$PHPSTAN" -eq 1 ] && [ "$(jq '[.picked[] | select(.status == "picked")] | length' "$BUNDLE_RESULT")" -gt 0 ]; then
    ./phpstan-check.sh "$(jq -r '.base_sha' "$BUNDLE_RESULT")" "$(jq -r '.head_sha' "$BUNDLE_RESULT")" "$PHPSTAN_OUT" "$BUNDLE_RESULT"
else
    jq -n --arg why "$([ "$PHPSTAN" -eq 0 ] && echo 'tắt bằng --no-phpstan' || echo 'không có commit mới')" '{errors: [], skipped: $why}' > "$PHPSTAN_OUT"
    echo "PHPStan: bỏ qua"
fi

LINT_OUT="$STATE_DIR/lint-$TODAY.txt"
echo ""
echo "=== Phase 2b: cú pháp trên PHP của release (${RELEASE_PHP_LINT_BIN:-${RELEASE_PHP_LINT_IMAGE:-php:8.3-cli}}) ==="
./lint-runtime.sh "$TARGET" "$(jq -r '.base_sha' "$BUNDLE_RESULT")" "$(jq -r '.head_sha' "$BUNDLE_RESULT")" "$LINT_OUT"
echo "Lỗi cú pháp: $(grep -vc '^SKIPPED:' "$LINT_OUT" 2>/dev/null || echo 0) file$(grep -q '^SKIPPED:' "$LINT_OUT" && echo " — $(head -1 "$LINT_OUT")")"

echo ""
echo "=== Phase 3: GitLab label/milestone ==="
if [ "$GITLAB" -eq 1 ]; then
    ./gitlab-update.sh
else
    echo "Bỏ qua (--no-gitlab) — sau này chạy ./gitlab-update.sh để gắn label"
fi
./report.sh "$BUNDLE_RESULT" "$PHPSTAN_OUT" "$LINT_OUT" > "$BUNDLE_REPORT"

cd "$TARGET"
cat <<EOF

=== Release Bundle — $TODAY (local only, NOT pushed) ===
Branch:  $BRANCH @ $TARGET ($(git log --oneline "$RELEASE_BASE"..HEAD | wc -l | tr -d ' ') commit ahead $RELEASE_BASE)
Ticket pick được: $(jq '[.picked[].ticket] | unique | length' "$BUNDLE_RESULT") | conflict: $(jq '.conflicts_manual | length' "$BUNDLE_RESULT") | chờ dev: $(jq '.pending_handoff | length' "$BUNDLE_RESULT") | PHPStan lỗi mới: $(jq '.errors | length' "$PHPSTAN_OUT")
Report (kèm danh sách test trên release): $BUNDLE_REPORT

Next steps:
  1. Đọc report: sửa lỗi PHPStan, xử lý ticket conflict (hướng dẫn trong report)
  2. Deploy branch lên môi trường test release, test theo danh sách
  3. cd $TARGET && git push -u origin $BRANCH
     glab mr create --repo $RELEASE_PROJECT --source-branch $BRANCH --target-branch $RELEASE_MAIN --title "[Release core $MONTH/$YEAR]"
$([ "$LAYOUT" = "single" ] && echo "  4. MR merge xong + test: gắn tag version trên $RELEASE_MAIN, rồi ./sync-release.sh <tag> để đẩy sang vtiger_release")
EOF

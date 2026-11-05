#!/usr/bin/env bash
# run.sh <ref> [--keep-db] [--skip <step,...>] — full PHP 8.3 smoke of a release ref on the local box.
# Steps: setup → db (fresh clone + migrations + QuickRepair) → phpstan → crawl → walk → export → api → report.
# Output: agents/release-queue/smoke-<date>-<sha>/ (json per step + report.md with every PHP/nginx error grouped).
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
REF="${1:?usage: run.sh <ref> [--keep-db] [--skip step,...]}"; shift
KEEP_DB=0; SKIP=","
while [ $# -gt 0 ]; do
    case "$1" in
        --keep-db) KEEP_DB=1 ;;
        --skip) SKIP=",$2,"; shift ;;
        *) echo "ABORT: tham số lạ $1" >&2; exit 2 ;;
    esac
    shift
done
skip() { [[ "$SKIP" == *",$1,"* ]]; }
SHA=$(git -C "$VT" rev-parse --short "$REF") || exit 2
OUT="$VT/agents/release-queue/smoke-$(date +%F)-$SHA$([ "$SOURCE" = beta ] && echo -beta)"
REPORT="$OUT/report.md"
[ "$SKIP" = "," ] && [ "$KEEP_DB" = 0 ] || REPORT="$OUT/report-partial-$(date +%H%M).md"    # keep the full run's report
mkdir -p "$OUT"
LOG0=$(wc -c < "$PHP_LOG"); NGX0=$(wc -c < "$NGINX_LOG" 2>/dev/null || echo 0)
step() { echo; echo "=== $1"; }

step "setup $REF"; bash "$HERE/setup.sh" "$REF" 2>&1 | tee "$OUT/setup.txt"; [ "${PIPESTATUS[0]}" = 0 ] || exit 1
if [ "$KEEP_DB" = 0 ] && ! skip db; then
    step "db"; bash "$HERE/db.sh" 2>&1 | tee "$OUT/db.txt"; [ "${PIPESTATUS[0]}" = 0 ] || { echo "ABORT: bước db lỗi — không test trên DB chưa dựng xong" >&2; exit 1; }
    code=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/index.php")
    [ "$code" = 200 ] || { echo "ABORT: sau db.sh trang login trả $code (xem $PHP_LOG)" >&2; exit 1; }
else
    # checkout -f restored the tracked tabdata.php/parent_tabdata.php and master's user_privileges were copied:
    # regenerate them for the clone, otherwise most modules answer "Không có quyền"
    step "QuickRepair (giữ DB)"; (cd "$WT" && "$PHP" -d memory_limit=4G "$HERE/quick_repair.php" "$WT") 2>&1 | tee "$OUT/quickrepair-$(date +%H%M).txt"    # db.txt keeps the migration run
    redis-cli -n "$REDIS_DB" flushdb >/dev/null
fi
skip phpstan || { step "phpstan"; bash "$HERE/phpstan.sh" "$REF" "$OUT" 2>&1 | tee "$OUT/phpstan.txt"; }
cd "$HERE" || exit 2
for s in crawl walk export api; do
    skip "$s" && continue
    if [ "$SOURCE" = beta ] && [ "$(docker inspect -f '{{.State.Running}}' "$BETA_CONTAINER" 2>/dev/null)" != true ]; then
        echo "ABORT: container $BETA_CONTAINER không chạy (Docker Desktop dừng?) — docker start $BETA_CONTAINER rồi chạy lại từ bước $s" >&2; exit 1
    fi
    step "$s"; python3 "$HERE/$s.py" "$OUT" 2>&1 | grep -v -i 'RequestsDependencyWarning\|warnings.warn' | tee "$OUT/$s.txt"
done
step "report"
python3 "$HERE/report_log.py" "$PHP_LOG" "$LOG0" "$NGINX_LOG" "$NGX0" "$REPORT" >/dev/null
{
    echo; echo "## Tóm tắt từng bước"; echo
    for s in setup db phpstan crawl walk export api; do
        [ -f "$OUT/$s.txt" ] && { echo "### $s"; echo '```'; grep -vE '^\s*$' "$OUT/$s.txt" | tail -25; echo '```'; }
    done
} >> "$REPORT"
echo "report: $REPORT"
sed -n '1,4p' "$REPORT"
grep -E '^## \[' "$REPORT" | head -30 || true

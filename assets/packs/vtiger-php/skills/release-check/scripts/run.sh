#!/usr/bin/env bash
# run.sh — Orchestrate release-check pipeline
#
# Usage:
#   run.sh [--dry-run] [--since YYYY-MM-DD] [--limit N] [--output-dir DIR]
#
# Outputs:
#   <OUTPUT_DIR>/mrs-slim-<DATE>.json
#   <OUTPUT_DIR>/status-<DATE>.tsv
#   <OUTPUT_DIR>/classified-<DATE>.json
#   <OUTPUT_DIR>/report-<DATE>.md

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUTPUT_DIR=".claude/release-queue"
DRY_RUN=0
SINCE=""
LIMIT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run|--skip-label) DRY_RUN=1; shift ;;
        --since) SINCE="$2"; shift 2 ;;
        --limit) LIMIT="$2"; shift 2 ;;
        --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
        *) echo "Unknown flag: $1" >&2; exit 2 ;;
    esac
done

mkdir -p "$OUTPUT_DIR"
TODAY=$(date +%Y-%m-%d)

# Check env vars
for v in PMS_BASE_URL PMS_USERNAME PMS_PASSWORD PMS_ACCESSKEY; do
    if [[ -z "${!v:-}" ]]; then
        echo "ERROR: Missing env var $v" >&2
        echo "Source them from PMS MCP: eval \"\$($SCRIPT_DIR/source-pms-env.sh)\"" >&2
        exit 2
    fi
done

# ── Step 1: Scan merged MRs ───────────────────────────────────
SCAN_ARGS=("$OUTPUT_DIR")
[[ -n "$SINCE" ]] && SCAN_ARGS=(--since "$SINCE" "${SCAN_ARGS[@]}")
[[ -n "$LIMIT" ]] && SCAN_ARGS=(--limit "$LIMIT" "${SCAN_ARGS[@]}")
SLIM_JSON=$(bash "$SCRIPT_DIR/scan-mrs.sh" "${SCAN_ARGS[@]}" | tail -1)

# ── Step 2: Extract unique ticket IDs ─────────────────────────
IDS_FILE="$OUTPUT_DIR/ticket-ids-$TODAY.txt"
jq -r '.[].title' "$SLIM_JSON" | grep -oE '#[0-9]{3,}' | tr -d '#' | sort -u > "$IDS_FILE"
N_IDS=$(wc -l < "$IDS_FILE" | tr -d ' ')
echo "Unique tickets: $N_IDS → $IDS_FILE" >&2

# ── Step 3: Batch fetch PMS statuses ──────────────────────────
STATUS_TSV="$OUTPUT_DIR/status-$TODAY.tsv"
node "$SCRIPT_DIR/pms-status-batch.js" < "$IDS_FILE" > "$STATUS_TSV"

# ── Step 4: Classify + render report ──────────────────────────
python3 "$SCRIPT_DIR/classify.py" "$SLIM_JSON" "$STATUS_TSV" "$OUTPUT_DIR"

# ── Step 5: Label transitions ─────────────────────────────────
CLASSIFIED="$OUTPUT_DIR/classified-$TODAY.json"
if [[ "$DRY_RUN" -eq 1 ]]; then
    echo "" >&2
    echo "[dry-run] Skipping label update" >&2
else
    echo "" >&2
    echo "Transitioning ready MRs: +Chờ release / -Dev done..." >&2
    jq -r '.ready[] | select((.labels | index("Chờ release")) | not) | .iid' "$CLASSIFIED" | while read -r iid; do
        [[ -z "$iid" ]] && continue
        echo "  !$iid" >&2
        glab mr update "$iid" --label "Chờ release" --unlabel "Dev done" >/dev/null 2>&1 || echo "    FAILED to transition !$iid" >&2
    done

    REGRESSED_COUNT=$(jq '.regressed | length' "$CLASSIFIED")
    if [[ "$REGRESSED_COUNT" -gt 0 ]]; then
        echo "" >&2
        echo "⚠️  Rolling back regressed MRs: +Dev done / -Chờ release..." >&2
        jq -r '.regressed[] | "\(.iid)\t\(.ticket_id)\t\(.ticket_status)"' "$CLASSIFIED" | while IFS=$'\t' read -r iid ticket status; do
            [[ -z "$iid" ]] && continue
            echo "  !$iid (ticket #$ticket now $status)" >&2
            glab mr update "$iid" --label "Dev done" --unlabel "Chờ release" >/dev/null 2>&1 || echo "    FAILED to rollback !$iid" >&2
        done
    fi
fi

echo "" >&2
echo "Done. Next: /release-bundle" >&2

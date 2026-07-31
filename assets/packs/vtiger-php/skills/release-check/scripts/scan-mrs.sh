#!/usr/bin/env bash
# scan-mrs.sh — Dump merged MRs with label "Dev done" OR "Chờ release" to slim JSON
#
# Usage:
#   scan-mrs.sh [--since YYYY-MM-DD] [--limit N] [OUTPUT_DIR]
#
# Scans both labels so MRs previously moved to `Chờ release` get re-verified
# against PMS ticket status — catches the case where a ticket was reopened
# after an earlier /release-check run.
#
# Output: <OUTPUT_DIR>/mrs-slim-<DATE>.json
#   Each row: {iid, title, author, merged_at, web_url, labels, ...}
#   Deduped by iid (an MR matching both labels appears once).

set -euo pipefail

OUTPUT_DIR="${1:-.claude/release-queue}"
SINCE=""
LIMIT=""

# Parse flags
while [[ $# -gt 0 ]]; do
    case "$1" in
        --since) SINCE="$2"; shift 2 ;;
        --limit) LIMIT="$2"; shift 2 ;;
        *) OUTPUT_DIR="$1"; shift ;;
    esac
done

mkdir -p "$OUTPUT_DIR"
TODAY=$(date +%Y-%m-%d)
RAW_DEV="$OUTPUT_DIR/merged-mrs-devdone-$TODAY.json"
RAW_PEND="$OUTPUT_DIR/merged-mrs-chorelease-$TODAY.json"
RAW="$OUTPUT_DIR/merged-mrs-$TODAY.json"
SLIM="$OUTPUT_DIR/mrs-slim-$TODAY.json"

echo "Scanning merged MRs (target=master) with label 'Dev done' or 'Chờ release'..." >&2
glab mr list --merged --label "Dev done" --target-branch master --per-page 100 --output json > "$RAW_DEV"
glab mr list --merged --label "Chờ release" --target-branch master --per-page 100 --output json > "$RAW_PEND"

# Merge + dedupe by iid (keep first occurrence)
jq -s '(.[0] + .[1]) | unique_by(.iid)' "$RAW_DEV" "$RAW_PEND" > "$RAW"

# Slim down + optional filters
# Include merge_commit_sha / squash_commit_sha / sha so /release-bundle can cherry-pick
# without an extra per-MR API round-trip.
JQ_FIELDS='{iid, title, author: .author.username, merged_at, web_url, labels, target_branch, merge_commit_sha, squash_commit_sha, sha}'
JQ_FILTER="[.[] | ${JQ_FIELDS}]"
if [[ -n "$SINCE" ]]; then
    JQ_FILTER="[.[] | select(.merged_at >= \"${SINCE}\") | ${JQ_FIELDS}]"
fi
jq "$JQ_FILTER" "$RAW" > "$SLIM"

# When --limit applies, keep the NEWEST N MRs (sort desc, take top N) so the
# most recent work is never dropped. Then always sort ascending by merged_at
# for output, so downstream consumers (report, /release-bundle) process
# oldest-first — this keeps cherry-pick order aligned with commit history
# and surfaces long-pending MRs as higher priority.
if [[ -n "$LIMIT" ]]; then
    jq "sort_by(.merged_at) | reverse | .[0:${LIMIT}] | sort_by(.merged_at)" "$SLIM" > "$SLIM.tmp" && mv "$SLIM.tmp" "$SLIM"
else
    jq "sort_by(.merged_at)" "$SLIM" > "$SLIM.tmp" && mv "$SLIM.tmp" "$SLIM"
fi

COUNT=$(jq 'length' "$SLIM")
echo "Scanned: $COUNT MRs → $SLIM" >&2
echo "$SLIM"

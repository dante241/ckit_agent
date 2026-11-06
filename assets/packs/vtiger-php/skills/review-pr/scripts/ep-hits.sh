#!/usr/bin/env bash
# Match a diff against the Trigger lines of rule://error-patterns and print only the matching EP entries ($EP_HITS).
# Usage (from the repo root): git diff ... | bash <skill dir>/scripts/ep-hits.sh [-]   or   ep-hits.sh <diff-file>
#   Catalog: the project's .omp/rules/error-patterns.md if present (it overrides the pack), else the pack copy.
# Needles = every `backticked` string on an entry's Trigger line, matched as fixed strings against the added
# lines and the "+++ b/<path>" headers of the diff. Output: matched entries + "Matched:" needles, then
# "EP_HITS: <n> (<ids>)"; "EP_HITS: 0" when nothing matches. Exit 0, or 2 on setup error.
set -uo pipefail

IN="${1:--}"
SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
CATALOG="$ROOT/.omp/rules/error-patterns.md"
[ -f "$CATALOG" ] || CATALOG="$SELF_DIR/../../../rules/error-patterns.md"
[ -f "$CATALOG" ] || { echo "SETUP ERROR: error-patterns.md not found" >&2; exit 2; }

ADDED="$(mktemp)"
trap 'rm -f "$ADDED"' EXIT
if [ "$IN" = "-" ]; then cat; else cat "$IN"; fi | grep -E '^\+' >"$ADDED"

awk -v added="$ADDED" '
function flush(   i, n, parts, hit, needles) {
	if (id == "") return
	n = split(trigger, parts, "`")
	hit = ""
	for (i = 2; i <= n; i += 2) {
		if (length(parts[i]) < 3) continue
		if (index(text, parts[i]) > 0) hit = hit (hit == "" ? "" : " · ") parts[i]
	}
	if (hit != "") {
		printf "%s\nMatched: %s\n\n", block, hit
		ids = ids (ids == "" ? "" : ", ") id
		count++
	}
}
BEGIN {
	while ((getline line < added) > 0) text = text "\n" line
	close(added)
}
/^## EP-[0-9]+/ { flush(); id = $2; block = $0; trigger = ""; next }
/^## / || /^---$/ { flush(); id = ""; next }
id != "" {
	if ($0 ~ /^Trigger:/) trigger = $0
	if ($0 != "") block = block "\n" $0
}
END {
	flush()
	if (count) printf "EP_HITS: %d (%s)\n", count, ids
	else print "EP_HITS: 0"
}
' "$CATALOG"

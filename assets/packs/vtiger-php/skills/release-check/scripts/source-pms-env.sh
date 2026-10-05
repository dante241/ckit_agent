#!/usr/bin/env bash
# source-pms-env.sh — Extract PMS_* env vars from the omp MCP config for shell export.
#
# Usage:
#   eval "$(bash .omp/skills/release-check/scripts/source-pms-env.sh)"
#
# Credentials stay in the omp MCP server config (never hardcoded here).
# Searches project + user omp config locations for a `pms` stdio server and
# emits its env block as `export KEY=value` lines.

set -euo pipefail

# Candidate omp MCP config files: project first, then default profile, then
# any named profile under ~/.omp/profiles/<name>/agent/mcp.json.
CANDIDATES=(
    "$PWD/.omp/mcp.json"
    "$HOME/.omp/agent/mcp.json"
)
for f in "$HOME"/.omp/profiles/*/agent/mcp.json; do
    [[ -f "$f" ]] && CANDIDATES+=("$f")
done

for cfg in "${CANDIDATES[@]}"; do
    [[ -f "$cfg" ]] || continue
    out=$(jq -r '
        (.mcpServers.pms.env // empty)
        | to_entries[]
        | select(.key | startswith("PMS_"))
        | "export \(.key)=\(.value | @sh)"
    ' "$cfg" 2>/dev/null || true)
    if [[ -n "$out" ]]; then
        echo "$out"
        exit 0
    fi
done

echo "ERROR: no 'pms' MCP server with PMS_* env found in omp config" >&2
echo "Checked: ${CANDIDATES[*]}" >&2
exit 1

#!/usr/bin/env bash
# source-pms-env.sh — Extract PMS env vars from `claude mcp get pms` for shell export.
#
# Usage:
#   eval "$(bash .omp/skills/release-check/scripts/source-pms-env.sh)"
#
# This avoids hardcoding credentials — they stay in the MCP server config.

set -euo pipefail

claude mcp get pms 2>/dev/null | awk '
    /^[[:space:]]+PMS_(BASE_URL|USERNAME|PASSWORD|ACCESSKEY)=/ {
        # Strip leading whitespace, then emit as export
        sub(/^[[:space:]]+/, "")
        print "export " $0
    }
'

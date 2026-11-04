#!/usr/bin/env bash
# audit-month.sh [branch] — "was any code lost" audit of the whole month branch against origin/release.
# Run after verify-resolves.sh, before deploying to the release test env. Review aid, not a gate:
# read every line it prints (audit_month.py explains the three checks).
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./lib.sh

BR="${1:-$BRANCH}"
git -C "$TARGET" rev-parse -q --verify "$BR" >/dev/null || { echo "ABORT: không thấy branch '$BR' trong $TARGET" >&2; exit 2; }
python3 ./audit_month.py "$TARGET" origin/release "$BR"

#!/usr/bin/env bash
# order-check.sh [branch] — files where picks went against master's merge order: rebuild each file by
# applying the batch's MR patches in merged_at order and compare with the branch (order_check.py).
# Run after verify-resolves.sh / audit-month.sh, before deploying to the release test env.
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
source ./lib.sh

BR="${1:-$BRANCH}"
git -C "$TARGET" rev-parse -q --verify "$BR" >/dev/null || { echo "ABORT: không thấy branch '$BR' trong $TARGET" >&2; exit 2; }
shopt -s nullglob
SLIMS=("$STATE_DIR"/mrs-slim-*.json)
[ "${#SLIMS[@]}" -gt 0 ] || { echo "ABORT: không có mrs-slim-*.json trong $STATE_DIR (chạy /release-check)" >&2; exit 2; }
python3 ./order_check.py "$TARGET" origin/release "$BR" "${SLIMS[@]}"

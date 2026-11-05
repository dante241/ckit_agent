#!/usr/bin/env bash
# lib.sh — shared settings for release-smoke. Every value can be overridden from the environment.
# Run the scripts from the vtiger checkout (master); it supplies the untracked local files the
# smoke site borrows: config.env.php, license.lic, user_privileges/*, config.csrf-secret.php, .omp/mcp.json.

set -uo pipefail

VT="${SMOKE_VTIGER:-$(git rev-parse --show-toplevel 2>/dev/null)}"
[ -f "$VT/config.inc.php" ] || { echo "ABORT: chạy từ checkout vtiger (không thấy config.inc.php)" >&2; exit 2; }
SRC="$(dirname "$VT")"
WT="${SMOKE_WORKTREE:-$SRC/vtiger-relsmoke}"           # detached worktree served by the smoke vhost
DB="${SMOKE_DB:-vtiger_relsmoke}"                       # clone of the master dev DB
SRC_DB="${SMOKE_SRC_DB:-vtiger}"
REDIS_DB="${SMOKE_REDIS_DB:-5}"                         # cache keys are prefixed by SERVER_NAME: never share db 0 with master
PORT="${SMOKE_PORT:-8085}"
HOSTNAME_LICENSED="${SMOKE_LICENSED_HOST:-vtiger74.localhost}"   # host the local license.lic is issued for
PHP="${SMOKE_PHP:-/opt/homebrew/opt/php@8.3/bin/php}"
FPM_SOCK="${SMOKE_FPM_SOCK:-/opt/homebrew/var/run/php@8.3-fpm.sock}"
PHP_LOG="${SMOKE_PHP_LOG:-/opt/homebrew/var/log/php83-vtiger-error.log}"
NGINX_DIR="${SMOKE_NGINX_DIR:-/opt/homebrew/etc/nginx}"
NGINX_BIN="${SMOKE_NGINX_BIN:-/opt/homebrew/opt/nginx/bin/nginx}"
NGINX_LOG="${SMOKE_NGINX_LOG:-/opt/homebrew/var/log/nginx/vtiger-release-error.log}"
ADMIN_PASS="${SMOKE_ADMIN_PASS:-Relwalk#2026}"         # set on admin (id 1) in the CLONE only
BASE="http://$HOSTNAME_LICENSED:$PORT"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA="$HERE/../data"

mysql_pass() {
    python3 -c "import json;print(json.load(open('$VT/.omp/mcp.json'))['mcpServers']['mcp_server_mysql']['env']['MYSQL_PASS'])"
}
export MYSQL_PWD="${MYSQL_PWD:-$(mysql_pass)}"
MYSQL=(mysql -h127.0.0.1 -uroot --default-character-set=utf8mb4)

export VT WT DB REDIS_DB PORT PHP PHP_LOG BASE ADMIN_PASS DATA

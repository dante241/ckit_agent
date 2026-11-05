#!/usr/bin/env bash
# setup.sh <ref> — check out <ref> (month branch or commit) in the smoke worktree and serve it on PHP 8.3.
# Local files come from the master checkout and are marked skip-worktree so they can never be committed.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
REF="${1:?usage: setup.sh <ref>}"
LOCAL_FILES=(config.env.php config_override.php)

git -C "$VT" rev-parse -q --verify "$REF^{commit}" >/dev/null || { echo "ABORT: không thấy ref $REF" >&2; exit 2; }
if [ ! -e "$WT/.git" ]; then
    git -C "$VT" worktree add -q --detach "$WT" "$REF" || exit 2
else
    git -C "$WT" update-index --no-skip-worktree "${LOCAL_FILES[@]}" 2>/dev/null
    git -C "$WT" checkout -q -f --detach "$REF" || exit 2
fi
cd "$WT" || exit 2

# Local config: master's DB credentials, this worktree's root and DB, Redis cache db REDIS_DB
sed -e "s#\$root_directory = '[^']*'#\$root_directory = '$WT/'#" \
    -e "s#\$dbconfig\['db_name'\] = '[^']*'#\$dbconfig['db_name'] = '$DB'#" "$VT/config.env.php" > config.env.php
python3 - "$REDIS_DB" <<'EOF'
import re, sys
p = 'config_override.php'
s = open(p, newline='').read()
i = s.find('$redisCacheConfig = [')
if i < 0:
    sys.exit('ABORT: config_override.php không có $redisCacheConfig')
j = s.find('];', i)
block = re.sub(r"'database' => \d+", f"'database' => {sys.argv[1]}", s[i:j])
open(p, 'w', newline='').write(s[:i] + block + s[j:])
EOF
git update-index --skip-worktree "${LOCAL_FILES[@]}"
cp -p "$VT/license.lic" .
cp -p "$VT"/user_privileges/*_[0-9]*.php user_privileges/ 2>/dev/null
[ -f config.csrf-secret.php ] || cp -p "$VT/config.csrf-secret.php" .
mkdir -p cache
rm -rf cache/templates_c
grep -q "db_name'\] = '$DB'" config.env.php || { echo "ABORT: config.env.php không trỏ DB $DB" >&2; exit 2; }

# nginx vhost on $PORT; HTTP_HOST is rewritten to the licensed host so redirects stay on this port
SNIP="$NGINX_DIR/snippets/php8.3-fpm-relsmoke.conf"
sed -e "s#vtiger74.localhost:8083#$HOSTNAME_LICENSED:$PORT#" -e '/HTTP_REFERER\|HTTP_ORIGIN/d' \
    -e "s#fastcgi_pass .*#fastcgi_pass unix:$FPM_SOCK;#" "$NGINX_DIR/snippets/php8.3-fpm.conf" > "$SNIP"
cat > "$NGINX_DIR/domains/vtiger-release-local.conf" <<EOF
# release-smoke: $WT on PHP 8.3 (remove after the release)
server {
    listen $PORT;
    server_name vtiger-release.localhost;
    access_log /opt/homebrew/var/log/nginx/vtiger-release-access.log timed_combined;
    error_log  $NGINX_LOG warn;
    root $WT;
    index index.php;
    include snippets/vtiger_rules_83.conf;
    location / { try_files \$uri \$uri/ =404; }
    location ~ \.php\$ { include snippets/php8.3-fpm-relsmoke.conf; }
    location ~ /\. { deny all; }
}
EOF
"$NGINX_BIN" -t 2>&1 | tail -1 && "$NGINX_BIN" -s reload
redis-cli -n "$REDIS_DB" flushdb >/dev/null
sleep 1
code=$(curl -s -o /dev/null -w '%{http_code}' "$BASE/index.php")
echo "smoke site $BASE → $(git log -1 --format='%h %s' | cut -c1-80) | login page HTTP $code"
[ "$code" = 200 ] || { echo "ABORT: trang login không trả 200 (xem $PHP_LOG)" >&2; exit 1; }

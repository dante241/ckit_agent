#!/usr/bin/env bash
# beta.sh [--refetch] — load the vm02 beta DB (rnd_beta, a DB the release code already runs on) into a
# MariaDB container of the same version and settings as vm02, so migrations + QuickRepair run on a real
# release schema instead of the master dev DB. The server is only read: mariadb-dump --single-transaction.
# The dump is reused from .cache/beta/ for the day; --refetch takes a new one.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
[ "$SOURCE" = beta ] || { echo "ABORT: beta.sh cần SMOKE_SOURCE=beta" >&2; exit 2; }
DUMP="$BETA_CACHE/rnd_beta-$(date +%F).sql.gz"
SETTINGS="$BETA_CACHE/server-settings.tsv"

remote() {    # run a command in the beta webroot with the beta DB password in MYSQL_PWD (never printed)
    SSHPASS=$(python3 -c "import json;print(json.load(open('$VT/.omp/ssh.json'))['hosts']['$BETA_SSH_HOST']['password'])") \
        sshpass -e ssh -o LogLevel=ERROR "$(python3 -c "import json;h=json.load(open('$VT/.omp/ssh.json'))['hosts']['$BETA_SSH_HOST'];print(h['username']+'@'+h['host'])")" \
        "cd '$BETA_DIR' && export MYSQL_PWD=\$(php -r 'include \"config.env.php\"; echo \$dbconfig[\"db_password\"];' 2>/dev/null) \
         && DBU=\$(php -r 'include \"config.env.php\"; echo \$dbconfig[\"db_username\"];') && DBN=\$(php -r 'include \"config.env.php\"; echo \$dbconfig[\"db_name\"];') && $1"
}

if [ "${1:-}" = --refetch ] || [ ! -s "$SETTINGS" ]; then
    remote 'mysql -h127.0.0.1 -u"$DBU" -N -e "SELECT VERSION(), @@GLOBAL.sql_mode, @@character_set_server, @@collation_server, @@innodb_strict_mode"' > "$SETTINGS.part"         && mv "$SETTINGS.part" "$SETTINGS" || { echo "ABORT: không đọc được cấu hình MariaDB beta" >&2; exit 2; }
fi
if [ "${1:-}" = --refetch ] || [ ! -s "$DUMP" ]; then
    set -o pipefail
    remote 'mariadb-dump -h127.0.0.1 -u"$DBU" --single-transaction --quick --routines --triggers --default-character-set=utf8mb4 "$DBN" | gzip -1' > "$DUMP.part" \
        || { rm -f "$DUMP.part"; echo "ABORT: dump beta lỗi" >&2; exit 2; }
    gzcat "$DUMP.part" | tail -1 | grep -q 'Dump completed' || { echo "ABORT: dump beta không trọn" >&2; exit 2; }
    mv "$DUMP.part" "$DUMP"
    set +o pipefail
fi
col() { cut -f"$1" "$SETTINGS"; }    # cut keeps empty fields (an empty sql_mode is the usual case)
ver=$(col 1); mode=$(col 2); cs=$(col 3); coll=$(col 4); strict=$(col 5)
echo "beta: dump $(du -h "$DUMP" | cut -f1) | server $ver sql_mode='${mode}' $cs/$coll innodb_strict_mode=$strict"

docker info >/dev/null 2>&1 || { open -a Docker; for _ in $(seq 1 60); do docker info >/dev/null 2>&1 && break; sleep 3; done; }
docker info >/dev/null 2>&1 || { echo "ABORT: Docker không chạy" >&2; exit 2; }
docker rm -f "$BETA_CONTAINER" >/dev/null 2>&1
docker run -d --name "$BETA_CONTAINER" -p "127.0.0.1:$DB_PORT:3306" -e MARIADB_ROOT_PASSWORD="$MYSQL_PWD" -e MARIADB_ROOT_HOST=% \
    "$BETA_IMAGE" --character-set-server="$cs" --collation-server="$coll" --sql-mode="$mode" \
    --innodb-strict-mode="$strict" --max-allowed-packet=256M --innodb-buffer-pool-size=1G >/dev/null || exit 2
for _ in $(seq 1 60); do "${MYSQL[@]}" -N -e 'SELECT 1' >/dev/null 2>&1 && break; sleep 2; done    # root password exists only after init
"${MYSQL[@]}" -N -e 'SELECT 1' >/dev/null || { docker logs --tail 5 "$BETA_CONTAINER" >&2; echo "ABORT: MariaDB container không lên" >&2; exit 2; }
"${MYSQL[@]}" -e "CREATE DATABASE \`$DB\`" || exit 2
# The dump goes in unmodified, through the container's own MariaDB client (MySQL clients reject its sandbox line)
gzcat "$DUMP" | "${MYSQL[@]}" "$DB" \
    || { echo "ABORT: nạp dump beta lỗi" >&2; exit 2; }
n=$("${MYSQL[@]}" -N -e "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='$DB'")
want=$(gzcat "$DUMP" | grep -cE '^CREATE TABLE|^/\*!50001 CREATE VIEW|^/\*!50001 VIEW')
echo "beta → $BETA_CONTAINER:$DB_PORT/$DB: $n bảng/view (dump có $want CREATE), migration đã chạy: $("${MYSQL[@]}" -N -e "SELECT COUNT(*) FROM \`$DB\`.vtiger_migration_history WHERE success=1")"
[ "$n" -ge "$(gzcat "$DUMP" | grep -c '^CREATE TABLE')" ] || { echo "ABORT: thiếu bảng sau khi nạp" >&2; exit 2; }

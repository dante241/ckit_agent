#!/usr/bin/env bash
# db.sh — fresh clone of the master dev DB for the smoke site, aligned to the release code, then the
# deploy steps: CPMigration up + QuickRepair. Prints what the deleting migrations removed.
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
cd "$WT" || { echo "ABORT: chưa có worktree $WT (chạy setup.sh)" >&2; exit 2; }
grep -q "db_name'\] = '$DB'" config.env.php || { echo "ABORT: $WT/config.env.php không trỏ DB $DB — không đụng DB khác" >&2; exit 2; }
[ "$DB" != "$SRC_DB" ] || [ "$SOURCE" = beta ] || { echo "ABORT: DB clone trùng DB nguồn" >&2; exit 2; }
Q=("${MYSQL[@]}" "$DB" -N -e)

if [ "$SOURCE" = beta ]; then
    bash "$HERE/beta.sh" || exit 2
else
    "${MYSQL[@]}" -e "DROP DATABASE IF EXISTS \`$DB\`; CREATE DATABASE \`$DB\` CHARACTER SET utf8mb4" || exit 2
    set -o pipefail    # a failed dump must stop here, not leave an empty clone behind
    mysqldump -h127.0.0.1 -uroot --default-character-set=utf8mb4 --single-transaction --quick --routines --triggers --no-tablespaces "$SRC_DB" \
        | "${MYSQL[@]}" "$DB" || { echo "ABORT: clone $SRC_DB → $DB lỗi" >&2; exit 2; }
    set +o pipefail
    count() { "${Q[@]}" "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='$1'"; }
    n_src=$(count "$SRC_DB"); n_dst=$(count "$DB")
    [ "$n_dst" -ge "$n_src" ] || { echo "ABORT: clone thiếu bảng ($n_dst/$n_src)" >&2; exit 2; }
    echo "clone $SRC_DB → $DB: $n_dst bảng"
fi

# The master DB registers handlers/modules the release code does not ship: disable them in the clone,
# otherwise every save fatals on a missing handler file (an artifact of the clone, not a release bug)
ids=$("${Q[@]}" "SELECT eventhandler_id, handler_path FROM vtiger_eventhandlers WHERE is_active=1" \
    | while IFS=$'\t' read -r id p; do [ -f "$p" ] || printf '%s,' "$id"; done)
[ -n "$ids" ] && "${Q[@]}" "UPDATE vtiger_eventhandlers SET is_active=0 WHERE eventhandler_id IN (${ids%,})"
mods=$("${Q[@]}" "SELECT name FROM vtiger_tab WHERE presence IN (0,2)" \
    | while read -r m; do [ -d "modules/$m" ] || printf "'%s'," "$m"; done)
[ -n "$mods" ] && "${Q[@]}" "UPDATE vtiger_tab SET presence=1 WHERE name IN (${mods%,})"
echo "tắt handler không có file: ${ids%,}"
echo "ẩn module release không có: ${mods%,}"
hash=$("$PHP" -r 'echo crypt($argv[1], "\$1\$ad0000000");' "$ADMIN_PASS")
"${Q[@]}" "UPDATE vtiger_users SET user_password='$hash', crypt_type='PHP5.3MD5' WHERE id=1 AND user_name='admin'"
# The clone carries real contacts and a live SMTP account: saves must never mail, text or call out to real people.
# SMTP goes to a closed local port; workflows holding outbound tasks are switched off (the CPQueue worker is never started)
"${Q[@]}" "UPDATE vtiger_systems SET server = '127.0.0.1', server_port = 1025 WHERE server_type = 'email'"
wf=$("${Q[@]}" "SELECT GROUP_CONCAT(DISTINCT workflow_id) FROM com_vtiger_workflowtasks WHERE task REGEXP 'VTEmailTask|SMS|Zalo|ZNS|Webhook|Mautic|Notification|SendMail'")
[ -n "$wf" ] && [ "$wf" != NULL ] && "${Q[@]}" "UPDATE com_vtiger_workflows SET status = 0 WHERE workflow_id IN ($wf)"
echo "chặn gửi ra ngoài: SMTP → 127.0.0.1:1025, tắt workflow có task gửi tin: ${wf:-không có}"
redis-cli -n "$REDIS_DB" flushdb >/dev/null

# Deploy steps, with a before/after count for the tables the deleting migrations touch. Most migrations run inside
# QuickRepair (not `CPMigration up`), so effects are measured after both and the run list comes from the history table
for t in vtiger_relatedlists vtiger_field; do
    "${Q[@]}" "DROP TABLE IF EXISTS zz_smoke_$t; CREATE TABLE zz_smoke_$t AS SELECT * FROM $t"
done
hist0=$("${Q[@]}" "SELECT COALESCE(MAX(id), 0) FROM vtiger_migration_history")
echo "--- CPMigration up"
"$PHP" -d memory_limit=2G modules/CPMigration/Main.php up 2>&1 | grep -E 'success|failed|"error"' | sed 's/Description: //'
echo "--- QuickRepair"
"$PHP" -d memory_limit=4G "$HERE/quick_repair.php" "$WT"
rc=$?
echo "--- migration đã chạy (vtiger_migration_history; success=1 không chứng minh mọi câu SQL bên trong chạy được — xem log PHP/SQL)"
"${Q[@]}" "SELECT CONCAT(IF(success = 1, '  ok    ', '  FAIL  '), migration_name, IF(success = 1, '', CONCAT(' — ', LEFT(IFNULL(error_message, ''), 200)))) FROM vtiger_migration_history WHERE id > $hist0 ORDER BY id"
echo "--- xoá bởi migration: relatedlists $("${Q[@]}" "SELECT COUNT(*) FROM zz_smoke_vtiger_relatedlists b LEFT JOIN vtiger_relatedlists r USING(relation_id) WHERE r.relation_id IS NULL"), field $("${Q[@]}" "SELECT COUNT(*) FROM zz_smoke_vtiger_field b LEFT JOIN vtiger_field f USING(fieldid) WHERE f.fieldid IS NULL")"
"${Q[@]}" "SELECT CONCAT('    relatedlist ', t.name, ' → ', b.label) FROM zz_smoke_vtiger_relatedlists b LEFT JOIN vtiger_relatedlists r USING(relation_id) LEFT JOIN vtiger_tab t ON t.tabid = b.tabid WHERE r.relation_id IS NULL"
"${Q[@]}" "DROP TABLE zz_smoke_vtiger_relatedlists; DROP TABLE zz_smoke_vtiger_field"
redis-cli -n "$REDIS_DB" flushdb >/dev/null
rm -rf cache/templates_c
exit $rc

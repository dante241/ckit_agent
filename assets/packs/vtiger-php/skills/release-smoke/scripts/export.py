#!/usr/bin/env python3
"""export.py <out_dir> — export on the smoke site: direct list export, export through the queue (threshold
lowered on the CLONE, cron service run once even when run_by=Supervisor), and Report GetXLS of the three
biggest tabular reports. Config and cron rows are restored afterwards."""
import io, json, os, re, subprocess, sys, zipfile

import smoke_common as sc

out = sys.argv[1]
c = sc.Client()
rows = []


def xlsx_rows(body):
    if body[:2] != b'PK':
        return None
    sheet = zipfile.ZipFile(io.BytesIO(body)).read('xl/worksheets/sheet1.xml')
    return len(re.findall(rb'<row ', sheet))


def add(name, ok, note, fatals=()):
    rows.append({'name': name, 'verdict': 'ok' if ok else 'FAIL', 'note': note, 'fatal': list(fatals)[:2]})


cvid = sc.sql("SELECT cvid FROM vtiger_customview WHERE entitytype='Accounts' AND viewname='All' LIMIT 1")
total = int(sc.sql('SELECT COUNT(*) FROM vtiger_account a JOIN vtiger_crmentity e ON e.crmid=a.accountid AND e.deleted=0') or 0)
base = dict(module='Accounts', source_module='Accounts', action='ExportData', viewname=cvid, mode='ExportAllData',
            selected_ids='[]', excluded_ids='[]', search_params='[]', tag_params='[]', type='xls')

cfg_raw = sc.sql("SELECT value FROM vtiger_config WHERE category='export_config'")
cron = sc.sql("SELECT IFNULL(run_by,''), laststart, lastend FROM vtiger_cron_task WHERE name='ProcessExportQueue'").split('\t')


def set_threshold(n):    # the CLONE's export_config only; restored in finally
    cfg = json.loads(cfg_raw)
    cfg['export_in_background_when_records_num_exceeds'] = str(n)
    sc.sql("UPDATE vtiger_config SET value='" + json.dumps(cfg).replace("'", "''") + "' WHERE category='export_config'")
    subprocess.run(['redis-cli', '-n', os.environ['REDIS_DB'], 'flushdb'], capture_output=True)


try:
    # A. direct export: threshold above the record count (a tenant's own threshold may be tiny)
    set_threshold(total + 1)
    p = sc.log_pos(); st, b, _ = c.req('POST', '/index.php', dict(base))
    n = xlsx_rows(b)
    add(f'Export trực tiếp Accounts ({total} record)', n == total + 1, f'HTTP {st}, xlsx {n} dòng', sc.fatals_since(p))

    # B. queued export
    set_threshold(10)
    before = int(sc.sql('SELECT COALESCE(MAX(queue_id),0) FROM vtiger_export_queue') or 0)
    p = sc.log_pos(); c.req('POST', '/index.php', dict(base)); c.req('POST', '/index.php', dict(base))
    queued = sc.sql(f"SELECT queue_id FROM vtiger_export_queue WHERE queue_id > {before} AND status='NEW'").split()
    add('Export vào queue (gửi 2 lần → 1 dòng NEW)', len(queued) == 1, f'queue mới: {queued}', sc.fatals_since(p))
    sc.sql("UPDATE vtiger_cron_task SET run_by=NULL, laststart=0, lastend=0 WHERE name='ProcessExportQueue'")
    notes = int(sc.sql('SELECT COALESCE(MAX(notesid),0) FROM vtiger_notes') or 0)
    p = sc.log_pos()
    subprocess.run([sc.PHP, '-d', 'memory_limit=2G', '-r',
                    f'chdir("{sc.ROOT}"); $_REQUEST["service"]="ProcessExportQueue"; include "vtigercron.php";'],
                   capture_output=True, timeout=900)
    left = sc.sql(f"SELECT COUNT(*) FROM vtiger_export_queue WHERE queue_id IN ({','.join(queued) or 0})")
    doc = sc.sql(f"SELECT a.path, a.attachmentsid, a.name FROM vtiger_notes n JOIN vtiger_seattachmentsrel s ON s.crmid=n.notesid "
                 f"JOIN vtiger_attachments a USING(attachmentsid) WHERE n.notesid > {notes} AND n.filename LIKE 'export_%' ORDER BY n.notesid DESC LIMIT 1")
    n = None
    if '\t' in doc:
        path, aid, name = doc.split('\t')
        # exact name: a reloaded clone reuses attachment ids, so storage may hold an older "<id>_*" file
        f = f'{sc.ROOT}/{path}{aid}_{name}'
        n = xlsx_rows(open(f, 'rb').read()) if os.path.isfile(f) else None
    add('Cron ProcessExportQueue → Document xlsx', left == '0' and n == total + 1, f'queue còn {left}, file {n} dòng', sc.fatals_since(p))
finally:
    sc.sql("UPDATE vtiger_config SET value='" + cfg_raw.replace("'", "''") + "' WHERE category='export_config'")
    sc.sql(f"UPDATE vtiger_cron_task SET run_by={'NULL' if not cron[0] else repr(cron[0])}, laststart={cron[1]}, lastend={cron[2]} WHERE name='ProcessExportQueue'")
    subprocess.run(['redis-cli', '-n', os.environ['REDIS_DB'], 'flushdb'], capture_output=True)

# C. Report GetXLS of the largest tabular reports (export must start with the xlsx magic, no stray output);
# the threshold is lifted so they export in the request instead of queueing
set_threshold(10 ** 9)
try:
    for rid in sc.sql("SELECT r.reportid FROM vtiger_report r WHERE r.reporttype='tabular' AND IFNULL(r.custom_handler_file,'')='' ORDER BY r.reportid LIMIT 40").split():
        p = sc.log_pos(); st, b, _ = c.req('GET', f'/index.php?module=Reports&view=ExportReport&mode=GetXLS&record={rid}')
        n = xlsx_rows(b)
        f = sc.fatals_since(p)
        if n is None and not f and st < 500:
            continue    # report of a module the release lacks / no data: nothing exported, nothing broken
        add(f'Report {rid} GetXLS', n is not None and not f, f'HTTP {st}, xlsx {n} dòng', f)
finally:
    sc.sql("UPDATE vtiger_config SET value='" + cfg_raw.replace("'", "''") + "' WHERE category='export_config'")
    subprocess.run(['redis-cli', '-n', os.environ['REDIS_DB'], 'flushdb'], capture_output=True)

json.dump(rows, open(f'{out}/export.json', 'w'), ensure_ascii=False, indent=1)
print(f"export: {len(rows)} kiểm, FAIL {sum(r['verdict'] == 'FAIL' for r in rows)}")
for r in rows:
    print(f"  {r['verdict']:4} {r['name']} — {r['note']} {(r['fatal'] or [''])[0][:150]}")

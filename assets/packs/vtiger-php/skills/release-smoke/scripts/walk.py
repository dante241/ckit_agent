#!/usr/bin/env python3
"""walk.py <out_dir> — write paths on the smoke site (CLONE DB only): every report Detail, record CRUD on
core modules, HelpDesk status modal / inline edit / comment, Settings pages touched by the PHP 8 work,
and the four write cases master was gated on (SponsorshipCategory, CPRequest, CPLeaving, CPReportTask).
Records created here are deleted again; a case whose module the release lacks is reported as SKIP."""
import json, os, re, secrets, sys

import smoke_common as sc

out = sys.argv[1]
c = sc.Client()
rows = []


def check(name, status, body, pos, ok=True, note=''):
    f = sc.fatals_since(pos)
    missing = sc.missing_module(body)
    if missing and not f:
        rows.append({'name': name, 'verdict': 'SKIP', 'status': status, 'bytes': len(body or b''),
                     'note': f'chạm module {missing} release không có (DB clone)', 'fatal': []})
        return
    bad = (not ok) or status >= 500 or bool(sc.FATAL_BODY.search(body or b'')) or bool(f)
    rows.append({'name': name, 'verdict': 'FAIL' if bad else 'ok', 'status': status, 'bytes': len(body or b''),
                 'note': note, 'fatal': f[:2]})


def has_module(m):
    return os.path.isdir(f'{sc.ROOT}/modules/{m}') and sql_one(f"SELECT presence FROM vtiger_tab WHERE name='{m}'") in ('0', '2')


def sql_one(q):
    return sc.sql(q).split('\n')[0]


def save(module, **f):
    st, b, loc = c.req('POST', '/index.php', dict(module=module, action='Save', record='', **f))
    m = re.search(r'record=(\d+)', loc) or re.search(r'record=(\d+)', b.decode('utf-8', 'replace'))
    return st, b, (m.group(1) if m else '')


def delete(module, rid):
    st, b, _ = c.req('POST', '/index.php', dict(module=module, action='Delete', record=rid, ajaxDelete='true'), xhr=True)
    flag = sql_one(f'SELECT deleted FROM vtiger_crmentity WHERE crmid={int(rid)}')
    gone = flag in ('1', '')    # some modules delete the crmentity row outright
    # A related list pointing to a module the release lacks makes the response report an error after the delete
    rows.append({'name': f'{module} delete', 'verdict': 'ok' if gone else 'FAIL', 'status': st, 'bytes': len(b),
                 'note': f'deleted={flag or "row removed"}' + ('' if b'success' in b else ' (response: ' + b[:80].decode('utf-8', 'replace').strip() + ')'),
                 'fatal': []})


# 1. every report Detail, as the browser opens it
for line in sc.sql('SELECT reportid, reportname FROM vtiger_report ORDER BY reportid').split('\n'):
    if '\t' not in line:
        continue
    rid, name = line.split('\t', 1)
    p = sc.log_pos()
    st, b, loc = c.req('GET', f'/index.php?module=Reports&view=Detail&record={rid}')
    check(f'Report {rid} {name[:40]}', st, b, p, st in (200, 302), loc[:60])

# 2. core records: create -> Detail -> inline edit -> status modal -> comment -> delete
p = sc.log_pos()
st, b, acc = save('Accounts', accountname='smoke acc', industry='Apparel', leadsource='Chatbot', assigned_user_id='1')
check('Accounts create', st, b, p, bool(acc), f'rid={acc}')
con = tk = ''
if acc:
    p = sc.log_pos(); st, b, _ = c.req('GET', f'/index.php?module=Accounts&view=Detail&record={acc}')
    check('Accounts Detail', st, b, p, st == 200 and len(b) > 20000)
    p = sc.log_pos(); st, b, _ = c.req('POST', '/index.php', dict(module='Accounts', action='SaveAjax', record=acc, field='accountname', value='smoke acc 2'), xhr=True)
    check('Accounts SaveAjax', st, b, p, b'"success":true' in b and sql_one(f'SELECT accountname FROM vtiger_account WHERE accountid={acc}') == 'smoke acc 2')
    p = sc.log_pos()
    st, b, con = save('Contacts', firstname='smoke', lastname='contact', mobile='0909' + str(secrets.randbelow(10**6)).zfill(6), assigned_user_id='1', account_id=acc)
    check('Contacts create', st, b, p, bool(con), f'rid={con}')
    p = sc.log_pos()
    st, b, tk = save('HelpDesk', ticket_title='smoke ticket', ticketstatus='Open', ticketpriorities='Normal', helpdesk_customer_group='sales',
                     assigned_user_id='1', parent_id=acc, contact_id=con, description='smoke')
    check('HelpDesk create', st, b, p, bool(tk), f'rid={tk}')
if tk:
    p = sc.log_pos(); st, b, _ = c.req('GET', f'/index.php?module=HelpDesk&view=Detail&record={tk}')
    check('HelpDesk Detail', st, b, p, st == 200 and len(b) > 20000)
    p = sc.log_pos(); st, b, _ = c.req('GET', f'/index.php?module=HelpDesk&view=ModalAjax&mode=openStatusModal&record={tk}', xhr=True)
    check('HelpDesk openStatusModal', st, b, p, st == 200 and b'ticketstatus' in b)
    p = sc.log_pos(); st, b, _ = c.req('POST', '/index.php', dict(module='HelpDesk', action='SaveAjax', record=tk, field='ticketstatus', value='In Progress'), xhr=True)
    check('HelpDesk status SaveAjax', st, b, p, b'"success":true' in b)
    p = sc.log_pos(); st, b, _ = c.req('POST', '/index.php', dict(module='ModComments', action='SaveAjax', commentcontent='smoke comment', related_to=tk, is_private='0'), xhr=True)
    check('HelpDesk add comment', st, b, p, b'"success":true' in b)
for module, rid in (('HelpDesk', tk), ('Contacts', con), ('Accounts', acc)):
    if rid:
        delete(module, rid)

# 3. Settings pages the PHP 8 work touched (full page, no XHR)
for name, path in (('MailConverter List', '/index.php?module=MailConverter&parent=Settings&view=List'),
                   ('Webforms List', '/index.php?module=Webforms&parent=Settings&view=List'),
                   ('Webforms new Edit', '/index.php?module=Webforms&parent=Settings&view=Edit'),
                   ('HelpDesk List', '/index.php?module=HelpDesk&view=List')):
    p = sc.log_pos(); st, b, loc = c.req('GET', path)
    check(name, st, b, p, st in (200, 302), loc[:60])
scanner = sql_one('SELECT MIN(scannerid) FROM vtiger_mailscanner')
if scanner and scanner != 'NULL':
    p = sc.log_pos(); st, b, _ = c.req('GET', f'/index.php?module=MailConverter&parent=Settings&view=Edit&mode=step1&record={scanner}&create=existing')
    check(f'MailConverter Edit step1 {scanner}', st, b, p, st == 200)

# 4. master's PHP 8.3 write gate cases; references are taken from the clone, not hardcoded ids
def first_id(table, col, where='1=1'):
    return sql_one(f'SELECT t.{col} FROM {table} t JOIN vtiger_crmentity e ON e.crmid=t.{col} AND e.deleted=0 WHERE {where} ORDER BY t.{col} DESC LIMIT 1')


def case(name, module, fn):
    if not has_module(module):
        rows.append({'name': name, 'verdict': 'SKIP', 'status': 0, 'bytes': 0, 'note': f'{module} không có trên release', 'fatal': []})
        return
    p = sc.log_pos()
    try:
        ok, note, body = fn()
    except Exception as e:
        ok, note, body = False, f'exception {e}', b''
    check(name, 200, body, p, ok, note)


def c_sponsor():
    ev = first_id('vtiger_cpeventmanagement', 'cpeventmanagementid')
    st, b, rid = save('CPSponsorshipCategory', name='smoke category', cpsponsorshipcategory_type='cash', sponsorship_value='1500000',
                      expected_sponsors_number='2', related_cpeventmanagement=ev, assigned_user_id='1')
    if not rid:
        return False, f'create {st}', b
    c.req('POST', '/index.php', dict(module='CPSponsorshipCategory', action='Save', record=rid, mode='edit', name='smoke category',
                                     cpsponsorshipcategory_type='cash', sponsorship_value='1700000', expected_sponsors_number='2',
                                     related_cpeventmanagement=ev, assigned_user_id='1'))
    val = sql_one(f'SELECT expected_sponsorship_value FROM vtiger_cpsponsorshipcategory WHERE cpsponsorshipcategoryid={rid}')
    delete('CPSponsorshipCategory', rid)
    return val.startswith('3400000'), f'rid={rid} expected={val} (1700000x2)', b


def c_request():
    st, b, rid = save('CPRequest', name='smoke request', description='smoke', assigned_user_id='1', related_process='', related_category='')
    if not rid:
        return False, f'create {st}', b
    st2, b2, _ = c.req('GET', f'/index.php?module=CPRequest&view=Detail&record={rid}')
    delete('CPRequest', rid)
    return st2 == 200, f'rid={rid} Detail {st2}', b2


def c_leaving():
    emp = first_id('vtiger_cpemployee', 'cpemployeeid')
    st, b, rid = save('CPLeaving', name='smoke leave', description='smoke', related_cpemployee=emp, cpleaving_type='Sick leave',
                      assigned_handle='1', assigned_user_id='1', **{'work_shift[]': ['2026-10-24|##|Sick leave|##|08:00-17:00|##|8|##|1']})
    if not rid:
        return False, f'create {st}', b
    det = sql_one(f'SELECT COUNT(*) FROM vtiger_cpleavingdetail WHERE related_cpleaving={rid}')
    delete('CPLeaving', rid)
    sc.sql(f'DELETE FROM vtiger_cpleavingdetail WHERE related_cpleaving={rid}')
    return det not in ('', '0'), f'rid={rid} detail_rows={det}', b


def c_reporttask():
    task = first_id('vtiger_activity', 'activityid', "t.activitytype='Task'")
    premax = int(sql_one('SELECT COALESCE(MAX(cpreporttaskid),0) FROM vtiger_cpreporttask') or 0)
    st, b, rid = save('CPReportTask', name='smoke report', cpreporttask_type='day', apply_date='20-04-2026',
                      cpreporttask_task_current=json.dumps([{'module': 'Calendar', 'record_id': int(task or 0), 'mode': 'current'}]),
                      cpreporttask_task_up_coming='[]', assigned_user_id='Users:1', description='smoke')
    rid = rid or sql_one(f'SELECT cpreporttaskid FROM vtiger_cpreporttask WHERE cpreporttaskid > {premax} ORDER BY cpreporttaskid LIMIT 1')
    if not rid or rid == 'NULL':
        return False, f'save {st}: không tạo record', b
    delete('CPReportTask', rid)
    return True, f'rid={rid}', b


case('CPSponsorshipCategory save+update', 'CPSponsorshipCategory', c_sponsor)
case('CPRequest save+Detail', 'CPRequest', c_request)
case('CPLeaving save+delete', 'CPLeaving', c_leaving)
case('CPReportTask save', 'CPReportTask', c_reporttask)

json.dump(rows, open(f'{out}/walk.json', 'w'), ensure_ascii=False, indent=1)
bad = [r for r in rows if r['verdict'] == 'FAIL']
print(f"walk: {len(rows)} kiểm, FAIL {len(bad)}, SKIP {sum(r['verdict'] == 'SKIP' for r in rows)}")
for r in bad + [r for r in rows if r['verdict'] == 'SKIP']:
    print(f"  {r['verdict']} {r['name']} {r['status']} {r['note'][:100]} {(r['fatal'] or [''])[0][:160]}")

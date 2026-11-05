#!/usr/bin/env python3
"""api.py <out_dir> — call every read action of every api/ entry on the smoke site (master's api_sweep pass).
Actions are called with empty Params, so "record not found" / SQL built from missing params is expected;
what matters is a fatal that the same call on master does not give (compare with code parity in the report)."""
import json, os, re, subprocess, sys, time
from collections import Counter
import requests

import smoke_common as sc

out = sys.argv[1]
inv_path = f'{out}/api_inventory.json'
subprocess.run([sys.executable, os.path.join(os.path.dirname(os.path.abspath(__file__)), 'api_inventory.py'), inv_path], capture_output=True, check=True)
INV = json.load(open(inv_path))
SKIP_API = {'OpenEventApi/Endpoint.php', 'WooCommerceApi.php', 'PartnerApi.php', 'OpenAPI/Endpoint.php', 'IntegrationAPI/Endpoint.php'}
PHP_ERR = re.compile(r'must be of type|must be of the type|Argument #\d|Argument \d+ passed|Cannot access offset|'
                     r'Unsupported operand|Division by zero|on null|on false|on bool|Undefined constant|Call to undefined')

login = requests.post(sc.BASE + '/api/CloudGoAppApi.php', json={'RequestAction': 'Login', 'Credentials': {'username': 'admin', 'password': sc.ADMIN_PASS}}, timeout=60).json()
token = login['data']['token']
for bad_pw, want in (('wrong-password', 401),):
    r = requests.post(sc.BASE + '/api/WebsiteApi.php', json={'RequestAction': 'Login', 'Credentials': {'username': 'admin', 'password': bad_pw}}, timeout=60)
    login_ok = r.status_code == want
r = requests.post(sc.BASE + '/api/WebsiteApi.php', json={'RequestAction': 'Login', 'Credentials': {'username': 'admin', 'password': sc.ADMIN_PASS}}, timeout=60)
login_ok = login_ok and r.status_code == 200 and 'token' in r.text

results = []
for e in INV:
    if e['api'] in SKIP_API:
        continue
    for a in [a for a in e['actions'] if a['kind'] == 'ĐỌC']:
        body = {'RequestAction': a['action'], 'Params': {}}
        if a.get('module'):
            body['Module'] = a['module']
        p = sc.log_pos()
        try:
            r = requests.post(f"{sc.BASE}/api/{e['api']}", json=body, headers={'Token': token}, timeout=90)
            status, text = r.status_code, r.text
        except Exception as ex:
            status, text = 0, 'NETERR ' + str(ex)[:80]
        time.sleep(0.05)
        fatal = sc.fatals_since(p)
        bad = status >= 500 or status == 0 or bool(fatal) or bool(PHP_ERR.search(text[:400]))
        results.append({'api': e['api'], 'module': a.get('module', ''), 'action': a['action'], 'status': status,
                        'bad': bad, 'fatal': fatal[:1], 'snippet': text[:160]})

json.dump({'website_login_200_401': login_ok, 'results': results}, open(f'{out}/api.json', 'w'), ensure_ascii=False, indent=1)
print(f"api: WebsiteApi Login 200/401 {'OK' if login_ok else 'FAIL'}; {len(results)} action đọc, lỗi {sum(r['bad'] for r in results)}")
sig = Counter(re.sub(r'\d+', 'N', (r['fatal'][0] if r['fatal'] else re.sub(r'\s+', ' ', r['snippet']))[:150]) for r in results if r['bad'])
for k, v in sig.most_common(20):
    print(f'  {v:3} × {k}')

#!/usr/bin/env python3
"""crawl.py <out_dir> — read-only crawl of the smoke site, the two lists used to test master on PHP 8.3:
  data/pages.txt       284 core pages (List/Detail/Edit/Settings) with a seeded admin session, no XHR
  data/crawl-urls.json 6,480 URLs of the php83 demo crawl (ajax ones sent with XHR, as recorded),
                       logged in through Users/Login; status/fatal compared with the demo result
A URL is bad on HTTP >= 500, an empty body or a PHP fatal text; "Không có quyền" pages (<5 KB) are
counted separately: they are license/permission pages, not checks that ran."""
import json, re, sys, threading, time
from concurrent.futures import ThreadPoolExecutor
import requests

import smoke_common as sc

out = sys.argv[1]
results = {'pages': [], 'crawl': []}

# 1. core pages, seeded session
c = sc.Client()
for u in [l.strip() for l in open(f'{sc.DATA}/pages.txt') if l.strip()]:
    t = time.time()
    try:
        st, body, loc = c.req('GET', '/' + u, timeout=120)
    except Exception as e:
        st, body, loc = 0, repr(e).encode(), ''
    bad = st >= 500 or st == 0 or not body.strip() or bool(sc.FATAL_BODY.search(body))
    results['pages'].append({'url': u, 'status': st, 'bytes': len(body), 'ms': int((time.time() - t) * 1000),
                             'bad': bad, 'denied': len(body) < 5000 and not bad})

# 2. demo crawl list, real login
s = requests.Session()
s.post(sc.BASE + '/index.php', data={'module': 'Users', 'action': 'Login', 'username': 'admin', 'password': sc.ADMIN_PASS}, timeout=60)
if 'name="username"' in s.get(sc.BASE + '/index.php', timeout=60).text:
    sys.exit('ABORT: đăng nhập admin thất bại trên DB clone (chạy db.sh)')
lock = threading.Lock()


def one(p):
    h = {'X-Requested-With': 'XMLHttpRequest'} if p.get('ajax') else {}
    try:
        r = s.get(sc.BASE + '/' + p['url'], headers=h, timeout=120, allow_redirects=False)
        code, body = r.status_code, r.text
    except Exception as e:
        code, body = 0, repr(e)
    raw = body.encode('utf-8', 'replace')
    missing = sc.missing_module(raw, p['url'])
    bad = (code >= 500 or code == 0 or bool(sc.FATAL_BODY.search(raw)) or (bool(sc.RESTRICTED.search(raw)) and not missing))
    row = {'url': p['url'], 'status': code, 'bad': bad, 'missing_module': missing,
           'demo_bad': bool(p.get('fatal')) or (p.get('status') or 0) >= 500}
    with lock:
        results['crawl'].append(row)


with ThreadPoolExecutor(4) as ex:
    list(ex.map(one, json.load(open(f'{sc.DATA}/crawl-urls.json'))))

json.dump(results, open(f'{out}/crawl.json', 'w'), ensure_ascii=False, indent=1)
pb = [r for r in results['pages'] if r['bad']]
cb = [r for r in results['crawl'] if r['bad'] and not r['demo_bad']]
print(f"pages: {len(results['pages'])} URL, lỗi {len(pb)}, trang <5KB (quyền/license) {sum(r['denied'] for r in results['pages'])}")
miss = sorted({r['missing_module'] for r in results['crawl'] if r.get('missing_module')})
print(f"crawl: {len(results['crawl'])} URL, lỗi {sum(r['bad'] for r in results['crawl'])}, lỗi mới so với demo {len(cb)}; "
      f"{sum(1 for r in results['crawl'] if r.get('missing_module'))} URL chạm module release không có (DB clone): {', '.join(miss)}")
for r in pb + cb:
    print('  BAD', r['status'], r['url'][:120])

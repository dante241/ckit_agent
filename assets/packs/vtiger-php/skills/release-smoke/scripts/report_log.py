#!/usr/bin/env python3
"""logreport.py <php_log> <offset> <nginx_log> <nginx_offset> <out.md> — group the release smoke site's PHP/nginx errors."""
import re, sys
from collections import OrderedDict

php_log, off, ngx_log, ngx_off, out = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4]), sys.argv[5]
ROOT = __import__('os').environ['WT'].rstrip('/') + '/'
with open(php_log, 'rb') as fh:
    fh.seek(off)
    text = fh.read().decode('utf-8', 'replace')

groups = OrderedDict()
last = None
for line in text.split('\n'):
    if last is not None and 'thrown in' in line and ' | ' in line:
        last['urls'][line.rsplit(' | ', 1)[1].strip()[:160]] = 1
        last = None
        continue
    m = re.match(r'\[([^\]]+)\] PHP (\w+) \(type (\d+)\): (.*)', line)
    if not m:
        continue
    if ROOT not in line:
        last = None
        continue
    ts, level, typ, rest = m.groups()
    msg, _, url = rest.partition(' | ')
    msg = msg.replace(ROOT, '')
    msg = re.sub(r'cache/templates_c/v7/[0-9a-f]+\.file\.', 'tpl:', msg)
    key = re.sub(r'\d{3,}', 'N', msg)
    g = groups.setdefault(key, {'level': level, 'msg': msg, 'count': 0, 'urls': OrderedDict(), 'first': ts})
    g['count'] += 1
    last = g
    if url:
        g['urls'][url.strip()[:160]] = 1

with open(ngx_log, 'rb') as fh:
    fh.seek(ngx_off)
    ngx = [l for l in fh.read().decode('utf-8', 'replace').split('\n') if '[error]' in l or '[crit]' in l]

lines = ['# Lỗi log — release-core-10.2026 trên PHP 8.3 (smoke 8085)', '',
         f'Log PHP từ offset {off}: {sum(g["count"] for g in groups.values())} dòng lỗi, {len(groups)} nhóm. nginx error: {len(ngx)} dòng.', '']
for g in sorted(groups.values(), key=lambda g: -g['count']):
    lines.append(f'## [{g["level"]}] ×{g["count"]} — {g["msg"][:300]}')
    for u in list(g['urls'])[:6]:
        lines.append(f'- `{u}`')
    lines.append('')
if ngx:
    lines.append('## nginx error.log')
    seen = OrderedDict()
    for l in ngx:
        k = re.sub(r'^\S+ \S+ \[\w+\] \d+#\d+: \*\d+ ', '', l)[:200]
        seen[k] = seen.get(k, 0) + 1
    for k, v in list(seen.items())[:40]:
        lines.append(f'- ×{v} {k}')
open(out, 'w').write('\n'.join(lines) + '\n')
print('\n'.join(lines[:80]))

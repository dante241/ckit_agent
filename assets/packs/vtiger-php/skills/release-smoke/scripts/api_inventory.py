#!/usr/bin/env python3
"""Inventory every API under api/: entry -> handler class chain -> callable actions
(public, request-taking, incl. inherited and per-module convertClass handlers) with
the params each action reads and an explorer sample when one exists."""
import json, os, re, sys

ROOT = os.environ['WT'].rstrip('/')
os.chdir(ROOT)

CLASS_FILE = {}
for base in ('include', 'api', 'modules'):
    for dp, _, files in os.walk(base):
        for fn in files:
            if fn.endswith('.php'):
                p = os.path.join(dp, fn)
                try:
                    t = open(p, encoding='utf-8', errors='ignore').read()
                except OSError:
                    continue
                for m in re.finditer(r'^\s*(?:abstract\s+|final\s+)?class\s+(\w+)(?:\s+extends\s+(\w+))?', t, re.M):
                    CLASS_FILE.setdefault(m.group(1), (p, m.group(2)))

# CloudWorkApi.php always forces X-Api-Version=2: resolve its handler to the v2 file
_v2 = 'include/Webservice/CloudWorkApi/v2/CloudWorkApiHandler.php'
_v2m = re.search(r'class\s+CloudWorkApiHandler(?:\s+extends\s+(\w+))?', open(_v2, encoding='utf-8', errors='ignore').read())
CLASS_FILE['CloudWorkApiHandler'] = (_v2, _v2m.group(1) if _v2m else None)

INFRA = {'getRequest', 'setResponse', 'saveLog', 'checkSession', 'auth', 'logout', 'login', 'resetPassword',
         '__construct', 'process', 'route', 'validateSecretKey'}
WRITE = re.compile(r'^(save|create|delete|update|add|remove|accept|reject|confirm|checkin|checkout|change|convert|'
                   r'reply|send|upload|register|mark|set|assign|approve|cancel|close|import|merge|transfer|toggle|'
                   r'link|unlink|scan|insert|edit|sync|push|post|put|pin|unpin|follow|unfollow|start|stop|finish|'
                   r'complete|submit|request|clone|duplicate|move|archive|restore|rate|like|react|join|leave|invite)', re.I)
OUT = re.compile(r'curl_exec|curl_init|Mailer|sendmail|send_mail|sendEmail|ZNS|sendSms|NotificationHelper::send|'
                 r'pushNotification|HubBridge|->post\(', re.I)


CLASS_LC = {k.lower(): (k, v) for k, v in CLASS_FILE.items()}


def chain(cls):
    # PHP class names are case-insensitive (the CloudWork map writes ThankYouCardAPI for ThankYouCardApi)
    out = []
    while cls and cls.lower() in CLASS_LC:
        cls, (p, parent) = CLASS_LC[cls.lower()]
        out.append((cls, p))
        cls = parent
    return out


def actions_of(cls, own_only=False):
    acts = {}
    for c, p in (chain(cls)[:1] if own_only else list(reversed(chain(cls)))):          # parent first, child overrides
        t = open(p, encoding='utf-8', errors='ignore').read()
        for m in re.finditer(r'\n[ \t]*((?:public|protected|private)\s+)?(static\s+)?function\s+(\w+)\s*\(([^)]*)\)', t):
            vis = (m.group(1) or 'public').strip()
            name, args = m.group(3), m.group(4)
            if vis != 'public' or name in INFRA or name.startswith('_'):
                continue
            if not re.search(r'\$request\b|Vtiger_Request', args):
                continue
            nxt = re.search(r'\n[ \t]*(?:(?:public|protected|private)\s+)?(?:static\s+)?function\s+\w+\s*\(', t[m.end():])
            body = t[m.end(): m.end() + (nxt.start() if nxt else 4000)]
            params = sorted(set(re.findall(r"\$request->get\(\s*['\"](\w+)['\"]", body)))
            sub = sorted(set(re.findall(r"\$params\[['\"](\w+)['\"]\]", body)))
            acts[name.lower()] = {'action': name[0].upper() + name[1:], 'owner': c, 'static': bool(m.group(2)),
                                  'kind': 'GHI' if WRITE.search(name) else 'ĐỌC', 'request_params': params,
                                  'params_keys': sub[:15], 'calls_out': bool(OUT.search(body)),
                                  'file': os.path.relpath(p, ROOT)}
    return acts


def explorer_samples(path):
    if not os.path.exists(path):
        return {}
    t = open(path, encoding='utf-8', errors='ignore').read()
    return {m.group(1).lower(): m.group(1) for m in re.finditer(r'\n\s{8}(\w+)\s*:\s*\{', t)}


entries = []
for f in sorted(os.listdir('api')):
    p = os.path.join('api', f)
    if f.endswith('Test.php') or f == 'AGENTS.md':
        continue
    if os.path.isdir(p):
        p = os.path.join(p, 'Endpoint.php')
        if not os.path.exists(p):
            continue
    if not p.endswith('.php'):
        continue
    t = open(p, encoding='utf-8', errors='ignore').read()
    m = re.search(r"callApiHandlerAction\('(\w+)'", t) or re.search(r'(\w+Handler)::(?:process|route)\(', t) \
        or re.search(r"require_once\(['\"]include/Webservice/(\w+)\.php", t)
    handler = m.group(1) if m else '?'
    auth = []
    if re.search(r"['\"]Login['\"]", t): auth.append('RequestAction=Login → Token')
    if re.search(r'X-Secret-Key|X-Api-Key', t): auth.append('header X-Secret-Key/X-Api-Key')
    if re.search(r"Authorization|Bearer|access_key", t): auth.append('Authorization/access_key')
    if re.search(r"\$headers\[['\"]Token['\"]\]|['\"]token['\"]", t): auth.append('header Token')
    acts = actions_of(handler) if handler in CLASS_FILE else {}
    # per-module handlers (CloudWork v2 / CloudData convertClass)
    hp = CLASS_FILE.get(handler, ('', ''))[0]
    if hp:
        ht = open(hp, encoding='utf-8', errors='ignore').read()
        cm = re.search(r'\$convertClass\s*=\s*\[(.*?)\];', ht, re.S)
        if cm:
            for mod, cls in re.findall(r"['\"](\w+)['\"]\s*=>\s*(?:['\"](\w+)['\"]|(\w+)::class)", cm.group(1)) and [(a, b or c) for a, b, c in re.findall(r"['\"](\w+)['\"]\s*=>\s*(?:['\"](\w+)['\"]|(\w+)::class)", cm.group(1))]:
                for k, v in actions_of(cls, own_only=True).items():
                    v = dict(v); v['module'] = mod
                    acts['%s.%s' % (mod.lower(), k)] = v
    test_file = p.replace('.php', 'Test.php') if not p.endswith('Endpoint.php') else ''
    samples = explorer_samples(test_file) if test_file else {}
    for k, v in acts.items():
        v['has_sample'] = k.split('.')[-1] in samples
    entries.append({'api': os.path.relpath(p, 'api'), 'handler': handler, 'auth': auth or ['?'],
                    'explorer': os.path.relpath(test_file, ROOT) if test_file and os.path.exists(test_file) else '',
                    'actions': sorted(acts.values(), key=lambda a: (a.get('module', ''), a['action']))})

json.dump(entries, open(sys.argv[1], 'w'), ensure_ascii=False, indent=1)
tot = 0
for e in entries:
    n = len(e['actions']); tot += n
    w = sum(1 for a in e['actions'] if a['kind'] == 'GHI')
    ns = sum(1 for a in e['actions'] if not a['static'])
    print('%-28s %-26s actions=%3d (ghi %3d, non-static %3d) auth=%s' % (e['api'], e['handler'], n, w, ns, ', '.join(e['auth'])))
print('TOTAL actions:', tot)

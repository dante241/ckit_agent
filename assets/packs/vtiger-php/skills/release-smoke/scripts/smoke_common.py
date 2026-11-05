"""smoke_common.py — shared helpers for the release-smoke checks (settings come from lib.sh via the environment)."""
import hashlib, http.client, os, re, secrets, subprocess, time, urllib.parse

ROOT = os.environ['WT'].rstrip('/')
DB = os.environ['DB']
PHP = os.environ['PHP']
PHP_LOG = os.environ['PHP_LOG']
BASE = os.environ['BASE']
PORT = int(os.environ['PORT'])
ADMIN_PASS = os.environ['ADMIN_PASS']
DATA = os.environ['DATA']
REDIS_SESSION_DB = '2'          # php-fpm stores sessions in Redis db 2 (php.ini session.save_path)
APP_KEY_RE = re.compile(r"\$application_unique_key\s*=\s*'([^']+)'")
FATAL_BODY = re.compile(rb'Fatal error|Uncaught |TypeError|ValueError|ArgumentCountError|DivisionByZeroError|Parse error')
RESTRICTED = re.compile(rb'Sorry! Attempt to access restricted file\.? - (modules/(\w*)/[\w.]*)')


def _on_origin_release(path):
    return subprocess.run(['git', '-C', ROOT, 'cat-file', '-e', f'origin/release:{path}'], capture_output=True).returncode == 0


def missing_module(body, url=''):
    """Clone-artifact label for vtiger's "restricted file" page, else None (a real failure).
    Artifact = the file was never on origin/release (module the release doesn't ship, module name from another DB),
    or the URL's record is absent/deleted in the clone. A file origin/release has but the branch lost is NOT one."""
    m = RESTRICTED.search(body or b'')
    if not m:
        return None
    path, name = m.group(1).decode(), m.group(2).decode()
    if not name:
        rid = urllib.parse.parse_qs(urllib.parse.urlsplit(url).query).get('record', [''])[0]
        if rid.isdigit() and sql(f'SELECT deleted FROM vtiger_crmentity WHERE crmid={rid}') == '0':
            return None    # the record exists yet resolves to no module: a real failure
        return '(record không có trong DB clone)'
    if os.path.isfile(f"{ROOT}/{path}"):
        return None
    if _on_origin_release(path):
        return None    # origin/release ships this file and the branch lost it
    return name if not os.path.isdir(f"{ROOT}/modules/{name}") else f'({path} không có trên release — tham số của DB khác)'


def sql(q):
    return subprocess.run(os.environ['MYSQL_CMD'].split() + [DB, '-N', '-e', q],
                          capture_output=True, text=True, errors='replace').stdout.strip()


def _s(v):
    return f's:{len(v)}:"{v}";'


def seed_session(user_id='1'):
    """An authenticated session written straight into Redis (no password needed)."""
    key = APP_KEY_RE.search(open(f'{ROOT}/config.inc.php', encoding='utf-8', errors='replace').read()).group(1)
    sid = 'smoke' + secrets.token_hex(8)
    payload = (f'__HTTP_Session2_Info|i:1;session_id|{_s(sid)}login_language|{_s("vn_vn")}AUTHUSERID|{_s(user_id)}'
               f'authenticated_user_id|{_s(user_id)}app_unique_key|{_s(key)}authenticated_user_language|{_s("vn_vn")}')
    subprocess.run(['redis-cli', '-n', REDIS_SESSION_DB, 'SET', 'PHPSESSID:' + sid, payload], capture_output=True, check=True)
    return sid


def csrf_token(sid):
    secret = subprocess.run([PHP, '-r', f'chdir("{ROOT}"); $secret=""; include "config.csrf-secret.php"; echo $secret;'],
                            capture_output=True, text=True).stdout
    t = int(time.time())
    return f'sid:{hashlib.sha1((secret + sid + str(t)).encode()).hexdigest()},{t}'


class Client:
    """Requests as the browser makes them: full page GETs without XHR (an XHR header skips preProcess)."""

    def __init__(self):
        self.sid = seed_session()
        self.tok = csrf_token(self.sid)

    def req(self, method, path, data=None, xhr=False, timeout=300):
        conn = http.client.HTTPConnection('127.0.0.1', PORT, timeout=timeout)
        hdr = {'Cookie': 'PHPSESSID=' + self.sid}
        if xhr:
            hdr['X-Requested-With'] = 'XMLHttpRequest'
        body = None
        if data is not None:
            body = urllib.parse.urlencode(dict(data, __vtrftk=self.tok), doseq=True)
            hdr['Content-Type'] = 'application/x-www-form-urlencoded'
        conn.request(method, path, body, hdr)
        r = conn.getresponse()
        out = r.read()
        loc = r.getheader('Location') or ''
        conn.close()
        return r.status, out, loc


def log_pos():
    return os.path.getsize(PHP_LOG)


def fatals_since(pos):
    """Fatal lines of THIS worktree only (master and other sites share the FPM log)."""
    with open(PHP_LOG, 'rb') as fh:
        fh.seek(pos)
        data = fh.read().decode('utf-8', 'replace')
    return [l[:240].replace(ROOT + '/', '') for l in data.split('\n') if re.search(r'Fatal|Uncaught', l) and ROOT + '/' in l]

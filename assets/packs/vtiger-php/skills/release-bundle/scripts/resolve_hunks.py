#!/usr/bin/env python3
"""resolve_hunks.py — hunk-level resolve of one ticket onto a pick/<ticket> branch (called by resolve.sh).

  resolve_hunks.py list   <bundle_result.json>
  resolve_hunks.py pick   <ticket> <worktree> <out_dir>
  resolve_hunks.py report <out_dir> <phpstan.json>

pick: cherry-picks every MR of the ticket in merge order. A file that merges cleanly keeps git's
3-way result. For a conflicted file the release copy is restored and the MR's own patch
(git diff <sha>^1 <sha>) is applied hunk by hunk:
  T1  applies with full context                                    -> kept
  T2  applies only with -C1 and lands in the same scope (@@ header)  -> kept, listed for review
  T3  anything else (no fit, or -C1 in another scope)                -> not applied, saved as .rej
Non-shipping paths keep the release copy; composer.lock/vendor/, files missing on release,
binary files and MR deletions are never applied (T3, whole-file patch saved).

Env (exported by resolve.sh via lib.sh): CLASSIFIED, RELEASE_BASE, NOISE_RE.
"""
import json
import os
import re
import subprocess
import sys
from pathlib import Path

NOISE = re.compile(os.environ.get('NOISE_RE', r'^(CHANGELOG\.md|\.gitignore|agents/|docs/|tools/|\.omp/|\.claude/)'))
REGEN = re.compile(r'^(composer\.lock$|vendor/)')
UNMERGED = {'UU', 'AA', 'DU', 'UD', 'AU', 'UA', 'DD'}


def git(*args, cwd, inp=None):
    return subprocess.run(['git', *args], cwd=cwd, capture_output=True, text=True,
                          input=inp, errors='surrogateescape')


def split_patch(patch):
    """(header lines, [hunk lines]) of a single-file patch."""
    lines = patch.split('\n')
    if lines and lines[-1] == '':
        lines.pop()
    head, hunks = [], []
    for line in lines:
        if line.startswith('@@'):
            hunks.append([line])
        elif hunks:
            hunks[-1].append(line)
        else:
            head.append(line)
    return head, hunks


def scope(hunk):
    m = re.match(r'@@[^@]*@@ ?(.*)', hunk[0])
    return m.group(1).strip() if m else ''


def apply_hunk(wt, head, hunk, ctx):
    args = ['apply', '--recount', '--ignore-whitespace', '--whitespace=nowarn']
    if ctx is not None:
        args.append(f'-C{ctx}')
    return git(*args, '-', cwd=wt, inp='\n'.join(head + hunk) + '\n').returncode == 0


def placed_scopes(wt, path, hunk):
    """@@ headers of the worktree-vs-index diff hunks that carry this hunk's changed lines."""
    key = [l for l in hunk[1:] if l[:1] == '+' and l[1:].strip()] or \
          [l for l in hunk[1:] if l[:1] == '-' and l[1:].strip()]
    _, got = split_patch(git('diff', '-U3', '--', path, cwd=wt).stdout)
    return sorted({scope(h) for h in got if key and all(k in h for k in key)})


# Nearest enclosing definition above a line: a function/method header or an array element key ('name' => array( / [).
# The @@ header git picks is often only the class or the outer $fields = array( of a register file, too coarse to
# tell two methods or two fields apart.
ANCHOR = re.compile(r"^\s*(?:(?:public|protected|private|static|final|abstract)\s+)*function\s+&?\w+"
                    r"|^\s*['\"][\w.-]+['\"]\s*=>\s*(?:array\s*\(|\[)\s*$")


def anchor_above(lines, idx):
    for k in range(min(idx, len(lines)) - 1, -1, -1):
        if ANCHOR.match(lines[k]):
            return re.sub(r'\s+', ' ', lines[k].strip())
    return ''


def first_change(hunk, side):
    """0-based line index, in the old ('-') or new ('+') file, of the hunk's first changed line."""
    m = re.match(r'@@ -(\d+)(?:,\d+)? \+(\d+)', hunk[0])
    start = int(m.group(1) if side == '-' else m.group(2))
    lead = 0
    for l in hunk[1:]:
        if l[:1] in '+-':
            break
        lead += 1
    return max(start - 1, 0) + lead


def same_anchor(wt, sha, path, hunk):
    """The -C1 placement sits under the same function / array element as the hunk did on master."""
    old = git('show', f'{sha}^1:{path}', cwd=wt).stdout.splitlines()
    want = anchor_above(old, first_change(hunk, '-'))
    if not want:
        return True
    key = [l for l in hunk[1:] if l[:1] == '+' and l[1:].strip()] or \
          [l for l in hunk[1:] if l[:1] == '-' and l[1:].strip()]
    new = (Path(wt) / path).read_text(encoding='utf-8', errors='surrogateescape').splitlines()
    _, got = split_patch(git('diff', '-U3', '--', path, cwd=wt).stdout)
    placed = [h for h in got if key and all(k in h for k in key)]
    return bool(placed) and all(anchor_above(new, first_change(h, '+')) == want for h in placed)


def reject_kind(wt, path, hunk):
    """Why a hunk does not fit: compares the lines it removes with the release file."""
    try:
        text = (Path(wt) / path).read_text(encoding='utf-8', errors='replace')
    except OSError:
        text = ''
    minus = [l[1:] for l in hunk[1:] if l[:1] == '-' and l[1:].strip()]
    if not minus:
        return 'chỉ thêm, mất chỗ neo'
    found = sum(1 for l in minus if l in text)
    if found == len(minus):
        return 'lệch ngữ cảnh'
    if found == 0:
        return 'sửa code release chưa có'
    return 'một phần'


def mr_sha(mr):
    for k in ('merge_commit_sha', 'squash_commit_sha', 'sha'):
        if mr.get(k):
            return mr[k]
    return ''


def ticket_mrs(ticket):
    """MRs of the ticket in merge order: the run's own grouping (bundle-result conflicts_manual) when the
    ticket is there — a multi-ticket MR belongs to the ticket the bundle filed it under — else classified."""
    data = json.load(open(os.environ['CLASSIFIED']))
    ready = data.get('ready', [])
    bundle = os.environ.get('BUNDLE_LAST', '')
    if bundle and os.path.exists(bundle):
        for c in json.load(open(bundle)).get('conflicts_manual', []):
            if str(c.get('ticket')) == ticket:
                iids = set(c.get('iids') or [c['iid']])
                return sorted((m for m in ready if m['iid'] in iids), key=lambda m: m.get('merged_at') or '')
    mrs = [m for m in ready if ticket in (m.get('ticket_ids') or [m.get('ticket_id')])]
    return sorted(mrs, key=lambda m: m.get('merged_at') or '')


def rej_write(out_dir, iid, path, text):
    p = Path(out_dir) / str(iid) / (path + '.rej')
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding='utf-8', errors='surrogateescape')
    return str(p)


def resolve_file(wt, sha, path, iid, out_dir):
    """Restore the release copy of a conflicted file and apply the MR's hunks tier by tier."""
    git('checkout', 'HEAD', '--', path, cwd=wt)
    git('add', '--', path, cwd=wt)
    head, hunks = split_patch(git('diff', f'{sha}^1', sha, '--', path, cwd=wt).stdout)
    rows, rej = [], []
    if not hunks:
        return [{'file': path, 'tier': 'T3', 'reason': 'file nhị phân', 'master_scope': '', 'placed_scope': '',
                 'rej': ''}]
    for i, h in enumerate(hunks, 1):
        row = {'file': path, 'hunk': i, 'master_scope': scope(h), 'placed_scope': ''}
        fits = apply_hunk(wt, head, h, None)
        if fits and same_anchor(wt, sha, path, h):
            row['tier'] = 'T1'
            rows.append(row)
            git('add', '--', path, cwd=wt)
            continue
        if fits:
            # A full-context fit at an offset can land on a look-alike field or method: undo, then try -C1
            git('checkout', '--', path, cwd=wt)
        if apply_hunk(wt, head, h, 1):
            placed = placed_scopes(wt, path, h)
            row['placed_scope'] = ' | '.join(placed)
            if row['master_scope'] and row['master_scope'] in placed and same_anchor(wt, sha, path, h):
                row['tier'] = 'T2'
            else:
                git('checkout', '--', path, cwd=wt)  # back to the index copy: hunk landed elsewhere
                row['tier'] = 'T3'
                row['reason'] = 'chỉ khớp -C1 nhưng nằm ở scope khác' if not fits else 'khớp ở chỗ khác (khác hàm/phần tử mảng)'
        else:
            row['tier'] = 'T3'
            row['reason'] = reject_kind(wt, path, h) if not fits else 'khớp ở chỗ khác (khác hàm/phần tử mảng)'
        if row['tier'] == 'T3':
            rej.append('\n'.join(h))
        else:
            git('add', '--', path, cwd=wt)
        rows.append(row)
    path_rej = rej_write(out_dir, iid, path, '\n'.join(head + rej) + '\n') if rej else ''
    for r in rows:
        r.setdefault('reason', '')
        r['rej'] = path_rej if r['tier'] == 'T3' else ''
    return rows


def whole_file_t3(wt, sha, path, iid, out_dir, reason):
    patch = git('diff', f'{sha}^1', sha, '--', path, cwd=wt).stdout
    return {'file': path, 'tier': 'T3', 'reason': reason, 'master_scope': '', 'placed_scope': '',
            'rej': rej_write(out_dir, iid, path, patch) if patch else ''}


def keep_release(wt, path):
    if git('cat-file', '-e', f'HEAD:{path}', cwd=wt).returncode == 0:
        git('checkout', 'HEAD', '--', path, cwd=wt)
        git('add', '--', path, cwd=wt)
    else:
        git('rm', '-q', '-f', '--', path, cwd=wt)


def pick_mr(wt, mr, out_dir):
    sha, iid = mr_sha(mr), mr['iid']
    res = {'iid': iid, 'sha': sha, 'title': mr.get('title', ''), 'hunks': [], 'kept_release': []}
    marker = f'cherry picked from commit {sha}'
    if marker in git('log', f"{os.environ['RELEASE_BASE']}..HEAD", '--format=%B', cwd=wt).stdout:
        res['status'] = 'already'
        return res

    is_merge = len(git('rev-list', '--parents', '-n1', sha, cwd=wt).stdout.split()) > 2
    args = ['cherry-pick', '-x', '--no-commit', '-Xignore-all-space'] + (['-m', '1'] if is_merge else []) + [sha]
    p = git(*args, cwd=wt)
    status = [(l[:2], l[3:]) for l in git('status', '--porcelain', cwd=wt).stdout.splitlines()]
    unmerged = [(x, f) for x, f in status if x in UNMERGED]
    if p.returncode and not unmerged:
        git('cherry-pick', '--abort', cwd=wt)
        res['status'] = 'failed'
        res['error'] = (p.stderr or p.stdout).strip()[-400:]
        return res

    for x, f in unmerged:
        if NOISE.search(f):
            keep_release(wt, f)
            res['kept_release'].append(f)
        elif x == 'DU':
            res['hunks'].append(whole_file_t3(wt, sha, f, iid, out_dir, 'file không có trên release'))
            git('rm', '-q', '-f', '--', f, cwd=wt)
        elif x == 'UD':
            res['hunks'].append(whole_file_t3(wt, sha, f, iid, out_dir, 'MR xoá file mà release còn dùng'))
            keep_release(wt, f)
        elif x != 'UU':
            res['hunks'].append(whole_file_t3(wt, sha, f, iid, out_dir, f'conflict {x}'))
            keep_release(wt, f)
        elif REGEN.search(f):
            res['hunks'].append(whole_file_t3(wt, sha, f, iid, out_dir, 'file sinh tự động (composer): chạy lại composer'))
            keep_release(wt, f)
        else:
            res['hunks'].extend(resolve_file(wt, sha, f, iid, out_dir))

    has_t3 = any(r['tier'] == 'T3' for r in res['hunks'])
    if not has_t3 and git('diff', '--cached', '--quiet', 'HEAD', cwd=wt).returncode == 0:
        git('cherry-pick', '--quit', cwd=wt)
        git('reset', '-q', '--hard', 'HEAD', cwd=wt)
        res['status'] = 'dropped'
        return res
    # Even with nothing applied, a T3-only MR gets its marker commit: hand fixes go in as --fixup of it
    msg = git('log', '-1', '--format=%B', sha, cwd=wt).stdout.rstrip() + f'\n\n({marker})\n'
    c = git('commit', '-q', '--allow-empty', '--no-verify', '-F', '-', cwd=wt, inp=msg)
    git('cherry-pick', '--quit', cwd=wt)
    if c.returncode:
        res['status'] = 'failed'
        res['error'] = c.stderr.strip()[-400:]
        return res
    res['new_sha'] = git('rev-parse', 'HEAD', cwd=wt).stdout.strip()
    res['status'] = 'clean' if not res['hunks'] else ('partial' if has_t3 else 'resolved')
    return res


def cmd_pick(ticket, wt, out_dir):
    mrs = ticket_mrs(ticket)
    if not mrs:
        sys.exit(f'ABORT: ticket #{ticket} không có trong ready[] của {os.environ["CLASSIFIED"]}')
    Path(out_dir).mkdir(parents=True, exist_ok=True)
    base_sha = git('rev-parse', 'HEAD', cwd=wt).stdout.strip()
    result = {'ticket': ticket, 'worktree': wt, 'branch': f'pick/{ticket}', 'base_sha': base_sha, 'mrs': []}
    for mr in mrs:
        r = pick_mr(wt, mr, out_dir)
        result['mrs'].append(r)
        n = {t: sum(1 for h in r['hunks'] if h['tier'] == t) for t in ('T1', 'T2', 'T3')}
        print(f"  MR !{r['iid']}: {r['status']}" + (f" — hunk T1 {n['T1']} · T2 {n['T2']} · T3 {n['T3']}" if r['hunks'] else '')
              + (f" — {r.get('error', '')}" if r['status'] == 'failed' else ''))
        if r['status'] == 'failed':
            break
    result['head_sha'] = git('rev-parse', 'HEAD', cwd=wt).stdout.strip()
    json.dump(result, open(Path(out_dir) / 'result.json', 'w'), ensure_ascii=False, indent=1)


# ---------- report ----------

SYMBOL_PATTERNS = [
    (re.compile(r'undefined (?:static )?method (\S+)::(\w+)\(\)'), 'method'),
    (re.compile(r'Call to static method \w+\(\) on an unknown class ([\w\\]+)'), 'class'),
    (re.compile(r'(?:Caught class|Instantiated class|Class|unknown class|extends unknown class|implements unknown interface|has invalid (?:return )?type) ([\w\\]+)'), 'class'),
    (re.compile(r'undefined function ([\w\\]+)\(\)'), 'function'),
    (re.compile(r'[Mm]ethod (\S+)::(\w+)\(\) invoked with'), 'signature'),
    (re.compile(r'Function ([\w\\]+) invoked with'), 'fsignature'),
    (re.compile(r'Constant (\w+) not found'), 'constant'),
    (re.compile(r'undefined (?:static )?property (\S+)::\$(\w+)'), 'property'),
]
MEMBER_KINDS = ('method', 'signature', 'property')


def symbol_of(msg):
    """{kind, name (short), full, owner (class of a member; 'X@anonymous/...' -> X)} or {}."""
    for rx, kind in SYMBOL_PATTERNS:
        m = rx.search(msg)
        if m:
            full = m.group(m.lastindex)
            owner = m.group(1).split('@', 1)[0] if kind in MEMBER_KINDS else ''
            return {'kind': kind, 'name': full.split('\\')[-1], 'full': full, 'owner': owner}
    return {}


def class_files(wt, rev, cls):
    """Files declaring <cls> and its parents (extends chain) at <rev>."""
    files, seen = [], set()
    while cls and cls not in seen and len(files) < 8:
        seen.add(cls)
        short = cls.split('\\')[-1]
        hits = git('grep', '-nE', rf'^[[:space:]]*((abstract|final)[[:space:]]+)*(class|interface|trait)[[:space:]]+{short}([^A-Za-z0-9_]|$)',
                   rev, '--', '*.php', '*.inc', cwd=wt).stdout.splitlines()
        if not hits:
            break
        _, path, _, text = hits[0].split(':', 3)
        files.append(path)
        parent = re.search(r'extends\s+([\\\w]+)', text)
        cls = parent.group(1) if parent else ''
    return files


def definition(kind, name):
    """(git grep args matching a definition of the symbol, substring for git log -S)."""
    if kind in ('method', 'function', 'signature', 'fsignature'):
        return ['-F', f'function {name}('], f'function {name}('
    if kind == 'class':
        return ['-E', rf'(class|interface|trait)[[:space:]]+{name}([^A-Za-z0-9_]|$)'], f'class {name}'
    if kind == 'property':
        return ['-E', rf'(public|protected|private|var)[^;=(]*\${name}([^A-Za-z0-9_]|$)'], f'${name}'
    return ['-F', name], name


def lookup(wt, sym):
    """Definition of the symbol on the pick branch (in the owner class chain for members) and the
    master commit that first added it there."""
    kind, name = sym['kind'], sym['name']
    owner = sym['owner']
    if owner in ('object', 'mixed', 'static', 'self'):
        return {'kind': kind, 'name': name, 'owner': owner, 'untyped': True, 'on_branch': [], 'origin': '', 'elsewhere': ''}
    grep_args, needle = definition(kind, name)
    anywhere = ['*.php', '*.inc']
    branch_paths = master_paths = anywhere
    info = {'kind': kind, 'name': name, 'owner': owner, 'elsewhere': ''}
    if owner:
        branch_paths = class_files(wt, 'HEAD', owner)
        master_paths = class_files(wt, 'origin/master', owner) or anywhere
        info['owner_missing'] = not branch_paths

    def search(b_paths, m_paths):
        hits = git('grep', '-n', *grep_args, 'HEAD', '--', *b_paths, cwd=wt).stdout.splitlines() if b_paths else []
        log = git('log', '-S', needle, '--format=%h %ad %s', '--date=short', 'origin/master', '--', *m_paths,
                  cwd=wt).stdout.strip().splitlines()
        return [l.split(':', 1)[1] for l in hits[:3]], (log[-1] if log else '')

    info['on_branch'], info['origin'] = search(branch_paths, master_paths)
    if owner and not info['on_branch'] and not info['origin']:
        # Not in the class chain on master either: PHPStan typed a base class, the method lives in a subclass
        hits, origin = search(anywhere, anywhere)
        info['elsewhere'] = hits[0].split(':', 1)[0] if hits else ''
        info['origin'] = origin
    if kind in ('signature', 'fsignature'):
        master = git('grep', '-n', *grep_args, 'origin/master', '--', *master_paths, cwd=wt).stdout.splitlines()
        info['master_def'] = [l.split(':', 1)[1] for l in master[:3]]
        if info['master_def']:
            # Origin of the master signature itself (not of the first version of the function)
            sig = info['master_def'][0].split(':', 2)[-1].strip()
            log = git('log', '-S', sig, '--format=%h %ad %s', '--date=short', 'origin/master', '--', *master_paths,
                      cwd=wt).stdout.strip().splitlines()
            info['origin'] = log[-1] if log else info['origin']
    return info


def builtin_arity(name):
    """(required, total) parameter counts of a PHP builtin function, or None."""
    code = (f'$f="{name}"; if (!function_exists($f)) exit; $r=new ReflectionFunction($f);'
            ' if ($r->isInternal()) echo $r->getNumberOfRequiredParameters()," ",$r->getNumberOfParameters();')
    p = subprocess.run(['php', '-r', code], capture_output=True, text=True)
    parts = p.stdout.split()
    return (int(parts[0]), int(parts[1])) if len(parts) == 2 else None


def known_noise(wt, base_sha, sym, msg):
    """Why an error is a PHPStan artefact of this setup rather than missing code, or ''."""
    if sym['kind'] == 'fsignature' and '\\' not in sym['full']:
        arity, n = builtin_arity(sym['name']), re.search(r'invoked with (\d+) parameter', msg)
        if arity and n and arity[0] <= int(n.group(1)) <= arity[1]:
            return (f"`{sym['name']}` là hàm builtin PHP nhận {arity[0]}–{arity[1]} tham số; PHPStan lấy chữ ký "
                    "của hàm cùng tên trong thư viện")
    if sym['kind'] == 'class' and '\\' not in sym['full']:
        grep_args, _ = definition('class', sym['name'])
        if git('grep', '-q', *grep_args, base_sha, '--', '*.php', '*.inc', cwd=wt).returncode == 0:
            return (f"class `{sym['full']}` đã có trên release trước khi pick (file legacy nạp bằng require), "
                    "PHPStan chỉ báo ở file mới")
    return ''


def psr4_prefixes(wt, rev):
    """Namespace prefixes composer really loads at <rev>: present in BOTH autoload_psr4.php and the
    $prefixDirsPsr4 map of autoload_static.php (the one autoload_real.php uses on PHP >= 5.6)."""
    def keys(text):
        return {p.replace('\\\\', '\\') for p in re.findall(r"^\s*'([^']+)'\s*=>", text, re.M)}
    psr4 = keys(git('show', f'{rev}:vendor/composer/autoload_psr4.php', cwd=wt).stdout)
    static = git('show', f'{rev}:vendor/composer/autoload_static.php', cwd=wt).stdout
    m = re.search(r'\$prefixDirsPsr4 = array \((.*?)\n    \);', static, re.S)
    return sorted(psr4 & keys(m.group(1)) if m else psr4)


def unloadable_prefix(cls, branch_prefixes, master_prefixes):
    """Master's PSR-4 prefix of a namespaced class when the branch has no prefix covering it."""
    if '\\' not in cls or any(cls.startswith(p) for p in branch_prefixes):
        return ''
    hits = sorted((p for p in master_prefixes if cls.startswith(p)), key=len, reverse=True)
    return hits[0] if hits else cls.rsplit('\\', 1)[0] + '\\'


def cmd_report(out_dir, phpstan_path):
    res = json.load(open(Path(out_dir) / 'result.json'))
    ps = json.load(open(phpstan_path)) if phpstan_path and os.path.exists(phpstan_path) else {'errors': []}
    wt = res['worktree']
    file_mr = {}
    for m in res['mrs']:
        if m.get('new_sha'):
            for f in git('diff', '--name-only', f"{m['new_sha']}^", m['new_sha'], cwd=wt).stdout.split():
                file_mr[f] = m['iid']

    hunks = [dict(h, iid=m['iid']) for m in res['mrs'] for h in m['hunks']]
    count = {t: sum(1 for h in hunks if h['tier'] == t) for t in ('T1', 'T2', 'T3')}
    out = [f"# Resolve ticket #{res['ticket']} — branch `{res['branch']}`", '',
           f"Worktree: `{wt}` · base `{res['base_sha'][:10]}` → head `{res['head_sha'][:10]}`", '',
           '| MR | Commit trên pick | Kết quả | T1 | T2 | T3 |', '|---|---|---|---|---|---|']
    label = {'clean': 'pick sạch', 'resolved': 'resolve hết (T1/T2)', 'partial': 'còn hunk T3',
             'dropped': 'không còn thay đổi', 'already': 'đã có trên branch', 'failed': 'LỖI'}
    for m in res['mrs']:
        n = {t: sum(1 for h in m['hunks'] if h['tier'] == t) for t in ('T1', 'T2', 'T3')}
        out.append(f"| !{m['iid']} | `{m.get('new_sha', '')[:10]}` | {label.get(m['status'], m['status'])} | {n['T1']} | {n['T2']} | {n['T3']} |")
        if m['status'] == 'failed':
            out.append(f"\n**MR !{m['iid']} lỗi:** `{m.get('error', '')}` — các MR sau chưa pick.\n")
    kept = sorted({f for m in res['mrs'] for f in m['kept_release']})
    if kept:
        out += ['', f"File không phát hành giữ bản release: {', '.join(f'`{f}`' for f in kept)}"]

    out += ['', f"## ● T2 — áp với ngữ cảnh rút gọn, cùng scope (cần đọc lại: {count['T2']})", '']
    t2 = [h for h in hunks if h['tier'] == 'T2']
    out += [f"- !{h['iid']} `{h['file']}` hunk {h['hunk']} — scope `{h['master_scope']}`" for h in t2] or ['_Không có_']

    out += ['', f"## ✗ T3 — chưa áp, người release quyết ({count['T3']})", '']
    t3 = [h for h in hunks if h['tier'] == 'T3']
    for h in t3:
        where = f" hunk {h['hunk']} (scope master `{h['master_scope']}`" + \
                (f", -C1 rơi vào `{h['placed_scope']}`" if h.get('placed_scope') else '') + ')' if h.get('hunk') else ''
        out.append(f"- !{h['iid']} `{h['file']}`{where} — **{h['reason']}**" + (f" · `{h['rej']}`" if h['rej'] else ''))
    if not t3:
        out.append('_Không có_')

    errs = ps.get('errors', [])
    out += ['', f"## PHPStan trước/sau trên `{res['branch']}` ({len(errs)} lỗi mới)", '']
    if ps.get('skipped'):
        out.append(f"PHPStan chưa chạy: {ps['skipped']}")
    branch_pfx, master_pfx = psr4_prefixes(wt, 'HEAD'), psr4_prefixes(wt, 'origin/master')
    ignored = 0
    groups = {}
    for e in errs:
        sym = symbol_of(e['message'])
        pfx = unloadable_prefix(sym['full'], branch_pfx, master_pfx) if sym.get('kind') == 'class' else ''
        key = ('autoload', pfx) if pfx else ((sym['kind'], sym['owner'], sym['full']) if sym else (None, '', re.sub(r'\d+', 'N', e['message'])))
        groups.setdefault(key, []).append((e, sym))
    for key, items in sorted(groups.items(), key=lambda kv: -len(kv[1])):
        errs_k = [e for e, _ in items]
        sym = items[0][1]
        mrs = sorted({file_mr[e['file']] for e in errs_k if e['file'] in file_mr})
        where = ', '.join(f"`{e['file']}:{e['line']}`" for e in errs_k[:3]) + (f" … (+{len(errs_k) - 3})" if len(errs_k) > 3 else '')
        by = f" — do MR {' '.join(f'!{i}' for i in mrs)}" if mrs else ''
        if key[0] == 'autoload':
            classes = sorted({s['full'] for _, s in items})
            out.append(f"- **namespace `{key[1]}` chưa đăng ký autoload trên branch** — {len(errs_k)} lỗi, {len(classes)} class "
                       f"(vd `{classes[0]}`) ở {where}{by}")
            out.append("  - class có file nhưng release không nạp được → fatal trên release. Xử lý T3 composer "
                       "(`composer.json` autoload + `composer dump-autoload`), rồi `--check`")
            continue
        if not sym:
            out.append(f"- {errs_k[0]['message']} — {len(errs_k)} lỗi [{errs_k[0].get('identifier', '')}] ở {where}{by}")
            continue
        label_sym = f"{sym['owner']}::{sym['full']}" if sym['owner'] else sym['full']
        out.append(f"- **{sym['kind']} `{label_sym}`** — {len(errs_k)} lỗi [{errs_k[0].get('identifier', '')}] ở {where}{by}")
        out.append(f"  - vd: {errs_k[0]['message']}")
        noise = known_noise(wt, res['base_sha'], sym, errs_k[0]['message'])
        if noise:
            out.append(f"  - **bỏ qua (PHPStan nhầm):** {noise}")
            ignored += len(errs_k)
            continue
        info = lookup(wt, sym)
        if info.get('untyped'):
            out.append(f"  - PHPStan không biết kiểu biến (`{info['owner']}`) → kiểm tay đối tượng gọi `{info['name']}`")
            continue
        defined = info['on_branch'][0].strip()[:110] if info['on_branch'] else ''
        if info.get('owner_missing'):
            out.append(f"  - class `{info['owner']}` không có trên branch")
        if info['kind'] in ('signature', 'fsignature'):
            out.append(f"  - chữ ký trên branch: `{defined or '?'}` · master: `{(info.get('master_def') or ['?'])[0].strip()[:110]}`")
        elif defined and info['kind'] == 'class':
            out.append(f"  - có định nghĩa trên branch (`{defined}`) nhưng không nạp được → kiểm autoload (composer classmap) hoặc require")
        elif defined:
            out.append(f"  - có định nghĩa trong class trên branch (`{defined}`) → kiểm tay: PHPStan có thể suy sai kiểu biến")
        elif info['elsewhere']:
            out.append(f"  - không có trong chuỗi class `{info['owner']}`, nhưng có ở `{info['elsewhere']}` trên branch "
                       "→ PHPStan suy kiểu class cha; kiểm tay biến có đúng là class đó không")
        if (not defined and not info['elsewhere']) or info['kind'] in ('signature', 'fsignature'):
            src = info['origin'].split(' ', 1)[0] if info['origin'] else ''
            own = [m['iid'] for m in res['mrs'] if src and m.get('sha')
                   and git('merge-base', '--is-ancestor', src, m['sha'], cwd=wt).returncode == 0
                   and git('merge-base', '--is-ancestor', src, f"{m['sha']}^1", cwd=wt).returncode != 0]
            if own:
                out.append(f"  - code này thuộc chính MR !{own[0]} ({info['origin']}) → nằm trong hunk T3 chưa áp của ticket, xử lý T3 trước")
            else:
                out.append(f"  - code `{info['name']}` vào master ở: {info['origin'] or 'không tìm thấy'} → code thiếu, mang sang hoặc hỏi")
    if not errs and not ps.get('skipped'):
        out.append('_Không có_')
    if ignored:
        out += ['', f"→ {ignored}/{len(errs)} lỗi là PHPStan nhầm (đánh dấu bỏ qua); còn {len(errs) - ignored} lỗi cần xử lý."]

    # Parse check with the PHP that release core runs (lint-runtime.sh): master moved to 8.3 syntax
    lint_path = Path(out_dir) / 'lint.txt'
    lint_lines = [l for l in lint_path.read_text().splitlines() if l.strip()] if lint_path.exists() else []
    lint_skipped = bool(lint_lines) and lint_lines[0].startswith('SKIPPED:')
    lint_errs = [] if lint_skipped else lint_lines
    out += ['', f"## Cú pháp trên PHP của release ({len(lint_errs)} file lỗi)", '']
    if lint_skipped or not lint_path.exists():
        out.append(f"Chưa chạy: {lint_lines[0][9:] if lint_skipped else 'chưa có lint.txt'}")
    elif lint_errs:
        out += [f"- `{l}`" for l in lint_errs]
        out.append('  - sửa trên pick (vd typed property → `/** @var T */` + property không kiểu, `fn() =>` → `function () use ()`, `??=` → `$a = $a ?? b`)')
    else:
        out.append('_Không có_')

    out += ['', '## Bước tiếp', '',
            '1. Đọc T2; xử lý từng T3 trong worktree: áp tay đúng dòng của MR (file `.rej`), hoặc bỏ nếu code đó không có trên release. '
            'Lưu thành `git commit --fixup <commit trên pick của MR>` để dòng resolve nằm trong commit có marker của MR.',
            '2. Mang code thiếu (PHPStan ở trên) sang thành commit riêng, message ghi commit master nguồn. '
            'Code thiếu thuộc ticket chưa release, hoặc không mang sang được → hỏi trước khi làm.',
            f"   Gộp fixup: `GIT_SEQUENCE_EDITOR=true git -C {wt} rebase -q -i --autosquash {res['base_sha'][:10]}`",
            f"3. Chạy lại kiểm tra: `resolve.sh {res['ticket']} --check`",
            f"4. Merge vào branch tháng + gắn label: `resolve.sh {res['ticket']} --merge`. Ticket resolve tay → test lại đầy đủ trên release."]
    print('\n'.join(out))
    json.dump({'head_sha': res['head_sha'], 'T2': count['T2'], 'T3': count['T3'], 'phpstan': len(errs) - ignored,
               'phpstan_ignored': ignored, 'phpstan_skipped': bool(ps.get('skipped')),
               'lint': len(lint_errs), 'lint_skipped': lint_skipped or not lint_path.exists()},
              open(Path(out_dir) / 'check.json', 'w'))


def cmd_list(bundle_result):
    data = json.load(open(bundle_result))
    for c in data.get('conflicts_manual', []):
        iids = ' '.join(f'!{i}' for i in (c.get('iids') or [c['iid']]))
        print(f"#{c['ticket']}\t{iids}\t@{c.get('author', '?')}")


if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else ''
    if cmd == 'pick' and len(sys.argv) == 5:
        cmd_pick(*sys.argv[2:5])
    elif cmd == 'report' and len(sys.argv) == 4:
        cmd_report(*sys.argv[2:4])
    elif cmd == 'list' and len(sys.argv) == 3:
        cmd_list(sys.argv[2])
    else:
        sys.exit(__doc__)

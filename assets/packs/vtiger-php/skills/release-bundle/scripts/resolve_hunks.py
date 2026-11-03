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
    data = json.load(open(os.environ['CLASSIFIED']))
    mrs = [m for m in data.get('ready', []) if ticket in (m.get('ticket_ids') or [m.get('ticket_id')])]
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
        if apply_hunk(wt, head, h, None):
            row['tier'] = 'T1'
        elif apply_hunk(wt, head, h, 1):
            placed = placed_scopes(wt, path, h)
            row['placed_scope'] = ' | '.join(placed)
            if row['master_scope'] and row['master_scope'] in placed:
                row['tier'] = 'T2'
            else:
                git('checkout', '--', path, cwd=wt)  # back to the index copy: hunk landed elsewhere
                row['tier'] = 'T3'
                row['reason'] = 'chỉ khớp -C1 nhưng nằm ở scope khác'
        else:
            row['tier'] = 'T3'
            row['reason'] = reject_kind(wt, path, h)
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
    (re.compile(r'(?:Instantiated class|Class|unknown class|extends unknown class|implements unknown interface) ([\w\\]+)'), 'class'),
    (re.compile(r'undefined function ([\w\\]+)\(\)'), 'function'),
    (re.compile(r'Method (\S+)::(\w+)\(\) invoked with'), 'signature'),
    (re.compile(r'Function ([\w\\]+) invoked with'), 'fsignature'),
    (re.compile(r'Constant (\w+) not found'), 'constant'),
    (re.compile(r'undefined (?:static )?property (\S+)::\$(\w+)'), 'property'),
]


def symbol_of(msg):
    """(kind, short name for grep, full name for display) of the symbol an error is about."""
    for rx, kind in SYMBOL_PATTERNS:
        m = rx.search(msg)
        if m:
            full = m.group(m.lastindex)
            return kind, full.split('\\')[-1], full
    return None, None, None


def definition(kind, name):
    """(git grep args matching a definition of the symbol, substring for git log -S)."""
    if kind in ('method', 'function', 'signature', 'fsignature'):
        return ['-F', f'function {name}('], f'function {name}('
    if kind == 'class':
        return ['-E', rf'(class|interface|trait) {name}([^A-Za-z0-9_]|$)'], f'class {name}'
    if kind == 'property':
        return ['-E', rf'(public|protected|private|var)[^;=(]*\${name}([^A-Za-z0-9_]|$)'], f'${name}'
    return ['-F', name], name


def lookup(wt, err):
    """Where the missing symbol lives: defined on the pick branch already, and which master commit added it."""
    kind, name, _ = symbol_of(err['message'])
    if not kind:
        return {}
    grep_args, needle = definition(kind, name)
    on_branch = git('grep', '-n', *grep_args, 'HEAD', '--', '*.php', '*.inc', cwd=wt).stdout.splitlines()
    info = {'kind': kind, 'name': name, 'on_branch': [l.split(':', 1)[1] for l in on_branch[:3]]}
    log = git('log', '-S', needle, '--format=%h %ad %s', '--date=short', 'origin/master', '--', '*.php', '*.inc',
              cwd=wt).stdout.strip().splitlines()
    info['origin'] = log[-1] if log else ''
    if kind in ('signature', 'fsignature'):
        master = git('grep', '-n', *grep_args, 'origin/master', '--', '*.php', '*.inc', cwd=wt).stdout.splitlines()
        info['master_def'] = [l.split(':', 1)[1] for l in master[:3]]
    return info


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
    groups = {}
    for e in errs:
        kind, _, full = symbol_of(e['message'])
        key = (kind, full) if kind else (None, re.sub(r'\d+', 'N', e['message']))
        groups.setdefault(key, []).append(e)
    for (kind, what), items in sorted(groups.items(), key=lambda kv: -len(kv[1])):
        mrs = sorted({file_mr[e['file']] for e in items if e['file'] in file_mr})
        where = ', '.join(f"`{e['file']}:{e['line']}`" for e in items[:3]) + (f" … (+{len(items) - 3})" if len(items) > 3 else '')
        head = f"- **{kind} `{what}`**" if kind else f"- {items[0]['message']}"
        out.append(f"{head} — {len(items)} lỗi [{items[0].get('identifier', '')}] ở {where}"
                   + (f" — do MR {' '.join(f'!{i}' for i in mrs)}" if mrs else ''))
        if kind:
            out.append(f"  - vd: {items[0]['message']}")
        info = lookup(wt, items[0])
        if not info:
            continue
        defined = info['on_branch'][0].strip()[:110] if info['on_branch'] else ''
        if info['kind'] in ('signature', 'fsignature'):
            out.append(f"  - chữ ký trên branch: `{defined or '?'}` · master: `{(info.get('master_def') or ['?'])[0].strip()[:110]}`")
        elif defined and info['kind'] == 'class':
            out.append(f"  - có định nghĩa trên branch (`{defined}`) nhưng không nạp được → kiểm autoload (composer PSR-4/classmap) hoặc require")
        elif defined:
            out.append(f"  - có định nghĩa trên branch (`{defined}`) → kiểm tay: PHPStan có thể suy sai kiểu biến")
        if not defined or info['kind'] in ('signature', 'fsignature'):
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

    out += ['', '## Bước tiếp', '',
            '1. Đọc T2; xử lý từng T3 trong worktree: áp tay đúng dòng của MR (file `.rej`), hoặc bỏ nếu code đó không có trên release. '
            'Lưu thành `git commit --fixup <commit trên pick của MR>` để dòng resolve nằm trong commit có marker của MR.',
            '2. Mang code thiếu (PHPStan ở trên) sang thành commit riêng, message ghi commit master nguồn. '
            'Code thiếu thuộc ticket chưa release, hoặc không mang sang được → hỏi trước khi làm.',
            f"   Gộp fixup: `GIT_SEQUENCE_EDITOR=true git -C {wt} rebase -q -i --autosquash {res['base_sha'][:10]}`",
            f"3. Chạy lại kiểm tra: `resolve.sh {res['ticket']} --check`",
            f"4. Merge vào branch tháng + gắn label: `resolve.sh {res['ticket']} --merge`. Ticket resolve tay → test lại đầy đủ trên release."]
    print('\n'.join(out))
    json.dump({'head_sha': res['head_sha'], 'T2': count['T2'], 'T3': count['T3'], 'phpstan': len(errs),
               'phpstan_skipped': bool(ps.get('skipped'))},
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

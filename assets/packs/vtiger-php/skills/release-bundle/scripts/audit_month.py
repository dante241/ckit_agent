#!/usr/bin/env python3
"""audit_month.py <checkout> <base> <branch> — whole-branch "was any code lost" audit.

Reads the "(cherry picked from commit <sha>)" markers on <base>..<branch> and checks, file by file,
with whitespace and CR stripped:

  A. every line an MR's master patch (<sha>^1..<sha>) adds is in the branch file.
     A line missing from the branch AND from master tip counts as superseded only when a LATER
     released MR removes it from the SAME file; otherwise it is reported.
  B. every line an MR's patch removes is gone from the branch, unless master tip still has it
     or a released MR adds it back (stale code left behind by a resolve).
  C. every release line the branch deletes is a line some released MR deletes in that file.

Matching is by line content per file (multiset). A short generic line (`}`, `return;`) is weak
evidence either way, so lines under 3 characters are skipped. Paths that never ship are skipped.
Exit 0 = nothing to read, 1 = findings printed.
"""
import collections
import re
import subprocess
import sys

SKIP = re.compile(r'^(CHANGELOG|agents/|docs/|tools/|\.omp/|\.claude/|tests?/|vendor/)|/tests/')
MARK = re.compile(r'cherry picked from commit ([0-9a-f]{40})')


def git(*args):
    return subprocess.run(['git', '-C', CHECKOUT, *args], capture_output=True, text=True, errors='replace').stdout


def norm(line):
    return re.sub(r'\s+', '', line)


_files = {}


def lines(ref, path):
    if (ref, path) not in _files:
        r = subprocess.run(['git', '-C', CHECKOUT, 'show', f'{ref}:{path}'], capture_output=True)
        _files[(ref, path)] = None if r.returncode else collections.Counter(
            norm(x) for x in r.stdout.decode('utf-8', 'replace').splitlines())
    return _files[(ref, path)]


def patch(a, b, extra=()):
    """{path: (added, removed)} of real lines in a..b"""
    out = collections.defaultdict(lambda: ([], []))
    path = None
    for l in git('diff', '-U0', '--no-renames', *extra, a, b).splitlines():
        if l.startswith('--- '):
            old = None if l == '--- /dev/null' else l[6:]
        elif l.startswith('+++ '):
            path = old if l == '+++ /dev/null' else l[6:]
        elif path and not SKIP.search(path) and l[:1] in '+-' and len(norm(l[1:])) > 2:
            out[path][0 if l[0] == '+' else 1].append(l[1:].strip())
    return out


def main():
    picks = []
    for rec in git('log', '--reverse', '--no-merges', '--format=%H%x00%B%x01', f'{BASE}..{BRANCH}').split('\x01'):
        if rec.strip():
            h, body = rec.strip().split('\x00', 1)
            m = MARK.search(body)
            if m:
                picks.append((m.group(1), h, body.splitlines()[0][:60]))
    order = [p[0] for p in picks]
    title = {p[0]: p[2] for p in picks}
    pat = {src: patch(f'{src}^1', src) for src in order}
    removed_later = lambda i, path, n: [s for s in order[i + 1:] if n in map(norm, pat[s][path][1])]
    added_any = collections.defaultdict(collections.Counter)
    removed_any = collections.defaultdict(collections.Counter)
    for s in order:
        for path, (add, rem) in pat[s].items():
            added_any[path].update(map(norm, add))
            removed_any[path].update(map(norm, rem))

    found = 0
    landed = superseded = 0
    print(f'== A. Dòng MR thêm chưa lên branch ({len(order)} pick)')
    for i, src in enumerate(order):
        for path, (add, _) in pat[src].items():
            if not add:
                continue
            br = lines(BRANCH, path)
            if br is None:
                print(f'  FILE-BỎ   {src[:10]} {title[src]} | {path}')
                found += 1
                continue
            ms = lines('origin/master', path)
            for a in add:
                n = norm(a)
                if br[n]:
                    landed += 1
                elif (ms is None or not ms[n]) and removed_later(i, path, n):
                    superseded += 1
                else:
                    print(f'  THIẾU     {src[:10]} {title[src]} | {path} | {a[:100]}')
                    found += 1
    print(f'  (có {landed} dòng · {superseded} dòng bị MR sau cùng đợt xoá trong cùng file)')

    print('== B. Dòng MR xoá còn sót trên branch')
    for src in order:
        for path, (_, rem) in pat[src].items():
            br = lines(BRANCH, path)
            ms = lines('origin/master', path)
            if br is None:
                continue
            for r in rem:
                n = norm(r)
                if br[n] and (ms is None or not ms[n]) and not added_any[path][n]:
                    print(f'  CÒN SÓT   {src[:10]} {title[src]} | {path} | {r[:100]}')
                    found += 1

    print('== C. Dòng của release bị branch xoá mà không MR nào xoá')
    budget = {p: collections.Counter(c) for p, c in removed_any.items()}
    for path, (_, rem) in patch(BASE, BRANCH, ['--ignore-cr-at-eol']).items():
        for r in rem:
            n = norm(r)
            if budget.get(path, {}).get(n, 0) > 0:
                budget[path][n] -= 1
            else:
                print(f'  RELEASE-MẤT {path} | {r[:100]}')
                found += 1
    print(f'Tổng cần đọc: {found}')
    return 1 if found else 0


if __name__ == '__main__':
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    CHECKOUT, BASE, BRANCH = sys.argv[1:]
    sys.exit(main())

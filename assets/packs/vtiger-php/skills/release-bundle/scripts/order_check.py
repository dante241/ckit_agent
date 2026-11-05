#!/usr/bin/env python3
"""order_check.py <checkout> <base> <branch> <slim-json>... — find code a pick-order inversion lost.

Picks are read from the "(cherry picked from commit <sha>)" markers on <base>..<branch>; their master
merge time from the mrs-slim-*.json files of /release-check. For every pair picked in the opposite order
to master's merged_at that changes the same file, the file is rebuilt from <base> by applying each
batch MR's own patch (<sha>^1..<sha>) in merged_at order with `git merge-file`, then compared with the
branch file (whitespace and CR ignored).

Per file:
  SAME      branch == merge-order rebuild
  CONFLICT  the rebuild itself conflicts (release lacks code a patch edits): read the listed lines
  DIFF      rebuild clean but branch differs: likely lost or misplaced code, read every line
Exit 0 = every inverted file is SAME, 1 = something to read.
"""
import json
import os
import re
import subprocess
import sys
import tempfile

SKIP = re.compile(r'^(CHANGELOG\.md|agents/|docs/|tools/|\.omp/|\.claude/)')
MARK = re.compile(r'cherry picked from commit ([0-9a-f]{40})')


def git(*args, text=True):
    return subprocess.run(['git', '-C', CHECKOUT, *args], capture_output=True, text=text,
                          errors='replace' if text else None)


def norm_lines(data):
    return [re.sub(r'\s+', '', l) for l in data.decode('utf-8', 'replace').splitlines() if l.strip()]


def main():
    merged = {}
    for path in SLIMS:
        for m in json.load(open(path)):
            for k in ('merge_commit_sha', 'squash_commit_sha', 'sha'):
                if m.get(k):
                    merged[m[k][:12]] = (m.get('merged_at') or '', m['iid'])
    picks = []
    for rec in git('log', '--reverse', '--no-merges', '--format=%x01%B', f'{BASE}..{BRANCH}').stdout.split('\x01'):
        m = MARK.search(rec)
        if m and m.group(1)[:12] in merged:
            picks.append((m.group(1),) + merged[m.group(1)[:12]])
    files = {sha: {f for f in git('diff', '--name-only', f'{sha}^1', sha).stdout.split() if not SKIP.search(f)}
             for sha, _, _ in picks}

    inverted = {}
    for i, (sa, ta, ia) in enumerate(picks):
        for sb, tb, ib in picks[i + 1:]:
            if tb < ta:
                for f in files[sa] & files[sb]:
                    inverted.setdefault(f, set()).update({ia, ib})
    if not inverted:
        print('Không có cặp pick ngược thứ tự merge sửa chung file.')
        return 0

    tmp = tempfile.mkdtemp()
    ours, base, theirs = (os.path.join(tmp, n) for n in ('ours', 'base', 'theirs'))
    by_time = sorted(picks, key=lambda p: p[1])
    bad = 0
    for f in sorted(inverted):
        cur = git('show', f'{BASE}:{f}', text=False).stdout
        conflicts = []
        for sha, _, iid in by_time:
            if f not in files[sha]:
                continue
            for p, data in ((ours, cur), (base, git('show', f'{sha}^1:{f}', text=False).stdout),
                            (theirs, git('show', f'{sha}:{f}', text=False).stdout)):
                open(p, 'wb').write(data)
            r = subprocess.run(['git', 'merge-file', '-p', ours, base, theirs], capture_output=True)
            if r.returncode:
                conflicts.append(iid)
            cur = r.stdout
        branch = git('show', f'{BRANCH}:{f}', text=False).stdout
        rebuilt = [l for l in norm_lines(cur) if not l.startswith(('<<<<<<<', '|||||||', '=======', '>>>>>>>'))]
        have = set(norm_lines(branch))
        missing = [l for l in rebuilt if l not in have]
        mrs = ' '.join(f'!{i}' for i in sorted(inverted[f]))
        if not missing:
            print(f'SAME      {f}  ({mrs})')
            continue
        bad += 1
        state = 'CONFLICT' if conflicts else 'DIFF    '
        extra = f"; rebuild conflicts at {' '.join(f'!{i}' for i in conflicts)}" if conflicts else ''
        print(f'{state}  {f}  ({mrs}{extra}) — {len(missing)} rebuilt lines missing on branch:')
        for l in missing[:8]:
            print(f'            {l[:140]}')
    print(f'Cần đọc: {bad} file')
    return 1 if bad else 0


if __name__ == '__main__':
    if len(sys.argv) < 5:
        sys.exit(__doc__)
    CHECKOUT, BASE, BRANCH, *SLIMS = sys.argv[1:]
    sys.exit(main())

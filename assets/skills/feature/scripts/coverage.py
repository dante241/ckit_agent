#!/usr/bin/env python3
"""Spec <-> plan coverage for a /feature that has an external spec.

  python3 coverage.py [FEATURE_DIR] [--spec FILE] [--phase Mx]

FEATURE_DIR defaults to the active feature (agents/planning/ACTIVE.md).
Spec path: --spec, else the `Spec: `<file>`` line in COVERAGE.md (relative to FEATURE_DIR).

Check mode (no --phase): every spec code and every US-xx AC-yy must have a row in
COVERAGE.md; prints counts per status; exit 1 when a row is missing or still GAP.
--phase Mx: prints the spec codes whose Phase cell covers Mx (ranges M4-M6 included),
which is the "read before planning" list for that phase.
"""
import argparse
import re
import sys
from pathlib import Path

CODE_RE = re.compile(r'\b(BO-\d+|UC-\d+|BR-\d+|P-[A-Z]{2}\d+|D-[A-Z]{2}\d+|W-[A-Z]{2}\d+|SCR-\w+)\b')
RANGE_RE = re.compile(r'\bM(\d+)\s*[-–]\s*M(\d+)\b')
PHASE_RE = re.compile(r'\bM(\d+|[A-Z][A-Z0-9]*)\b')


def active_feature_dir() -> Path:
    active = Path('agents/planning/ACTIVE.md')
    if not active.exists():
        sys.exit('no FEATURE_DIR given and agents/planning/ACTIVE.md not found')
    text = re.sub(r'<!--.*?-->', '', active.read_text(encoding='utf-8'), flags=re.S)
    for line in text.splitlines():
        if line.strip():
            return Path('agents/planning') / line.strip()
    sys.exit('ACTIVE.md has no active slug')


def spec_codes(spec: str, code_re):
    codes = list(dict.fromkeys(m.group(0) for m in code_re.finditer(spec)))
    acs, us = [], None
    for line in spec.splitlines():
        m = re.search(r'\*\*(US-\d+)\*\*', line)
        if m:
            us = m.group(1)
        m = re.match(r'\|\s*(AC-\d+)\s*\|', line)
        if m and us:
            acs.append(f'{us}·{m.group(1)}')
    return codes, list(dict.fromkeys(acs))


def coverage_rows(text: str):
    """code -> (phase cell, status cell) for every table row whose first cell is a code."""
    rows, phase_col = {}, None
    for line in text.splitlines():
        if not line.lstrip().startswith('|'):
            phase_col = None
            continue
        cells = [c.strip() for c in line.strip().strip('|').split('|')]
        if all(set(c) <= set('-: ') for c in cells):
            continue
        if phase_col is None:
            phase_col = next((i for i, c in enumerate(cells) if c.lower() == 'phase'), -1)
            continue
        if len(cells) >= 3 and cells[0]:
            phase = cells[phase_col] if 0 <= phase_col < len(cells) else ''
            rows[cells[0]] = (phase, cells[-1])
    return rows


def phases_in(cell: str):
    found = set()
    for a, b in RANGE_RE.findall(cell):
        found.update(f'M{n}' for n in range(int(a), int(b) + 1))
    found.update(f'M{p}' for p in PHASE_RE.findall(cell))
    return found


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('feature_dir', nargs='?')
    ap.add_argument('--spec')
    ap.add_argument('--phase')
    ap.add_argument('--code-re', help='regex for spec codes (else the "Codes: `<regex>`" line in COVERAGE.md, else built-in)')
    args = ap.parse_args()

    fdir = Path(args.feature_dir) if args.feature_dir else active_feature_dir()
    cov_path = fdir / 'COVERAGE.md'
    if not cov_path.exists():
        sys.exit(f'{cov_path} not found (feature without external spec: REQUIREMENTS.md is the source)')
    coverage = cov_path.read_text(encoding='utf-8')
    spec_path = Path(args.spec) if args.spec else None
    if spec_path is None:
        m = re.search(r'^>?\s*Spec:\s*`([^`]+)`\s*$', coverage, re.M)
        if not m:
            sys.exit('COVERAGE.md has no "Spec: `<file>`" line; pass --spec')
        spec_path = fdir / m.group(1)
    m = re.search(r'^>?\s*Codes:\s*`([^`]+)`\s*$', coverage, re.M)
    code_re = re.compile(args.code_re or (m.group(1) if m else CODE_RE.pattern))
    codes, acs = spec_codes(spec_path.read_text(encoding='utf-8'), code_re)
    if not codes:
        sys.exit(f'FAIL - no spec code matched {code_re.pattern!r} in {spec_path}')
    rows = coverage_rows(coverage)

    if args.phase:
        want = args.phase.upper()
        groups = {}
        for key in codes + acs:
            phase, status = rows.get(key, ('', 'MISSING'))
            if want in phases_in(phase):
                prefix = 'US·AC' if '·' in key else key.split('-')[0]
                groups.setdefault(prefix, []).append(key if status.startswith('OK') else f'{key} [{status}]')
        if not groups:
            sys.exit(f'no spec code mapped to {want}')
        order = ['BO', 'UC', 'BR', 'D', 'W', 'P', 'SCR']
        for prefix in sorted(groups, key=lambda p: (p == 'US·AC', order.index(p) if p in order else len(order), p)):
            print(f'{prefix:6} {", ".join(groups[prefix])}')
        return

    by_status = {}
    for key in codes + acs:
        status = rows.get(key, ('', 'MISSING'))[1]
        by_status.setdefault(re.split(r'[\s→]', status)[0] or 'EMPTY', []).append(key)
    print(f'spec: {len(codes)} codes, {len(acs)} US·AC')
    for status, keys in sorted(by_status.items()):
        shown = '' if status == 'OK' else ' ' + ', '.join(keys)
        print(f'  {status:8} {len(keys):3}{shown}')
    bad = by_status.get('MISSING', []) + by_status.get('GAP', []) + by_status.get('EMPTY', [])
    if bad:
        print('FAIL - missing/GAP:', ', '.join(bad))
        sys.exit(1)
    print('PASS - every spec code and US·AC has a row; no open GAP')


if __name__ == '__main__':
    main()

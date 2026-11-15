#!/usr/bin/env python3
"""Mechanical gates for one /feature phase: the parts a model must not judge for itself.

  python3 phase_gate.py [FEATURE_DIR] --phase Mx [--check plan|verification|all]

FEATURE_DIR defaults to the active feature (agents/planning/ACTIVE.md).

plan          Mx-NN-PLAN.md: a task that carries [AC:] must have [verify:]; when
              its tier is not verify-only, the verify must contain a real check,
              not only syntax/existence probes (php -l, node --check, test -s, ls, cat).
              [trivial] exempts a task. A must-test task without [red: <tests>] is a warning,
              and so is a [red:] task with no "Hợp đồng:" line (its tests have no stated API to
              bind to, so the red-first fairness audit tends to refuse them).
              Mx-CONTEXT.md: every rule code in the AC table's Nguồn column (AR-12, BR-04...;
              not UC/US/AC) must be quoted under "Luật trích" - task specs carry only the
              CONTEXT, so an unquoted rule reaches no dev, tester or reviewer. A must-test AC
              not carried by an [acceptance] task with a [verify:] is a warning.
verification  Mx-VERIFICATION.md: every Verdict cell is exactly PASS, FAIL or
              NEEDS-CONFIRM ("PASS (… NEEDS-CONFIRM)" is a NEEDS-CONFIRM in disguise);
              every NEEDS-CONFIRM AC has a row with an owner in the NEEDS-CONFIRM table
              and a line naming the phase and the AC under STATE.md Blockers.
all           both (default).

Prints the verdict counts and DONE / CONDITIONAL / NOT DONE; exit 1 on any error.
"""
import argparse
import re
import sys
from pathlib import Path

VERDICTS = {'PASS', 'FAIL', 'NEEDS-CONFIRM'}
AC_RE = re.compile(r'\bAC-\d+\b')
TAG_RE = re.compile(r'\[([a-zA-Z]+):\s*([^\]]*)\]')
TASK_RE = re.compile(r'^\s*-\s*\[[ xX~]\]\s*(T\d+|TZ)\b(.*)$')
# The task's contract line: `  - Hợp đồng: …` under the task, or inline in it.
CONTRACT_RE = re.compile(r'(Hợp đồng|Contract)\s*:', re.I)
# Rule codes a Nguồn cell may cite; UC/US/AC are requirement ids, not rules to quote.
RULE_RE = re.compile(r'\b([A-Z]{1,4})-\d+[a-z]?\b')
NOT_RULES = {'UC', 'US', 'AC'}
# Checks that prove only that a file parses or exists, never behaviour.
WEAK = [re.compile(p) for p in (
    r'^(\$\w+|php)\s+-l\b', r'^node\s+--check\b', r'^(test|\[)\s+-[sfed]\b',
    r'^(ls|cat|true|echo|stat|wc)\b',
)]


def active_feature_dir() -> Path:
    active = Path('agents/planning/ACTIVE.md')
    if not active.exists():
        sys.exit('no FEATURE_DIR given and agents/planning/ACTIVE.md not found')
    text = re.sub(r'<!--.*?-->', '', active.read_text(encoding='utf-8'), flags=re.S)
    for line in text.splitlines():
        if line.strip():
            return Path('agents/planning') / line.strip()
    sys.exit('agents/planning/ACTIVE.md names no feature')


def phase_dir(feature: Path, phase: str) -> Path:
    hits = sorted((feature / 'phases').glob(f'{phase}-*')) + sorted((feature / 'phases').glob(phase))
    if not hits:
        sys.exit(f'no phase directory {phase}-* under {feature}/phases')
    return hits[0]


def cells(line: str) -> list[str]:
    return [c.strip() for c in line.strip().strip('|').split('|')]


def section(text: str, heading_re: str) -> str:
    """Body of the first markdown section whose heading matches, up to the next heading of the same or higher level."""
    m = re.search(rf'^(#+)\s*{heading_re}.*$', text, flags=re.M | re.I)
    if not m:
        return ''
    level = len(m.group(1))
    rest = text[m.end():]
    end = re.search(rf'^#{{1,{level}}}\s', rest, flags=re.M)
    return rest[:end.start()] if end else rest


def is_weak(cmd: str) -> bool:
    parts = [p.strip().lstrip('(').strip() for p in re.split(r'&&|\|\||;|\s\+\s', cmd) if p.strip()]
    return bool(parts) and all(any(w.search(p) for w in WEAK) for p in parts)


def check_context(pdir: Path, acceptance_acs: set[str], errors: list[str], warnings: list[str]) -> None:
    ctx = sorted(pdir.glob('*-CONTEXT.md'))
    if not ctx:
        return
    text = ctx[0].read_text(encoding='utf-8')
    quoted = section(text, r'Luật trích')
    src_col = tier_col = None
    missing: dict[str, list[str]] = {}
    must_test_acs: set[str] = set()
    for line in text.splitlines():
        if not line.lstrip().startswith('|'):
            src_col = tier_col = None  # a table ends at its first non-row line
            continue
        row = cells(line)
        if 'Nguồn' in row:
            src_col = row.index('Nguồn')
            tier_col = row.index('Tier') if 'Tier' in row else None
            continue
        if src_col is None or not row or not AC_RE.fullmatch(row[0]) or len(row) <= src_col:
            continue
        ac = row[0]
        for code in dict.fromkeys(m.group(0) for m in RULE_RE.finditer(row[src_col])):
            if code.split('-')[0] not in NOT_RULES and not re.search(rf'\b{re.escape(code)}\b', quoted):
                missing.setdefault(code, []).append(ac)
        if tier_col is not None and len(row) > tier_col and row[tier_col].strip() == 'must-test':
            must_test_acs.add(ac)
    for code, acs in missing.items():
        errors.append(f'{ctx[0].name}: {code} (cited by {",".join(acs)}) is not quoted under "Luật trích" — copy the rule verbatim so task specs carry it')
    uncovered = sorted(must_test_acs - acceptance_acs)
    if uncovered:
        warnings.append(f'{pdir.name}: must-test {",".join(uncovered)} not carried by an [acceptance] task with [verify:] — add T0 (write the acceptance suite) + TZ (run it)')


def check_plan(pdir: Path, errors: list[str], warnings: list[str]) -> None:
    plans = sorted(pdir.glob('*-PLAN.md'))
    if not plans:
        errors.append(f'{pdir.name}: no *-PLAN.md')
        return
    acceptance_acs: set[str] = set()
    for plan in plans:
        lines = plan.read_text(encoding='utf-8').splitlines()
        for i, line in enumerate(lines):
            m = TASK_RE.match(line)
            if not m:
                continue
            key, rest = m.group(1), m.group(2)
            tags = {k.lower(): v.strip() for k, v in TAG_RE.findall(rest)}
            trivial = bool(re.search(r'\[trivial\]', rest, flags=re.I))
            acceptance = bool(re.search(r'\[acceptance\]', rest, flags=re.I))
            acs = AC_RE.findall(tags.get('ac', ''))
            if acceptance and tags.get('verify', '').strip() not in {'', '—', '-'}:
                acceptance_acs.update(acs)
            where = f'{plan.name} {key}'
            # Indented lines right under the task belong to it (until the next task or a blank/unindented line).
            body = [rest]
            for nxt in lines[i + 1:]:
                if TASK_RE.match(nxt) or not nxt.startswith((' ', '\t')):
                    break
                body.append(nxt)
            if 'red' in tags and not any(CONTRACT_RE.search(b) for b in body):
                warnings.append(f'{where}: [red:] task without a "Hợp đồng:" line — state the API its tests may bind to (signatures, error codes, file layout)')
            if trivial and acs:
                errors.append(f'{where}: [trivial] but carries {",".join(acs)} — a task that proves an AC needs a [verify:]')
                continue
            if not acs or trivial:
                continue
            verify = tags.get('verify', '')
            if not verify or verify in {'—', '-'}:
                errors.append(f'{where}: carries {",".join(acs)} but has no [verify:]')
                continue
            tier = tags.get('tier', '')
            if 'verify-only' not in tier and is_weak(verify):
                errors.append(f'{where}: [verify: {verify}] only checks syntax/existence — add the test that proves {",".join(acs)}')
            if 'must-test' in tier and 'red' not in tags and not acceptance:
                warnings.append(f'{where}: must-test without [red: <test paths>] — red-first is not enforced for it')
    check_context(pdir, acceptance_acs, errors, warnings)


def check_verification(feature: Path, pdir: Path, phase: str, errors: list[str]) -> dict:
    counts = {v: 0 for v in VERDICTS}
    vfiles = sorted(pdir.glob('*-VERIFICATION.md'))
    if not vfiles:
        errors.append(f'{pdir.name}: no *-VERIFICATION.md')
        return counts
    name = vfiles[0].name
    text = vfiles[0].read_text(encoding='utf-8')
    # One AC may appear in several tables (main matrix + converge rounds): key verdicts by AC id.
    verdicts: dict[str, list[str | None]] = {}
    header = None
    for line in text.splitlines():
        if not line.lstrip().startswith('|'):
            header = None
            continue
        row = cells(line)
        if header is None:
            header = row
            continue
        if all(re.fullmatch(r':?-+:?', c) for c in row if c):
            continue
        vcol = next((header.index(h) for h in ('Verdict', 'Kết quả') if h in header), None)
        if vcol is None or 'AC' not in header:
            continue
        verdict = row[vcol].replace('*', '').strip() if len(row) > vcol else ''
        acol = header.index('AC')
        keys = AC_RE.findall(row[acol]) if len(row) > acol else []
        keys = keys or [row[acol] if len(row) > acol and row[acol] else row[0]]
        if verdict in VERDICTS:
            norm = verdict
        elif 'NEEDS-CONFIRM' in verdict:
            norm = 'NEEDS-CONFIRM'
            errors.append(f'{name} {",".join(keys)}: verdict "{verdict}" is not a single word — the unchecked part is NEEDS-CONFIRM only if blocked outside our reach; missing/unfinished code is FAIL')
        else:
            norm = None
            errors.append(f'{name} {",".join(keys)}: verdict "{verdict}" is not a single PASS/FAIL/NEEDS-CONFIRM — partial/unfinished work is FAIL')
        for k in keys:
            verdicts.setdefault(k, []).append(norm)
    if not verdicts:
        errors.append(f'{name}: no AC verdict table (header needs an AC column and Verdict/Kết quả)')

    pending: list[str] = []
    for k, vs in verdicts.items():
        known = set(v for v in vs if v)
        if len(known) > 1:
            errors.append(f'{name} {k}: conflicting verdicts across tables ({", ".join(sorted(known))}) — keep one')
        final = next((v for v in ('FAIL', 'NEEDS-CONFIRM', 'PASS') if v in known), None)
        if final:
            counts[final] += 1
        if final == 'NEEDS-CONFIRM':
            pending.append(k)

    nc_rows = {}
    for line in section(text, 'NEEDS-CONFIRM').splitlines():
        if line.lstrip().startswith('|'):
            row = cells(line)
            for ac in AC_RE.findall(row[0] if row else ''):
                nc_rows[ac] = row
    state = feature / 'STATE.md'
    blockers = section(state.read_text(encoding='utf-8'), r'Blockers') if state.exists() else ''
    for ac in dict.fromkeys(pending):
        row = nc_rows.get(ac)
        if not row:
            errors.append(f'{vfiles[0].name} {ac}: NEEDS-CONFIRM without a row in the NEEDS-CONFIRM table')
        elif len(row) < 3 or row[2] in {'', '—', '-'}:
            errors.append(f'{vfiles[0].name} {ac}: NEEDS-CONFIRM row has no owner (Ai xác nhận)')
        if not any(phase in l and ac in l for l in blockers.splitlines()):
            errors.append(f'STATE.md Blockers: no line naming {phase} and {ac}')
    return counts


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split('\n')[0])
    ap.add_argument('feature', nargs='?')
    ap.add_argument('--phase', required=True)
    ap.add_argument('--check', choices=['plan', 'verification', 'all'], default='all')
    a = ap.parse_args()
    feature = Path(a.feature) if a.feature else active_feature_dir()
    pdir = phase_dir(feature, a.phase)
    errors: list[str] = []
    warnings: list[str] = []
    if a.check in ('plan', 'all'):
        check_plan(pdir, errors, warnings)
    if a.check in ('verification', 'all'):
        c = check_verification(feature, pdir, a.phase, errors)
        status = 'NOT DONE' if c['FAIL'] or errors else 'CONDITIONAL' if c['NEEDS-CONFIRM'] else 'DONE'
        print(f"{a.phase}: PASS {c['PASS']} · FAIL {c['FAIL']} · NEEDS-CONFIRM {c['NEEDS-CONFIRM']} → {status}")
    for w in warnings:
        print(f'WARN  {w}')
    for e in errors:
        print(f'ERROR {e}')
    sys.exit(1 if errors else 0)


if __name__ == '__main__':
    main()

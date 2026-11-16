#!/usr/bin/env python3
"""Everything one task needs from PLAN + CONTEXT + REQUIREMENTS, nothing else.

  python3 task_brief.py agents/planning/<slug> --phase M<x> --task T<n>

A dev pointed at the whole CONTEXT (~100 KB on a real phase) and PLAN (~70 KB) reads
far more than its task and carries it in every later turn. This cuts, verbatim:

  - the task line + its indented lines (Hợp đồng) from M<x>-*-PLAN.md
  - the AC rows of the task's [AC:] + their `AC-NN.x` clauses
  - the quoted rules (### Luật trích) those ACs cite
  - the Decisions (D<n>) cited by the task, its ACs or clauses
  - the REQUIREMENTS.md rows of the task's [UC:]

Each block carries its source file:line so the dev can `read path:a-b` around it.
Exit 1 when the task is not in any PLAN of the phase.
"""
import argparse
import re
import sys
from pathlib import Path

from phase_gate import AC_RE, NOT_RULES, RULE_RE, TAG_RE, TASK_RE, cells, phase_dir

UC_RE = re.compile(r'\bUC-?\d+\b')
D_RE = re.compile(r'\bD\d+\b')


def numbered(path: Path) -> list[tuple[int, str]]:
    return list(enumerate(path.read_text(encoding='utf-8').splitlines(), 1))


def task_block(plans: list[Path], task: str) -> tuple[Path, int, list[str]] | None:
    for plan in plans:
        lines = numbered(plan)
        for i, (n, line) in enumerate(lines):
            m = TASK_RE.match(line)
            if not (m and m.group(1) == task):
                continue
            block = [line]
            for _, nxt in lines[i + 1:]:
                if not nxt.startswith((' ', '\t')) or not nxt.strip():
                    break
                block.append(nxt)
            return plan, n, block
    return None


def in_section(lines: list[tuple[int, str]], heading_re: str) -> list[tuple[int, str]]:
    """Numbered lines of the first section whose heading matches, up to the next heading of the same or higher level."""
    out, level = [], None
    for n, line in lines:
        h = re.match(r'^(#+)\s*(.*)$', line)
        if level is None:
            if h and re.match(heading_re, h.group(2), re.I):
                level = len(h.group(1))
            continue
        if h and len(h.group(1)) <= level:
            break
        out.append((n, line))
    return out


def bullets(lines: list[tuple[int, str]], wanted) -> list[tuple[int, str]]:
    """`- X…` / `| X |` lines whose first id satisfies `wanted`, with their indented continuation lines."""
    out, keep = [], False
    for n, line in lines:
        if line.startswith((' ', '\t')) and line.strip():
            if keep:
                out.append((n, line))
            continue
        head = re.match(r'^\s*(?:[-*]\s*(?:\*\*)?|\|\s*)([A-Z]{1,4}-?\d+(?:\.[a-z]+)?)', line)
        keep = bool(head and wanted(head.group(1)))
        if keep:
            out.append((n, line))
    return out


def emit(title: str, path: Path, rows: list[tuple[int, str]]) -> None:
    if not rows:
        return
    print(f'\n## {title} — {path}')
    for n, line in rows:
        print(f'{n}: {line}')


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('feature', type=Path)
    ap.add_argument('--phase', required=True)
    ap.add_argument('--task', required=True)
    a = ap.parse_args()

    pdir = phase_dir(a.feature, a.phase)
    found = task_block(sorted(pdir.glob(f'{a.phase}-*-PLAN.md')), a.task)
    if not found:
        sys.exit(f'{a.task} not in any {pdir}/{a.phase}-*-PLAN.md - converge/rescue tasks carry their spec in the prompt')
    plan, start, block = found
    tags = {k.lower(): v for k, v in TAG_RE.findall(' '.join(block))}
    acs = set(AC_RE.findall(tags.get('ac', '')))
    ucs = {u.replace('-', '') for u in UC_RE.findall(tags.get('uc', ''))}

    print(f'# Brief {a.task} — {a.phase}')
    emit('Task (PLAN)', plan, [(start + i, line) for i, line in enumerate(block)])

    ctx_path = next(iter(sorted(pdir.glob(f'{a.phase}-CONTEXT.md'))), None)
    cited = set(D_RE.findall(' '.join(block)))
    if ctx_path:
        ctx = numbered(ctx_path)
        ac_rows = [(n, l) for n, l in in_section(ctx, r'.*Acceptance Criteria')
                   if l.startswith('|') and cells(l)[0] in acs]
        clauses = bullets(ctx, lambda i: i.split('.')[0] in acs and '.' in i)
        rules = {m.group(0) for _, l in ac_rows for m in RULE_RE.finditer(cells(l)[2] if len(cells(l)) > 2 else '')
                 if m.group(1) not in NOT_RULES}
        quoted = bullets(in_section(ctx, r'.*Luật trích'), lambda i: i in rules)
        cited |= {d for _, l in ac_rows + clauses for d in D_RE.findall(l)}
        decisions = bullets(in_section(ctx, r'Decisions'), lambda i: i in cited)
        emit('Acceptance Criteria', ctx_path, ac_rows)
        emit('Mệnh đề', ctx_path, clauses)
        emit('Luật trích', ctx_path, quoted)
        emit('Decisions', ctx_path, decisions)
        missing = sorted(acs - {cells(l)[0] for _, l in ac_rows})
        if missing:
            print(f'\n> AC not found in CONTEXT: {", ".join(missing)}')

    req = a.feature / 'REQUIREMENTS.md'
    if req.exists() and ucs:
        emit('REQUIREMENTS', req, [(n, l) for n, l in numbered(req)
                                   if l.startswith('|') and cells(l)[0].replace('-', '') in ucs])


if __name__ == '__main__':
    main()

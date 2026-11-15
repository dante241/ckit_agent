#!/usr/bin/env python3
"""Context size of live subagents, so the orchestrator can decide reuse vs a fresh spawn.

  python3 agent_ctx.py <AgentName> [<AgentName>...] [--limit 200000]

The orchestrator cannot see a subagent's context in tool results. Each subagent writes
its own transcript to ~/.omp/agent/sessions/<project>/<parent-session>/<Name>.jsonl;
this reads the newest one per name and prints the last turn's prompt size:

  DevT4Transport  272k  spawn-new
  DevT2Model       97k  reuse

reuse      under --limit: send the next task to agent://<Name> (it already holds the area).
spawn-new  at/over --limit: every message re-sends its whole context, so a fresh agent
           (pointer prompt: task, files, findings) is cheaper. Not found -> spawn-new.
"""
import argparse
import json
import os
from pathlib import Path


def project_dir() -> Path:
    # omp names a project's session dir after its cwd: /a/b -> --a-b--
    cwd = os.path.realpath(os.getcwd())
    return Path.home() / '.omp/agent/sessions' / ('-' + cwd.replace('/', '-') + '--')


def context_tokens(path: Path) -> int | None:
    last = None
    with path.open(encoding='utf-8') as f:
        for line in f:
            if '"assistant"' not in line:
                continue
            try:
                r = json.loads(line)
            except ValueError:
                continue
            m = r.get('message') or {}
            u = m.get('usage') if m.get('role') == 'assistant' else None
            if u:
                last = sum(u.get(k, 0) or 0 for k in ('input', 'cacheRead', 'cacheWrite', 'output'))
    return last


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument('names', nargs='+')
    ap.add_argument('--limit', type=int, default=200_000)
    a = ap.parse_args()
    root = project_dir()
    width = max(len(n) for n in a.names)
    for name in a.names:
        hits = sorted(root.glob(f'**/{name}.jsonl'), key=lambda p: p.stat().st_mtime) if root.is_dir() else []
        tokens = context_tokens(hits[-1]) if hits else None
        if tokens is None:
            print(f'{name:<{width}}     ?  spawn-new (no transcript under {root})')
            continue
        verdict = 'reuse' if tokens < a.limit else 'spawn-new'
        print(f'{name:<{width}}  {round(tokens / 1000):>4}k  {verdict}')


if __name__ == '__main__':
    main()

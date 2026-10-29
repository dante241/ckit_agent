---
name: codegraph
description: Use this skill on EVERY question that touches code in a project where `.codegraph/` exists. Triggers — "where is X defined", "what calls Y", "find references to Z", "explore <subsystem>", "understand the dependency graph", "outline this file", any keyword/symbol search. Saves ~35% tokens and ~70% tool calls vs raw grep/Read/Glob. The AI MUST prefer the codegraph MCP tool / `codegraph` commands over `rg`/`fd`/`grep`/`Read`-whole-file whenever a semantic answer is wanted.
---

# codegraph — semantic code intelligence (use this BEFORE grep)

You are running inside a 8sync project that already has `codegraph` installed and a `.codegraph/` index. **Use it.** Dumping whole files into context is wasteful when the index can answer in one call.

## 1. When to use codegraph (always, when applicable)

| User intent | First tool |
|---|---|
| Almost anything: "how does X work", a bug, "explore the auth subsystem", symbols you are about to edit | MCP `xd://mcp__codegraph_explore` (`{"query": "..."}`); CLI equivalent `codegraph explore <query...>` |
| "Where is `<symbol>` defined?" / fuzzy keyword search | `codegraph query "<symbol>"` |
| One symbol's source + caller/callee trail | `codegraph node <name>` |
| "What calls `<function>`?" / "what does it call?" | `codegraph callers <symbol>` / `codegraph callees <symbol>` |
| "What breaks if I change `<symbol>`?" | `codegraph impact <symbol>` |
| "Outline this file before editing" | `codegraph node <file> --symbols-only` |

If `.codegraph/` does NOT exist in cwd → `codegraph init .` (init builds the initial index). `8sync harness init` already does this when it stands up a project; you only need to re-init if the directory has been wiped.

## 2. Canonical commands

```bash
codegraph explore <query...>   # PRIMARY — relevant symbols' source + call paths in one shot (same output as the MCP tool)
codegraph query "<term>"       # symbol/keyword search with path:line hits (-l limit, -k kind, -j json)
codegraph node <name>          # one symbol's source + callers/callees, or a file with line numbers (-f <file>)
codegraph callers|callees|impact <symbol>
```

`explore` returns verbatim line-numbered source grouped by file plus the call path among the symbols — treat it as already Read. `query` returns symbol locations with `path:line` citations — copy them verbatim.

## 3. Secondary commands (one-liner each)

- `codegraph files` — project file structure from the index (`--filter <dir>`, `--pattern <glob>`, `--format tree|flat|grouped`). Cheaper than `find` / `ls -R`.
- `codegraph affected <files...>` — which test files cover a source file. Use before refactoring.
- `codegraph status` — index freshness + stats. Run when results look stale.
- `codegraph unlock` — remove a stale lock if indexing is blocked.
- `codegraph index .` — full rebuild from scratch.
- `codegraph serve --mcp` — MCP server mode (the harness already mounts it as `xd://mcp__codegraph_explore`).
- **Graph as ONE image** (vision models / layout questions): `8sync shot 'http://127.0.0.1:8731/codegraph?shot=1' -o /tmp/cg.png` — canvas-only React-Flow package graph (`8sync harness web` must be running). Details stay text: `/api/codegraph/overview|search|trace`. Non-vision models can read positions from the shot via `8sync locate` (needs its one-time `8sync locate --setup`; see skill `locate-anything`).

## 4. Citation convention

`codegraph` already prints `path/to/file.rs:42` style locations. Echo them verbatim. **Never** translate to "in file.rs around line 42" — keep the machine-grep-able form so future tools can jump straight to the cursor.

## 5. When codegraph isn't enough

Use codegraph to **locate**, then `read` to fetch the narrow slice you actually need. The pattern is:

1. `codegraph query "thing"` → see 3 hits, each with `path:line`.
2. `read <best hit's file>:<line-10>-<line+30>` → exact context, no waste.

Never `read` an entire file > 200 lines without checking codegraph first.

## 6. After editing

The MCP server runs a file watcher that re-syncs edited files within ~1s, so normally nothing is needed — when a response carries the "files were edited since the last index sync" banner, `read` just those files. Run `codegraph sync` manually only when no watcher is running (CLI-only use, `serve --no-watch`) or `codegraph status` shows the index is stale. For batch refactors (many files): `codegraph index .` to rebuild from scratch.

---

For a one-page command cheatsheet see `references/cli-cheatsheet.md`. **Do NOT read it during normal coding** — only when the user asks about install / uninstall / agent integration.

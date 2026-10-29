# codegraph CLI cheatsheet (verified against codegraph 1.1.2 `--help`)

One-page reference. For day-to-day usage rules see `../SKILL.md`.

| Command | When to use | Example |
|---|---|---|
| `codegraph init [path]` | Bootstrap `.codegraph/` and build the initial index. Non-interactive. | `codegraph init .` |
| `codegraph uninit [path]` | Delete `.codegraph/` from a project (`-f` skips the prompt). | `codegraph uninit -f .` |
| `codegraph index [path]` | Full rebuild from scratch. | `codegraph index .` |
| `codegraph sync [path]` | Incremental update — only files changed since last index. Needed only without the MCP watcher. | `codegraph sync` |
| `codegraph status [path]` | Index stats + freshness. | `codegraph status` |
| `codegraph explore <query...>` | **Primary tool** — relevant symbols' source + call paths (same as MCP `codegraph_explore`). | `codegraph explore "auth flow"` |
| `codegraph query <search>` | Symbol/keyword search with `path:line` hits. | `codegraph query "handle_login"` |
| `codegraph node <name>` | One symbol's source + caller/callee trail, or a file with line numbers. | `codegraph node src/auth.rs --symbols-only` |
| `codegraph callers <symbol>` / `callees <symbol>` | Who calls it / what it calls. | `codegraph callers handle_login` |
| `codegraph impact <symbol>` | What is affected by changing a symbol. | `codegraph impact handle_login -d 3` |
| `codegraph files` | Indexed file structure. Cheaper than `find`. | `codegraph files --filter src --format flat` |
| `codegraph affected [files...]` | Test files impacted by source changes. | `codegraph affected src/auth.rs` |
| `codegraph unlock [path]` | Remove a stale lock if indexing is blocked. | `codegraph unlock` |
| `codegraph serve --mcp` | Run as an MCP server (stdio; `--no-watch` disables auto-sync). | `codegraph serve --mcp` |
| `codegraph install` | Install the codegraph MCP into Claude Code / Cursor / Codex CLI / opencode. | `codegraph install -y` |

## Flags worth remembering

- `-v, --verbose` (on `init` / `index` only): show worker lifecycle + memory.
- `-q, --quiet` (on `index` / `sync`): suppress progress output.
- `-j, --json` (on `status`, `query`, `callers`, `callees`, `impact`, `files`, `affected`): machine-readable output.
- `-p, --path <path>` (on query/explore/node/callers/callees/impact/files/affected): query another project.
- `init -i` is deprecated (indexing now runs by default) — use plain `codegraph init .`.

## Output format

`query`, `explore` and `node` emit `path:line` citations. Echo them verbatim in your reply.

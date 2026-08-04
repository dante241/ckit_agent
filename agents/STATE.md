# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Document post-install config (omp config files, gateway key + endpoint URL, MCP `/mcp list` verify, Claude→Mnemopi migration) and confirm the deployed gateway runs.

## Definition of Done
- [x] Docs + README `Configure & verify`: `config.yml`, `models.yml`, `models.toml`, `mcp.json`.
- [x] Gateway **key AND endpoint URL** documented: `ckit harness gateway key <KEY>` + `url <URL>` + `verify` (no live key/IP in docs).
- [x] `/mcp list` block shows the 4 servers; wording split macOS/Linux (all four) vs Windows (codebase-memory-mcp may skip).
- [x] Claude Code migration via `omp --from-claude`; example path generic (`<your-project>`).
- [x] Confirmed live gateway healthy (`ckit harness gateway verify` → HTTP 200).
- [x] HTML parses; browser DOM confirms `#configure` renders.

## Current step
DONE — docs/README updated, gateway verified healthy, ready to commit.

## Next
_none — awaiting further instructions._

## Assumptions (auto-decided — user can correct)
- The provided key + endpoint already in `~/.omp/agent/models.yml` are correct (verify → HTTP 200); no value change needed, only documentation.
- Gateway endpoint is set via `ckit harness gateway url`, not hand-editing models.yml.

## Open questions / blockers
- none.

## Handoff (compaction)
Configure & verify section (docs + README) now covers gateway key + URL + verify, MCP `/mcp list` (Windows caveat), and `omp --from-claude`. Live gateway verified HTTP 200. No `sk-` key or `<host>:<port>` committed. CHANGELOG/KNOWLEDGE updated. SECURITY NOTE: the live 9router `sk-` key + endpoint surfaced in tool reads this session — advise the user to rotate the key (`ckit harness gateway key <NEW_KEY>`).

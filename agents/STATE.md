# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Make native-Windows install actually work (`ckit setup`/`ckit harness`) and document the Windows path.

## Definition of Done
- [x] `ckit setup` no longer aborts on Windows: omp + codegraph install via bun/npm instead of POSIX `sh`.
- [x] `ensure_codebase_memory_mcp` registers the MCP only when the binary is present (no broken omp entry).
- [x] `ensure_codegraph`/`ensure_uv`/headroom fallback have Windows-safe branches.
- [x] `README.md` + `docs/index.html` document the Windows install path and prerequisites.
- [x] `cargo build/check/test` + `ckit setup --dry-run` pass on host.

## Checklist
- [x] setup.rs `install_omp`/`install_codegraph` → `deploy::install_node_pkg` (bun/npm) on Windows.
- [x] deploy.rs `ensure_codegraph` npm branch; `ensure_uv` PowerShell branch; headroom pip-fallback gated off Windows.
- [x] deploy.rs `ensure_codebase_memory_mcp` binary-gated registration + stale-entry cleanup.
- [x] Dry-run text is platform-accurate (bun/npm vs curl).
- [x] README + docs Windows section with bun/npm/winget/uv prerequisites.
- [x] CHANGELOG + KNOWLEDGE updated.

## Current step
DONE — Windows install path implemented and host-verified.

## Next
Optional: push; real Windows compile is CI's `windows-latest` runner in `release.yml`.

## Assumptions (auto-decided — user can correct)
- omp (`@oh-my-pi/pi-coding-agent`) and codegraph (`@colbymchenry/codegraph`) are installable from npm on Windows via bun/npm.
- `bun add -g` is the correct global-install form (verified via `bun add --help`).

## Open questions / blockers
- `cargo check --target x86_64-pc-windows-msvc` can't run on this mac (no MSVC/Windows-SDK C toolchain for `zstd-sys`); CI covers the real Windows build.

## Handoff (compaction)
Windows install blockers fixed in setup.rs + deploy.rs via runtime `platform::os()==Windows` branches (not cfg-gated, so host build type-checks them). Docs updated. Verified: cargo build/check/test EXIT 0, dry-run clean.

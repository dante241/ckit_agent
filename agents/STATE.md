# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Fix Windows `ckit setup`: `models.yml` and `codegraph` missing after install.

## Definition of Done
- [x] Root cause: strict setup aborted at the `codegraph` step (its install can fail on Windows) BEFORE `models.yml` was seeded.
- [x] `install_codegraph` best-effort (warn, never abort) on all platforms.
- [x] Stage A reordered: local config (PATH, configs, `models.yml`, `config.yml`) seeds BEFORE fallible tool installs; MCP registration stays after installs.
- [x] Fixed fish PATH file brand mismatch: `brand::ns_file("path.fish")` (`ckit-path.fish`) + remove stale `8sync-path.fish`; fish comment uses `brand::CMD`/`NS`.
- [x] `cargo build`/`test` (20) pass; `ckit setup --dry-run` shows local-first order + consistent `ckit-path.fish`.

## Current step
DONE — Windows setup robustness fix implemented, host-verified. Ready to commit.

## Next
Consider a patch release (0.1.6) so `ckit up`/install ships the fix to Windows users.

## Assumptions (auto-decided — user can correct)
- codegraph is optional; its install failing must not abort setup. omp remains fatal (essential engine).
- Namespace artifacts follow `brand::ns_file`; the zsh/bash idempotency marker string is left unchanged to avoid duplicate PATH blocks on existing machines.

## Open questions / blockers
- The exact codegraph-on-Windows failure cause is unconfirmed (no Windows host here); handled generically (best-effort + retry via `ckit harness`).

## Handoff (compaction)
setup.rs: codegraph best-effort + Stage A local-config-first reorder + fish file via ns_file (ckit-path.fish) with legacy cleanup. Root cause of "Windows thiếu models.yml + codegraph": codegraph step bailed before models.yml seed. cargo build/test green; dry-run verified. CHANGELOG/KNOWLEDGE updated. Not yet released — suggest 0.1.6.

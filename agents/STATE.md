# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Ship hotfix `v0.1.4` for `ckit up` downloading the wrong release asset on macOS.

## Definition of Done
- [x] Local `~/.local/bin/ckit` restored to an executable Darwin arm64 binary after the bad Linux asset install.
- [x] `ckit up` resolves release asset suffix by runtime OS/arch instead of hard-coded `-linux-x86_64`.
- [x] Downloaded temp asset is chmodded, executed with `--version`, and only renamed into place when it is executable on this machine and reports the expected version.
- [x] CLI help/prose for `ckit up` uses `ckit`, not legacy `8sync`.
- [x] Build/test/install local hotfix binary.
- [x] Commit and tag `v0.1.4`.

## Checklist
- [x] Restore local binary from `target/release/ckit`.
- [x] Patch `crates/cli/src/verbs/selfup.rs` platform suffix selection.
- [x] Patch `crates/cli/src/verbs/selfup.rs` temp binary guard before rename.
- [x] Add regression tests for platform suffix and version guard.
- [x] Patch `crates/cli/src/verbs/up.rs` user-facing rebrand leftovers.
- [x] Bump workspace/Cargo.lock to `0.1.4` and document changelog.
- [x] Run cargo checks/tests/build and install local hotfix.
- [x] Commit + tag.

## Current step
DONE — hotfix commit `2cd26ca` created and local annotated tag `v0.1.4` created.

## Next
Push commit and tag when ready: `git push origin HEAD && git push origin v0.1.4`.

## Assumptions (auto-decided — user can correct)
- `v0.1.3` was already pushed and must not be moved; use new hotfix tag `v0.1.4`.
- The correct Darwin arm64 release asset name is `ckit-v0.1.4-darwin-arm64`, matching `.github/workflows/release.yml` and `install.sh`.

## Open questions / blockers
- none.

## Handoff (compaction)
Root cause confirmed: `selfup.rs` hard-coded the Linux x86_64 asset, so macOS arm64 could install an ELF binary and break `ckit`. Hotfix source now maps OS/arch to release suffix, chmods/verifies downloaded temp binaries before rename, restores local `~/.local/bin/ckit` as Mach-O arm64 `ckit 0.1.4`, and validates top-level/root/up/theme/bg help has zero `8sync`/`sync8` hits.

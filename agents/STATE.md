# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Commit and tag release `v0.1.3` for the public `8sync` → `ckit` rebrand cleanup.

## Definition of Done
- [x] Shipped docs/install paths no longer show legacy `sync8`, `8-Sync-Dev/su-code`, or `8sync` command examples where raw text is user-facing.
- [x] `scripts/alexdev-install.sh` installs and invokes `ckit` and writes profile overrides under `~/.config/ckit/profiles`.
- [x] Workspace version and `Cargo.lock` package version bumped to `0.1.3`; CLI reports `ckit 0.1.3`.
- [x] `CHANGELOG.md` has `0.1.3` release heading and explicit rebrand/installer fix bullets.
- [x] `cargo check -q`, `cargo build -q`, `bash -n scripts/alexdev-install.sh`, runtime version, and shipped stale-string scan pass.
- [x] Commit release changes and create tag `v0.1.3`.

## Checklist
- [x] Update root/flow help repository and installer URLs.
- [x] Update `docs/index.html` header/footer branding.
- [x] Update `scripts/alexdev-install.sh` from `8sync` to `ckit` user-facing install flow.
- [x] Keep bundled skill ID `8sync-cli` stable; do not half-rename to `ckit-cli`.
- [x] Bump release metadata to `0.1.3`.
- [x] Update `CHANGELOG.md` and `agents/KNOWLEDGE.md`.
- [x] Commit + tag.

## Current step
DONE — local release commit created and local annotated tag `v0.1.3` points at it.

## Next
Push commit and tag when ready: `git push origin HEAD && git push origin v0.1.3`.

## Assumptions (auto-decided — user can correct)
- `v0.1.2` is an existing release identity and must not be moved.
- `v0.1.3` is the correct new patch tag for this update.
- `8sync-cli` remains the stable skill directory/frontmatter/mapping ID; only prose/commands are rebranded.

## Open questions / blockers
- none.

## Handoff (compaction)
Release changes are committed locally and tagged locally as `v0.1.3`; `.serena/memories/` remains untracked and intentionally excluded.

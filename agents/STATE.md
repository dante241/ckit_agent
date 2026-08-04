# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Trim `docs/index.html`: remove the Dashboard section, keep Install next to How-to-use, and add post-install verification commands.

## Definition of Done
- [x] Dashboard (`ckit harness web`) removed: section, `#dashboard` nav link, feature card, and command-table row (grep `dashboard`/`harness web` = 0).
- [x] Install (`#install`) sits directly before How-to-use (`#usage`).
- [x] "Verify the install" block added (`ckit doctor` + `ckit`/`omp`/`codegraph --version` + `gh auth status`), no hard-coded version.
- [x] HTML parses; browser DOM check confirms order + rendering.

## Current step
DONE — docs trimmed, verified in browser, ready to commit.

## Next
_none — awaiting further instructions._

## Assumptions (auto-decided — user can correct)
- `ckit doctor` is the primary post-install check; per-tool `--version` calls are the manual fallback.
- Replaced the "Dashboard CRUD" feature card with a "Code intelligence" card to keep the 4-card grid.

## Open questions / blockers
- none.

## Handoff (compaction)
docs/index.html: dashboard fully removed (0 refs), Install→Usage adjacent, verify block present (no hard-coded version). CHANGELOG [Unreleased] + KNOWLEDGE updated. Verified via HTMLParser + browser DOM (order install,usage,commands,skills,update,docs).

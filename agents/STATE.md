# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Add practical public docs for setup, daily use, and long-feature workflow without rewriting published `v0.1.4`.

## Definition of Done
- [x] `docs/index.html` separates first-time machine/project setup from daily commands.
- [x] Docs explain the purpose of each common setup and daily command.
- [x] Docs show long-task entrypoint `/plan @assets/skills/feature/SKILL.md <task>` and the `/feature new|plan|go|ship|status` loop.
- [x] Stale STATE hash corrected in a normal follow-up commit path; published `v0.1.4` tag is not moved.
- [x] Static HTML parse, stale-name scan, changelog contamination scan, and browser DOM checks pass.

## Checklist
- [x] Add `Usage` nav item and practical usage section.
- [x] Add setup table: install, setup, doctor, project harness, session start.
- [x] Add daily table: session, one-shot AI, find, run, note, ship.
- [x] Add feature workflow command block.
- [x] Repair `CHANGELOG.md` contamination from malformed edit.
- [x] Update `agents/STATE.md` with published `40cff7a` hash and no tag rewrite.
- [ ] Commit docs follow-up normally.

## Current step
Ready to commit docs follow-up.

## Next
Commit changed docs/state/knowledge/changelog as a normal commit. Do not amend `40cff7a`; do not move `v0.1.4`.

## Assumptions (auto-decided — user can correct)
- The published release commit/tag are immutable for this task.
- `assets/skills/feature/SKILL.md` remains the correct path to reference in the `/plan` command.

## Open questions / blockers
- none.

## Handoff (compaction)
Docs follow-up is implemented and verified locally. Commit as a new normal commit on top of published `40cff7a`. Verification run: Python HTMLParser + content asserts, grep scans for changelog contamination and stale user-facing names, browser open/run checking `#usage`, setup/daily labels, `/plan @assets/skills/feature/SKILL.md`, `/feature new customer-portal`, and nav link list.

# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Board = virtual project room in `ckit harness web` that delivers every task through the `/feature` workflow; user ↔ Leader (PO) only; PM status every 5 min.

## Definition of Done
- [x] Steps mapped onto /feature: intake=new · solution=plan (+`ba` agent) · review=PO · approval=Gate 1/2 · dev=go · test=ship (multi-phase loop) · report · acceptance (merge).
- [x] Whole-tree lock per project (ACTIVE.md single pointer); leftovers committed before merge/park.
- [x] PM: deterministic 5-min status, AI only on anomalies (tool-less, json-parsed, backoff), report on task end + button.
- [x] Room view + kanban; drawer shows /feature docs (GFM tables); other-projects strip.
- [x] build.rs rerun on assets/ (stale-embed fix).
- [x] cargo test 35 pass; real end-to-end smoke (M0 feature: plan → go → ship → archive → merge, $3.35); browser-verified.

## Current step
DONE — not committed, not released.
## Next
- Live-verify the two reply-at-acceptance paths (question → back to accept gate; change request → un-archive + revision phase). Blocked: omp `modelRoles.default` = `cloudgo-cx/gpt-5.5` returns 400 — user must restore a working default (or set `[tasks]` in `~/.config/ckit/models.toml`).
- Then commit + release (0.1.33).
- `modelRoles.smol` also points at the broken gpt-5.5 → PM AI falls back to default.

## Assumptions (auto-decided — user can correct)
- Every Board task uses /feature (small task = 1 phase M0); `/cook` no longer used by Board.
- Change request after delivery → new revision phase (needs plan approval); questions return to the accept gate.
- Tasks in one project are serialized (one owns the tree until done); other projects run in parallel.

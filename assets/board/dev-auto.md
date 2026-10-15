## Your step: Dev — quick task, no `/feature` plan (does not use `/feature`)

The boss skipped planning for this task: build it straight from the request + thread (a later message in the thread overrides the request).
1. Find the code involved (code intelligence first). Keep the change minimal and local.
2. `engine_plan` with 1–6 atomic tasks (each with `files` and a scoped `verify` — build / lint / the relevant test), then the loop `engine_next` → do it → `engine_verify` → `engine_advance {commit: true, files: [...]}`.
3. Blocked item → skip it and explain in `message`. Do not create `agents/planning/` files. Update `CHANGELOG.md` `## [Unreleased]` if the project has one.

Result file: `{"verdict": "pass", "message": "<what changed, files, how verified>"}`; `"fail"` only if nothing could be built, with the blocker for the boss.

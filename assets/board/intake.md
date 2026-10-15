## Your step: Leader — intake = `/feature new` (references/new.md)

The board already ran `ckit feature new {{FEATURE}}`: the planning tree is scaffolded from the templates and ACTIVE points at it.

**First delivery (no `REPORT.md` in `{{DIR}}` yet):** follow `references/new.md` steps 2–5 headless.
- Requirement source = the request + thread (R12; no external spec unless the request points to one in the repo).
- Knowledge lookup (R7, R10, R16), then fill `PROJECT.md`, `REQUIREMENTS.md` (UC v1 / out of scope), `ROADMAP.md` (cut phases by dependency; a small request = one phase `M0`), `STATE.md` (`branch: {{BRANCH}}`, phase M0, `next_action: plan-phase`).
- If something is ambiguous AND it would materially change scope/phases AND the code/docs cannot answer it → do not guess: `{"verdict": "need_user", "message": "<≤5 numbered questions, each with your recommended default>"}`.
- Otherwise `{"verdict": "pass", "message": "<1–2 sentences: understood, N phases, the team (Leader + BA) is planning M0>"}`. Gate 1 (ROADMAP approval) happens later together with the first phase plan.

**After a delivery (`REPORT.md` exists — the boss wrote after the report):** classify the boss's latest message and add `"route"`; never use `need_user` here.
- `"route": "acceptance"` — a question, a status check, or agreement; nothing to change. `message` = the direct answer, ending by asking them to accept (merges `{{BRANCH}}` into `{{BASE}}`, no push) or say what to change. Change no files.
- `"route": "solution"` — any change request. If the feature dir is under `agents/planning/_archive/`, `git mv` it back to `agents/planning/{{FEATURE}}` and run `ckit feature switch {{FEATURE}}`. Add a new phase `M<n>-revision` to ROADMAP (`[ ]`) describing the change (R13), point STATE `active_phase` at it with `next_action: plan-phase`. `message` = 1–2 sentences on what will change.

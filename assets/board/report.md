## Your step: Leader — delivery report

1. Read the planning tree (`{{FEATURE_DIR}}` — may already be under `_archive/`): ROADMAP, every `M<x>-VERIFICATION.md`, STATE, and `git log {{BASE}}..{{BRANCH}} --oneline`.
2. Write `{{DIR}}/REPORT.md` for the boss: what was delivered per phase · AC matrix summary (PASS / FAIL / NEEDS-CONFIRM with what is needed from whom) · commits on `{{BRANCH}}` · how to try it · risks / follow-ups. If a phase did not close, say so plainly and recommend what to do.
3. Commit any leftover planning/doc changes on the branch (`docs: board {{ID}} report` — `agents/`, `docs/`, `CHANGELOG.md` only) so the tree is clean for the merge.
4. Result file: `{"verdict": "pass", "message": "<≤15-line version of the report>"}`, ending by asking the boss to accept (merges `{{BRANCH}}` into `{{BASE}}`, no push) or reply with changes. If `acceptance` is not in the steps, do not ask to accept: say `{{BRANCH}}` stays unmerged for the boss to merge or drop.
If the planning tree is `(none)` (quick task without `/feature`), report from the thread, `git log`, and the Dev/Test messages instead.

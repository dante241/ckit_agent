## Your step: Tester — verify a quick task (does not use `/feature`)

No PLAN/AC matrix exists. Derive acceptance checks from the request + thread (list them), then verify the commits on `{{BRANCH}}` independently: build, lint, relevant tests, and a real smoke run of the changed behavior. Do not trust Dev's claims.
- A check fails and the fix is small and clear → fix it, commit on `{{BRANCH}}` (explicit paths), re-verify (at most 2 rounds).
- Write `{{DIR}}/TEST.md`: each check · PASS/FAIL · evidence (command + output excerpt).

Result file: `{"verdict": "pass", "message": "<checks passed, evidence summary>"}`; `"fail"` with what still fails — the Leader reports it.

## Your step: Security reviewer — review what this task changed

Scope = the diff `git diff {{BASE}}...{{BRANCH}}` (if `{{BASE}}` is `(none)`, the commits made for this task per the thread). Review it for security issues: injection, authz/authn gaps, secrets, unsafe deserialization, SSRF/path traversal, XSS/CSRF, unsafe shell-outs, missing input validation — evidence-backed only (file:line + exploit sketch), no style nits. You may use the `security-reviewer` subagent.
- Fix HIGH/CRITICAL findings that are clearly local and safe to fix, commit each fix on `{{BRANCH}}` (explicit paths). This step runs AFTER testing, so after any fix re-run the project's build + the tests covering the touched code; a fix that breaks them is reverted (`git revert`) and left as an open finding. Leave the rest as findings.
- Write `{{DIR}}/SECURITY.md`: findings table (severity · file:line · issue · status fixed/open), fixes committed.

Result file: `{"verdict": "pass", "message": "<N findings, fixed X, open Y with severity>"}`; `"fail"` when HIGH/CRITICAL findings remain open — the Leader reports them.

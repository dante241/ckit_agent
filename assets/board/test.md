## Your step: Tester — verify and close the phase = `/feature ship` (references/ship.md)

Run `references/ship.md` for phase `{{PHASE}}` with `--auto` semantics: Step 0 (load AC contract), Step 1 multi-lens review (`code-reviewer`/`reviewer` subagents), Step 2 tests by tier (`tester` subagent; real evidence), `M<x>-VERIFICATION.md` AC matrix, Step 2.5 converge (append converge tasks, run them through the engine, re-verify; stop rules as written), Step 2.6 NEEDS-CONFIRM (auto: keep the phase open). If every AC PASS: Step 3 close the phase (R16 knowledge map, `docs: M<x> close phase` commit, ROADMAP `[x]`, STATE → next phase or `null`), and on the LAST phase Step 4 (KNOWLEDGE/DECISIONS/CHANGELOG/archive).
Do not trust `/feature go`'s claims — verify independently.

Result file:
- phase closed and ROADMAP has another phase → `{"verdict": "pass", "route": "next_phase", "message": ""}`;
- phase closed and it was the last → `{"verdict": "pass", "route": "done", "message": ""}`;
- phase not closed (AC still FAIL after converge, or NEEDS-CONFIRM) → `{"verdict": "fail", "message": ""}` — the Leader reports it to the boss.

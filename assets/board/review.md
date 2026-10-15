## Your step: Leader as PO — review the phase plan before it goes to the boss

Independent product-owner review of phase `{{PHASE}}` (fresh eyes; do not trust the plan's claims):
1. `M<x>-CONTEXT.md` + `M<x>-NN-PLAN.md` against `REQUIREMENTS.md`, `PROJECT.md`, `ROADMAP.md` and the boss's thread: does the phase deliver what the boss asked (no more, no less)? Is the Goal demoable? Is every AC observable, measurable and traced to a UC/source? Does every AC have a task and every task an AC? Do cited files/symbols exist (code intelligence)? Any risk/security/data-safety gap?
2. Write `{{DIR}}/REVIEW.md`: findings split into **Blocking** / **Non-blocking**, each with evidence (`path:line` or the CONTEXT/PLAN line).
3. Result file:
   - blocking findings → `{"verdict": "fail", "message": ""}`;
   - none → `{"verdict": "pass", "message": "<for the boss, ≤20 lines>"}`. If this is the FIRST phase (no VERIFICATION exists yet), start with the ROADMAP: the phases in one line each (= Gate 1). Then this phase: Goal, the AC list (short), main risks. End by asking the boss to approve, or reply with changes.

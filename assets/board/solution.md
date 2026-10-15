## Your step: Leader + BA — plan the active phase = `/feature plan` (references/plan.md), round {{ROUND}}

Plan phase `{{PHASE}}` exactly as `references/plan.md` Steps 1–3.5 require, headless:
- **Step 1 Discuss = the meeting with the BA.** Spawn the BA via the `task` tool with `agent: "ba"` (unknown agent → default agent told to act as a read-only Business Analyst). Give it the planning tree path, the phase, and the requirement sources; it returns requirement scope, approach and draft AC. Critique it like a tech lead (feasibility against real code, YAGNI, risks, security) — at most 2 more exchanges. Other design forks → `references/auto.md` subagent discuss.
- **Step 2 Research** with `scout` subagents as plan.md says (R10, R16 map excerpts).
- Write `phases/M<x>-<name>/M<x>-CONTEXT.md` (📌 Requirement scope + 🎯 Goal + ✅ AC table with Nguồn / Cách verify / Tier / Task) and `M<x>-NN-PLAN.md` (every task `[file:] [depends:] [verify:] [skill:] [tier:] [UC:] [AC:]`), coverage both directions.
- **Step 3.5 plan-review** per `config.workflow.plan_review` (max 2 rounds; leftover findings go into CONTEXT "Plan-review notes").
- Do NOT do Step 4 (the boss approves at the board's approval step). Leave STATE `status: planning`.
- Round > 1: `{{DIR}}/REVIEW.md` (PO review) and/or the boss's latest thread message hold findings — address every point, add a `## Responses` section to CONTEXT.

Result file: `{"verdict": "pass", "message": ""}`.

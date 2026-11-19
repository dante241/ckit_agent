## Your step: Leader (also the BA) — plan the active phase = `/feature plan` (references/plan.md), round {{ROUND}}

Plan phase `{{PHASE}}` exactly as `references/plan.md` Steps 1–3.5 require, headless:
- **Step 1 Discuss = your BA analysis.** You are the BA too: you may spawn a read-only analyst via the `task` tool with `agent: "ba"` (unknown agent → default agent told to act as a read-only Business Analyst), giving it the planning tree path, the phase, and the requirement sources; it returns requirement scope, approach and draft AC. Critique it like a tech lead (feasibility against real code, YAGNI, risks, security) — at most 2 more exchanges. Other design forks → `references/auto.md` subagent discuss.
- **Step 2 Research** with `scout` subagents as plan.md says (R10, R16 map excerpts).
- Write `phases/M<x>-<name>/M<x>-CONTEXT.md` (📌 Requirement scope + 🎯 Goal + ✅ AC table with Nguồn / Cách verify / Tier / Task) and `M<x>-NN-PLAN.md` (every task `[file:] [depends:] [verify:] [skill:] [tier:] [UC:] [AC:]`), coverage both directions: every AC of the phase is tagged by ≥1 task and every task tags ≥1 AC (`[AC: —]` only with `[tier: verify-only]` or `[tier: infra]`). The board checks this before the Reviewer and sends a plan with gaps straight back to you. An AC whose wording differs from REQUIREMENTS, or that REQUIREMENTS does not have, is a scope change: mark its Nguồn honestly (the coordinator cannot approve such a plan for the boss).
- **Record every decision** in the CONTEXT `## Decisions` section (the template's D1, D2…): the decision, the reason, and the alternatives you rejected (and why), plus who decided (R15). Later in the task the team's questions come to you, answered from this planning session and that section: a decision not written there is lost.
- **Step 3.5 plan-review** per `config.workflow.plan_review` (max 2 rounds; leftover findings go into CONTEXT "Plan-review notes").
- Do NOT do Step 4 (the boss approves at the board's approval step). Leave STATE `status: planning`.
- Round > 1: `{{DIR}}/REVIEW.md` (the Reviewer's critique) and/or the boss's latest thread message hold findings — address every point, add a `## Responses` section to CONTEXT, and update `## Decisions` where a decision changed.

Result file: `{"verdict": "pass", "message": ""}`.

# Review, Audit, and Decision Rules

Use this file when reviewing code, applying audit feedback, or cutting scope.

## Verified Decisions

Once a decision is verified by source, tests, or an empirical check, do not reverse it because an audit raises an abstract concern. Reverse only when the audit adds new evidence or the context changed.

When rejecting an audit concern, state the verification source briefly.

## User Decisions

Do not silently undo explicit user decisions. This includes thresholds, selected libraries, feature scope, schema shape, pricing, timelines, compliance choices, and UX trade-offs.

If an audit suggests reversing a user decision, present:

- the original decision
- the audit concern
- the trade-off
- the concrete options

Then wait for the user.

## Threat Model

Before applying a security or robustness finding, identify what the code actually stores, protects, or exposes. Fix real failure modes. Document non-issues briefly. Ask when the risk is plausible but depends on product intent.

## Scout First

For questions answerable by reading the repo, scout before asking. Ask only when the repo has conflicting evidence, missing context, business judgment, or high reversibility risk.

## Stable Code Artifacts

Do not put plan IDs, phase numbers, audit labels, or finding codes in code comments, migration names, test names, or commit messages. Explain the invariant or behavior directly.

**Ticket IDs banned again (decided 2026-07-07, supersedes the 2026-07-03 exception below).** `/feature` (GSD) workflow: never put a ticket number in a code comment. See `php-conventions.md` → Modification Tracking Comments for the ownership-based rule that replaces it (own-class new function = summary only, other's-class new function = Added by, editing other's function = Modified by, editing your own function = no comment). Plan IDs, phase numbers, audit labels, and internal finding codes (e.g. `EP-NNN`, `WR-NN`) remain banned as before.

Other workflows (`/cook`, `/fix`) may still use the old ticket-ID-prefixed format if their own skill reference requires it — this reversal is scoped to `/feature` only, not global.

<details>
<summary>Superseded 2026-07-03 exception (kept for history — no longer in effect for /feature)</summary>

Modification-tracking comments MUST lead with the ticket ID: `// #NNNNN: Added/Modified by <Name> on <DATE> to <REASON>`. This applies to the `// Added by`/`// Modified by` comment blocks described in `php-conventions.md` — ticket ID is now a required prefix, not a banned artifact.

</details>

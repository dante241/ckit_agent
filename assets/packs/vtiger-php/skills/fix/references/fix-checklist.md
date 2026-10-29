# Fix Checklist (Quick Reference)

> Compact step matrix for bug fixes. Investigation shortcuts, review checklist, test tiers and the todolist template live in `../SKILL.md` (single source).

## Pre-flight

- [ ] Extract ticket # or inline description
- [ ] Assess bug complexity: simple / medium / complex
- [ ] Derive bug slug for plan directory

## Step Matrix (what to run per complexity)

| Step | Simple | Medium | Complex |
|------|--------|--------|---------|
| 1. Classify | YES | YES | YES |
| 2. Load Skills | SKILL.md only | SKILL.md + refs | SKILL.md + refs |
| 3. Investigate | codegraph / grep quick | Serena trace chain | debugger agent |
| 4. TodoList | Lite todolist | todolist + user gate | todolist + user gate |
| 5. Fix | Direct edit | Direct edit | Direct edit (from report) |
| 6. Review | Built-in check | code-reviewer agent | code-reviewer agent |
| 7. Test | verify-only / E2E | must-test / E2E | must-test (tester agent) + E2E if UI |
| 8. Complete | Update todolist | Update todolist | Update todolist |
| 9. Error pattern | Draft PEND if triggered | Draft PEND if triggered | Draft PEND if triggered |

## Commit Handoff

After Step 8, suggest:
```
/fix #XXXXX -> done -> /commit #XXXXX
```

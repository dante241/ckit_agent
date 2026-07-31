# Cook Phase Checklist (Quick Reference)

> Compact checklist for each phase. Use as a runtime guide during cooking.

## Pre-flight

- [ ] Extract ticket # or inline description
- [ ] Classify: feature / bugfix / config / report / integration
- [ ] Assess complexity: simple / medium / complex
- [ ] Derive feature slug for plan directory

## Phase Matrix (what to run per complexity)

| Phase | Simple | Medium | Complex |
|-------|--------|--------|---------|
| 1. Classify | YES | YES | YES |
| 2. Load Skills | SKILL.md only | SKILL.md + refs | SKILL.md + refs |
| 3. Research | Skip if known | Serena | Serena + agents |
| 4. Plan | Lite todolist | plan.md + todolist | Planner agent + todolist |
| 5. User Gate | NO | YES | YES |
| 6. UI Confirm | Skip | Text/ASCII | Designer agent |
| 7. Implement | Direct | Direct | Subagents |
| 7.5. Simplify | NO | NO | code-simplifier agent |
| 8. Review | Built-in check | code-reviewer agent | code-reviewer agent |
| 9. Test | verify-only / E2E | must-test / E2E | must-test + E2E |
| 10. Complete | Update todolist | Update todolist | Update todolist |

## File Separation Quick Check

```
CSS      -> modules/<Module>/resources/<View>.css          (ALWAYS)
JS core  -> layouts/v7/modules/<Module>/resources/<View>.js (List, Edit, Detail)
JS custom-> modules/<Module>/resources/<View>.js            (Config, Report, custom)
TPL core -> layouts/v7/modules/<Module>/<View>.tpl
TPL custom-> modules/<Module>/tpls/<View>.tpl
```

## Code Review Essentials

### Runtime (php -l cannot catch)
- [ ] Parent methods exist (Serena `find_symbol`)
- [ ] Classes in `extends`/`new` exist
- [ ] DB columns match actual schema
- [ ] Config keys exist

### Security
- [ ] `pquery()` with `?` params (no string concat SQL)
- [ ] `.text()` not `.html()` for dynamic JS content
- [ ] `(string)`/`(int)` on all `$request->get()` values
- [ ] `checkPermission()` on all controllers

### Performance
- [ ] No DB queries inside loops
- [ ] WHERE/JOIN columns have indexes
- [ ] Use LIMIT for large result sets

## Test Tier Decision

```
Changed Action/Model/Helper code?  -> must-test (unit tests)
Changed Report/SQL queries?        -> verify-sql (run on real DB)
Changed DDL/language/config/CSS?   -> verify-only (php -l + review)
Changed UI (TPL/JS)?               -> E2E (Playwright)
```

## TodoList Template (Lite)

```markdown
# <Feature> -- TodoList

**Date:** YYYY-MM-DD | **Ticket:** #XXXXX | **Complexity:** simple
**Status:** In progress

## Tasks
- [ ] ...
- [ ] Code review
- [ ] Verify/test

## Files to modify
- ...

## Test Results
_(Phase 9)_

## Completion Notes
_(Phase 10)_
```

## Commit Handoff

After Phase 10, suggest:
```
/commit #XXXXX
```

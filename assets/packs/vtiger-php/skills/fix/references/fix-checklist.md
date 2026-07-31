# Fix Checklist (Quick Reference)

> Compact checklist for bug fixes. Use as a runtime guide.

## Pre-flight

- [ ] Extract ticket # or inline description
- [ ] Assess bug complexity: simple / medium / complex
- [ ] Derive bug slug for plan directory

## Step Matrix (what to run per complexity)

| Step | Simple | Medium | Complex |
|------|--------|--------|---------|
| 1. Classify | YES | YES | YES |
| 2. Load Skills | SKILL.md only | SKILL.md + refs | SKILL.md + refs |
| 3. Investigate | Serena quick | Serena trace chain | debugger agent |
| 4. TodoList | Lite todolist | todolist + user gate | todolist + user gate |
| 5. Fix | Direct edit | Direct edit | Direct edit (from report) |
| 6. Review | Built-in check | code-reviewer agent | code-reviewer agent |
| 7. Test | verify-only / E2E | must-test / E2E | must-test + E2E |
| 8. Complete | Update todolist | Update todolist | Update todolist |

## Investigation Quick Guide

```
PHP error/blank page    -> php -l, error log
AJAX error              -> Find Action, check process()
Data not saving         -> Trace save() chain, check handlers
UI not rendering        -> TPL syntax, JS console, browser cache
SQL error               -> Extract query, run on real DB
Permission denied       -> checkPermission(), user roles
JS not working          -> Controller name, registerEvents(), cache
```

## Fix Implementation Checks

- [ ] `php -l` on every modified PHP file
- [ ] Parent/base methods exist (Serena `find_symbol`)
- [ ] No inline CSS/JS added
- [ ] SQL changes tested on real DB
- [ ] No side effects (`find_referencing_symbols`)
- [ ] Modification tracking comment added

## Code Review Essentials

### Runtime (php -l cannot catch)
- [ ] Parent methods exist (Serena `find_symbol`)
- [ ] Classes in `extends`/`new` exist
- [ ] DB columns match actual schema

### Security
- [ ] `pquery()` with `?` params (no string concat SQL)
- [ ] `.text()` not `.html()` for dynamic JS content
- [ ] `(string)`/`(int)` on all `$request->get()` values
- [ ] `checkPermission()` on controllers

### Performance
- [ ] No DB queries inside loops
- [ ] No N+1 queries introduced

## Test Tier Decision

```
Fixed Action/Model/Helper code?  -> must-test (unit tests)
Fixed Report/SQL queries?        -> verify-sql (run on real DB)
Fixed DDL/language/config/CSS?   -> verify-only (php -l + review)
Fixed UI (TPL/JS)?               -> E2E (Playwright)
```

## TodoList Template

```markdown
# Fix: <Bug Description> -- TodoList

**Date:** YYYY-MM-DD | **Ticket:** #XXXXX | **Complexity:** simple
**Status:** In progress

## Bug Summary
- **Module:** <module>
- **Symptom:** <what user sees>
- **Root Cause:** <cause>
- **Fix Strategy:** <approach>

## Tasks
- [ ] Investigate root cause
- [ ] Apply fix
- [ ] php -l syntax check
- [ ] Code review
- [ ] Verify fix
- [ ] Regression check

## Files to modify
- ...

## Test Results
_(Step 7)_

## Completion Notes
_(Step 8)_
```

## Commit Handoff

After Step 8, suggest:
```
/fix #XXXXX -> done -> /commit #XXXXX
```

---
name: review-pr
argument-hint: '<MR-iid | MR-url> [--no-merge] [--dry-run]'
description: "Review GitLab Merge Request for bugs, security, logic, and AGENTS.md/.omp/rules compliance. Posts comment and auto-merges if clean."
allowed-tools: Bash(glab mr *), Bash(glab api *), Bash(git diff*), Bash(git log*), Bash(git fetch*), Bash(git show*), Bash(git merge-base*), Bash(php -l *), Bash(bash .omp/skills/review-pr/scripts/*), mcp__pms_*, Agent, Read, Grep, Glob
---

# /review-pr — review a GitLab MR

`$ARGUMENTS` = `<MR-iid | MR-url> [--no-merge] [--dry-run]`.

Read `.omp/skills/review-pr/SKILL.md` and execute its workflow EXACTLY (glab → current `.omp/rules` → mechanical PHP gate `scripts/php-gate.sh` (lint 7.4/8.3 + PHPStan on the merge result; `GATE: FAIL` blocks merge) → diff review for bugs/security/logic/convention → post comment with the "Gate tự động" section → auto-merge only if clean AND gate PASS/SKIP, unless `--no-merge`/`--dry-run`) on `$ARGUMENTS`. Obey `~/.omp/agent/APPEND_SYSTEM.md`. Mechanical severity — no subjective downgrade.

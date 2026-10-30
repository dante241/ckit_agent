---
name: review-pr
argument-hint: '<MR-iid | MR-url> [--no-merge] [--dry-run]'
description: "Review GitLab Merge Request for bugs, security, logic, and AGENTS.md/.omp/rules compliance. Posts comment and auto-merges if clean."
allowed-tools: Bash(glab mr *), Bash(glab api *), Bash(git diff*), Bash(git log*), Bash(git fetch*), Bash(git show*), Bash(git merge-base*), Bash(php -l *), Bash(bash .omp/skills/review-pr/scripts/*), Bash(bash ~/.omp/skills/review-pr/scripts/*), mcp__pms_*, Agent, Read, Grep, Glob
---

# /review-pr — review a GitLab MR

`$ARGUMENTS` = `<MR-iid | MR-url> [--no-merge] [--dry-run]`.

Read `skill://review-pr` (`.omp/skills/review-pr/SKILL.md` in the project, or `~/.omp/skills/review-pr/SKILL.md` when the pack is installed with `--global`) and execute its workflow EXACTLY (glab → current rules via `rule://` → mechanical PHP gate `scripts/php-gate.sh` (lint 7.4/8.3 + PHPStan on PHP 8.3 against the merge result; `GATE: FAIL` blocks merge) → diff review for bugs/security/logic/convention → post comment with the "Gate tự động" section → auto-merge only if clean AND gate PASS/SKIP, unless `--no-merge`/`--dry-run`) on `$ARGUMENTS`. Obey `~/.omp/agent/APPEND_SYSTEM.md`. Mechanical severity — no subjective downgrade.

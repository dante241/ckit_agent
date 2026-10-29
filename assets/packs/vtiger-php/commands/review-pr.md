---
name: review-pr
argument-hint: '<MR-iid | MR-url> [--no-merge] [--dry-run]'
description: "Review GitLab Merge Request for bugs, security, logic, and AGENTS.md/.omp/rules compliance. Posts comment and auto-merges if clean."
allowed-tools: Bash(glab mr *), Bash(glab api *), Bash(git diff*), Bash(git log*), Bash(git fetch*), Bash(git show*), Bash(git merge-base*), Bash(php -l *), mcp__pms_*, Agent, Read, Grep, Glob
---

# /review-pr — review a GitLab MR

`$ARGUMENTS` = `<MR-iid | MR-url> [--no-merge] [--dry-run]`.

Read `.omp/skills/review-pr/SKILL.md` and execute its workflow EXACTLY (glab → diff review for bugs/security/logic/convention against the rules it loads → post comment → auto-merge if clean unless `--no-merge`/`--dry-run`) on `$ARGUMENTS`. Obey `~/.omp/agent/APPEND_SYSTEM.md`. Mechanical severity — no subjective downgrade.

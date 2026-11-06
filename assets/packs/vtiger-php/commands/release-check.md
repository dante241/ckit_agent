---
name: release-check
argument-hint: '[--dry-run] [--since YYYY-MM-DD] [--limit N] [--month MM.YYYY] [--no-preview]'
description: "Scan merged MRs có label 'Dev done' hoặc 'Chờ release' + cross-check ticket status trong PMS qua script batch (không dùng MCP ticket_detail). Transition label 'Dev done' → 'Chờ release' cho MR ready. Auto-rollback 'Chờ release' → 'Dev done' khi ticket bị reopen. Sinh report JSON + markdown."
allowed-tools: Bash(bash ~/.config/8sync/packs/vtiger-php/skills/release-check/scripts/*), Bash(glab mr *), Bash(jq*), Bash(cat*), Bash(head*), Read, Write
---

# /release-check — MR nào sẵn sàng release?

`$ARGUMENTS` = `[--dry-run] [--since YYYY-MM-DD] [--limit N] [--month MM.YYYY] [--no-preview]`.

Read `skill://release-check` and execute its workflow EXACTLY (source PMS env → batch-fetch ticket status via scripts → classify → two-way label transition → pick preview → JSON + markdown report) on `$ARGUMENTS`. Obey `~/.omp/agent/APPEND_SYSTEM.md`. Invoke the bundled scripts, don't reimplement the logic.

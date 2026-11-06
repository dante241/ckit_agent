---
name: release-bundle
argument-hint: '[--month MM.YYYY] [--preview] [--handoff] [skip <iids>] [--no-gitlab] [--no-phpstan] [--pause-on-conflict]'
description: "Cherry-pick ticket ready (từ /release-check) lên branch release-core-MM.YYYY tạo từ branch `release` của onlinecrm/vtiger (layout single — 1 repo) theo nhóm ticket (cả ticket hoặc không gì), giữ từng commit. Conflict file không phát hành → giữ bản release; conflict thật → bỏ cả ticket + label `Conflict cherry-pick` (+ MR bàn giao với --handoff). PHPStan trước/sau, report kèm danh sách test. Pick xong gắn `Đã release` + milestone. Không push branch release."
allowed-tools: Bash(bash ~/.config/8sync/packs/vtiger-php/skills/release-bundle/scripts/*), Bash(git *), Bash(jq*), Bash(cat *), Bash(glab *), Read, Write, AskUserQuestion
---

# /release-bundle — cherry-pick MR ready lên release-core-MM.YYYY

`$ARGUMENTS` = `[--month MM.YYYY] [--preview] [--handoff] [skip <iids>] [--no-gitlab] [--no-phpstan] [--pause-on-conflict]`. Precondition: `/release-check` đã chạy (dùng `classified-*.json` mới nhất).

Read `skill://release-bundle` and execute its workflow EXACTLY (`scripts/run.sh`: pick theo ticket → PHPStan trước/sau → label `Đã release` / `Conflict cherry-pick` + milestone → report kèm danh sách test) on `$ARGUMENTS`. Obey `~/.omp/agent/APPEND_SYSTEM.md`. Invoke the bundled scripts; KHÔNG push branch release.

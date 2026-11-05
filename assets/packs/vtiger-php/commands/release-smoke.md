---
name: release-smoke
argument-hint: '<ref> [--keep-db] [--skip step,...]'
description: "Test toàn bộ branch release (release-core-MM.YYYY) trên PHP 8.3 ở local: site + DB clone riêng, migration + QuickRepair, PHPStan so master, crawl 6.480 URL, ghi record, export/queue, API, gom lỗi log PHP/nginx vào report. Không push, không merge."
allowed-tools: Bash(bash .omp/skills/release-smoke/scripts/*), Bash(bash ~/.omp/skills/release-smoke/scripts/*), Bash(git *), Bash(jq*), Bash(cat *), Read, Write
---

# /release-smoke — test branch release trên PHP 8.3 ở local

`$ARGUMENTS` = `<ref> [--keep-db] [--skip step,...]` (ref mặc định: branch tháng hiện tại `release-core-MM.YYYY`).

Read `.omp/skills/release-smoke/SKILL.md` and execute its workflow EXACTLY (`scripts/run.sh <ref>` → đọc `agents/release-queue/smoke-<ngày>-<sha>/report.md` → phân loại từng nhóm lỗi theo bảng "Đọc kết quả" và ghi mục "Phân loại" vào report) on `$ARGUMENTS`. Obey `~/.omp/agent/APPEND_SYSTEM.md`. Lỗi chỉ có trên PHP 8 → sửa trên master qua MR rồi pick; KHÔNG sửa thẳng trên branch release, KHÔNG push.

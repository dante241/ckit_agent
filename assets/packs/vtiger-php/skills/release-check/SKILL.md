---
name: release-check
disable-model-invocation: true
description: "Scan merged MRs có label 'Dev done' hoặc 'Chờ release' + cross-check ticket status trong PMS qua script batch (không dùng MCP ticket_detail). Transition label 'Dev done' → 'Chờ release' cho MR ready. Auto-rollback 'Chờ release' → 'Dev done' khi ticket bị reopen. Sinh report JSON + markdown."
user-invokable: true
allowed-tools: Bash(bash .omp/skills/release-check/scripts/*), Bash(glab mr *), Bash(jq*), Bash(cat*), Bash(head*), Read, Write
---

# Release Check Skill

> "MR nào đã test xong và sẵn sàng release?"
>
> Scan merged MRs label `Dev done` **và `Chờ release`** (kiểm tra lại lần nữa), batch-fetch PMS ticket statuses qua script Node.js (1 lần login + 8 concurrent requests), classify + transition label hai chiều:
> - `Dev done` + ticket ready → **promote** `Chờ release`
> - `Chờ release` + ticket reopen/Testing → **rollback** `Dev done`

## Usage

```
/release-check                # Default: full pipeline, label ready MRs
/release-check --dry-run      # Skip label step
/release-check --since 2026-04-20  # Filter MRs merged after date
/release-check --limit 10     # Cap MRs processed
```

**One-time setup:** PMS credentials phải có trong shell env. Source từ MCP config:
```bash
eval "$(bash .omp/skills/release-check/scripts/source-pms-env.sh)"
```

## Architecture

AI orchestrates, scripts do the heavy lifting. **Không còn dùng `mcp__pms__pms_ticket_detail` 40+ lần** — 1 script Node.js batch-fetch qua PMS CloudWorkApi.

```
scripts/
├── run.sh              # Orchestrator — calls all below
├── scan-mrs.sh         # glab list 2x (Dev done + Chờ release) → dedupe by iid → slim JSON
├── pms-status-batch.js # Login 1x + parallel fetch 8 concurrent → TSV (ticket_no, status, record_id, label, categories, assignee)
├── classify.py         # Merge MRs + statuses → classified.json + report.md (5 buckets + author matrix, PMS-linked ticket IDs)
└── source-pms-env.sh   # Extract PMS_* env vars from `claude mcp get pms`
```

**Token budget:** ~5KB context (vs. ~300KB với MCP-per-ticket cũ).

## Workflow

### Step 1: AI invokes run.sh

```bash
bash .omp/skills/release-check/scripts/run.sh [--dry-run] [--since DATE] [--limit N]
```

Script pipeline:
1. `scan-mrs.sh` → `.claude/release-queue/mrs-slim-<DATE>.json` (gọi `glab mr list` 2 lần: `--label "Dev done"` + `--label "Chờ release"`, merge + dedupe by iid)
2. `jq` extract unique ticket IDs từ titles (regex `#(\d{3,})`) → `ticket-ids-<DATE>.txt`
3. `pms-status-batch.js` login PMS + fetch tất cả statuses song song → `status-<DATE>.tsv`
4. `classify.py` merge + render → `classified-<DATE>.json` + `report-<DATE>.md` + terminal summary (5 buckets: ready / **regressed** / still_testing / no_ticket / ticket_not_found)
5. Transition labels (skip nếu `--dry-run`):
   - Ready mới → `+Chờ release -Dev done`
   - **Regressed** → `+Dev done -Chờ release` (rollback khi ticket đã reopen/quay về Testing)

## Classification matrix

| Current label | Ticket status | Category | Action |
|---------------|---------------|----------|--------|
| `Dev done` | Wait Close / Closed | `ready` | promote `→ Chờ release` |
| `Dev done` | Testing / In Progress / Reopen / ... | `still_testing` | no-op |
| `Chờ release` | Wait Close / Closed | `ready` (đã labeled) | no-op (idempotent) |
| `Chờ release` | **Testing / Reopen / ...** | `regressed` ⚠️ | rollback `→ Dev done` |
| any | NOT_FOUND / ERROR | `ticket_not_found` | manual review |
| no ticket in title | — | `no_ticket` | manual review |

### Step 2: AI reads report

```bash
cat .claude/release-queue/report-<DATE>.md
```

AI chỉ cần đọc report markdown (small, ~2-5KB) và present summary cho user. **Không cần read JSON chi tiết** unless user hỏi follow-up.

### Step 3: Terminal summary (AI presents)

Script in sẵn summary dạng:
```
Scanned: 55 MRs
  ready: 2
  regressed: 1
  still_testing: 46
  no_ticket: 6
  ticket_not_found: 0

Ready MRs: !240 !239
Regressed MRs: !233 (will rollback label)
Report: .claude/release-queue/report-2026-04-23.md
JSON:   .claude/release-queue/classified-2026-04-23.json
```

AI: truyền y nguyên terminal summary + gợi ý next step (`/release-bundle`).

## Rules

| Rule | Detail |
|------|--------|
| Scope | CHỈ xử lý MR có `target_branch = master`. Filter ở scan-mrs.sh qua `--target-branch master`. MR vào develop/release-*/hotfix/* bị skip. |
| Scan labels | Quét **cả** `Dev done` và `Chờ release` — dedupe by iid. Mục đích: re-verify MR đã promote trước đó, bắt trường hợp ticket bị reopen. |
| AI role | Chỉ orchestrate + read report — KHÔNG gọi `mcp__pms__pms_ticket_detail` |
| Script role | Batch fetch, classify, transition label (cả promote + rollback) |
| Credentials | KHÔNG bao giờ hardcode. Reuse PMS MCP env vars qua `source-pms-env.sh` |
| Ready statuses | `Wait Close` và `Closed` (title case, exact) |
| Ticket regex | `#(\d{3,})` — match ticket_no trong MR title |
| Label promote | `glab mr update <iid> --label "Chờ release" --unlabel "Dev done"` — idempotent (skip nếu MR đã có `Chờ release`) |
| Label rollback | `glab mr update <iid> --label "Dev done" --unlabel "Chờ release"` — áp dụng cho `regressed[]` |
| Fallback | Nếu `--dry-run` → script skip cả promote lẫn rollback |

## Error handling

| Error | Hành động |
|-------|-----------|
| `glab` không auth | Script exit, báo user chạy `glab auth status` |
| Missing env vars | Script exit với hint source từ MCP |
| PMS login fail | Script báo error, exit 1 |
| PMS ticket NOT_FOUND | TSV row `ticket_no<TAB>NOT_FOUND<TAB><TAB>` → classified thành `ticket_not_found` |
| `glab mr update` fail | Log MR iid ra stderr, continue các MR khác |

## Debugging

Test individual scripts:
```bash
# Test scan only
bash .omp/skills/release-check/scripts/scan-mrs.sh .claude/release-queue

# Test PMS batch (needs env vars)
echo -e "22036\n1346675" | node .omp/skills/release-check/scripts/pms-status-batch.js

# Test classify (needs inputs from above)
python3 .omp/skills/release-check/scripts/classify.py \
    .claude/release-queue/mrs-slim-2026-04-23.json \
    .claude/release-queue/status-2026-04-23.tsv \
    .claude/release-queue/
```

## References

- `references/classify-logic.md` — Status classification rules, JSON schema (legacy context)
- `scripts/run.sh` — Main entry point
- `scripts/pms-status-batch.js` — PMS API client (Node.js, no deps beyond built-ins)

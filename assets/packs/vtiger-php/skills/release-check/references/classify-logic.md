# Classify Logic

Implementation details cho `/release-check` Step 2–4.

## §1. glab mr list — filter + fields

### CLI
```bash
glab mr list --merged --label "Dev done" --target-branch master --per-page 100 --page <N> --output json
```

`scan-mrs.sh` gọi lệnh này cho cả `Dev done` và `Chờ release`, lặp `--page` tới khi một trang trả < 100 MR.

**Quan trọng:**
- Flag đúng là `--merged` (hoặc `-M`), KHÔNG phải `--state=merged` (glab không support `--state`)
- Label `Dev done` có khoảng trắng → phải quote `"Dev done"`
- `--target-branch master` BẮT BUỘC — skill chỉ xử lý MR merge vào `master`. MR vào `develop`, `release-*`, `hotfix/*`, v.v. bị exclude ngay ở scan step.

### Fields dùng (verified từ smoke test)

Response schema thực tế của `glab mr list` trả về (đã kiểm):
```
iid, title, author.username, source_branch, target_branch,
sha, merge_commit_sha, squash_commit_sha, labels, merged_at, web_url,
state, draft, description
```

**Ưu tiên cho cherry-pick:**
1. `merge_commit_sha` — commit trên master sau khi merge (non-null với GitLab merge commit)
2. Nếu `merge_commit_sha` null + `squash_commit_sha` non-empty → dùng `squash_commit_sha` (GitLab squash-merge)
3. Fallback `sha` (HEAD của source branch — hiếm khi cần)

**Chú ý:** `squash_commit_sha` thường là string rỗng `""`, không phải null. Check:
```bash
SHA=$(echo "$mr" | jq -r 'if .merge_commit_sha then .merge_commit_sha elif (.squash_commit_sha | length > 0) then .squash_commit_sha else .sha end')
```

### Parse ticket_id

```bash
TICKET_ID=$(echo "$title" | grep -oE '#[0-9]{3,}' | head -1 | tr -d '#')
```

Regex: `/#(\d{3,})/` — lấy match đầu.

Ví dụ:
- `[Chatbox] Request #22284: Improve Zalo template` → `22284`
- `MR: [Bug] #17236: Fix productid` → `17236`
- `[Refactor] Remove dead code` → null

## §2. Classify rules (`scripts/classify.py` → `classify()`)

Status lấy từ `status-<DATE>.tsv` do `pms-status-batch.js` sinh (1 lần login PMS, fetch song song) — KHÔNG gọi MCP từng ticket.

```
IF ticket_id IS NULL:                              → no_ticket        (ticket_status = "NO_TICKET")
ELSE IF ticket không có trong TSV
     OR status IN ("NOT_FOUND", "ERROR"):          → ticket_not_found (ticket_status = status | "UNQUERIED")
ELSE IF status IN ("Wait Close", "Closed"):        → ready
ELSE IF MR đang có label "Chờ release":            → regressed (ticket bị reopen/quay lại Testing → rollback label)
ELSE:                                              → still_testing
```

**Chú ý giá trị enum:**
- Exact title-case: `"Wait Close"` (có space), `"Closed"` (`READY_STATUSES` trong classify.py)
- **Không phải** `"WaitClose"`, `"closed"`, `"waitclose"`

## §3. Output schema — classified-YYYY-MM-DD.json

```json
{
  "scan_date": "2026-04-23",
  "repo": "acme/vtiger",
  "label_filter": "Dev done | Chờ release",
  "total_scanned": 55,
  "summary": {"ready": 2, "regressed": 1, "still_testing": 46, "no_ticket": 6, "ticket_not_found": 0},
  "ready": [
    {
      "iid": 218,
      "ticket_id": "22284",
      "title": "[Chatbox] Request #22284: Improve Zalo template",
      "author": "dev.a",
      "merged_at": "2026-04-20T10:30:00Z",
      "web_url": "https://git.example.com/acme/vtiger/-/merge_requests/218",
      "labels": ["Dev done"],
      "merge_commit_sha": "abc123...",
      "squash_commit_sha": "",
      "sha": "def456...",
      "ticket_status": "Closed",
      "ticket_record_id": "1346675",
      "ticket_label": "...",
      "ticket_categories": "...",
      "ticket_assignee": "..."
    }
  ],
  "regressed": [],
  "still_testing": [],
  "no_ticket": [{"iid": 225, "ticket_id": null, "title": "[Refactor] cleanup", "ticket_status": "NO_TICKET", "...": "..."}],
  "ticket_not_found": [{"iid": 227, "ticket_id": "99999", "ticket_status": "NOT_FOUND", "...": "..."}]
}
```

`ticket_record_id`/`ticket_label`/`ticket_categories`/`ticket_assignee` chỉ có ở MR đã lấy được status (ready / regressed / still_testing).

## §4. Label transitions (`scripts/run.sh` Step 5, skip khi `--dry-run`)

```bash
# Promote: ready MR chưa có "Chờ release"
glab mr update <iid> --label "Chờ release" --unlabel "Dev done"
# Rollback: mọi MR trong regressed[]
glab mr update <iid> --label "Dev done" --unlabel "Chờ release"
```

- Idempotent: ready MR đã có `Chờ release` bị lọc bằng `jq 'select((.labels | index("Chờ release")) | not)'` — không gọi API.
- `glab mr update` fail → in `FAILED to transition/rollback !<iid>` ra stderr, chạy tiếp MR khác. Không có counter trong JSON.

## §5. Error handling

| Error scenario | Hành vi thật |
|---|---|
| `glab mr list` lỗi auth/network | `scan-mrs.sh` chạy `set -euo pipefail` → dừng với lỗi của glab; user tự chạy `glab auth status` |
| PMS không có ticket | TSV `NOT_FOUND` → `ticket_not_found` |
| PMS lỗi request | TSV `ERROR` → `ticket_not_found` |
| `glab mr update` lỗi | Log stderr, continue |

## §6. Markdown report

`report-<DATE>.md` do `render_markdown()` trong classify.py sinh (bảng theo 5 bucket, ticket link sang PMS khi có `PMS_BASE_URL` + record id, ma trận theo author). Đọc hàm đó khi cần đổi format — không duy trì template riêng ở đây.

**Next step:** Chạy `/release-bundle` để cherry-pick các MR ready (label `Chờ release`) sang `vtiger_release`.

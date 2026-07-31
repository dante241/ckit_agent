# Classify Logic

Implementation details cho `/release-check` Step 2–4.

## §1. glab mr list — filter + fields

### CLI
```bash
glab mr list --merged --label "Dev done" --target-branch master --per-page 100 --output json
```

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

## §2. Classify rules

```
IF ticket_id IS NULL:
    category = "no_ticket"
ELSE:
    ticketstatus = mcp__pms__pms_ticket_detail(id=ticket_id).ticketstatus
    IF call fail OR ticket not found:
        category = "ticket_not_found"
    ELSE IF ticketstatus IN ("Wait Close", "Closed"):
        category = "ready"
    ELSE:
        category = "testing"
```

**Chú ý giá trị enum:**
- Exact title-case: `"Wait Close"` (có space), `"Closed"`
- **Không phải** `"WaitClose"`, `"closed"`, `"waitclose"`
- Lấy từ `languages/en_us/HelpDesk.php` và CloudWORK portal API

## §3. Output schema — classified-YYYY-MM-DD.json

```json
{
  "generated_at": "2026-04-23T14:30:00Z",
  "repo": "acme/vtiger",
  "label_filter": "Dev done",
  "flags": {
    "dry_run": false,
    "since": null,
    "limit": null
  },
  "stats": {
    "total_scanned": 12,
    "ready": 5,
    "testing": 6,
    "no_ticket": 0,
    "ticket_not_found": 1,
    "transitioned_new": 3,
    "transitioned_already": 2
  },
  "ready": [
    {
      "iid": 218,
      "title": "[Chatbox] Request #22284: Improve Zalo template",
      "author": "dev.a",
      "source_branch": "feat/22284-chatbox-zalo",
      "sha": "def456...",
      "merge_commit_sha": "abc123...",
      "labels": ["Chờ release"],
      "merged_at": "2026-04-20T10:30:00Z",
      "web_url": "https://git.example.com/acme/vtiger/-/merge_requests/218",
      "ticket_id": "22284",
      "ticketstatus": "Closed",
      "labeled_in_this_run": false
    }
  ],
  "testing": [
    {
      "iid": 230,
      "title": "[Fix] #17670: ...",
      "ticket_id": "17670",
      "ticketstatus": "In Progress",
      ...
    }
  ],
  "no_ticket": [
    { "iid": 225, "title": "[Refactor] cleanup", ... }
  ],
  "ticket_not_found": [
    { "iid": 227, "title": "[Bug] #99999: ...", "ticket_id": "99999", "error": "Ticket not found" }
  ]
}
```

**Field `labeled_in_this_run`:** true nếu Step 3 vừa gọi `glab mr update --label "Chờ release" --unlabel "Dev done"` cho MR này trong run hiện tại. False nếu MR đã có `Chờ release` từ trước hoặc `--dry-run`.

## §4. glab label transition API

### Transition label (add + remove trong 1 call)
```bash
glab mr update <iid> --label "Chờ release" --unlabel "Dev done"
```

**Verify:** `glab mr update --label X --unlabel Y` thực hiện `+X / -Y` trong một request, idempotent. Test:
```bash
glab mr view 218 --output json | jq '.labels'
# Trước: ["Dev done"]
glab mr update 218 --label "Chờ release" --unlabel "Dev done"
glab mr view 218 --output json | jq '.labels'
# Sau: ["Chờ release"]
```

Nếu MR đã có `Chờ release` + `Dev done` đồng thời (edge case): `--label X` no-op (đã có), `--unlabel Y` vẫn remove Y → kết quả vẫn đúng `["Chờ release"]`.

## §5. Idempotent check

Trước khi gọi `glab mr update`, check từ response `labels[]` ở Step 1:

```bash
LABELS=$(jq -r '.labels' /tmp/mr-$iid.json)
if echo "$LABELS" | jq -e 'any(. == "Chờ release")' > /dev/null; then
    TRANSITIONED_ALREADY=true
else
    glab mr update $iid --label "Chờ release" --unlabel "Dev done"
    TRANSITIONED_NEW=true
fi
```

## §6. Stats counter

Khi sinh terminal summary:

```
transitioned_new     = Số MR ready được transition "Dev done → Chờ release" TRONG lần chạy này
transitioned_already = Số MR ready đã có "Chờ release" trước đó (không gọi API)

Nếu --dry-run: transitioned_new = 0, transitioned_already = <count of ready with "Chờ release">
```

## §7. Error classification

| Error scenario | Category | Log |
|---|---|---|
| `glab mr list` lỗi auth/network | ABORT | Hướng dẫn `glab auth status` |
| PMS trả `{error: "not found"}` | `ticket_not_found` | Log vào ticket_not_found[].error |
| PMS timeout/connection error | Retry 1 lần, fail → `ticket_not_found` | Log error |
| `glab mr update` lỗi | Continue (log warning) | Ghi vào stats.transition_failed |

## §8. Markdown report template

```markdown
# Release Check — {{generated_at}}

**Repo:** {{repo}} | **Filter:** `Dev done` | **Total scanned:** {{total}}
**Generated:** {{generated_at}} | **Dry-run:** {{dry_run}}

## Summary

| Category | Count |
|----------|-------|
| ✅ Ready to release (transitioned → Chờ release) | {{ready.length}} |
| ⏳ Still testing | {{testing.length}} |
| ⚠️ No ticket | {{no_ticket.length}} |
| ❌ Ticket not found | {{ticket_not_found.length}} |

{{#if not dry_run}}
**Transitions in this run:** {{transitioned_new}} new, {{transitioned_already}} already had `Chờ release`.
{{/if}}

## ✅ Ready to release

| MR | Ticket | Status | Title | Author | Merged at | Transitioned |
|----|--------|--------|-------|--------|-----------|--------------|
| [!{{iid}}]({{web_url}}) | #{{ticket_id}} | {{ticketstatus}} | {{title}} | {{author}} | {{merged_at | date}} | {{#if labeled_in_this_run}}🆕{{else}}✓{{/if}} |

## ⏳ Still testing

| MR | Ticket | Status | Title | Author | Merged at |
|----|--------|--------|-------|--------|-----------|
| [!{{iid}}]({{web_url}}) | #{{ticket_id}} | {{ticketstatus}} | {{title}} | {{author}} | {{merged_at | date}} |

## ⚠️ No ticket

| MR | Title | Author |
|----|-------|--------|
| [!{{iid}}]({{web_url}}) | {{title}} | {{author}} |

## ❌ Ticket not found

| MR | Ticket ID | Title | Error |
|----|-----------|-------|-------|
| [!{{iid}}]({{web_url}}) | #{{ticket_id}} | {{title}} | {{error}} |

---

**Next step:** Chạy `/release-bundle` để cherry-pick các MR ready (label `Chờ release`) sang `vtiger_release`.
```

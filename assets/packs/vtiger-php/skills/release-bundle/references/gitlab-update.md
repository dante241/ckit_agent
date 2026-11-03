# GitLab Update Logic

Chi tiết implementation cho Phase 2 của `/release-bundle` — update label + milestone cho các MR đã pick thành công.

## §1. Milestone naming convention

| Thành phần | Giá trị |
|-----------|--------|
| Pattern | `Release core tháng MM/YYYY` |
| MM | 2-digit month, zero-padded (`01`, `02`, ..., `12`) |
| YYYY | 4-digit year |
| Ví dụ | `Release core tháng 04/2026` |

Tính từ tháng release trong `lib.sh`: `--month MM.YYYY` (env `RELEASE_MONTH`), mặc định tháng hiện tại:
```bash
MILESTONE_TITLE="Release core tháng $MONTH/$YEAR"   # MONTH/YEAR do lib.sh đặt
```

## §2. Milestone lookup hoặc auto-create

### Lookup

```bash
# URL-encode title cho query param
TITLE_ENC=$(printf %s "$MILESTONE_TITLE" | jq -sRr @uri)

MILESTONE_ID=$(glab api "projects/onlinecrm%2Fvtiger/milestones?search=$TITLE_ENC&state=active" \
    | jq -r --arg t "$MILESTONE_TITLE" '.[] | select(.title == $t) | .id' \
    | head -1)
```

**Lưu ý:** `search` param của GitLab API trả về partial match (LIKE `%title%`), nên cần filter exact với `select(.title == $t)`.

### Auto-create nếu không tồn tại

```bash
if [ -z "$MILESTONE_ID" ] || [ "$MILESTONE_ID" = "null" ]; then
    # Tính due_date = ngày cuối tháng
    DUE_DATE=$(
        # macOS BSD date
        date -v1d -v+1m -v-1d +%Y-%m-%d 2>/dev/null \
        || \
        # GNU date (Linux)
        date -d "$(date +%Y-%m-01) +1 month -1 day" +%Y-%m-%d
    )

    # start_date = ngày 1 đầu tháng
    START_DATE="${YEAR}-${MONTH}-01"

    MILESTONE_JSON=$(glab api -X POST "projects/onlinecrm%2Fvtiger/milestones" \
        -f title="$MILESTONE_TITLE" \
        -f description="Auto-created bởi /release-bundle" \
        -f start_date="$START_DATE" \
        -f due_date="$DUE_DATE")

    MILESTONE_ID=$(echo "$MILESTONE_JSON" | jq -r .id)
    echo "  ✓ Tạo milestone: $MILESTONE_TITLE (id=$MILESTONE_ID, due=$DUE_DATE)"
fi
```

### Lưu ý permissions

Tạo milestone cần role ≥ **Developer** trên project. Nếu `glab api POST` trả `403 Forbidden`:
- Báo user: "Không đủ quyền tạo milestone. Nhờ maintainer tạo tay `$MILESTONE_TITLE` rồi chạy lại."
- Exit != 0.

## §3. Auto-create label `Conflict cherry-pick`

```bash
LABEL_NAME="Conflict cherry-pick"
EXISTS=$(glab api "projects/onlinecrm%2Fvtiger/labels?search=$(printf %s "$LABEL_NAME" | jq -sRr @uri)" \
    | jq -r --arg n "$LABEL_NAME" '.[] | select(.name == $n) | .name' | head -1)

if [ -z "$EXISTS" ]; then
    glab api -X POST "projects/onlinecrm%2Fvtiger/labels" \
        -f "name=$LABEL_NAME" \
        -f "color=#d9534f" \
        -f "description=MR có conflict khi cherry-pick lên release branch — cần resolve thủ công" \
        >/dev/null
    echo "  ✓ Tạo label: $LABEL_NAME"
fi
```

## §4. Update MR (label + milestone)

### Endpoint

`PUT /projects/:id/merge_requests/:iid`

### Outcomes & label flow

| Outcome | add_labels | remove_labels | milestone_id |
|---------|-----------|---------------|--------------|
| **PICKED** (commit mới) | `Đã release` | `Chờ release` | `$MILESTONE_ID` |
| **DROPPED** (patch already upstream) | `Đã release` | `Chờ release` | `$MILESTONE_ID` |
| **CONFLICT** (auto-skip) | `Conflict cherry-pick` | (giữ `Chờ release`) | (không set) |
| **USER_SKIPPED** | (không thay đổi) | (không thay đổi) | (không thay đổi) |

DROPPED nghĩa là changes ĐÃ trên branch (git phát hiện trùng patch) → vẫn label `Đã release` vì release branch đã có nội dung này.

### Code

```bash
BUNDLE_RESULT="$STATE_DIR/bundle-result-$TODAY.json"

# 1. PICKED + DROPPED → Đã release + milestone
PICKED_IIDS=$(jq -r '.picked[].iid' "$BUNDLE_RESULT")
for iid in $PICKED_IIDS; do
    echo "→ MR !$iid (released)..."

    RESPONSE=$(glab api -X PUT "projects/onlinecrm%2Fvtiger/merge_requests/$iid" \
        -f "add_labels=Đã release" \
        -f "remove_labels=Chờ release" \
        -f "milestone_id=$MILESTONE_ID" 2>&1)

    if echo "$RESPONSE" | jq -e .iid >/dev/null 2>&1; then
        echo "  ✓ Đã release | Milestone: $MILESTONE_TITLE"
    else
        echo "  ✗ FAILED: $RESPONSE" >&2
    fi
done

# 2. CONFLICTS → label Conflict cherry-pick cho MỌI MR của ticket (cả ticket bị bỏ, giữ Chờ release để retry)
CONFLICT_IIDS=$(jq -r '.conflicts_manual[]? | (.iids // [.iid])[]' "$BUNDLE_RESULT")
for iid in $CONFLICT_IIDS; do
    echo "→ MR !$iid (conflict — manual)..."

    glab api -X PUT "projects/onlinecrm%2Fvtiger/merge_requests/$iid" \
        -f "add_labels=Conflict cherry-pick" >/dev/null

    echo "  ✓ +Conflict cherry-pick (giữ Chờ release)"
done

# 3. USER_SKIPPED → không update (giữ Chờ release để retry)
SKIPPED_IIDS=$(jq -r '.skipped_by_user[]? | (.iids // [.iid])[]' "$BUNDLE_RESULT")
for iid in $SKIPPED_IIDS; do
    echo "→ MR !$iid (user-skipped — giữ nguyên label)"
done

# bundle-result của --preview (.preview = true) bị từ chối; pending_handoff[] không đổi label.
```

## §4. Idempotency

Phase 2 an toàn khi chạy lại:
- `add_labels=Đã release` — đã có thì GitLab no-op
- `remove_labels=Chờ release` — đã remove thì no-op
- `milestone_id=X` — set lại cùng value = no-op

Nếu user chạy `/release-bundle` nhiều lần trong ngày (lần 2 sau khi đã push), Phase 2 sẽ chạy lại update an toàn.

## §5. Edge cases

### 5.1 — MR không còn tồn tại (deleted)

```bash
RESPONSE=$(glab api -X PUT ...)
if echo "$RESPONSE" | grep -q "404 Not Found"; then
    echo "  ⚠ MR !$iid không tồn tại, skip" >&2
    continue
fi
```

### 5.2 — Rate limit

Batch update nhiều MR cùng lúc có thể gặp 429. Sleep 1s giữa calls nếu ≥ 10 MR:

```bash
COUNT=0
for iid in $PICKED_IIDS; do
    COUNT=$((COUNT + 1))
    [ "$COUNT" -gt 10 ] && sleep 1
    # ... update logic
done
```

### 5.3 — glab auth expired

Check `glab auth status` ở Phase 0 precondition. Nếu expired → báo user chạy `glab auth login`.

### 5.4 — Milestone tồn tại ở state `closed`

Query với `state=active` loại bỏ milestone đã closed. Nếu milestone bị closed (hết tháng cũ), sẽ fallback vào auto-create milestone mới — đúng ý.

## §6. Rollback (manual)

Recipe duy nhất (git reset + revert label/milestone): `SKILL.md` § Rollback.

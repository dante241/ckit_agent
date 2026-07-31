# Step 6.5–7.5: Labels, merge decision, PMS update

### Step 6.5: Gán label kết quả review

```bash
# Xóa label cũ (clean trạng thái trước đó nếu review lại)
glab mr update <MR_NUMBER> --repo $REPO --unlabel "Cần Fix,Review Failed,Dev done,Chờ release,Đã release"

# PASS (không có lỗi CRITICAL/HIGH):
#   Gắn label "Dev done" — release-check sẽ transition sang "Chờ release" sau khi verify PMS ticket
glab mr update <MR_NUMBER> --repo $REPO --label "Dev done"

# Có lỗi CRITICAL/HIGH:
glab mr update <MR_NUMBER> --repo $REPO --label "Cần Fix"
```

### Step 7: Merge Decision

**Default: Auto-merge nếu không có lỗi CRITICAL/HIGH/PERFORMANCE.**

**Điều kiện merge:**
- ✅ Không có lỗi CRITICAL hoặc HIGH
- ✅ Không có lỗi PERFORMANCE (bất kỳ severity nào do Agent 1 phát hiện)
- ✅ Không có flag `--no-merge` hoặc `--dry-run`
- ✅ Comment đã được post thành công ở Step 6
- ✅ Label "Dev done" đã gán xong ở Step 6.5

**Nếu đủ điều kiện:**

1. Merge commit title = `MR: ` + tên MR gốc trên GitLab:
   - Format: `MR: <Tên MR gốc>`
   - Ví dụ:
     - Tên MR: `[Campaigns] Request #21766: Individual Zalo, Telegram`
       → Commit: `MR: [Campaigns] Request #21766: Individual Zalo, Telegram`
     - Tên MR: `[Chatbox] Bug #21737: Fix phone filter on single-page chatbox`
       → Commit: `MR: [Chatbox] Bug #21737: Fix phone filter on single-page chatbox`
     - Tên MR: `[CORE] #19396: Release UI/UX CxDay 2025`
       → Commit: `MR: [CORE] #19396: Release UI/UX CxDay 2025`

2. Merge the MR (squash + xoá source branch):
   ```bash
   glab mr merge <MR_NUMBER> --repo $REPO --message "<MERGE_COMMIT_TITLE>" --squash --remove-source-branch --yes
   ```

3. Confirm merge succeeded and report URL.

**BẮT BUỘC:** Luôn dùng `--remove-source-branch` khi merge để tự động xoá branch nguồn sau merge. Tránh để branch cũ tồn đọng trên remote.

**Nếu có lỗi CRITICAL/HIGH/PERFORMANCE:**
- KHÔNG merge
- Báo cáo: "MR có <N> lỗi chặn merge (gồm <X> lỗi PERFORMANCE). Vui lòng fix trước khi merge."

**Nếu có flag `--no-merge`:**
- Post comment + label, nhưng KHÔNG merge

**Nếu có flag `--dry-run`:**
- Chỉ in kết quả ra terminal, KHÔNG làm gì thêm

### Step 7.5: Update PMS ticket (BẮT BUỘC sau khi merge thành công)

**Điều kiện chạy:** Step 7 đã merge thành công VÀ không có `--dry-run`/`--no-merge`.

1. Kiểm tra `$TICKET_VALID_NEXT_STATUSES` từ Step 0.6 (ticket_detail response) để biết status nào đang hợp lệ.

2. **Update 4 trường cùng lúc qua `pms_ticket_update` (dùng OpenAPI, cho phép update nhiều field):**

   **⚠️ QUAN TRỌNG:** Dùng `$TICKET_RECORD_ID` (record_id của HelpDesk lấy từ Step 0.6), KHÔNG phải `$TICKET_ID` (ticket_no như `22369`). Sai sẽ trả `Record not found!`.

   ```
   mcp__pms__pms_ticket_update(id=$TICKET_RECORD_ID, data={
       "ticketstatus": "Testing",
       "helpdesk_released_version": "unreleased",
       "helpdesk_project_type": "rnd",
       "rating_note": "<RATING_NOTE_HTML>"
   })
   ```

   **Cấu trúc `rating_note` (Markdown + `\n` thật — KHÔNG dùng HTML):**

   PMS strip tất cả HTML tags (kể cả literal `<table>` trong backticks) nhưng **giữ nguyên** `\n`, Markdown syntax (`**bold**`, `- list`, `` `code` ``), Unicode, URLs. Đã verify trên ticket thực tế.

   ```
   **MR !<NUMBER>:** <MR_URL>

   **Kết quả review:**
   - Files: <COUNT> | Dòng thay đổi: +<ADD>/-<DEL>
   - Lỗi CRITICAL: <N1> | HIGH: <N2> | MEDIUM: <N3> | LOW: <N4>

   **Tóm tắt lỗi phát hiện:**
   - [SEVERITY] Tiêu đề lỗi 1 — file:line
   - [SEVERITY] Tiêu đề lỗi 2 — file:line

   _Reviewed by Dante at <TIMESTAMP>_
   ```

   Nếu clean (không có lỗi):
   ```
   **MR !<NUMBER>:** <MR_URL>

   ✅ Không phát hiện lỗi CRITICAL/HIGH. Đã auto-merge.

   _Reviewed by Dante at <TIMESTAMP>_
   ```

   **GOTCHAS:**
   - Truyền `\n` thật trong JSON string (escape là `"\n"`, KHÔNG phải `\\n`). MCP `pms_ticket_update` nhận data JSON nên MCP client đã handle escape.
   - TRÁNH literal HTML tag trong text (VD `<table>`, `<script>`, `<div>`) ngay cả khi bọc trong backticks — PMS vẫn strip. Dùng tên plain: `table element`, `script tag`, `div node`.
   - Không cần JSON.stringify bên ngoài — MCP đã serialize.

3. **Lưu ý về `ticketstatus`:**
   - Nếu `Testing` không có trong `$TICKET_VALID_NEXT_STATUSES` → transition fail
   - Fallback: dùng `pms_ticket_status(ticket_id=$TICKET_RECORD_ID, ticketstatus="Testing")` trước để validate transition, nếu lỗi → log warning và chỉ update 3 field còn lại
   - Hoặc thử các alias: `Testing`, `Wait Close`, `In Progress` theo workflow dự án

4. **Handle response:**
   - Thành công → tiếp tục bước 5
   - Lỗi → log warning, không block flow (vì merge đã xong)

5. Thêm comment note trên ticket PMS (tách khỏi `rating_note` để giữ lịch sử — **optional**, có thể skip nếu `rating_note` đã đầy đủ):

   **⚠️ API yêu cầu `assigned_user_id`** — nếu không truyền sẽ trả `These fields are required: assigned_user_id.`. Ưu tiên dùng user ID của reviewer (người chạy skill). Nếu không xác định được → **skip bước này**, không block flow vì `rating_note` đã có đầy đủ context.

   ```
   mcp__pms__pms_comments(action="add", related_to=$TICKET_RECORD_ID,
       assigned_user_id=$REVIEWER_USER_ID,
       commentcontent="<p>MR !<NUMBER> đã merge vào master — <TIMESTAMP>.<br/>Link MR: <a href='<MR_URL>'><MR_URL></a><br/>Trạng thái: Chờ release deploy.</p>")
   ```

   **Lấy `$REVIEWER_USER_ID`:**
   - Cách 1: Dùng `mcp__pms__pms_users(action="list")` search theo username (lấy username từ `git config user.name` hoặc git remote)
   - Cách 2: Hardcode user ID của Dante/reviewer trong config team
   - Cách 3: Skip comment nếu không có — `rating_note` đã đủ

6. Báo cáo kết quả cho user:
   ```
   ✅ MR !<NUMBER> merged.
   ✅ Label MR: "Dev done"
   ✅ Ticket #<TICKET_ID> (record <RECORD_ID>) updated:
      - ticketstatus = Testing
      - helpdesk_released_version = unreleased
      - helpdesk_project_type = rnd
      - rating_note = <tóm tắt review + link MR>
   ```

**Nếu PMS update thất bại (toàn bộ hoặc 1 field):**
- Không rollback merge (đã push lên master)
- Log warning chi tiết field nào fail, giá trị gì
- Yêu cầu user update ticket thủ công
- Không block flow chung


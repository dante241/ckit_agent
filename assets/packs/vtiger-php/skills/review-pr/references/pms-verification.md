# Step 0.5 + 0.6: Load previous comments & PMS ticket verification

### Step 0.5: Load Previous Review Comments (BẮT BUỘC)

**Mục đích:** Nếu MR đã có comment review trước đó (từ Dante hoặc Claude Code), phải đọc và phân tích trước khi review mới.

1. Fetch tất cả comments trên MR:
   ```bash
   glab api "projects/$(echo $REPO | sed 's|/|%2F|g')/merge_requests/<MR_NUMBER>/notes?per_page=100" --repo $REPO
   ```

2. Lọc comments chứa "Reviewed by Dante" hoặc "Reviewed by Claude Code" hoặc "Code Review — MR"

3. Nếu **có** review cũ:
   - Parse danh sách lỗi đã phát hiện (CRITICAL, HIGH, MEDIUM)
   - Ghi nhận số lỗi cũ theo severity
   - Lưu vào context: `$PREVIOUS_ISSUES` = danh sách lỗi cũ
   - Ở Step 3, truyền `$PREVIOUS_ISSUES` cho tất cả agents kèm instruction:
     > "Đây là danh sách lỗi đã phát hiện từ review trước. Kiểm tra xem:
     > (a) Lỗi nào đã được fix? → đánh dấu RESOLVED
     > (b) Lỗi nào vẫn còn? → giữ nguyên, không cần mô tả lại chi tiết
     > (c) Có lỗi MỚI nào chưa phát hiện lần trước? → flag là NEW"
   - Ở Step 6, comment format thêm section so sánh:
     ```markdown
     ### So sánh với review trước
     - :white_check_mark: Đã fix: <count> lỗi
     - :warning: Chưa fix: <count> lỗi
     - :new: Phát hiện mới: <count> lỗi

     **Chi tiết lỗi đã fix:**
     - ~~Lỗi cũ #1: mô tả~~ → RESOLVED
     - ~~Lỗi cũ #2: mô tả~~ → RESOLVED
     ```

4. Nếu **không** có review cũ → bỏ qua, review bình thường (first review)

4. **Kiểm tra tên MR — BẮT BUỘC phải có `#<số_ticket>`**

   Format tên MR: `[Category] #Ticket: Description`

   Ví dụ đúng:
   - `[Campaigns] #21766: Individual Zalo, Telegram, Internal Communications`
   - `[Chatbox] Bug #21737: Fix phone filter` (tag "Bug/Request/Ticket" trước `#` OK)
   - `[CORE] #19396: Release UI/UX CxDay 2025`

   Ví dụ **SAI** (BẮT BUỘC block):
   - `[Core] Pipeline: Fix bug some case...` — **không có `#<number>`**
   - `[CloudWORK] Updated apply date retrieval...` — **không có `#<number>`**
   - `[Core] Update link URL format...` — **không có `#<number>`**

   **Extract ticket ID (BẮT BUỘC dùng regex chính xác):**
   ```bash
   TICKET_ID=$(echo "$MR_TITLE" | grep -oE '#[0-9]+' | head -1 | tr -d '#')
   ```

   Quy tắc:
   - **`[Category]`**: Tên module hoặc nhóm tính năng (BẮT BUỘC có ngoặc vuông)
   - **`#Ticket`**: Số ticket từ PMS (BẮT BUỘC — pattern `#<digits>`)
   - **`Description`**: Mô tả ngắn gọn thay đổi

   **Nếu regex không match `#[0-9]+` → BLOCK review ngay:**
   1. KHÔNG chạy Step 1+ (không review code)
   2. Post comment yêu cầu rename MR (dùng template `/review-pr-block-no-ticket.md` bên dưới)
   3. Gắn label `Cần Fix`
   4. Dừng skill

   **Template comment khi thiếu ticket ID:**
   ```markdown
   ## Code Review — MR !<NUMBER> — BLOCKED

   :rotating_light: **Không tìm thấy số ticket trong tên MR.** Review bị chặn.

   **Tên hiện tại:** `<MR_TITLE>`

   **Rule (BẮT BUỘC):** `[Category] #Ticket: Description` — phải có `#<số_ticket>`.

   ### Cần fix
   1. Rename MR theo đúng format, ví dụ:
      - `[Module] #1234: <description>`
      - `[Module] Bug #1234: <description>`
   2. Trigger lại review: comment `/review-pr` hoặc update MR.

   ---
   :robot: Reviewed by Dante
   ```

   **Không có ngoại lệ — dù code đúng đến đâu cũng phải block khi thiếu ticket ID.**

5. Extract MR metadata:
   - `$MR_TITLE` — MR title
   - `$MR_AUTHOR` — author username
   - `$MR_SOURCE` — source branch
   - `$MR_TARGET` — target branch (usually `master`)
   - `$MR_DESCRIPTION` — MR description
   - `$TICKET_ID` — số ticket extract từ title (BẮT BUỘC có, nếu null đã block ở bước 4)

### Step 0.6: Verify ticket trên PMS (BẮT BUỘC)

**Mục đích:** Đảm bảo ticket tồn tại trên hệ thống PMS trước khi review, để gắn context vào comment và update status khi xong.

**⚠️ QUAN TRỌNG:** `$TICKET_ID` extract từ MR title là **ticket_no** (ví dụ `22369`), KHÔNG phải **record_id** của HelpDesk. `pms_ticket_detail(id=...)` cần record_id → PHẢI search trước để lấy record_id.

1. **Search ticket bằng `ticket_no`** (BẮT BUỘC — stored format trong DB là `#22369` với dấu hash):

   ```
   mcp__pms__pms_tickets(filters=[{name: "ticket_no", value: "$TICKET_ID", operator: "c"}])
   ```

   Dùng operator `c` (contains) vì DB lưu `#22369` chứ không phải `22369`. Operator `e` (exact) sẽ không match.

2. Xử lý response:
   - **`entry_list` rỗng** → ticket không tồn tại → BLOCK (xem step 4 bên dưới)
   - **`entry_list` có record** → extract `ticketid` (hoặc `id`) → đây là `$TICKET_RECORD_ID`

3. **Gọi `pms_ticket_detail` với record_id vừa lấy** để lấy full context:
   ```
   mcp__pms__pms_ticket_detail(id=$TICKET_RECORD_ID)
   ```

   Lưu vào context các trường:
   - `$TICKET_RECORD_ID` — record_id của HelpDesk (dùng cho tất cả API PMS sau này)
   - `$TICKET_TITLE` — `ticket.label` / `ticket_title`
   - `$TICKET_STATUS` — `ticket.ticketstatus`
   - `$TICKET_ASSIGNED_TO` — `ticket.main_owner_name` hoặc `assigned_owners[0].name`
   - `$TICKET_PRIORITY` — `ticket.ticketpriorities`
   - `$TICKET_MODULE` — `ticket.related_project_label` hoặc `record_module`
   - `$TICKET_URL` — build từ PMS domain + record_id (optional, có thể skip nếu không biết domain)
   - `$TICKET_VALID_NEXT_STATUSES` — `valid_next_statuses.data[].value` (để dùng ở Step 7.5)

4. **KHÔNG tìm thấy** (entry_list rỗng ở bước 1):
   1. KHÔNG chạy Step 1+
   2. Post comment BLOCK (template bên dưới)
   3. Gắn label `Cần Fix`
   4. Dừng skill

3. **Template comment khi ticket không tồn tại:**
   ```markdown
   ## Code Review — MR !<NUMBER> — BLOCKED

   :rotating_light: **Ticket #<TICKET_ID> không tồn tại trên PMS.** Review bị chặn.

   **Tên MR:** `<MR_TITLE>`
   **Ticket ID extract được:** `#<TICKET_ID>`

   ### Cần fix
   1. Kiểm tra số ticket có đúng không. Search trên PMS để xác nhận ticket tồn tại.
   2. Nếu ticket ID sai → rename MR với ticket ID đúng.
   3. Nếu chưa có ticket → tạo ticket trên PMS trước, rồi rename MR.
   4. Trigger lại review sau khi fix.

   ---
   :robot: Reviewed by Dante
   ```

4. **Nếu ticket tìm thấy → tiếp tục Step 1.**


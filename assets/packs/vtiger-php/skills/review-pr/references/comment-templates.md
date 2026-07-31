# Comment Templates (post lên GitLab MR)

## Code Review — MR !<NUMBER>

**Reviewer:** Dante | **Files:** <COUNT> | **Dòng thay đổi:** +<ADD>/-<DEL>

### Thông tin ticket
- **Ticket:** [#<TICKET_ID>](<TICKET_URL>) — <TICKET_TITLE>
- **Trạng thái hiện tại:** <TICKET_STATUS>
- **Assignee:** <TICKET_ASSIGNED_TO>
- **Priority:** <TICKET_PRIORITY>
- **Module liên quan:** <TICKET_MODULE>

### Phạm vi nghiệp vụ đã review

**Mục tiêu MR:** <feature_intent từ Business Manifest>

### Checklist tính năng theo yêu cầu ticket

**BẮT BUỘC.** Đối chiếu từng yêu cầu trong ticket description với code thực tế.

Quy tắc:
- ✅ **Done** — Có đủ code thực hiện yêu cầu, đã verify file:line
- ⚠️ **Partial** — Có code nhưng thiếu nhánh / thiếu validate / chưa wire đủ flow
- ❌ **Missing** — Không tìm thấy code tương ứng trong diff
- ❓ **Not verifiable** — Không thể verify (vd: yêu cầu UI mà reviewer không chạy browser)

**Nhóm 1: <category từ ticket, vd: "Tích hợp kênh đăng bài">**
- ✅ **REQ-01** <mô tả requirement nguyên văn>
  - Evidence: `path/to/file.php:123`, `path/to/template.tpl:45`
- ⚠️ **REQ-02** <mô tả>
  - Evidence: `path/to/file.js:67`
  - Gap: <ngắn gọn — thiếu gì, làm thiếu nhánh nào>
- ❌ **REQ-03** <mô tả>
  - Gap: Không tìm thấy implementation trong diff. Có thể bị bỏ sót hoặc tách sang MR khác.

**Nhóm 2: <category tiếp theo>**
- ...

**Tổng kết yêu cầu:** ✅ <X>/<TOTAL> done · ⚠️ <Y> partial · ❌ <Z> missing · ❓ <W> not verifiable

> Lỗi MISSING/PARTIAL chặn merge ở mức HIGH (trừ khi author confirm tách sang MR khác).

### Business rules (<N>):
- [x|!|✗] **BR-01** <rule statement ngắn gọn>
  - Knowledge consistency: ✓ OK với `<knowledge_file.md>` / ✗ Mâu thuẫn với `<rule>`
  - Edge cases: <liệt kê đã cover / thiếu>
  - Side effects: <list handler/event bị trigger, đã verify hay chưa>
- [!] **BR-02** <rule statement> — phát hiện <N> lỗi (xem phần "Các lỗi" bên dưới)
- [x] **BR-03** <rule statement> — OK

**Status transitions:** <liệt kê từ manifest hoặc "Không thay đổi">

**Side effect audit:** <handler chain đã trace, risk loop/duplicate event...>

**Coverage:** <reviewed>/<total> rules reviewed. <unknowns nếu có>

### Các lỗi phát hiện (<COUNT>)

#### :rotating_light: NGHIÊM TRỌNG (CRITICAL)
1. **<Tiêu đề lỗi>** — `<file>:<line>`
   <Mô tả chi tiết bằng tiếng Việt — nguyên nhân, hậu quả, cách sửa>

#### :warning: HIỆU NĂNG (PERFORMANCE)
2. **<Tiêu đề lỗi>** — `<file>:<line>`
   <Mô tả: query nào có vấn đề, ảnh hưởng thế nào, cách tối ưu>

#### :orange_circle: QUAN TRỌNG (HIGH)
3. **<Tiêu đề lỗi>** — `<file>:<line>`
   <Mô tả chi tiết>

#### :yellow_circle: TRUNG BÌNH (MEDIUM)
4. **<Tiêu đề lỗi>** — `<file>:<line>`
   <Mô tả chi tiết>

### Các tiêu chí đã kiểm tra

**Build động dựa trên kết quả thực tế từ 5 agents.**

Quy tắc:
- `[x]` = Agent đã kiểm tra tiêu chí này VÀ không phát hiện lỗi
- `[!]` = Agent đã kiểm tra VÀ phát hiện lỗi (kèm số lỗi)
- `[-]` = Không áp dụng cho MR này (VD: không có SQL → bỏ SQL injection)
- `[ ]` = Chưa kiểm tra được (VD: agent bị lỗi, diff quá lớn)

Danh sách tiêu chí (chỉ hiển thị những tiêu chí áp dụng cho MR):

**Hiệu năng & Bảo mật** (từ Agent 1, 2, 5):
- `Hiệu năng SQL` — N+1, missing index, LIMIT/OFFSET, SELECT *
- `SQL injection` — prepared statements, pquery() với ? params
- `XSS` — .text() thay vì .html(), jQuery DOM construction, vtlib_purify()
- `CSRF` — validateWriteAccess trên write actions
- `Ép kiểu request` — (int)/(string) trên $request->get()
- `Logic nghiệp vụ` — status transitions, handlers, relationships

**Cấu trúc & Clean Code** (từ Agent 3):
- `Tách file` — không inline CSS/JS trong PHP/TPL
- `Vị trí file` — CSS/JS/TPL đúng convention directory
- `Method ≤ 30 dòng` — method quá dài cần tách
- `Class ≤ 200 dòng` — file quá lớn cần modularize
- `Naming conventions` — class, method, JS controller đúng pattern
- `Early return` — không deep nesting, dùng guard clauses
- `Dead code` — commented-out code, unused variables, duplicated logic
- `PHP headers` — @author, @email, @create date
- `Error handling` — try/catch, type declarations

**Ví dụ output:**
```
### Các tiêu chí đã kiểm tra

**Hiệu năng & Bảo mật:**
- [x] Hiệu năng SQL (N+1, missing index, LIMIT/OFFSET)
- [!] SQL injection — 1 lỗi (string concatenation trong date condition)
- [!] XSS — 3 lỗi (string concatenation trong form HTML)
- [x] CSRF (validateWriteAccess)
- [!] Ép kiểu request — 1 lỗi (missing cast trên record_id)
- [x] Logic nghiệp vụ (knowledge base)

**Cấu trúc & Clean Code:**
- [!] Tách file — 2 lỗi (inline script trong TPL, CSS trong vendor file)
- [x] Vị trí file CSS/JS/TPL đúng convention
- [x] Method ≤ 30 dòng
- [x] Class ≤ 200 dòng
- [x] Naming conventions
- [x] Early return pattern
- [!] Dead code — 1 lỗi (commented-out code trong Detail.js)
- [x] PHP headers
- [x] Error handling
```

---
:robot: Reviewed by Dante
```

**Nếu không có lỗi:**

```markdown
## Code Review — MR !<NUMBER>

**Reviewer:** Dante | **Files:** <COUNT> | **Dòng thay đổi:** +<ADD>/-<DEL>

### Thông tin ticket
- **Ticket:** [#<TICKET_ID>](<TICKET_URL>) — <TICKET_TITLE>
- **Trạng thái hiện tại:** <TICKET_STATUS>
- **Assignee:** <TICKET_ASSIGNED_TO>
- **Priority:** <TICKET_PRIORITY>
- **Module liên quan:** <TICKET_MODULE>

### Phạm vi nghiệp vụ đã review

**Mục tiêu MR:** <feature_intent từ Business Manifest>

### Checklist tính năng theo yêu cầu ticket

**Nhóm 1: <category>**
- ✅ **REQ-01** <mô tả nguyên văn> — `file.php:123`
- ✅ **REQ-02** <mô tả> — `file.tpl:45`

**Nhóm 2: <category>**
- ✅ **REQ-03** <mô tả> — `file.js:67`

**Tổng kết yêu cầu:** ✅ <TOTAL>/<TOTAL> done — đầy đủ theo ticket.

### Business rules (<N>):
- [x] **BR-01** <rule statement> — OK
- [x] **BR-02** <rule statement> — OK
- [x] **BR-03** <rule statement> — OK

**Status transitions:** <liệt kê hoặc "Không thay đổi">

**Side effect audit:** <handler chain đã trace — OK không loop/duplicate>

**Knowledge consistency:** ✓ Đồng bộ với `<knowledge_file.md>`

**Coverage:** <N>/<N> rules reviewed.

:white_check_mark: **Không phát hiện lỗi.** Đã kiểm tra: hiệu năng SQL, bảo mật, logic nghiệp vụ, cấu trúc file, và clean code.

### Các tiêu chí đã kiểm tra
_(Build động — tất cả [x] vì không có lỗi)_

**Hiệu năng & Bảo mật:**
- [x] <chỉ liệt kê tiêu chí áp dụng cho MR này>

**Cấu trúc & Clean Code:**
- [x] <chỉ liệt kê tiêu chí áp dụng cho MR này>

---
:robot: Reviewed by Dante
```

Post with:
```bash
glab mr comment <MR_NUMBER> --repo $REPO --message "<COMMENT>"
```


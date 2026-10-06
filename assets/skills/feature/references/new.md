# /feature new <slug>

> Đã load `references/feature-rules.md` (luật xuyên suốt, gồm **R10 code-intelligence FIRST**) ở Dispatch chưa? Nếu chưa → load trước.

Scaffold 1 feature lớn mới. Output: `agents/planning/<slug>/` + 4 file + set ACTIVE.

> Bước 1/4/5 (validate slug, tạo file từ template, set ACTIVE + config) cũng làm được **deterministic** bằng verb `8sync feature new <slug>` (nhanh, không cần model). `/feature new` trong session dùng khi cần thêm phán đoán (spec ở đâu, cắt phase). Cả hai đều thay placeholder `SLUG`/`FEATURE_NAME`/`DATE`.

## Steps

1. **Validate slug** — kebab-case, chưa tồn tại `agents/planning/<slug>/`. Tồn tại → hỏi user (resume? overwrite?) — KHÔNG clobber tự động.

2. **Lấy đặc tả + chốt nguồn yêu cầu (R12)** — hỏi user spec ở đâu (dùng `ask` nếu tương tác):
   - **File đặc tả ngoài** (BA giữ) → để file trong `agents/planning/<slug>/`, giữ nguyên tên. Ghi ở `REQUIREMENTS.md` dòng **Nguồn yêu cầu**: đường dẫn + bản/ngày đặc tả. Đặc tả là gốc — REQUIREMENTS/ROADMAP chỉ tóm tắt + trỏ mã, KHÔNG chép nguyên văn.
   - **Mô tả ngắn / không có file** → ghi vào PROJECT.md + REQUIREMENTS.md; nguồn yêu cầu = `REQUIREMENTS.md`. Bỏ qua bước 4b.

3. **Knowledge lookup (brownfield — BẮT BUỘC, R7)**:
   - Đọc `AGENTS.md` + `agents/PROJECT.md` → tổng thể + stack.
   - Đọc `agents/KNOWLEDGE.md` + `agents/DECISIONS.md` → nghiệp vụ/quyết định module feature sẽ đụng/dùng lại.
   - **R10 áp dụng**: khi cần khảo cấu trúc module thật (không chỉ đọc memory), dùng `xd://mcp__codegraph_explore` / CLI `codegraph query/impact "<module>"` hoặc `xd://mcp__codebase_memory_mcp_get_architecture` TRƯỚC, KHÔNG grep/read tràn lan toàn module.
   - Mục đích: PROJECT.md ghi đúng "cắm vào module nào", "KHÔNG đụng gì".

4. **Tạo 4 file** từ `templates/` — thay placeholder:
   - `SLUG` → slug, `FEATURE_NAME` → tên đẹp (human title), `DATE` → hôm nay (YYYY-MM-DD; hỏi nếu cần, không tự bịa).
   - `PROJECT.md`: điền What/Core value/Cắm vào codebase/Ràng buộc. Key Decisions ghi cột "Ai quyết" (R15).
   - `REQUIREMENTS.md`: dòng Nguồn yêu cầu + chia UC v1/v2/out-of-scope.
   - `ROADMAP.md`: **cắt phase theo dependency** (xem thuật toán dưới). Phase có UI → ghi màn hình thiết kế (node Figma / mockup) của phase.
   - `STATE.md`: phase=M0, status=planning, next_action=plan-phase, progress 0%.

4b. **Có đặc tả ngoài → COVERAGE (BẮT BUỘC)**:
   - Tạo `COVERAGE.md` từ `templates/COVERAGE.md`: dòng `Spec:` trỏ file đặc tả; liệt kê **mọi** mã đặc tả (UC/BR/BO/D/W/P/SCR, US·AC) — mỗi mã 1 dòng, cột Phase gán theo ROADMAP, cột "Ở đâu trong kế hoạch", trạng thái. Mã không có chỗ → GAP → bổ sung REQUIREMENTS/ROADMAP rồi đổi thành ADDED.
   - Chạy `python3 ~/.omp/skills/feature/scripts/coverage.py agents/planning/<slug>` → phải PASS (không mã thiếu dòng, không GAP).
   - Điền mục **"Đọc bắt buộc khi `/feature plan` một phase"** trong REQUIREMENTS: mỗi phase 1 dòng; cột "Mã phải đọc" = output `coverage.py … --phase Mx` (không chọn tay).
   - Đặc tả dùng dạng mã khác mặc định (`BO-/UC-/BR-/P-XX/D-XX/W-XX/SCR-`, US·AC = dòng `**US-01**` + bảng `| AC-01 |`) → ghi regex vào header COVERAGE: ``> Codes: `<regex>` `` (hoặc `--code-re`). Không sửa script.

5. **Set active + (tuỳ chọn) git**:
   - `agents/planning/ACTIVE.md` ← dòng đầu (không comment) = slug (giữ comment header).
   - `agents/planning/config.json` ← `active_feature: "<slug>"`.
   - **Ticket (TUỲ CHỌN)**: nếu user có ticket number → lưu raw numeric ở `STATE.frontmatter.ticket`. KHÔNG ép ticket — để trống được.
   - **Feature branch (TUỲ CHỌN)**: nếu user muốn tách nhánh cho cả feature → tạo 1 nhánh lớn, ghi `STATE.frontmatter.branch`. KHÔNG tự tạo nhánh nếu user không yêu cầu (8sync mặc định commit local thẳng nhánh hiện tại). KHÔNG `git push`.

6. **USER DUYỆT (gate 1)** — trình 4 file (+ COVERAGE: số mã, số PROBE/HUB/BLOCKED), dùng `ask` xác nhận kiến trúc + cách cắt phase. KHÔNG sang plan tới khi duyệt. Quyết định user chốt ở gate → Key Decisions với "Ai quyết" = `user`.

## Thuật toán cắt phase (ROADMAP)

1. Liệt kê **thực thể dữ liệu** từ spec (danh từ nghiệp vụ được lưu/thao tác).
2. Vẽ dependency: "X tồn tại được mà không cần Y?" → cạnh phụ thuộc.
3. Topological sort → tầng. Tầng 0 (không cần gì) = Foundation = M0.
4. Gộp/tách theo khối lượng: phase code được 1-2 ngày. Quá nhỏ → gộp, quá lớn → tách.
5. Mỗi phase phải **demo được 1 thứ** (test: "sau phase này user làm được gì?"). Không → cắt sai (đang cắt theo layer kỹ thuật).
6. Ghi Integration Contracts: M trước export gì → M sau dùng gì.
7. Có đặc tả ngoài: mỗi mã đặc tả phải rơi vào ≥1 phase (cột Phase của COVERAGE). Mã không phase nào nhận = cắt thiếu.

Quy tắc vàng: **Foundation trước · nghiệp vụ giữa · tích hợp cuối**. Cái nhiều thứ phụ thuộc vào → làm trước.

## Sau khi xong

Báo user: đã tạo, đang ở M0 planning. Next: `/feature plan`.

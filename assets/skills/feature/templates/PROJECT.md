# PROJECT — FEATURE_NAME

> Vision + ràng buộc + decisions. Gần như bất biến. "Tại sao + luật chơi" của feature.

## What This Is

[1-2 đoạn: feature giải gì, cho ai. Done toàn bộ khi nào.]

## Core Value

[1 câu: giá trị cốt lõi. Session sau đọc 5 giây hiểu.]

## Cắm vào codebase (brownfield)

> Dự án đã có sẵn — feature này KHÔNG đứng riêng. Neo vào `AGENTS.md` + `agents/`.

- **Dùng lại module:** [tên — xem `agents/KNOWLEDGE.md` / `xd://mcp__codebase_memory_mcp_get_architecture`]
- **Tham khảo:** [`agents/PROJECT.md` / `agents/DECISIONS.md` mục liên quan]
- **Module/thành phần mới:** [tên nếu có]
- **KHÔNG đụng:** [module cấm sửa — chống lan]

## Ràng buộc / Architecture Decisions

- Data: [bảng/store]
- API: [loại/giao thức]
- Convention: theo `AGENTS.md` + `agents/DECISIONS.md` + `agents/PREFERENCES.md`

## Key Decisions (table — append khi chốt)

> Cột "Ai quyết": `user` (user chốt rõ trong hội thoại / `ask`) · `đề xuất` (AI đề xuất, user chưa chốt) · `auto` (auto-mode tự quyết). Chỉ dòng `user` mới là ràng buộc; dòng `đề xuất`/`auto` phải trình lại user khi chạm tới.

| Ngày | Phase | Quyết định | Lý do | Ai quyết |
|------|-------|-----------|-------|----------|
| DATE | M0 | [decision] | [why] | user |

## Requirements

### Validated (đã ship, xác nhận giá trị)
<!-- tick khi phase ship xong, map UC -->
- (chưa có)

### Active (đang làm — chi tiết ở REQUIREMENTS.md)
- [ ] [UC chính]

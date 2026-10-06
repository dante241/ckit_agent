# REQUIREMENTS — FEATURE_NAME

> Ý ĐỊNH (làm gì + ranh giới), KHÔNG phải bản ghi đã-làm.
> Chỉ sửa khi ý định đổi (thêm/bỏ UC). "Đã code tới đâu" → ROADMAP + STATE, KHÔNG ở đây.
> UC-ID là khóa traceability: ROADMAP phase → M<x>-CONTEXT Requirement scope → AC → PLAN task → VERIFICATION. KHÔNG đổi UC-ID sau khi đã plan/code nếu không cập nhật toàn bộ trace.

**Nguồn yêu cầu:** [chọn 1] chính file này (không có đặc tả ngoài) · đặc tả ngoài `spec.md` (BA giữ, bản/ngày: DATE) + `COVERAGE.md` (mã đặc tả → phase).
> Có đặc tả ngoài: bảng UC dưới chỉ là **tóm tắt**; nguyên văn luồng, BR, tiêu chí nghiệm thu nằm trong đặc tả. Mọi AC của phase ghi cột **Nguồn** trỏ về mã đặc tả.

## Đọc bắt buộc khi `/feature plan` một phase

> Bỏ mục này nếu không có đặc tả ngoài. Cột "Mã phải đọc" **sinh bằng** `python3 ~/.omp/skills/feature/scripts/coverage.py <feature-dir> --phase Mx`, không chọn tay; COVERAGE đổi thì sinh lại.

**Chung cho mọi phase:** `PROJECT.md` (Key Decisions mới nhất thắng), `ROADMAP.md` (dòng phase + Integration Contracts + màn hình thiết kế của phase).

| Phase | Đặc tả (tên mục) | Mã phải đọc (từ COVERAGE) | File khác |
|---|---|---|---|
| M0 | [§ tên mục] | [BR-xx, P-xx, …] | [—] |

## v1 (làm ngay)

| UC | Mô tả | Phase |
|----|-------|-------|
| UC1 | [mô tả] | M0 |
| UC2 | [mô tả] | M1 |

## v2 (sau — không làm milestone này)

- UC-x: [mô tả]

## Out-of-scope (KHÔNG làm — ranh giới cứng)

> Agent KHÔNG tự ý làm các mục dưới. Vượt ranh giới = dừng, hỏi user.

- [tính năng loại trừ]

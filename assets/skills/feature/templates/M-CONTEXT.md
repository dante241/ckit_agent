# Mx-CONTEXT — <phase name>

> Hợp đồng "tại sao + nghiệm thu" của phase Mx. `/feature go` + `/feature ship` đọc file này làm chuẩn.
> BẮT BUỘC có đủ: 📌 Requirement scope + 🎯 Goal + ✅ Acceptance Criteria TRƯỚC khi plan/code.

## 📌 Requirement scope (UC từ REQUIREMENTS.md)

| UC | Mô tả (literal từ REQUIREMENTS.md) | Trong phase này làm gì | Không làm ở phase này |
|----|-----------------------------------|------------------------|-----------------------|
| UC-01 | [copy mô tả] | [phạm vi cụ thể phase này] | [ranh giới future/out-of-scope] |

## 📖 Nguồn yêu cầu đã đọc

> Có đặc tả ngoài: mục đặc tả + danh sách mã của phase (`coverage.py --phase Mx`) đã đọc nguyên văn. Không có: ghi "REQUIREMENTS.md".

- Đặc tả: [§ tên mục đã đọc]
- Mã của phase: [BR-xx, D-xx, P-xx, SCR-xx, US-0x·AC-yy…]
- Integration Contracts nhận từ phase trước: [contract — còn khớp Key Decisions mới nhất? có / đã sửa ROADMAP]
- Thiết kế (phase có UI): [node/link Figma hoặc mockup đã đọc]

## 🎯 Goal

[1 câu: output đo được của phase + ranh giới (CHƯA làm gì → tránh review đòi hỏi quá phạm vi).]

## ✅ Acceptance Criteria (UAT)

> Mỗi UC ⇒ ≥1 AC. AC đo được (số/trạng thái/output cụ thể — KHÔNG "chạy ổn"). Cột "Cách verify" cũng là `verify` của engine task ở `/feature go`.
> Cột **Nguồn**: mã đặc tả AC này chứng minh (`US-01·AC-03`, `BR-04`, `UC-02 A4`, `E1`); không có đặc tả ngoài → `REQUIREMENTS`. Mọi mã của phase phải có ≥1 AC, hoặc nằm ở mục "Chuyển phase".

| AC | UC | Nguồn | GIVEN / WHEN / THEN (đo được) | Cách verify | Tier | Task nguồn |
|----|----|-------|-------------------------------|-------------|------|------------|
| AC-01 | UC-01 | US-01·AC-02, BR-01 | GIVEN [tiền đề] WHEN [hành động] THEN [kết quả đo được] | [lệnh/SQL/script/thao tác] | must-test / verify-sql / verify-only | T1 |

### Chuyển phase (mã của phase này nhưng không có AC ở đây)

| Mã | Chuyển sang | Lý do |
|----|-------------|-------|
| — | — | — |

## Decisions (D1, D2… — quyết định riêng phase, append khi chốt)

- D1: [quyết định — vì sao]. (nguồn: discuss / `ask` / (auto-decided via <role>))

## Plan-review notes (điền sau Step 3.5 nếu có chạy)

- (chưa có)

---

**Phase DONE khi mọi AC PASS (ghi ở Mx-VERIFICATION.md; NEEDS-CONFIRM → `ship.md` Step 2.6). AC FAIL → không ship.**

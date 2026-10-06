# Mx-VERIFICATION — <phase name>

> Nghiệm thu phase Mx bám AC. Nguồn chân lý: REQUIREMENTS.md + Mx-CONTEXT.md.
> Phase done ⇔ MỌI AC = PASS. Còn ≥1 FAIL hoặc UC chưa có verdict → phase CHƯA done.

## UC/AC verdicts

| UC | AC | Nguồn | Verdict | Bằng chứng (output/SQL/file:line) |
|----|----|-------|---------|-----------------------------------|
| UC-01 | AC-01 | US-01·AC-02, BR-01 | PASS | [test/script → output thật ✓] |
| UC-02 | AC-05 | BR-04 | FAIL | [lỗi cụ thể — file:line / migration lỗi dòng X] |

> Verdict ∈ PASS / FAIL / NEEDS-CONFIRM (bị chặn bởi môi trường/dữ liệu/quyền ngoài tầm — KHÔNG dùng cho lỗi code).
> Có đặc tả ngoài: mọi mã của phase (`coverage.py --phase Mx`) phải xuất hiện ở cột Nguồn của ≥1 dòng, hoặc ở mục "Chuyển phase" của CONTEXT.

## NEEDS-CONFIRM (đóng phase có điều kiện)

| AC | Chặn bởi gì | Ai xác nhận | Khi nào / ở đâu | Theo dõi tại |
|----|-------------|-------------|-----------------|--------------|
| — | — | — | — | STATE.md Blockers |

## Review findings (per dimension)

- **security:** [finding đã fix / còn lại — file:line, hoặc "clean"]
- **correctness:** [...]
- **convention:** [...]

## Converge rounds

| Round | AC/finding FAIL vào | Task sinh ra | Kết quả |
|-------|---------------------|--------------|---------|
| 1 | AC-05 (thiếu code), security: XSS `file:line` | T9, T10 | AC-05 PASS; finding fixed |

## Kết luận

<N/M AC PASS, K NEEDS-CONFIRM>. Phase done? **YES / NO / YES có điều kiện** (chỉ khi user chấp nhận NEEDS-CONFIRM — `ship.md` Step 2.6)

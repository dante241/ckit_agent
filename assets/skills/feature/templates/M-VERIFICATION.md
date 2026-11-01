# Mx-VERIFICATION — <phase name>

> Nghiệm thu phase Mx bám AC. Nguồn chân lý: REQUIREMENTS.md + Mx-CONTEXT.md.
> Phase done ⇔ MỌI AC = PASS (NEEDS-CONFIRM → `ship.md` Step 2.6). Còn ≥1 FAIL hoặc UC chưa có verdict → phase CHƯA done.

## UC/AC verdicts

| UC | AC | Nguồn | Verdict | Bằng chứng (output/SQL/file:line) |
|----|----|-------|---------|-----------------------------------|
| UC-01 | AC-01 | US-01·AC-02, BR-01 | PASS | [test/script → output thật ✓] |
| UC-02 | AC-05 | BR-04 | FAIL | [lỗi cụ thể — file:line / migration lỗi dòng X] |

> Verdict = ĐÚNG một từ PASS / FAIL / NEEDS-CONFIRM. Code thiếu/chưa xong ("một phần", "chưa làm") = FAIL. NEEDS-CONFIRM chỉ khi code đã PASS đủ và phần còn lại bị chặn ngoài tầm (môi trường/dữ liệu/quyền) — ghi phần đã PASS ở cột Bằng chứng. Một AC chỉ có một verdict, kể cả khi xuất hiện ở nhiều bảng. Kiểm bằng `phase_gate.py --phase Mx --check verification`.
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

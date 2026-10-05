# Mx-NN-PLAN — <phase name>

> Batch task của phase Mx. Mỗi task truy được về ≥1 AC + ≥1 UC (từ Mx-CONTEXT.md).
> **Thực thi:** `/feature go` chạy song song mọi task đã đủ `[depends:]` (không chờ theo đợt); task dev xong → chạy `[verify:]` ngay; lỗi → dev lại. `[verify:]` = lint/test THẬT, scoped vào task.

## Tasks

- [ ] T1: <việc>   [file: path]   [depends: —]    [verify: <lệnh>]   [skill: <name>]   [tier: must-test]     [UC: UC-01]   [AC: AC-01,AC-03]
- [ ] T2: <việc>   [file: path]   [depends: —]    [verify: <lệnh>]   [skill: <name>]   [tier: verify-sql]    [UC: UC-02]   [AC: AC-05]
- [ ] T3: <việc>   [file: path]   [depends: T1]   [verify: <lệnh>]   [skill: <name>]   [tier: verify-only]   [UC: UC-01]   [AC: AC-08]

## Checkpoints / Gates

- **Review dimensions:** [từ config.workflow.review_dimensions, vd security, correctness, convention]
- **Verify (engine gate):** mỗi task chạy `[verify:]` qua `engine_verify`; `engine_advance` từ chối task chưa pass. Hết task → chạy test đầy đủ 1 lần.
- **Acceptance:** phase done ⇔ mọi AC trong Mx-CONTEXT PASS (verify ở `/feature ship` → Mx-VERIFICATION.md). KHÔNG dùng DoD mơ hồ — dùng AC.

## Kiểm tra phủ (trước khi trình gate 2)

- [ ] Mọi UC trong Requirement scope có ≥1 AC.
- [ ] Mọi AC-NN xuất hiện ở cột `[AC:]` của ≥1 task.
- [ ] Mọi task có `[UC:]` + `[AC:]` + `[skill:]` (task code `[skill: —]` = cờ đỏ) + `[verify:]` scoped.
- [ ] `[depends:]` chỉ có cạnh thật (cần symbol/bảng/file của task kia); 2 task cùng file/tài nguyên dùng chung có phụ thuộc nhau; không vòng.

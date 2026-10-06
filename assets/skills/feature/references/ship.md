# /feature ship

> Đã load `references/feature-rules.md` (R1 config-resolve, R5 AC nghiệm thu, **R10 code-intelligence FIRST — áp dụng cả reviewer/tester subagent**) ở Dispatch chưa? Nếu chưa → load trước.

Verify (review + test) → close phase → archive khi hết feature → cập nhật bản đồ. = GSD Verify + Ship.
Gate: review/test FAIL → fix → re-run. KHÔNG ship khi fail.

## Step 0 — Load hợp đồng nghiệm thu (BẮT BUỘC trước review/test)

Đọc `M<x>-CONTEXT.md` → **📌 Requirement scope (UC từ REQUIREMENTS.md) + 🎯 Goal + bảng ✅ Acceptance Criteria (AC-NN)**. Chuẩn nghiệm thu phase.
- Requirement scope + AC là **nguồn chân lý** cho cả Step 1 (review) lẫn Step 2 (test): mọi reviewer/tester nhận **UC literal + bảng AC literal** trong prompt + verify ĐÚNG từng UC/AC liên quan. Subagent KHÔNG tự đặt tiêu chí ngoài AC; cũng KHÔNG đòi hỏi vượt Goal (ranh giới phase).
- CONTEXT thiếu Goal/AC → DỪNG, quay lại `/feature plan` bổ sung (không nghiệm thu mò).
- Đầu ra cuối: `M<x>-VERIFICATION.md` (dùng `templates/M-VERIFICATION.md`) có **bảng AC → verdict** (mỗi AC: PASS/FAIL + bằng chứng cụ thể). Phase done ⇔ MỌI AC PASS.

## Step 1 — Review (FAN-OUT multi-lens song song)

**Gate:** `config.workflow.code_review === false` → BỎ Step 1, ghi STATE Log "review skipped per config" + cảnh báo user "review tắt theo config — chất lượng tự chịu". Ngược lại (`true`/thiếu) → chạy review:

Spawn ĐỒNG THỜI `task` subagent `agent: code-reviewer` (dự án có `.omp/agents/code-reviewer.md`; không có → `agent: reviewer`). Số agent = số phần tử `config.workflow.review_dimensions`; nhúng **tên dimension thật** vào prompt mỗi agent (vd "dimension: security"), KHÔNG ghi chữ `config.workflow.review_dimensions` vào prompt. Dimension mặc định (`["security","correctness","convention"]`):
- **security**: injection (query tham số hoá?), XSS/escape output, CSRF/permission check, type cast, secret leak.
- **correctness**: symbol/method tồn tại (`xd://mcp__serena_find_symbol`), logic, runtime, config key/DB column đúng.
- **convention**: `AGENTS.md` + `agents/DECISIONS.md`/`PREFERENCES.md`, naming, tách file, error-handling.

Scope = file phase này đụng (từ PLAN). Barrier → gộp findings.
Có lỗi → fix (main thread hoặc spawn) → re-review tới sạch. Phase nhỏ → 1 reviewer tổng hợp cũng được.

**Nhúng UC + AC vào prompt reviewer:** mỗi prompt kèm Requirement scope + bảng AC literal (từ Step 0) + yêu cầu: "Ngoài lens <dimension>, soát code có thỏa đúng UC/AC thuộc lens này không (vd security lens ↔ AC nào về permission/inject); báo UC/AC nào code KHÔNG thỏa kèm `file:line`." Reviewer trả findings gắn UC-ID/AC-NN khi liên quan. Nhúng R10 literal (dùng code-intel qua `xd://` device định vị, tóm tắt output dài).

## Step 2 — Test (theo tier, fan-out per-component nếu nhiều)

**Gate:** `config.workflow.verifier === false` → bỏ tầng verify/test sâu, chỉ chạy lint/build của dự án + cảnh báo "verifier tắt theo config". Ngược lại → chạy đủ theo tier.

**Tester là nguồn viết test authoritative** — spawn `task` subagent `agent: tester` (dự án có `.omp/agents/tester.md`; không có → `agent: task` + đọc skill `testing` nếu có) (NEVER tự viết test). Theo tier:

| Tier | Áp dụng | Làm |
|------|---------|-----|
| must-test | logic/handler/model/helper cốt lõi | tester agent → unit + edge/security theo AC. KHÔNG mock cái đang test. |
| verify-sql | report/migration/SELECT | chạy SQL/script trên môi trường thật + build/lint |
| verify-only | config/DDL/asset tĩnh | lint/build của dự án + review đủ |

Nhiều component độc lập → spawn tester song song (1 component/agent). Barrier.

**Test BÁM UC/AC, không test mò:** mỗi tester nhận Requirement scope + bảng AC literal + chỉ thị "viết test chứng minh ĐÚNG các UC/AC được giao (dùng cột 'Cách verify' của AC làm kịch bản; Given/When/Then làm assertion). Mỗi AC must-test/verify-sql → ≥1 test thực thi, trả PASS/FAIL kèm output thật." Test bổ sung ngoài AC (edge/security) vẫn khuyến khích, nhưng KHÔNG được thiếu UC/AC nào.

> Test lint/build đã chạy như GATE của từng task trong `/feature go` (engine `verify`); Step 2 là tầng nghiệm thu AC end-to-end, bổ sung chứ không thay verify-gate của engine.

### Ghi `M<x>-VERIFICATION.md` — bắt buộc dạng AC-matrix
```markdown
# M<x>-VERIFICATION
## UC/AC verdicts (nguồn: REQUIREMENTS.md + M<x>-CONTEXT)
| UC | AC | Verdict | Bằng chứng (output/SQL/file:line) |
|----|----|---------|-----------------------------------|
| UC-15 | AC-01 | PASS | test/test-...  → "handler not called" ✓ |
| UC-16 | AC-05 | FAIL | migration lỗi dòng X |
...
## Review findings (per dimension) — đã fix / còn lại
## Kết luận: <N/M AC PASS>. Phase done? YES/NO
```
**Gate cứng:** còn ≥1 UC/AC FAIL hoặc UC trong Requirement scope chưa có AC verdict → phase CHƯA done → **Step 2.5 Converge**. KHÔNG ship khi matrix còn FAIL.

## Step 2.5 — Converge (biến phần FAIL thành task, chạy lại tới khi hội tụ)

Bỏ qua khi matrix đã toàn PASS. Ngược lại, lặp tối đa `config.workflow.max_converge_rounds` vòng (thiếu → 3):

1. **Phân loại** mỗi AC FAIL, UC chưa có verdict và review finding chưa fix (đọc code thật bằng code-intel, không đoán từ tên file):
   | Loại | Dấu hiệu | Xử lý |
   |---|---|---|
   | Thiếu code | AC đòi hành vi chưa có chỗ nào cài | task mới |
   | Code sai | có code nhưng test/review chứng minh sai | task mới sửa đúng file đó |
   | AC sai/mơ hồ | AC mâu thuẫn Decision, không đo được, hoặc vượt Goal | DỪNG → `ask` user sửa CONTEXT (R6). KHÔNG tự đổi AC |
   | Bị chặn | thiếu data/môi trường/quyền | NEEDS-CONFIRM, không sinh task |
2. **Sinh task** — append vào `M<x>-NN-PLAN.md` dưới `## Converge round <n>`, đánh số nối tiếp (`T<max+1>`…), đủ cột như task thường: `[file:]`, `[depends:]` (chỉ giữa các task converge với nhau — task cũ đã done), `[verify:]` = lệnh của cột "Cách verify" của AC đó (scoped), `[skill:]`, `[UC:]`, `[AC:]`. Mỗi task gánh ≥1 AC FAIL hoặc 1 finding; không thêm việc ngoài matrix.
3. **Gate người duyệt** — không `--auto`: trình bảng task converge (AC ↔ task ↔ file) + `ask` (Chạy / Sửa / Dừng). `--auto`: chạy luôn.
4. **Chạy** — `engine_plan` chỉ với các task converge (`goal` = "M<x> converge round <n>", slice = `Converge <n>`), rồi đúng vòng Bước 2 của `references/execute.md` (dev ∥ test, `engine_verify` → `engine_advance` commit từng task). STATE `next_action: converge` để session sau resume đúng chỗ.
5. **Nghiệm thu lại** — chạy lại Step 2 cho các AC vừa FAIL + chạy lại lệnh "Cách verify" (rẻ) của mọi AC đã PASS để bắt hồi quy; review lại chỉ các file round này đụng. Cập nhật matrix + ghi round vào `## Converge rounds` của VERIFICATION.
6. **Dừng** khi: matrix toàn PASS → Step 3; hoặc **cùng một AC FAIL với cùng bằng chứng 2 vòng liền** → coi là kẹt: ghi `failure:` vào `agents/KNOWLEDGE.md`, báo user AC đó + 2 lần thử; hoặc hết số vòng → báo user danh sách AC còn FAIL. Không đóng phase trong 2 trường hợp sau.

## Step 3 — Ship (đóng phase)

1. **Commit**: code đã commit atomic per-task trong `/feature go` (qua `engine_advance`) rồi — KHÔNG commit gộp lại. Ship chỉ:
   - Commit nốt phần phụ của phase chưa thuộc task nào (`M<x>-VERIFICATION.md`/STATE/ROADMAP đổi): `docs: M<x> close phase <tên>` (Conventional Commits, tiếng Anh, milestone ở đầu — xem `execute.md` §Commit).
   - Verify **AC matrix trong M<x>-VERIFICATION.md MỌI AC = PASS** (Step 0+1+2) TRƯỚC khi tính phase done. Còn FAIL → KHÔNG đóng phase.
   - **KHÔNG `git push` / mở PR** — chỉ push khi user yêu cầu rõ.
2. **ROADMAP**: phase `[~]` → `[x]` + ghi Phase log (range commit `<first>..<last>` của phase, contract đã export).
3. **STATE cập nhật**:
   - `progress.completed_phases` +1, `percent` lại.
   - `active_phase` → phase tiếp (unblocked theo dependency) hoặc `null` nếu hết.
   - `next_action` → `plan-phase` cho phase sau, hoặc `done`.
   - `## Session Continuity`: ghi vừa ship phase nào.

## Step 4 — Nếu là phase CUỐI: chống drift + archive

BẮT BUỘC khi feature hoàn tất:
- [ ] `agents/KNOWLEDGE.md` — append nghiệp vụ/gotcha mới học (`validated:`/`failure:`). Lớn → spawn `task` (`agent: task`).
- [ ] `agents/DECISIONS.md` — append quyết định kiến trúc feature chốt.
- [ ] `PROJECT.md` Validated — tick UC đã ship.
- [ ] `REQUIREMENTS.md` — rà UC thực tế bỏ/đổi, cập nhật intent.
- [ ] `CHANGELOG.md` (Unreleased) — thêm dòng feature (convention 8sync).
- [ ] Archive: `mv agents/planning/<slug> agents/planning/_archive/` + clear `agents/planning/ACTIVE.md` (dòng slug về trống) + `config.active_feature = ""`.

## Sau ship

Báo user: phase X done. Còn phase nào → next `/feature plan`. Hết → feature hoàn thành, đã archive.

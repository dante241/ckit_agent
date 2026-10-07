# /feature ship

> Đã load `references/feature-rules.md` (R1 config-resolve, R5 AC nghiệm thu, **R10 code-intelligence FIRST — áp dụng cả reviewer/tester subagent**) ở Dispatch chưa? Nếu chưa → load trước.

Verify (review + test) → close phase → cập nhật bản đồ kiến thức (mỗi phase) → archive khi hết feature. = GSD Verify + Ship.
Gate: review/test FAIL → fix → re-run. KHÔNG ship khi fail.

## Step 0 — Load hợp đồng nghiệm thu (BẮT BUỘC trước review/test)

Đọc `M<x>-CONTEXT.md` → **📌 Requirement scope (UC từ REQUIREMENTS.md) + 🎯 Goal + bảng ✅ Acceptance Criteria (AC-NN)**. Chuẩn nghiệm thu phase.
- Requirement scope + AC là **nguồn chân lý** cho cả Step 1 (review) lẫn Step 2 (test): mọi reviewer/tester nhận **UC literal + bảng AC literal (kèm cột Nguồn)** trong prompt + verify ĐÚNG từng UC/AC liên quan. Subagent KHÔNG tự đặt tiêu chí ngoài AC; cũng KHÔNG đòi hỏi vượt Goal (ranh giới phase).
- **Nguồn yêu cầu (R12)**: có đặc tả ngoài → chạy `python3 ~/.omp/skills/feature/scripts/coverage.py agents/planning/<slug> --phase M<x>` lấy danh sách mã của phase + trích **đoạn đặc tả nguyên văn** mà cột Nguồn của từng AC trỏ tới. AC chấm theo nguyên văn đó: code thỏa câu chữ AC nhưng trái nguyên văn đặc tả = FAIL (loại "AC sai/mơ hồ" ở Step 2.5). Mã của phase không có AC và không ở mục Chuyển phase của CONTEXT → DỪNG, quay lại `/feature plan` bổ sung.
- CONTEXT thiếu Goal/AC → DỪNG, quay lại `/feature plan` bổ sung (không nghiệm thu mò).
- Đầu ra cuối: `M<x>-VERIFICATION.md` (dùng `templates/M-VERIFICATION.md`) có **bảng AC → verdict** (mỗi AC: Nguồn + PASS/FAIL/NEEDS-CONFIRM + bằng chứng cụ thể). Phase done ⇔ MỌI AC PASS (NEEDS-CONFIRM theo luật Step 2.6).

## Step 1 — Review (FAN-OUT multi-lens song song)

**Gate:** `config.workflow.code_review === false` → BỎ Step 1, ghi STATE Log "review skipped per config" + cảnh báo user "review tắt theo config — chất lượng tự chịu". Ngược lại (`true`/thiếu) → chạy review:

Spawn ĐỒNG THỜI `task` subagent `agent: code-reviewer` (dự án có `.omp/agents/code-reviewer.md`; không có → `agent: reviewer`). Số agent = số phần tử `config.workflow.review_dimensions`; nhúng **tên dimension thật** vào prompt mỗi agent (vd "dimension: security"), KHÔNG ghi chữ `config.workflow.review_dimensions` vào prompt. Dimension mặc định (`["security","correctness","convention"]`):
- **security**: injection (query tham số hoá?), XSS/escape output, CSRF/permission check, type cast, secret leak.
- **correctness**: symbol/method tồn tại (`xd://mcp__serena_find_symbol`), logic, runtime, config key/DB column đúng.
- **convention**: `AGENTS.md` + `agents/DECISIONS.md`/`PREFERENCES.md`, naming, tách file, error-handling.

Scope = file phase này đụng (từ PLAN). Barrier → gộp findings.
Có lỗi → fix (main thread hoặc spawn) → re-review tới sạch. Phase nhỏ → 1 reviewer tổng hợp cũng được.

**Nhúng UC + AC + nguồn vào prompt reviewer:** mỗi prompt kèm Requirement scope + bảng AC literal (từ Step 0) + đoạn đặc tả nguyên văn mà các AC trỏ tới + đoạn bản đồ (R16) của vùng phase đụng + yêu cầu: "Ngoài lens <dimension>, soát code có thỏa đúng UC/AC và nguyên văn nguồn thuộc lens này không (vd security lens ↔ AC/BR nào về permission/inject); báo UC/AC/mã nguồn nào code KHÔNG thỏa kèm `file:line`." Reviewer trả findings gắn UC-ID/AC-NN/mã nguồn khi liên quan. Nhúng R10 literal (dùng code-intel qua `xd://` device định vị, tóm tắt output dài).

## Step 2 — Test (theo tier, fan-out per-component nếu nhiều)

**Gate:** `config.workflow.verifier === false` → bỏ tầng verify/test sâu, chỉ chạy lint/build của dự án + cảnh báo "verifier tắt theo config". Ngược lại → chạy đủ theo tier.

**Tester là nguồn viết test authoritative** — spawn `task` subagent `agent: tester` (dự án có `.omp/agents/tester.md`; không có → `agent: task` + đọc skill `testing` nếu có) (NEVER tự viết test). Theo tier:

| Tier | Áp dụng | Làm |
|------|---------|-----|
| must-test | logic/handler/model/helper cốt lõi | tester agent → unit + edge/security theo AC. KHÔNG mock cái đang test. |
| verify-sql | report/migration/SELECT | chạy SQL/script trên môi trường thật + build/lint |
| verify-only | config/DDL/asset tĩnh | lint/build của dự án + review đủ |

Nhiều component độc lập → spawn tester song song (1 component/agent). Barrier.

**Test BÁM UC/AC, không test mò:** mỗi tester nhận Requirement scope + bảng AC literal + đoạn đặc tả nguyên văn của cột Nguồn + chỉ thị "viết test chứng minh ĐÚNG các UC/AC được giao (dùng cột 'Cách verify' của AC làm kịch bản; Given/When/Then làm assertion; nguyên văn nguồn là chuẩn khi AC viết thiếu điều kiện). Mỗi AC must-test/verify-sql → ≥1 test thực thi, trả PASS/FAIL kèm output thật." Test bổ sung ngoài AC (edge/security) vẫn khuyến khích, nhưng KHÔNG được thiếu UC/AC nào.

> Test lint/build đã chạy như GATE của từng task trong `/feature go` (engine `verify`); Step 2 là tầng nghiệm thu AC end-to-end, bổ sung chứ không thay verify-gate của engine.

### Ghi `M<x>-VERIFICATION.md` — bắt buộc dạng AC-matrix
```markdown
# M<x>-VERIFICATION
## UC/AC verdicts (nguồn: REQUIREMENTS.md / đặc tả + M<x>-CONTEXT)
| UC | AC | Nguồn | Verdict | Bằng chứng (output/SQL/file:line) |
|----|----|-------|---------|-----------------------------------|
| UC-15 | AC-01 | US-01·AC-02 | PASS | test/test-...  → "handler not called" ✓ |
| UC-16 | AC-05 | BR-04 | FAIL | migration lỗi dòng X |
...
## Review findings (per dimension) — đã fix / còn lại
## NEEDS-CONFIRM (nếu có) — AC · chặn bởi gì · ai xác nhận · khi nào · theo dõi tại
## Kết luận: <N/M AC PASS>. Phase done? YES/NO/YES-có-điều-kiện
```
**Gate cứng:** còn ≥1 UC/AC FAIL, UC trong Requirement scope chưa có AC verdict, hoặc (có đặc tả ngoài) mã của phase không xuất hiện ở cột Nguồn và không ở mục Chuyển phase → phase CHƯA done → **Step 2.5 Converge**. KHÔNG ship khi matrix còn FAIL.

## Step 2.5 — Converge (biến phần FAIL thành task, chạy lại tới khi hội tụ)

Bỏ qua khi matrix đã toàn PASS. Ngược lại, lặp tối đa `config.workflow.max_converge_rounds` vòng (thiếu → 3):

1. **Phân loại** mỗi AC FAIL, UC chưa có verdict và review finding chưa fix (đọc code thật bằng code-intel, không đoán từ tên file):
   | Loại | Dấu hiệu | Xử lý |
   |---|---|---|
   | Thiếu code | AC đòi hành vi chưa có chỗ nào cài | task mới |
   | Code sai | có code nhưng test/review chứng minh sai | task mới sửa đúng file đó |
   | AC sai/mơ hồ | AC mâu thuẫn Decision, không đo được, hoặc vượt Goal | DỪNG → `ask` user sửa CONTEXT (R6). KHÔNG tự đổi AC |
   | Bị chặn | thiếu data/môi trường/quyền | NEEDS-CONFIRM, không sinh task |
2. **Sinh task** — append vào `M<x>-NN-PLAN.md` dưới `## Converge round <n>`, đánh số nối tiếp (`T<max+1>`…), đủ cột như task thường: `[file:]`, `[depends:]` (được trỏ tới key cũ của phase, vd `T3`), `[verify:]` = lệnh của cột "Cách verify" của AC đó (scoped), `[skill:]`, `[UC:]`, `[AC:]`. Mỗi task gánh ≥1 AC FAIL hoặc 1 finding; không thêm việc ngoài matrix.
3. **Gate người duyệt** — không `--auto`: trình bảng task converge (AC ↔ task ↔ file) + `ask` (Chạy / Sửa / Dừng). `--auto`: chạy luôn.
4. **Chạy** — `engine_plan {append: true, goal: "M<x> converge round <n>", slices: [{title: "Converge <n>", tasks: [...]}]}`: engine giữ nguyên plan của phase (task cũ vẫn `done`) và nối slice mới vào, nên `engine_status` thấy cả phase. Engine chưa có plan của phase (session khác đã xoá `.cache/`) → append tự thành plan mới; khi đó `[depends:]` chỉ được trỏ giữa task converge. Rồi chạy đúng vòng Bước 2 của `references/execute.md` (dev ∥ test, `engine_verify` → `engine_advance` commit từng task). STATE `next_action: converge` để session sau resume đúng chỗ.
5. **Nghiệm thu lại** — chạy lại Step 2 cho các AC vừa FAIL + chạy lại lệnh "Cách verify" (rẻ) của mọi AC đã PASS để bắt hồi quy; review lại chỉ các file round này đụng. Cập nhật matrix + ghi round vào `## Converge rounds` của VERIFICATION.
6. **Dừng** khi: matrix toàn PASS → Step 3; hoặc **cùng một AC FAIL với cùng bằng chứng 2 vòng liền** → coi là kẹt: ghi `failure:` vào `agents/KNOWLEDGE.md`, báo user AC đó + 2 lần thử; hoặc hết số vòng → báo user danh sách AC còn FAIL. Không đóng phase trong 2 trường hợp sau.

## Step 2.6 — NEEDS-CONFIRM (đóng phase có điều kiện)

NEEDS-CONFIRM chỉ dành cho AC **bị chặn ngoài tầm** (môi trường prod, quyền/credential, dữ liệu thật, bên thứ ba) — không bao giờ cho lỗi code hay test chưa viết. Phase còn NEEDS-CONFIRM (không còn FAIL) được đóng **có điều kiện** khi đủ:
1. Mỗi dòng ghi ở mục NEEDS-CONFIRM của VERIFICATION: chặn bởi gì, **ai** xác nhận (user/QA/ops), khi nào/ở đâu, bằng chứng đã có (vd test local PASS).
2. Không `--auto`: `ask` user chấp nhận đóng có điều kiện (Đóng có điều kiện / Giữ phase mở). `--auto`: KHÔNG tự đóng — giữ phase `[~]`, báo user.
3. User chấp nhận → ROADMAP ghi `[x]` kèm "(NEEDS-CONFIRM: AC-xx)", STATE `## Blockers/Concerns` thêm từng dòng (AC + phase + ai xác nhận). Phase sau KHÔNG được phụ thuộc vào hành vi của AC đang chờ, trừ khi user chốt.
4. Khi xác nhận xong → cập nhật verdict PASS + bằng chứng trong VERIFICATION của phase cũ, xoá dòng Blockers, bỏ ghi chú ở ROADMAP. Xác nhận ra FAIL → mở converge cho phase cũ (hoặc task ở phase hiện tại nếu user chọn).

## Step 3 — Ship (đóng phase)

1. **Bản đồ kiến thức (R16)** — sau khi AC matrix đã đóng, TRƯỚC commit ở mục 2:
   - Đầu vào (đều nằm trong repo): mục **🗺 Bổ sung bản đồ** của `M<x>-CONTEXT.md` (plan đã chép từ báo cáo scout) + `git diff <first>..<last>` của phase + Decisions của CONTEXT làm đổi hành vi vùng code.
   - Ghi vào `docs/knowledge/modules/<Module>.md` / `flows/<flow>.md` của vùng phase đụng (sửa tại chỗ, không append nhật ký); file mới → thêm dòng `INDEX.md`. Đúng format R16; mọi symbol/key ghi vào phải tồn tại ở HEAD (kiểm bằng code-intel/grep); tên đã bỏ chỉ được nhắc ở mục bẫy.
   - Không `--auto`: trình diff bản đồ + `ask` (Ghi / Sửa / Bỏ qua). `--auto`: ghi luôn, STATE Log ghi file bản đồ đã đổi. Nhiều vùng → spawn `task` (`agent: task`), prompt nhúng R16 literal + mục Bổ sung bản đồ.
2. **Commit**: code đã commit atomic per-task trong `/feature go` (qua `engine_advance`) rồi — KHÔNG commit gộp lại. Ship chỉ:
   - Commit nốt phần phụ của phase chưa thuộc task nào (`M<x>-VERIFICATION.md`/STATE/ROADMAP đổi + file bản đồ của mục 1): `docs: M<x> close phase <tên>` (Conventional Commits, tiếng Anh, milestone ở đầu — xem `execute.md` §Commit).
   - Verify **AC matrix trong M<x>-VERIFICATION.md MỌI AC = PASS** (Step 0+1+2) TRƯỚC khi tính phase done. Còn FAIL → KHÔNG đóng phase. Còn NEEDS-CONFIRM → chỉ đóng có điều kiện theo Step 2.6.
   - **KHÔNG `git push` / mở PR** — chỉ push khi user yêu cầu rõ.
3. **ROADMAP**: phase `[~]` → `[x]` + ghi Phase log (range commit `<first>..<last>` của phase, contract đã export).
4. **STATE cập nhật**:
   - `progress.completed_phases` +1, `percent` lại.
   - `active_phase` → phase tiếp (unblocked theo dependency) hoặc `null` nếu hết.
   - `next_action` → `plan-phase` cho phase sau, hoặc `done`.
   - `## Session Continuity`: ghi vừa ship phase nào.

**Ship gộp nhiều phase** (vd phase chèn MC thay một phần phase đã code xong): chỉ khi user chốt. Mỗi phase vẫn giữ `M<x>-VERIFICATION.md` riêng; phase sau ghi thêm các AC của phase trước mà nó thay đổi thành **AC hồi quy** (chạy lại "Cách verify", phải PASS). Đóng theo thứ tự phụ thuộc trong cùng một lượt ship; mỗi phase 1 commit `docs: M<x> close phase …`; ROADMAP Phase log ghi "shipped together with M<y>".

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

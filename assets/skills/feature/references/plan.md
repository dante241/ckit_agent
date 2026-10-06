# /feature plan

> Đã load `references/feature-rules.md` (luật xuyên suốt: R1 config-resolve, R3 load skill, R5 AC, R10 code-intel FIRST) ở Dispatch chưa? Nếu chưa → load trước.

Discuss + Plan cho phase hiện tại (`active_phase` trong STATE). Output: `M<x>-CONTEXT.md` + `M<x>-NN-PLAN.md` trong `agents/planning/<slug>/phases/M<x>-<name>/`.

## Step 1 — Discuss (chốt quyết định TRƯỚC khi plan)

- Đọc PROJECT.md (ràng buộc + Key Decisions, dòng mới nhất thắng; chỉ dòng "Ai quyết" = `user` là ràng buộc — R15) + ROADMAP.md (dòng phase + Integration Contracts) + REQUIREMENTS.md (UC của phase + dòng **Nguồn yêu cầu**).
- **Đọc nguồn yêu cầu của phase (R12, BẮT BUỘC):**
  - Có đặc tả ngoài → đọc mục "Đọc bắt buộc" của phase trong REQUIREMENTS; chạy `python3 ~/.omp/skills/feature/scripts/coverage.py agents/planning/<slug> --phase M<x>` lấy danh sách mã; đọc **nguyên văn** từng mục UC (luồng chính, luồng phụ, ngoại lệ, tiêu chí US·AC) và từng mã BR/D/W/P/SCR trong đặc tả (tìm theo mã/tên mục, số dòng có thể lệch). Không đọc bản tóm tắt thay đặc tả.
  - Không có → REQUIREMENTS.md là nguồn.
  - Ghi những gì đã đọc vào mục **📖 Nguồn yêu cầu đã đọc** của CONTEXT.
- **Contract nhận từ phase trước**: đối chiếu Integration Contracts mà phase này dùng với Key Decisions mới nhất + code thật (code-intel). Contract đã bị thay (phase chèn, quyết định mới) → sửa ROADMAP trước khi plan (R13).
- **Phase có UI**: đọc thiết kế của phase (node Figma ghi ở ROADMAP / mockup) trước khi viết AC giao diện; AC giao diện trích từ thiết kế + mã SCR.
- Trích **Requirement scope** cho phase: liệt kê UC-ID + mô tả từ REQUIREMENTS.md mà phase này chịu trách nhiệm; nếu ROADMAP và REQUIREMENTS lệch phase/UC → sửa/hỏi trước khi plan.
- Đọc knowledge liên quan: `agents/KNOWLEDGE.md` + (R10) `xd://mcp__codegraph_explore` / `xd://mcp__codebase_memory_mcp_get_architecture` cho module sẽ đụng.
- Chốt quyết định triển khai mơ hồ (API nào, schema, pattern). Mơ hồ → dùng `ask` (tương tác). Auto-mode: thay `ask` bằng spawn `task` discuss (xem `auto.md`).
- Ghi `agents/planning/<slug>/phases/M<x>-<name>/M<x>-CONTEXT.md`: quyết định riêng phase + Requirement scope (dùng `templates/M-CONTEXT.md`).
- Append quyết định lớn vào STATE.Decisions + PROJECT Key Decisions table.

### ★ BẮT BUỘC: Goal + Acceptance Criteria (UAT) trong CONTEXT — KHÔNG được bỏ

Mỗi `M<x>-CONTEXT.md` PHẢI có 3 mục dưới TRƯỚC khi plan. Đây là **hợp đồng nghiệm thu** mà `/feature go` và `/feature ship` đọc làm chuẩn — thiếu thì subagent code/review/test không biết đang phục vụ phần nào của REQUIREMENTS.

1. **📌 Requirement scope** — bảng UC-ID từ `REQUIREMENTS.md` mà phase này làm, mỗi dòng:
   - **UC**: id đúng như REQUIREMENTS.md (vd UC-15).
   - **Mô tả**: copy/rút gọn literal từ REQUIREMENTS.md.
   - **Trong phase này làm gì**: phạm vi cụ thể của UC ở phase này.
   - **Không làm ở phase này**: ranh giới nếu UC còn phần future/out-of-scope.
2. **🎯 Goal** — 1 câu: output đo được của phase + ranh giới (CHƯA làm gì → tránh review đòi hỏi quá phạm vi).
3. **✅ Acceptance Criteria (UAT)** — bảng `AC-NN`, mỗi dòng:
   - **Given/When/Then** điều kiện PASS **đo được** (số, trạng thái, output cụ thể — KHÔNG "chạy ổn", "đúng").
   - **UC**: UC-ID từ Requirement scope mà AC này chứng minh (vd UC-15).
   - **Nguồn**: mã đặc tả AC này chứng minh — tiêu chí `US-01·AC-03`, quy tắc `BR-04`, luồng `UC-02 A4`, ngoại lệ `E1`; không có đặc tả ngoài → `REQUIREMENTS`. Viết GIVEN/WHEN/THEN bám nguyên văn nguồn, không diễn giải lại theo ý mình.
   - **Cách verify**: lệnh/SQL/script/thao tác cụ thể để chứng minh (đây cũng là `verify` của engine task ở `go`).
   - **Tier**: must-test / verify-sql / verify-only.
   - **Task nguồn**: AC này do task nào trong PLAN thỏa.
   - AC phải phủ HẾT Goal + map về contract export (ROADMAP). Mỗi UC của phase ⇒ ≥1 AC. Có đặc tả ngoài: mỗi mã của phase (`--phase M<x>`) ⇒ ≥1 AC có mã đó ở cột Nguồn, hoặc 1 dòng ở mục **Chuyển phase** (mã → phase nhận + lý do; đồng thời sửa cột Phase trong COVERAGE).
4. Cuối mục ghi: **"Phase DONE khi mọi AC PASS, ghi ở M<x>-VERIFICATION.md. AC FAIL → không ship."**

> Template Requirement scope: `| UC | Mô tả REQUIREMENTS.md | Trong phase này làm gì | Không làm ở phase này |`
> Template AC: `| AC-01 | UC-15 | US-02·AC-03, BR-04 | GIVEN <tiền đề> WHEN <hành động> THEN <kết quả đo được> | <cách verify> | <tier> | <task> |`
> Mọi task trong PLAN (Step 3) phải truy được về ≥1 AC và ≥1 UC. AC không có task thỏa = thiếu task; task không gắn UC = không biết phục vụ phần nào của REQUIREMENTS.

## Step 2 — Research (FAN-OUT song song nếu cần)

**Áp dụng R10 (code-intelligence FIRST) — kể cả khi tự research ở main thread lẫn khi spawn subagent.**

Nếu phase cần khảo nhiều mặt codebase (và `config.workflow.parallelization === true`):
- Spawn ĐỒNG THỜI nhiều `task` subagent `agent: scout` (1 message, nhiều tool-call) — mỗi agent 1 khía cạnh:
  - "tìm pattern <X> trong codebase"
  - "schema/migration module tương tự"
  - "module tham khảo đã làm <Y>"
- **Prompt mỗi subagent nhúng chỉ thị R10 literal**: MCP tool là `xd://` device (gọi bằng `write` JSON args vào path); dùng `xd://mcp__codegraph_explore` / CLI `codegraph query/callers/impact "<query>"` hoặc codebase-memory-mcp (`xd://mcp__codebase_memory_mcp_search_graph` / `_trace_path` / `_get_architecture`) / serena (`xd://mcp__serena_find_symbol`) để tìm/hiểu code TRƯỚC grep/read thô; chỉ read khi cần xem chi tiết 1 file cụ thể đã định vị; kết quả dài (>300 dòng) → tóm tắt phần liên quan trong báo cáo cuối, không dump thô.
- Barrier → tổng hợp ở main thread.
- Phase nhỏ/pattern đã rõ, hoặc `parallelization === false` → skip, không spawn (chạy tuần tự main thread).

## Step 3 — Plan (decompose + đồ thị phụ thuộc)

Tạo `M<x>-NN-PLAN.md` (NN bắt đầu 01, dùng `templates/M-PLAN.md`). BẮT BUỘC mỗi task có:
- **file ownership** `[file:]`: đụng file/folder nào — `go` dùng để chặn 2 task sửa cùng file chạy song song + để commit đúng file của task.
- **phụ thuộc** `[depends:]`: task nào PHẢI xong (test PASS + commit) trước. Ghi `—` nếu không phụ thuộc. → `go` chạy song song mọi task đã đủ phụ thuộc, không chờ theo đợt.
- **verify** `[verify:]`: lệnh lint/test THẬT, **scoped vào task** (lint file của task, test filter của task) — `go` chạy nó ngay khi task dev xong, trong lúc task khác còn đang sửa.
- **test tier**: must-test / verify-sql / verify-only.
- **skill**: skill repo nào chi phối task (`.omp/skills/<name>` hoặc `~/.omp/skills/<name>`). BẮT BUỘC — subagent KHÔNG tự biết skill nào áp dụng; cột này là nguồn để `go` resolve. Task không rõ skill → ghi `—` nhưng tự hỏi đã đúng chưa (đa số task code có ≥1 skill chi phối). Sai/thiếu skill = nguồn lỗi "code đúng task nhưng sai convention".
- **UC phục vụ**: task này phục vụ UC-ID nào trong `REQUIREMENTS.md`/Requirement scope (vd UC-15).
- **AC thỏa**: task này thỏa AC-NN nào (truy ngược về CONTEXT). Mọi AC phải có ≥1 task thỏa; task không gắn AC/UC nào = nghi vấn thừa, soát lại.

Template PLAN:
```markdown
# M<x>-NN-PLAN — <phase name>
## Tasks
- [ ] T1: <việc>   [file: path]   [depends: —]    [verify: <lệnh>]   [skill: <name>]   [tier: ...]   [UC: UC-15]   [AC: AC-01,AC-03]
- [ ] T2: <việc>   [file: path]   [depends: —]    [verify: <lệnh>]   [skill: <name>]   [tier: ...]   [UC: UC-16]   [AC: AC-05]
- [ ] T3: <việc>   [file: path]   [depends: T1]   [verify: <lệnh>]   [skill: <name>]   [tier: ...]   [UC: UC-15]   [AC: AC-08]
## Checkpoints / Gates
- review dimensions: <từ config.workflow.review_dimensions>
- **Acceptance**: phase done ⇔ mọi AC trong M<x>-CONTEXT PASS (verify ở /feature ship → M<x>-VERIFICATION.md). KHÔNG dùng DoD mơ hồ — dùng AC.
```

**Luật phụ thuộc** (quyết định tốc độ — càng ít cạnh thừa, càng nhiều task chạy song song):
- Chỉ ghi `B [depends: A]` khi B **thật sự cần thứ A tạo ra**: symbol/class/interface, bảng/cột DB, file, config mà B gọi/đọc. "A nên làm trước cho gọn" KHÔNG phải phụ thuộc.
- B chỉ cần **chữ ký** của A (interface, DTO) → tách task nhỏ "định nghĩa interface/DTO" làm trước; phần cài đặt A và B chạy song song sau nó.
- Hai task **cùng file** hoặc cùng **tài nguyên dùng chung** (migration sequence, `composer.json`/`vendor/composer/*`, file ngôn ngữ, config dùng chung) → BẮT BUỘC có phụ thuộc giữa nhau (tránh ghi đè).
- Task dựng nền (bootstrap, tooling) → giữ nhỏ, tách riêng, để không chặn cả phase.
- Không vòng phụ thuộc.

**Kiểm tra phủ trước khi trình (2 chiều):**
- Xuôi: mọi UC trong Requirement scope đều có ≥1 AC; mọi AC-NN trong CONTEXT đều xuất hiện ở cột [AC:] của ≥1 task; mọi task có `[UC:]` + `[AC:]`. Thiếu UC/AC nào không task thỏa → thêm task (hoặc UC/AC sai phạm vi → sửa CONTEXT).
- Ngược (có đặc tả ngoài): mọi mã trong output `coverage.py --phase M<x>` xuất hiện ở cột Nguồn của ≥1 AC hoặc ở mục Chuyển phase. Liệt kê mã còn hở trước khi trình gate 2 — không được bỏ im lặng.

## Step 3.5 — Plan-review (gate theo `config.workflow.plan_review`)

Soát PLAN TRƯỚC khi code — bắt lỗi cấp-kế-hoạch (AC hở, phụ thuộc sai/race, thiếu skill) rẻ hơn bắt sau khi đã code. Đọc `config.workflow.plan_review`:

| Giá trị | Khi nào chạy review |
|---------|---------------------|
| `always` | luôn review PLAN |
| `complex` (mặc định khuyến nghị) | chỉ review khi phase **phức tạp**: ≥3 task, HOẶC chuỗi phụ thuộc dài (≥3 tầng), HOẶC chạm ≥2 module/domain, HOẶC có task DB/migration/schema. Phase nhỏ, ít task, đơn giản → BỎ. |
| `never` / thiếu | BỎ Step này. |

Khi chạy: spawn 1 `task` subagent `agent: reviewer` review **bản PLAN + bảng AC** (không phải code — chưa có code). Prompt nhúng literal: Goal + bảng AC + PLAN + **nguồn yêu cầu của phase** (có đặc tả ngoài: danh sách mã `--phase M<x>` + đoạn đặc tả nguyên văn của các mục UC/BR liên quan, hoặc đường dẫn file đặc tả + tên mục để reviewer tự đọc), yêu cầu soát:
- **Khớp nguồn**: AC nào lệch/làm yếu nguyên văn đặc tả (Then thiếu điều kiện, bỏ ngoại lệ)? Mã nào của phase không AC nào phủ và không ở mục Chuyển phase?
- **Phủ UC/AC**: mọi UC trong Requirement scope có ≥1 AC? Mọi AC-NN có ≥1 task thỏa? Task nào không gắn UC/AC (nghi thừa)?
- **Phụ thuộc đúng**: `[depends:]` có cạnh thừa (chặn song song vô lý) hoặc thiếu cạnh (2 task cùng file/tài nguyên dùng chung không phụ thuộc nhau → race)? `[verify:]` có scoped vào task không?
- **Skill đúng**: cột `[skill:]` hợp lý? Task code mà `[skill: —]` → cờ đỏ.
- **Thiếu task**: Goal/contract ROADMAP có phần nào chưa task nào phủ?
- "Trả về findings cụ thể (PLAN dòng nào) + 1 verdict: READY / NEEDS-FIX. KHÔNG hỏi lại."

Có NEEDS-FIX → sửa PLAN/CONTEXT (main thread) → ghi tóm tắt vào `M<x>-CONTEXT.md` (mục Plan-review notes) → tiếp Step 4. Không loop vô hạn: tối đa 2 vòng, còn lỗi thì nêu ở gate 2 cho user quyết.

> Auto-mode: plan-review chạy **THAY** gate 2 (xem `auto.md`) — verdict READY thì tự duyệt; NEEDS-FIX thì sửa rồi tự duyệt, ghi `(auto-decided)`.

## Step 4 — USER DUYỆT plan (gate 2)

Trình **Goal + bảng AC (kèm cột Nguồn) + PLAN** (AC là phần user nghiệm thu — nêu rõ để user soát tiêu chí đúng ý) + danh sách mã chuyển phase + tóm tắt verdict plan-review (Step 3.5) nếu có chạy. Dùng `ask` xác nhận trước khi code. Cập nhật STATE:
- `status: planned`, `next_action: execute-phase`.
- ROADMAP: phase `[ ]` → `[~]`.

Next: `/feature go`.

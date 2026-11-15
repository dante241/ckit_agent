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
- **Đọc bản đồ kiến thức (R16) TRƯỚC khi dò code:** `docs/knowledge/INDEX.md` → file module/flow của vùng phase sẽ đụng; + `agents/KNOWLEDGE.md`. Rồi mới (R10) `xd://mcp__codegraph_explore` / `xd://mcp__codebase_memory_mcp_get_architecture` cho phần bản đồ chưa có. Ghi file + mục bản đồ đã dùng vào mục **📖 Nguồn yêu cầu đã đọc** của CONTEXT.
- Chốt quyết định triển khai mơ hồ (API nào, schema, pattern). Mơ hồ → dùng `ask` (tương tác). Auto-mode: thay `ask` bằng spawn `task` discuss (xem `auto.md`).
- Ghi `agents/planning/<slug>/phases/M<x>-<name>/M<x>-CONTEXT.md`: quyết định riêng phase + Requirement scope (dùng `templates/M-CONTEXT.md`). **Main KHÔNG tự soạn file dài** (CONTEXT, PLAN): chốt quyết định xong → spawn 1 `task` writer (`agent: task`) với con trỏ (đường dẫn template, PROJECT/REQUIREMENTS/ROADMAP, đặc tả, báo cáo scout đã lưu file) + danh sách Decision đã chốt (literal, ngắn) → writer ghi file và trả **tóm tắt ≤ 30 dòng** (Goal · mỗi AC 1 dòng `AC-NN: Then …` · mỗi task 1 dòng `T<n> [depends] [AC]`). Lý do: nội dung main tự viết nằm lại trong context của nó suốt các bước sau (lần chạy thật: ~30% context main là tài liệu main tự soạn). Sửa nhỏ sau review → main `edit` đúng dòng, không viết lại cả file.
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
   - **Given/When/Then** điều kiện PASS **đo được** (số, trạng thái, output cụ thể — KHÔNG "chạy ổn", "đúng"), kèm **ví dụ cụ thể**: ≥1 dòng `đầu vào → kết quả mong đợi` của luồng chính + ≥1 trường hợp biên/ngoại lệ (rỗng, trùng, hết quyền, giá trị 0…). User duyệt ví dụ ở gate 2 — đây là chỗ bắt "hiểu sai ý" rẻ nhất; tester dùng chính ví dụ làm assertion.
   - **UC**: UC-ID từ Requirement scope mà AC này chứng minh (vd UC-15).
   - **Nguồn**: mã đặc tả AC này chứng minh — tiêu chí `US-01·AC-03`, quy tắc `BR-04`, luồng `UC-02 A4`, ngoại lệ `E1`; không có đặc tả ngoài → `REQUIREMENTS`. Viết GIVEN/WHEN/THEN bám nguyên văn nguồn, không diễn giải lại theo ý mình.
   - **Cách verify**: lệnh/SQL/script/thao tác cụ thể để chứng minh (đây cũng là `verify` của engine task ở `go`).
   - **Tier**: must-test / verify-sql / verify-only.
   - **Task nguồn**: AC này do task nào trong PLAN thỏa.
   - AC phải phủ HẾT Goal + map về contract export (ROADMAP). Mỗi UC của phase ⇒ ≥1 AC. Có đặc tả ngoài: mỗi mã của phase (`--phase M<x>`) ⇒ ≥1 AC có mã đó ở cột Nguồn, hoặc 1 dòng ở mục **Chuyển phase** (mã → phase nhận + lý do; đồng thời sửa cột Phase trong COVERAGE).
4. **Mệnh đề** (mục `### Mệnh đề` của CONTEXT) — mỗi AC tách thành mệnh đề có mã `AC-NN.a`, `.b`…: mỗi điều kiện THEN, mỗi ví dụ, mỗi trường hợp biên một dòng. Đây là đơn vị mà `engine_red` chấm độ phủ (thiếu assertion cho mệnh đề nào → INCOMPLETE), test nghiệm thu đặt tên theo, và ship chấm verdict. Mệnh đề không test tự động được (môi trường thật, thao tác tay) → `[manual]` + lý do, ngay lúc plan — không để tới khi bị từ chối.
5. **Luật trích** (mục `### Luật trích`) — chép **nguyên văn** mọi mã luật ở cột Nguồn (AR/BR/E/D…, trừ UC/US/AC) kèm nơi lấy. Spec của task chỉ mang những gì nằm trong CONTEXT; luật chỉ được nhắc mã mà không chép thì dev, tester và reviewer không đọc tới (lần chạy thật: AC-02/AC-14 FAIL ở ship vì AR-12/AR-44 không có trong spec). `phase_gate.py --check plan` báo lỗi khi một mã ở cột Nguồn thiếu ở đây.
6. Cuối mục ghi: **"Phase DONE khi mọi AC PASS, ghi ở M<x>-VERIFICATION.md (NEEDS-CONFIRM → `ship.md` Step 2.6). AC FAIL → không ship."**

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
- **Prompt mỗi subagent nhúng đoạn bản đồ (R16) của vùng nó dò** + "chỉ dò phần bản đồ chưa có hoặc nghi lệch; báo cáo thêm mục **Bổ sung bản đồ** (điểm cắm, công thức 'muốn làm X → sửa A → B', bẫy — neo bằng tên symbol)".
- **Prompt mỗi subagent nhúng chỉ thị R10 literal**: MCP tool là `xd://` device (gọi bằng `write` JSON args vào path); dùng `xd://mcp__codegraph_explore` / CLI `codegraph query/callers/impact "<query>"` hoặc codebase-memory-mcp (`xd://mcp__codebase_memory_mcp_search_graph` / `_trace_path` / `_get_architecture`) / serena (`xd://mcp__serena_find_symbol`) để tìm/hiểu code TRƯỚC grep/read thô; chỉ read khi cần xem chi tiết 1 file cụ thể đã định vị; kết quả dài (>300 dòng) → tóm tắt phần liên quan trong báo cáo cuối, không dump thô.
- Barrier → tổng hợp ở main thread; chép mục **Bổ sung bản đồ** của mọi scout vào mục **🗺 Bổ sung bản đồ** của `M<x>-CONTEXT.md` (đầu vào `ship.md` Step 3 mục 1 — transcript subagent không sống qua session/harness).
- Phase nhỏ/pattern đã rõ, hoặc `parallelization === false` → skip, không spawn (chạy tuần tự main thread).

## Step 3 — Plan (decompose + đồ thị phụ thuộc)

Tạo `M<x>-NN-PLAN.md` (NN bắt đầu 01, dùng `templates/M-PLAN.md`). BẮT BUỘC mỗi task có:
- **file ownership** `[file:]`: đụng file/folder nào — `go` dùng để chặn 2 task sửa cùng file chạy song song + để commit đúng file của task.
- **phụ thuộc** `[depends:]`: task nào PHẢI xong (test PASS + commit) trước. Ghi `—` nếu không phụ thuộc. → `go` chạy song song mọi task đã đủ phụ thuộc, không chờ theo đợt.
- **verify** `[verify:]`: lệnh lint/test THẬT, **scoped vào task** (lint file của task, test filter của task) — `go` chạy nó ngay khi task dev xong, trong lúc task khác còn đang sửa. Task gánh AC mà tier ≠ verify-only: verify phải chạy test hành vi; chỉ `php -l` / `node --check` / `test -s` / `ls` là bị `phase_gate.py` báo lỗi. Task không code (doc, config tĩnh) → `[trivial]`, không cần verify — và KHÔNG được gắn `[AC:]` (AC cần bằng chứng chạy được).
- **test tier**: must-test / verify-sql / verify-only.
- **red-first** `[red: <test paths>]` (BẮT BUỘC cho must-test có test tự động): test viết TRƯỚC và phải ĐỎ trên code cũ — `engine_red` cho reviewer độc lập soát test có công bằng với các AC ở cột `[AC:]` không (không bám tên/đường dẫn/câu thông báo mà AC không nêu), rồi ghi bằng chứng ĐỎ; `engine_verify` từ chối task chưa có. Vì vậy AC của task red phải tự đủ nghĩa (Then đo được). Các path này phải nằm trong `[file:]`, và `[file:]` phải còn file code. Thiếu `[red:]` ở task must-test = `phase_gate.py` cảnh báo; ghi lý do ở Decisions nếu cố ý bỏ (vd test cần dữ liệu thật).
- **skill**: skill repo nào chi phối task (`.omp/skills/<name>` hoặc `~/.omp/skills/<name>`). BẮT BUỘC — subagent KHÔNG tự biết skill nào áp dụng; cột này là nguồn để `go` resolve. Task không rõ skill → ghi `—` nhưng tự hỏi đã đúng chưa (đa số task code có ≥1 skill chi phối). Sai/thiếu skill = nguồn lỗi "code đúng task nhưng sai convention".
- **UC phục vụ**: task này phục vụ UC-ID nào trong `REQUIREMENTS.md`/Requirement scope (vd UC-15).
- **AC thỏa**: task này thỏa AC-NN nào (truy ngược về CONTEXT). Mọi AC phải có ≥1 task thỏa; task không gắn AC/UC nào = nghi vấn thừa, soát lại.
- **Hợp đồng** (dòng thụt `  - Hợp đồng:` ngay dưới task; BẮT BUỘC cho task tạo/đổi API công khai mà task khác hoặc test sẽ gọi): chữ ký hàm/method, tên type + field, **mã lỗi/thông báo** mà test sẽ so, bố cục file/thư mục trên đĩa, cờ CLI, route/endpoint. Đây là thứ test được phép bám: reviewer của `engine_red` từ chối test dựa vào tên/đường dẫn/mã lỗi mà spec không nêu — thiếu hợp đồng thì test viết đúng ý vẫn bị UNFAIR, phải viết lại (lần chạy thật: 4 lần từ chối + 1 lần plan lại engine chỉ vì hợp đồng nằm ngoài spec). Hợp đồng đi vào `spec` của engine task (`execute.md` Bước 1).
- **Nghiệm thu trước** (BẮT BUỘC khi phase có AC tier must-test) — 2 task gắn `[acceptance]`:
  - `T0` **viết bộ test nghiệm thu**: `[file: <thư mục test nghiệm thu, vd tests/acceptance/>]` `[depends: —]` `[trivial]` `[acceptance]` — do **tester** viết ngay khi `go` bắt đầu, chạy song song với các task code, từ CONTEXT (mệnh đề + luật trích) + các dòng `Hợp đồng:`, KHÔNG nhìn code. Mỗi mệnh đề (trừ `[manual]`) ≥1 test, tên theo mã (`ac04_b_disabled_param_not_sent`); black-box qua API công khai/CLI, được đi xuyên nhiều module (đây là chỗ bắt hành vi mà test của từng task không thấy). `[trivial]` vì lúc đó code chưa có nên chưa chạy được — bằng chứng nằm ở `TZ`.
  - `TZ` **chạy bộ nghiệm thu**: `[file: —]` `[depends: T0 + mọi task code]` `[verify: <lệnh chạy riêng bộ nghiệm thu>]` `[acceptance]` `[tier: must-test]` `[AC: mọi AC must-test]`. Engine chỉ báo phase xong khi `TZ` xanh; `engine_accept` gắn bằng chứng các AC qua `TZ`.
  Vì sao: verify của từng task chỉ chứng minh code pass test do chính dev viết; lần chạy thật để lọt 6/14 AC tới ship (mệnh đề không ai test, hành vi xuyên crate). Bộ nghiệm thu do người khác viết, từ hợp đồng, trước khi có code.

Template PLAN:
```markdown
# M<x>-NN-PLAN — <phase name>
## Tasks
- [ ] T1: <việc>   [file: path, tests/XTest.php]   [depends: —]    [verify: <lệnh test>]   [red: tests/XTest.php]   [skill: <name>]   [tier: must-test]   [UC: UC-15]   [AC: AC-01,AC-03]
  - Hợp đồng: `fn send(req: &Request, opts: &SendOptions) -> Result<Outcome, HttpError>`; lỗi `http.timeout`, `http.too_many_redirects`; file `collections/<slug>/_collection.yaml`
- [ ] T2: <việc>   [file: path]   [depends: —]    [verify: <lệnh>]   [skill: <name>]   [tier: verify-sql]   [UC: UC-16]   [AC: AC-05]
- [ ] T3: <việc không code: doc/config tĩnh>   [file: path]   [depends: T1]   [trivial]   [skill: —]   [tier: verify-only]   [UC: UC-15]
- [ ] T0: viết bộ test nghiệm thu theo mệnh đề AC   [file: tests/acceptance/]   [depends: —]   [trivial]   [acceptance]   [skill: <test skill>]   [UC: UC-15,UC-16]
- [ ] TZ: chạy bộ nghiệm thu   [file: —]   [depends: T0,T1,T2]   [verify: <lệnh chạy tests/acceptance>]   [acceptance]   [skill: —]   [tier: must-test]   [UC: UC-15,UC-16]   [AC: AC-01,AC-03]
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
- **Độ rộng**: đếm số task chạy được cùng lúc ở mỗi tầng của đồ thị. Phase ≥4 task mà hầu hết các tầng chỉ có 1 task (chuỗi dọc) → `max_parallel_tasks` bị bỏ phí, phase chạy tuần tự. Tách theo hợp đồng: task "định nghĩa type/interface/hợp đồng" nhỏ đi trước, các task cài đặt từng crate/module/endpoint phụ thuộc vào nó thay vì vào nhau. Ghi độ rộng (vd `tầng: 1·3·2·1`) ở đầu PLAN.

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

Khi chạy: spawn 1 `task` subagent `agent: reviewer` review **bản PLAN + bảng AC** (không phải code — chưa có code). Prompt đưa **đường dẫn** (`M<x>-CONTEXT.md`, `M<x>-NN-PLAN.md`, ROADMAP, đặc tả + tên mục) để reviewer tự đọc — KHÔNG dán nội dung các file này vào prompt; có đặc tả ngoài thì kèm danh sách mã `--phase M<x>`. Yêu cầu soát:
- **Khớp nguồn**: AC nào lệch/làm yếu nguyên văn đặc tả (Then thiếu điều kiện, bỏ ngoại lệ)? Mã nào của phase không AC nào phủ và không ở mục Chuyển phase?
- **Phủ UC/AC**: mọi UC trong Requirement scope có ≥1 AC? Mọi AC-NN có ≥1 task thỏa? Task nào không gắn UC/AC (nghi thừa)?
- **Phụ thuộc đúng**: `[depends:]` có cạnh thừa (chặn song song vô lý) hoặc thiếu cạnh (2 task cùng file/tài nguyên dùng chung không phụ thuộc nhau → race)? `[verify:]` có scoped vào task không? **Độ rộng**: tầng nào chỉ 1 task mà tách được theo hợp đồng?
- **Hợp đồng**: task tạo/đổi API công khai có dòng `Hợp đồng:` đủ thứ test sẽ bám (chữ ký, mã lỗi, bố cục file)? Test của task red có cần tên nào không nằm trong hợp đồng/AC?
- **Mệnh đề + luật**: mỗi AC đã tách đủ mệnh đề (mọi THEN, ví dụ, biên)? Mệnh đề `[manual]` có lý do thật (không phải né việc viết test)? Mọi mã luật ở cột Nguồn có nguyên văn ở `### Luật trích`?
- **Nghiệm thu**: phase có AC must-test thì có `T0` + `TZ` `[acceptance]`, `TZ` phụ thuộc mọi task code và gánh mọi AC must-test?
- **Skill đúng**: cột `[skill:]` hợp lý? Task code mà `[skill: —]` → cờ đỏ.
- **Thiếu task**: Goal/contract ROADMAP có phần nào chưa task nào phủ?
- "Trả về ≤ 25 dòng: findings cụ thể (PLAN dòng nào) + 1 verdict: READY / NEEDS-FIX. KHÔNG hỏi lại." (Báo cáo dài thì ghi vào `M<x>-CONTEXT.md` mục Plan-review notes và chỉ trả verdict + đường dẫn.)

Có NEEDS-FIX → sửa PLAN/CONTEXT (main thread) → ghi tóm tắt vào `M<x>-CONTEXT.md` (mục Plan-review notes) → tiếp Step 4. Không loop vô hạn: tối đa 2 vòng, còn lỗi thì nêu ở gate 2 cho user quyết.

> Auto-mode: plan-review chạy **THAY** gate 2 (xem `auto.md`) — verdict READY thì tự duyệt; NEEDS-FIX thì sửa rồi tự duyệt, ghi `(auto-decided)`.

## Step 4 — USER DUYỆT plan (gate 2)

**Trước khi trình:** `python3 ~/.omp/skills/feature/scripts/phase_gate.py agents/planning/<slug> --phase M<x> --check plan` phải exit 0 (task gánh AC thiếu `[verify:]`, hoặc verify chỉ kiểm cú pháp → sửa PLAN, không trình). Dán các dòng WARN vào phần trình gate 2.

Trình **Goal + bảng AC (kèm cột Nguồn) + PLAN** (AC là phần user nghiệm thu — nêu rõ để user soát tiêu chí đúng ý) + danh sách mã chuyển phase + tóm tắt verdict plan-review (Step 3.5) nếu có chạy. Dùng `ask` xác nhận trước khi code. Cập nhật STATE:
- `status: planned`, `next_action: execute-phase`.
- ROADMAP: phase `[ ]` → `[~]`.

Next: `/feature go`.

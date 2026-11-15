# /feature go

> Đã load `references/feature-rules.md` (R3 load skill 2-lớp, R8 commit, **R10 code-intelligence FIRST — áp dụng cả subagent**) ở Dispatch chưa? Nếu chưa → load trước. Bước dưới KHÔNG lặp lại các luật đó.

Execute phase hiện tại theo PLAN, theo **đồ thị phụ thuộc (DAG)**: task nào đủ điều kiện thì dev ngay, dev xong thì test ngay, trong lúc test thì các task khác vẫn đang dev; test lỗi thì trả về dev, lặp tới khi hết task.

## Nguyên lý: engine điều phối + giữ gate, feature layer giữ hợp đồng

Engine `engine_*` (state ở `.cache/ckit/engine/state.json`) enforce trong code: **thứ tự phụ thuộc** (`depends`), **chặn 2 task chồng file chạy cùng lúc** (`files`, so theo tiền tố đường dẫn), **verify-gate + doom-loop guard + retry**, **block dây chuyền** task phụ thuộc task bị block, **commit đúng file của task** (`engine_advance {files}`; từ chối commit cả tree khi task khác đang chạy). Feature layer giữ hợp đồng AC + bookkeeping STATE/PLAN. Kỷ luật guardrail mirror `/auto` (`.omp/commands/auto.md`).

## Trước khi code

- **Đọc `M<x>-CONTEXT.md` TRƯỚC** → 📌 Requirement scope (UC) + 🎯 Goal + ✅ AC + **Decisions (D1, D2…)** + ranh giới. Code đúng task nhưng sai UC/Decision = vẫn hỏng.
- Đọc `M<x>-NN-PLAN.md` → mỗi task: `[file:]`, `[depends:]`, `[verify:]`, `[skill:]`, `[UC:]`, `[AC:]`.
- Đọc STATE.next_action + `engine_status` → resume đúng chỗ nếu session trước dở.
- **Load skill repo theo cột `[skill:]` (R3, BẮT BUỘC)** trước khi spawn.
- **Rà PLAN trước khi nạp** (sai → sửa PLAN, không chạy; engine cũng từ chối vòng phụ thuộc + key không tồn tại):
  - PLAN cũ chia `## Wave` mà task thiếu `[depends:]` → coi task wave N phụ thuộc mọi task wave N-1; thiếu `[verify:]` → lấy cột "Cách verify" của AC task gánh.
  - Hai task **cùng file** hoặc cùng **tài nguyên dùng chung** (migration sequence, `composer.json`/`vendor/composer/*`, file ngôn ngữ, config dùng chung) mà không có đường phụ thuộc giữa nhau → thêm `[depends:]` để chạy tuần tự.
  - Mỗi task phải có `[verify:]` **scoped vào chính task** (lint file của task, test filter của task) — KHÔNG chạy cả suite ở cấp task, vì task khác đang sửa dở cùng lúc.

> **Orchestrator giữ CONTEXT trong đầu suốt phase.** Lệch Decision → DỪNG (R6).

> **Orchestrator KHÔNG tự sửa code/test** — kể cả task scaffold, 1 dòng, `sed -i`, `cat > file <<EOF`. Mọi thay đổi code/test đi qua dev subagent; orchestrator chỉ sửa file planning (STATE/PLAN/CONTEXT). Lý do: mỗi dòng orchestrator tự viết nằm lại trong context của nó và bị đọc lại ở MỌI lượt sau (lần chạy thật: context main 43k → 325k, cache-read 35M token, chi phí main ≈ chi phí toàn bộ dev).

## Bước 1 — `engine_plan`: nạp phase vào engine

Gọi **`engine_plan`** một lần:
- `goal` = literal 🎯 **Goal** của phase.
- `criteria` = mỗi AC của phase 1 dòng (`AC-01: <Then …>`): engine chỉ báo DONE khi mọi dòng có bằng chứng qua `engine_accept` (xem Bước 2 mục 5).
- `slices` = 1 slice (tên phase) — thứ tự chạy do `depends` quyết, không do slice.
- Mỗi task: `key` = `T<n>`, `title` = mô tả task, `depends` = `[depends:]` (key), `files` = `[file:]`, `verify` = `[verify:]` (lệnh THẬT).
- Task có `[red: <test paths>]` → `red: true`, `tests` = các path đó (phải nằm trong `files`, và `files` phải còn ≥1 file không phải test), `spec` = các AC task gánh (Given/When/Then nguyên văn) **+ dòng `Hợp đồng:` của task** (chữ ký, mã lỗi, bố cục file mà test được phép bám) — thiếu `spec` thì engine REJECT. Task `[trivial]` → `trivial: true` (được advance không verify). Task không trivial mà thiếu `verify` → engine REJECT cả plan.
- Trước khi gọi: `python3 ~/.omp/skills/feature/scripts/phase_gate.py agents/planning/<slug> --phase M<x> --check plan` phải exit 0 (gate 2 đã chạy nó; PLAN sửa sau gate 2 → chạy lại).

Resume (đã có plan engine) → `engine_status`, KHÔNG plan lại.

## Bước 2 — Vòng điều phối (dev ∥ test, lặp tới hết)

Trạng thái mỗi task: `pending → dev → test → done`, hoặc `test → dev` (test lỗi), hoặc `blocked`.
Giới hạn số task dev đồng thời: `config.workflow.max_parallel_tasks` (thiếu → 4). `parallelization === false` → giới hạn = 1 (tuần tự, cùng vòng lặp).

Lặp:

1. **`engine_ready {limit: max_parallel_tasks}`** → engine trả mọi task đủ phụ thuộc + không chồng file với task đang chạy (tổng số task đang chạy không vượt `limit`), và đánh dấu `in_progress`. Task test lỗi vẫn giữ `in_progress` (giữ chỗ file) trong lúc dev lại.
2. **Spawn dev** cho các task sẵn sàng (tới giới hạn) — **1 lời gọi `task`, nhiều item**, `agent: task`, chạy nền (kết quả tự về, orchestrator không chờ từng cái). **Task `red: true` chạy 2 lượt:** (a) agent CHỈ viết/sửa file trong `tests`, **chạy formatter/lint-autofix của dự án lên chính các file test đó (vd `cargo fmt -p <crate>`, `prettier --write`) rồi mới dừng** → orchestrator gọi **`engine_red {taskId}`**: engine cho một reviewer độc lập (omp mới, không tool, không thấy phiên này) soát test theo `spec` trước — UNFAIR (bám tên/đường dẫn/số lần gọi/câu thông báo/giá trị mặc định mà spec không nêu, hoặc test rỗng) → quay lại (a) kèm lý do reviewer; sau đó engine từ chối nếu file code đã đổi, nếu test đã PASS sẵn, hoặc lỗi chỉ là exit 126/127 → đọc lỗi: phải là assertion đúng hành vi AC, không phải lỗi cú pháp/setup — sai thì quay lại (a); (b) RED đã ghi → spawn dev implement như dưới. **Sau `engine_red` KHÔNG ai sửa file test — kể cả chỉ format** (orchestrator lẫn dev): guard red-first coi mọi byte đổi là test mới → mọi `engine_verify` FAIL giống nhau → BLOCKED sau 3 lần. Verify có `fmt --check` mà test lệch format → đó là lỗi của lượt (a): quay lại (a), re-red.

   Prompt mỗi agent = **con trỏ, không chép nội dung**: subagent tự đọc được file trong repo; orchestrator chép spec/AC/Decision vào prompt thì chính orchestrator phải mang chúng trong context tới hết phase. Prompt BẮT BUỘC có:
   1. Task cụ thể (`T<n>` + 1 câu) + **danh sách file được phép sửa** + "chỉ làm task này, KHÔNG commit, KHÔNG chạy cả test suite".
   2. **Đọc nguyên văn TRƯỚC khi code** (dev tự đọc, đúng các mục này): task `T<n>` trong `agents/planning/<slug>/phases/M<x>-<name>/M<x>-NN-PLAN.md` (spec + hợp đồng API của task); `M<x>-CONTEXT.md` các AC `AC-..` task gánh + Decisions `D..` liên quan + Requirement scope; dòng UC trong `REQUIREMENTS.md`; có đặc tả ngoài (R12) → đường dẫn + mục đặc tả mà cột Nguồn của các AC trỏ tới. Dev phải thấy quy tắc gốc, không chỉ bản AC viết lại.
   3. **Skill (cột `[skill:]`) — 2 lớp:** (a) nhúng luật cốt lõi + anti-pattern (vài dòng); (b) ra lệnh Read `.omp/skills/<skill>/SKILL.md` (không có thì `~/.omp/skills/<skill>/SKILL.md`) TRƯỚC khi code.
   4. Convention: đường dẫn `AGENTS.md` + `agents/DECISIONS.md`/`PREFERENCES.md`.
   5. **Interface mà task phụ thuộc đã commit** (symbol + file:line, hoặc hash commit) + đường dẫn bản đồ (R16) của vùng task đụng.
   6. **R10 literal** (xem `feature-rules.md` R10 (a)) — đoạn ngắn, chép nguyên.
   7. Lệnh `[verify:]` của task — agent tự chạy trước khi báo xong.
   8. "Báo cáo cuối ≤ 15 dòng: danh sách file đã sửa/tạo + kết quả lệnh verify (dòng tổng kết, không dán log)."
   Lần dev lại sau test lỗi: nhúng thêm **output lỗi nguyên văn** (chỉ phần lỗi) + "sửa NGUYÊN NHÂN, không lặp cách sửa cũ". Task kế tiếp cùng vùng code → gửi cho dev agent còn sống (`agent://<Name>`) thay vì spawn mới: nó đã có context vùng đó.
3. **Dev xong 1 task → test ngay** (các agent khác vẫn đang dev):
   - **Kiểm phạm vi** — các agent dùng CHUNG working tree, nên `git status --porcelain` có cả file của task khác đang dev. File ngoài phạm vi của task = (file agent báo ∪ file mới thay đổi trong `git status --porcelain`) − `[file:]` của task − `files` của các task còn `in_progress` (liệt kê trong output `engine_ready`/`engine_status`) − file đã bẩn sẵn trước khi `go` bắt đầu (chụp `git status --porcelain` 1 lần lúc vào Bước 2). Còn file nào → coi như test lỗi, nêu rõ file. Agent báo đã sửa file nằm trong `files` của task khác → cũng là lỗi (đè việc task đó).
   - **`engine_verify {taskId}`** — gate chạy đúng `[verify:]`; `commands` truyền thêm chỉ được CỘNG vào, không thay. Task red chưa có RED → REFUSED.
4. **Kết quả test:**
   - **PASS** → **`engine_advance {taskId, commit:true, files:[đúng các file agent báo đã sửa/tạo], message:"<type>: M<x> - T<n> <English desc>"}`** — engine chỉ stage + commit các file đó (R8). KHÔNG truyền thư mục của `[file:]` (task khác có thể đang tạo file trong cùng thư mục). → Bookkeeping (Bước 3) → quay lại 1 (task phụ thuộc vừa được mở khóa).
   - **FAILED** → spawn lại dev cho task đó (mục 2, kèm output lỗi). Engine đếm retry: 2 lỗi giống nhau = cảnh báo, 3 hoặc chạm `maxRetries` = BLOCKED.
   - **BLOCKED** → engine tự block mọi task phụ thuộc nó (báo trong output); các nhánh khác chạy tiếp. Đọc output lỗi, chẩn đoán, rồi **`engine_unblock {taskId, diagnosis, action}`**: `retry` (nguyên nhân nằm ngoài task, đã xử lý), `split` (truyền `tasks` thay thế; task phụ thuộc chờ các task mới) hoặc `drop` (không cần nữa; task phụ thuộc chạy tiếp). Engine tự ghi `failure:` vào `agents/KNOWLEDGE.md` và mở lại các task bị block dây chuyền. Không chẩn đoán được (auto) → SKIP + NEEDS-CONFIRM.
5. **`engine_ready` báo `TASKS DONE, goal INCOMPLETE`** → gắn bằng chứng cho từng AC bằng **`engine_accept {criterion, kind}`**: `verify` (`taskId` của task done đã verify gánh AC đó), `file` (`path`+`line`) hoặc `shot` (ảnh chụp sau khi plan bắt đầu, vd. UAT bằng browser). AC chưa quan sát được thì để `incomplete` và báo đúng như vậy, KHÔNG coi là đạt. **`engine_ready` báo DONE** (mọi task xong + mọi AC có bằng chứng; không task nào đang chạy hoặc sẵn sàng) → thoát vòng. Trong lúc chờ kết quả agent mà không còn việc gì → `wait`, KHÔNG poll.

> **Gộp lượt** — mỗi lượt orchestrator đọc lại toàn bộ context, nên chi phí ≈ số lượt × độ dài context. Lời gọi không phụ thuộc kết quả nhau đi chung 1 lượt: nhiều task xong cùng lúc → `engine_verify` của cả nhóm 1 lượt; `engine_advance` → bookkeeping STATE/PLAN + `engine_ready` cùng lượt. KHÔNG đọc lại file vừa ghi, KHÔNG `cat` log dài (lấy dòng tổng kết), KHÔNG đọc nguyên báo cáo của agent khi chỉ cần verdict.

> Task lớn/rủi ro (sửa nhiều file lõi) → **`engine_ready {limit, worktree: true}`**: mỗi task một worktree + nhánh riêng (đường dẫn trong output — prompt dev phải ghi "chỉ sửa trong <path>"), `engine_verify` chạy trong worktree, `engine_advance` tự commit ở đó, cho reviewer độc lập chấm diff, chỉ PASS mới squash-merge vào nhánh hiện tại và chạy lại verify trên cây đã merge trước khi commit (FAIL → trả lỗi, task về chưa verify → dev lại; cùng diff không được chấm lại; checkout chính có file staged → từ chối). Không cần truyền `files`. Cả slice → **`engine_worktree`** (open → work → merge squash → remove).

## Bước 3 — Bookkeeping mỗi task done (engine KHÔNG quản)

- Append `agents/planning/<slug>/STATE.md` `## Log`: `- DATE M<x> T<n> ✓ <việc> [file] (<hash ngắn>)`.
- Tick checkbox task trong `M<x>-NN-PLAN.md` (`[ ]` → `[x]`).
- Cập nhật STATE `Current Position` + `next_action` (task đang chạy / còn lại).
- **Guardrail R6**: việc đang làm phải thuộc `active_phase`. Lệch ROADMAP/Decision → DỪNG, hỏi user (auto: SKIP + NEEDS-CONFIRM).

## Commit per-task (R8)

- **Verify-gate trước commit**: chỉ commit sau `engine_verify` PASS (`engine_advance` từ chối task chưa verify).
- **Chỉ commit file của task** — luôn truyền `files` cho `engine_advance`; engine từ chối commit cả tree khi task khác đang chạy.
- **Branch**: mặc định nhánh hiện tại. Có `STATE.branch` → verify `git branch --show-current` khớp trước khi commit; lệch → DỪNG.
- **Message tiếng Anh**: `feat: M2 - T1 sync group-info onto the group record`. `type` ∈ feat/fix/docs/refactor. KHÔNG `[<Category>]` prefix, no AI ref.
- **KHÔNG `git push` / PR** trừ khi user yêu cầu. Ghi commit hash vào STATE.Log.

## `--auto` (autonomous) — cùng vòng, không user-gate

`/feature go --auto` = đúng vòng trên chạy tự động (mirror kỷ luật `/auto` trong `~/.omp/agent/commands/auto.md`), **scoped 1 phase**, dừng ở ranh giới phase kế:
- Không yield giữa các task; chạy tới khi vòng điều phối thoát.
- Task block (dữ liệu/môi trường) → SKIP + NEEDS-CONFIRM vào VERIFICATION/STATE, nhánh khác chạy tiếp.
- Ranh giới an toàn: KHÔNG push/merge ra ngoài, KHÔNG xoá data, KHÔNG gọi API production gửi tin thật — trừ khi user đã duyệt ở plan. Chi tiết: `references/auto.md`.

## Khi hết task của phase

- **Chạy test đầy đủ 1 lần** (cả suite/lint của các module phase đụng) — bắt lỗi tương tác giữa các task mà verify scoped không thấy. Lỗi → xác định task gây lỗi → đưa task đó về dev (vòng Bước 2).
- STATE: `status: executing`, `next_action: ship-phase`.
- **Self-check**: mọi UC trong Requirement scope có task phủ; mọi AC có code thỏa; có đặc tả ngoài → mọi mã ở cột Nguồn của AC có chỗ cài trong code (đọc lại nguyên văn, không đoán từ tên hàm). Thiếu → làm nốt trước khi ship.
- KHÔNG review/test sâu theo AC ở đây — đó là việc `/feature ship`.

Next: `/feature ship`.

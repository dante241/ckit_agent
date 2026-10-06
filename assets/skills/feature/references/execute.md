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

## Bước 1 — `engine_plan`: nạp phase vào engine

Gọi **`engine_plan`** một lần:
- `goal` = literal 🎯 **Goal** của phase.
- `slices` = 1 slice (tên phase) — thứ tự chạy do `depends` quyết, không do slice.
- Mỗi task: `key` = `T<n>`, `title` = mô tả task, `depends` = `[depends:]` (key), `files` = `[file:]`, `verify` = `[verify:]` (lệnh THẬT).

Resume (đã có plan engine) → `engine_status`, KHÔNG plan lại.

## Bước 2 — Vòng điều phối (dev ∥ test, lặp tới hết)

Trạng thái mỗi task: `pending → dev → test → done`, hoặc `test → dev` (test lỗi), hoặc `blocked`.
Giới hạn số task dev đồng thời: `config.workflow.max_parallel_tasks` (thiếu → 4). `parallelization === false` → giới hạn = 1 (tuần tự, cùng vòng lặp).

Lặp:

1. **`engine_ready {limit: max_parallel_tasks}`** → engine trả mọi task đủ phụ thuộc + không chồng file với task đang chạy (tổng số task đang chạy không vượt `limit`), và đánh dấu `in_progress`. Task test lỗi vẫn giữ `in_progress` (giữ chỗ file) trong lúc dev lại.
2. **Spawn dev** cho các task sẵn sàng (tới giới hạn) — **1 lời gọi `task`, nhiều item**, `agent: task`, chạy nền (kết quả tự về, orchestrator không chờ từng cái). Prompt mỗi agent BẮT BUỘC nhúng (subagent KHÔNG đọc được CONTEXT/config/skill):
   1. Task cụ thể + **danh sách file được phép sửa** + "chỉ làm task này, KHÔNG commit, KHÔNG chạy cả test suite".
   2. **UC literal** task phục vụ.
   3. **AC literal** task gánh (Given/When/Then nguyên văn).
   4. **Decisions liên quan** (copy literal — KHÔNG ghi "theo D4").
   5. **Skill (cột `[skill:]`) — 2 lớp:** (a) nhúng luật cốt lõi + anti-pattern; (b) ra lệnh Read `.omp/skills/<skill>/SKILL.md` (không có thì `~/.omp/skills/<skill>/SKILL.md`) TRƯỚC khi code.
   6. Convention: `AGENTS.md` + `agents/DECISIONS.md`/`PREFERENCES.md` liên quan.
   7. Ground-truth (schema/symbol thật) + **interface mà task phụ thuộc đã commit** (symbol, file:line).
   8. **R10 literal** (xem `feature-rules.md` R10 (a)).
   9. Lệnh `[verify:]` của task — agent tự chạy trước khi báo xong.
   10. "Báo cáo cuối: danh sách file đã sửa/tạo + kết quả lệnh verify."
   Lần dev lại sau test lỗi: nhúng thêm **output lỗi nguyên văn** + "sửa NGUYÊN NHÂN, không lặp cách sửa cũ".
3. **Dev xong 1 task → test ngay** (các agent khác vẫn đang dev):
   - **Kiểm phạm vi** — các agent dùng CHUNG working tree, nên `git status --porcelain` có cả file của task khác đang dev. File ngoài phạm vi của task = (file agent báo ∪ file mới thay đổi trong `git status --porcelain`) − `[file:]` của task − `files` của các task còn `in_progress` (liệt kê trong output `engine_ready`/`engine_status`) − file đã bẩn sẵn trước khi `go` bắt đầu (chụp `git status --porcelain` 1 lần lúc vào Bước 2). Còn file nào → coi như test lỗi, nêu rõ file. Agent báo đã sửa file nằm trong `files` của task khác → cũng là lỗi (đè việc task đó).
   - **`engine_verify {taskId}`** — gate chạy đúng `[verify:]`.
4. **Kết quả test:**
   - **PASS** → **`engine_advance {taskId, commit:true, files:[đúng các file agent báo đã sửa/tạo], message:"<type>: M<x> - T<n> <English desc>"}`** — engine chỉ stage + commit các file đó (R8). KHÔNG truyền thư mục của `[file:]` (task khác có thể đang tạo file trong cùng thư mục). → Bookkeeping (Bước 3) → quay lại 1 (task phụ thuộc vừa được mở khóa).
   - **FAILED** → spawn lại dev cho task đó (mục 2, kèm output lỗi). Engine đếm retry: 2 lỗi giống nhau = cảnh báo, 3 hoặc chạm `maxRetries` = BLOCKED.
   - **BLOCKED** → ghi `failure:` vào `agents/KNOWLEDGE.md`; engine tự block mọi task phụ thuộc nó (báo trong output); các nhánh khác chạy tiếp.
5. **`engine_ready` báo DONE** (không task nào đang chạy hoặc sẵn sàng) → thoát vòng. Trong lúc chờ kết quả agent mà không còn việc gì → `wait`, KHÔNG poll.

> Task lớn/rủi ro (sửa nhiều file lõi) → cô lập bằng **`engine_worktree`** (open → work → merge squash → remove).

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
- **Self-check**: mọi UC trong Requirement scope có task phủ; mọi AC có code thỏa. Thiếu → làm nốt trước khi ship.
- KHÔNG review/test sâu theo AC ở đây — đó là việc `/feature ship`.

Next: `/feature ship`.

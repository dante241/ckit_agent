# feature-rules — Luật xuyên suốt MỌI subcommand

> **Load FILE NÀY ĐẦU TIÊN ở mọi subcommand** (`new`/`plan`/`go`/`ship`/`--auto`), TRƯỚC khi làm việc. Với subcommand khác `new`: đọc ACTIVE+STATE+config trước rồi load file này. Riêng `new`: chưa có ACTIVE/STATE thì đọc config nếu có, load file này, rồi scaffold ACTIVE/STATE.
> Đây là "hợp đồng luôn áp" — gom các luật rải rác trong SKILL.md và từng reference. Reference khác chỉ thêm bước RIÊNG của subcommand, KHÔNG lặp lại luật ở đây.
>
> **Đã lược cho 8sync (repo-agnostic):** R2 (routing model 4-slot của Claude-Code), R4 (audit brace ngôn ngữ cụ thể), R9 (cập nhật ticket quản-lý-dự-án ngoài), R11 (comment convention gắn ticket-ID) đã bị BỎ HẲN — chúng gắn chặt vào 1 repo/1 ngôn ngữ và không portable. R-number của các luật còn lại GIỮ NGUYÊN để cross-reference cũ vẫn resolve.

## R1 — config.X là chỉ thị cho ORCHESTRATOR, resolve về literal trước khi dùng

`config.X` (đọc từ `agents/planning/config.json`) là tham số cho orchestrator (main thread), **KHÔNG phải chuỗi đưa cho subagent**. Subagent KHÔNG đọc được config.json và KHÔNG kế thừa context — nó chỉ thấy prompt bạn soạn. Vì vậy:
- **Vai/role** → orchestrator tự chọn `agent: <role>` khi spawn `task` (xem bảng **Agent theo vai** dưới). Subagent không cần biết role của chính nó; model do omp chọn qua `~/.config/8sync/models.toml`.
- **Tham số nội dung** (workflow.review_dimensions, tier, convention, ticket…) → nhúng **giá trị thật** vào prompt (vd "review dimension: security"). TUYỆT ĐỐI không viết chữ `config.workflow.review_dimensions` vào prompt subagent.
- Thiếu key → dùng default rồi cảnh báo user.

**Agent theo vai** (agent dự án ở `.omp/agents/` ưu tiên hơn bản có sẵn của omp; không có → dùng cột dự phòng):

| Vai | Agent | Dự phòng |
|---|---|---|
| Khảo sát code (plan, auto-discuss) | `scout` | — |
| Thư viện/API bên ngoài | `librarian` | `scout` + `web_search` |
| Cân nhắc phương án, viết code (`go`) | `task` | — |
| Review plan | `reviewer` | — |
| Review code (`ship`) | `code-reviewer` | `reviewer` |
| Viết test (`ship`) | `tester` | `task` + skill `testing` |
| Bảo mật (PR đụng quyền/input) | `security-reviewer` | `reviewer` |

## R3 — Load skill repo theo cột [skill:] (2 lớp, BẮT BUỘC)

Subagent KHÔNG tự biết skill nào áp dụng và KHÔNG đọc được session context. Với MỖI skill ghi ở cột `[skill:]` của task:
1. **Orchestrator đọc `.omp/skills/<skill>/SKILL.md`** (project-local) hoặc `~/.omp/skills/<skill>/SKILL.md` (global) + references liên quan TRƯỚC khi code/spawn — KHÔNG mirror file có sẵn rồi suy đoán convention (mirror dễ trật; chỉ đọc SKILL.md mới biết anti-pattern thật).
2. **Khi spawn**, prompt subagent phải: (a) nhúng **literal luật cốt lõi + anti-pattern** của skill; (b) ra lệnh subagent **Read `.omp/skills/<skill>/SKILL.md` (hoặc `~/.omp/skills/<skill>/SKILL.md`) TRƯỚC khi code** và tuân theo. Hai lớp bù nhau.

Task ghi `[skill: —]` mà vẫn là task code → DỪNG, xác minh thật sự không skill nào chi phối (đa số task code có ≥1 skill).

## R5 — AC là hợp đồng nghiệm thu xuyên phase

Mỗi phase có 🎯 Goal + ✅ Acceptance Criteria (AC-NN, đo được) trong `M<x>-CONTEXT.md`. AC là nguồn chân lý: `plan` viết, `go` code bám, `ship` verify từng AC → `M<x>-VERIFICATION.md`. Phase done ⇔ MỌI AC PASS (NEEDS-CONFIRM → `ship.md` Step 2.6). Mọi AC PHẢI map về ≥1 UC trong `REQUIREMENTS.md`; mọi task PLAN truy được về ≥1 AC + ≥1 UC; mọi AC có ≥1 task thỏa. KHÔNG dùng "DoD" mơ hồ.

## R6 — Guardrail chống "đi 1 nẻo"

- Việc đang code phải thuộc `active_phase`. Lệch ra ngoài ROADMAP → DỪNG, hỏi user (auto-mode: SKIP+NEEDS-CONFIRM).
- Lệch Decision trong CONTEXT → DỪNG (như lệch ROADMAP).
- Traceability bắt buộc: `REQUIREMENTS.md` UC → `M<x>-CONTEXT.md` Requirement scope → AC-NN (cột Nguồn) → PLAN task → go prompt → `M<x>-VERIFICATION.md`. Có đặc tả ngoài thì đầu chuỗi là mã đặc tả qua `COVERAGE.md` (R12). Code task mà không biết UC nào đang phục vụ = DỪNG, bổ sung trace trước.
- STATE.md < 100 dòng, digest không archive. Cập nhật: task xong → STATE.Log + next_action; phase xong → ROADMAP tick + progress.

## R7 — Neo vào codebase (brownfield)

- Tổng thể: `AGENTS.md` + `agents/PROJECT.md` — KHÔNG mô tả lại.
- Nghiệp vụ/kiến trúc: `agents/KNOWLEDGE.md` + codebase-memory-mcp (`xd://mcp__codebase_memory_mcp_get_architecture`, `xd://mcp__codebase_memory_mcp_search_graph`) — tra trước khi code module.
- Convention + quyết định: `AGENTS.md` + `agents/DECISIONS.md` + `agents/PREFERENCES.md`.
- Không mô tả lại thứ đã có trong các nguồn trên; trích dẫn (vd "theo `agents/DECISIONS.md` đã chốt X").

## R8 — Commit model riêng của feature

Commit **atomic mỗi task xong** trong `go`: sau `engine_verify` PASS → `engine_advance {taskId, commit:true, files:[đúng các file task đã sửa/tạo], message}` — engine chỉ stage + commit các file đó (KHÔNG `git add -A`, vì task khác đang dev song song; engine từ chối commit cả tree khi còn task khác đang chạy). Self-report "xong" KHÔNG phải tín hiệu dừng. Message theo Conventional Commits, **tiếng Anh**, milestone/task ở ĐẦU: `<type>: M<x> - T<n> <English description>` (`type` ∈ feat/fix/docs/refactor; KHÔNG `[<Category>]` prefix), no AI ref.
- **Feature branch + ticket là TUỲ CHỌN** (`STATE.branch`/`STATE.ticket` có thể trống — KHÔNG ép tạo nhánh/ticket ở `new`). Nếu user muốn 1 feature branch lớn → verify `git branch --show-current` khớp `STATE.branch` trước khi commit.
- **KHÔNG `git push` / mở PR** trừ khi user yêu cầu rõ (convention 8sync). Commit local làm checkpoint.
- Ghi commit hash vào STATE.Log dòng task để `ship`/revert truy ngược.

## R10 — Code-intelligence FIRST (mọi lookup code, ÁP DỤNG CẢ SUBAGENT — bắt buộc, không tuỳ chọn)

Mọi thao tác TÌM/HIỂU/ĐỊNH VỊ code (không phải sắp EDIT ngay) → dùng code-intelligence engine TRƯỚC grep/Read thô, theo đúng RULE #0 (`~/.omp/agent/APPEND_SYSTEM.md`). Áp dụng cho **CẢ main thread LẪN MỌI subagent** (`scout` ở `plan.md` Step 2, `task` executor ở `execute.md`, reviewer/tester ở `ship.md`, discuss subagent ở `auto.md`). Ưu tiên:

MCP tool là **`xd://` device**, KHÔNG phải top-level tool: gọi bằng `write` JSON args vào path (vd `write` path `xd://mcp__codebase_memory_mcp_search_graph`, content `{"project":"…","query":"…"}`); tool chưa rõ schema → `read xd://<tool>` trước.

1. **codegraph** — `xd://mcp__codegraph_explore` (1 call = source + call path + blast radius) hoặc CLI `codegraph query/callers/callees/impact "<symbol|query>"`. Skill: `~/.omp/skills/codegraph/SKILL.md`.
2. **codebase-memory-mcp**: `xd://mcp__codebase_memory_mcp_search_graph` (tham số `semantic_query`), `…_trace_path`, `…_get_architecture`, `…_detect_changes`, `…_query_graph`, `…_get_code_snippet` (tiền tố `xd://mcp__codebase_memory_mcp_`). Server chưa connected → dùng codegraph, KHÔNG loay hoay grep.
3. **serena** (LSP): `xd://mcp__serena_find_symbol`, `xd://mcp__serena_find_referencing_symbols`, `xd://mcp__serena_get_symbols_overview` để định vị + `xd://mcp__serena_replace_symbol_body` để sửa symbol-level. Chỉ `read` raw file khi SẮP SỬA nó (read-before-edit) — KHÔNG dùng read/grep để survey.
4. Output lớn (>~300 dòng: log/diff/test dump/kết quả research) → tóm tắt phần liên quan TRƯỚC khi đưa vào context/báo cáo — không dump thô.

**Subagent KHÔNG tự biết luật này** (không đọc APPEND_SYSTEM, không kế thừa session context — chỉ thấy prompt bạn soạn, giống R3). Khi spawn BẤT KỲ subagent nào cần tìm/hiểu code, prompt BẮT BUỘC nhúng 2 lớp:
- (a) **Chỉ thị literal**: "MCP tool là `xd://` device — gọi bằng `write` JSON args vào path. Dùng `xd://mcp__codegraph_explore` / CLI `codegraph query/callers/callees/impact \"<query>\"` hoặc codebase-memory-mcp (`xd://mcp__codebase_memory_mcp_search_graph` / `_trace_path` / `_get_architecture`) / serena (`xd://mcp__serena_find_symbol`) để tìm/hiểu/định vị code TRƯỚC — KHÔNG grep/read thô để khảo sát. Chỉ read file khi sắp sửa đổi nó. Nếu output/log/test result dài (>300 dòng), tóm tắt phần liên quan trong báo cáo cuối, không dump thô."
- (b) Nếu subagent type có quyền đọc skill (đa số có tool Read) → thêm: "Đọc `~/.omp/skills/codegraph/SKILL.md` nếu cần chi tiết cách dùng."

Vi phạm (subagent grep/Read tràn lan để survey thay vì code-intel, hoặc dump log thô >300 dòng vào báo cáo) = lệch quy tắc dự án, không phải style nit — sửa ngay khi phát hiện, không đợi review pass mới bắt.

## R12 — Nguồn yêu cầu: đặc tả ngoài là gốc, kế hoạch chỉ trỏ về nó

`REQUIREMENTS.md` ghi dòng **Nguồn yêu cầu** (đặt ở `new`):
- **Không có đặc tả ngoài** → nguồn là chính `REQUIREMENTS.md`. Cột Nguồn của AC ghi `REQUIREMENTS`. Không cần `COVERAGE.md`.
- **Có đặc tả ngoài** (file BA giữ: UC, BR, luồng, tiêu chí US·AC, mã kỹ thuật) → đặc tả là gốc, nằm trong `agents/planning/<slug>/` (giữ nguyên tên file, không chép nội dung sang file khác). Bắt buộc có `COVERAGE.md` (mỗi mã đặc tả 1 dòng: mã → phase → trạng thái) và gate `python3 ~/.omp/skills/feature/scripts/coverage.py <feature-dir>` PASS.

Khi có đặc tả ngoài, chuỗi trace là: **mã đặc tả → COVERAGE (phase) → M<x>-CONTEXT AC (cột Nguồn) → PLAN task → prompt dev (đoạn đặc tả nguyên văn) → M<x>-VERIFICATION (cột Nguồn)**. Danh sách mã của 1 phase lấy bằng `coverage.py <feature-dir> --phase Mx` — KHÔNG chọn tay (chọn tay đã từng sót BR quan trọng). Mã chỉ có nguyên văn trong đặc tả: ai cần (plan, dev, reviewer, tester) phải nhận đoạn nguyên văn, không nhận bản viết lại.

## R13 — Chèn/đổi phase giữa chừng

Thêm phase mới (vd `MC` giữa M1 và M2) hoặc đổi phạm vi phase đã plan → làm đủ, cùng một lượt:
1. `ROADMAP.md`: dòng phase + phụ thuộc + **Integration Contracts** — contract cũ bị thay thì đánh dấu "bị thay bởi M<y>" (không xoá), ghi contract mới.
2. `COVERAGE.md`: gán lại cột Phase cho các mã phase mới gánh; chạy lại `coverage.py`.
3. `REQUIREMENTS.md`: UC/NFR/out-of-scope bị ảnh hưởng; dòng "Đọc bắt buộc" của phase mới (sinh bằng `--phase`).
4. `STATE.md`: `next_phases`, `progress.total_phases`, `percent`.
5. `PROJECT.md` Key Decisions: 1 dòng ghi lý do chèn, cột "Ai quyết".
6. Phase đã ship mà phase mới đổi phần nó giao → ghi rõ AC nào của phase cũ phải giữ PASS sau thay đổi (thành AC hồi quy của phase mới).

## R14 — Đặc tả đổi (BA sửa) giữa chừng

`REQUIREMENTS.md` ghi bản/ngày đặc tả đang dùng. BA gửi bản mới → (1) diff với bản cũ (giữ bản cũ đến khi đối soát xong); (2) mã mới/đổi/bỏ → cập nhật `COVERAGE.md`, chạy `coverage.py`; (3) mã đổi thuộc phase đã plan → sửa CONTEXT/PLAN của phase đó; thuộc phase đã ship → thành AC của phase kế hoặc task converge, hỏi user; (4) cập nhật bản/ngày. Không plan/ship phase mới trên đặc tả chưa đối soát.

## R15 — Quyết định của user ≠ đề xuất của AI

Key Decisions (`PROJECT.md`) và Decisions trong CONTEXT ghi cột/nhãn **Ai quyết**: `user` / `đề xuất` / `auto`. Chỉ ghi `user` khi user chốt rõ trong hội thoại hoặc qua `ask` — AI đề xuất mà user chưa trả lời = `đề xuất`. Dòng `đề xuất`/`auto` không phải ràng buộc: chạm tới thì trình lại user (auto-mode: giữ nguyên + NEEDS-CONFIRM).

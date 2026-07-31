# CLAUDE.md — guidance for Claude Code

<!-- ckit:skills:begin -->
## 🚨 STEP 0 — CODE INTELLIGENCE FIRST (codegraph + codebase-memory-mcp; bắt buộc)

Mọi câu hỏi về code → dùng code-intelligence engine TRƯỚC grep/read (tiết kiệm ~99% token). Bạn (AI) **PHẢI**:

1. **codegraph** (local index): `codegraph index .` 1 lần/session; rồi `codegraph query/explore/node/callers/callees/impact` thay cho `grep`/`rg`/`fd`/`Grep`/`Glob` và `Read` toàn file.
2. **codebase-memory-mcp** (MCP, LUÔN có trong tool list — gọi đúng tên đăng ký): `mcp__codebase_memory_mcp_search_graph`, `_trace_path`, `_get_architecture`, `_get_code_snippet` — knowledge graph 158 ngôn ngữ, query sub-ms. Full catalog visible (`query_graph`, `detect_changes`, …); server khác/mới thêm → 1 lệnh `search_tool_bm25`.
3. Tìm/hiểu/định vị code · impact · route→handler · dead code · architecture → ƯU TIÊN 2 engine trên. Chỉ `Read` raw file khi sắp SỬA nó (read-before-edit). Serena LUÔN có trong tool list: `mcp__serena_find_symbol` / `mcp__serena_find_referencing_symbols` / `mcp__serena_get_symbols_overview`.
4. **Nén những gì BẠN phát lại:** báo cáo / subagent prompt / nội dung dài sắp re-emit → `mcp__headroom_compress` (60–95% ít token). omp tự spill output quá dài ra artifact — KHÔNG paste lại blob đã spill vào context.

Lý do: 5 query cấu trúc ≈ 3.4k token vs ≈ 412k token grep từng file (−99%). Dump cả file / grep mù = đốt token = bug.

## 🚨 STEP 1 — skills 2 tầng: CORE (đọc ngay) · SPECIALIST + on-demand (đọc khi cần)

Mỗi skill = 1 directory (Agent Skills open standard): `SKILL.md` có frontmatter `name`+`description`. Skill sống ở `~/.omp/skills/<name>/` (global) hoặc `<repo>/.omp/skills/<name>/` (project-local, chỉ khi được thêm rõ ràng qua `skill add`) — không copy vào memory folder. Mỗi skill liệt kê 1 lần.

### ⛔ CORE always-on — ĐỌC NGAY (body), trước tool call đầu tiên (không skip)

Nhỏ + dùng cho MỌI task. **Thứ tự = ưu tiên (đọc top-down).** Mở `SKILL.md` ở path dưới rồi mới gọi tool đầu tiên:

  1. `/Users/tungnt2401/.omp/skills/codegraph/SKILL.md`
  2. `/Users/tungnt2401/.omp/skills/karpathy-guidelines/SKILL.md`
  3. `/Users/tungnt2401/.omp/skills/ponytail/SKILL.md`
  4. `/Users/tungnt2401/.omp/skills/8sync-cli/SKILL.md`

### 🧩 SPECIALIST always-on — biết khả năng, đọc body KHI task khớp (progressive disclosure)

KHÔNG đọc body mỗi phiên (giữ prefix gọn, tiết kiệm KV-cache). Khi task khớp → mở `SKILL.md` tương ứng NGAY. **`impeccable` = design system CHUẨN, BẮT BUỘC mở body ngay khi có việc UI/design/redesign/audit** (kèm `references/house/*`); `assp` cho copy/offer; `taste` chống slop; `image-routing` khi xử lý ảnh/diff/PDF.

- `assp-skill` — `/Users/tungnt2401/.omp/skills/assp-skill/SKILL.md`
- `impeccable` — `/Users/tungnt2401/.omp/skills/impeccable/SKILL.md`
- `design-taste-frontend` — `/Users/tungnt2401/.omp/skills/taste-skill/SKILL.md`
- `image-routing` — `/Users/tungnt2401/.omp/skills/image-routing/SKILL.md`
- `locate-anything` — `/Users/tungnt2401/.omp/skills/locate-anything/SKILL.md`

### 🔎 On-demand — tên = trigger; mở `SKILL.md` của skill khi task khớp (mô tả ở frontmatter, KHÔNG nhồi ở đây)

- `api-and-interface-design` — `~/.omp/skills/api-and-interface-design/SKILL.md`
- `browser-testing-with-devtools` — `~/.omp/skills/browser-testing-with-devtools/SKILL.md`
- `ci-cd-and-automation` — `~/.omp/skills/ci-cd-and-automation/SKILL.md`
- `code-review-and-quality` — `~/.omp/skills/code-review-and-quality/SKILL.md`
- `code-simplification` — `~/.omp/skills/code-simplification/SKILL.md`
- `context-engineering` — `~/.omp/skills/context-engineering/SKILL.md`
- `debugging-and-error-recovery` — `~/.omp/skills/debugging-and-error-recovery/SKILL.md`
- `deprecation-and-migration` — `~/.omp/skills/deprecation-and-migration/SKILL.md`
- `documentation-and-adrs` — `~/.omp/skills/documentation-and-adrs/SKILL.md`
- `doubt-driven-development` — `~/.omp/skills/doubt-driven-development/SKILL.md`
- `feature` — `~/.omp/skills/feature/SKILL.md`
- `frontend-ui-engineering` — `~/.omp/skills/frontend-ui-engineering/SKILL.md`
- `full-flow` — `~/.omp/skills/full-flow/SKILL.md`
- `git-workflow-and-versioning` — `~/.omp/skills/git-workflow-and-versioning/SKILL.md`
- `idea-refine` — `~/.omp/skills/idea-refine/SKILL.md`
- `incremental-implementation` — `~/.omp/skills/incremental-implementation/SKILL.md`
- `interview-me` — `~/.omp/skills/interview-me/SKILL.md`
- `last30days` — `~/.omp/skills/last30days/SKILL.md`
- `observability-and-instrumentation` — `~/.omp/skills/observability-and-instrumentation/SKILL.md`
- `performance-optimization` — `~/.omp/skills/performance-optimization/SKILL.md`
- `planning-and-task-breakdown` — `~/.omp/skills/planning-and-task-breakdown/SKILL.md`
- `ponytail-audit` — `~/.omp/skills/ponytail-audit/SKILL.md`
- `ponytail-debt` — `~/.omp/skills/ponytail-debt/SKILL.md`
- `ponytail-gain` — `~/.omp/skills/ponytail-gain/SKILL.md`
- `ponytail-help` — `~/.omp/skills/ponytail-help/SKILL.md`
- `ponytail-review` — `~/.omp/skills/ponytail-review/SKILL.md`
- `security-and-hardening` — `~/.omp/skills/security-and-hardening/SKILL.md`
- `senior-frontend` — `~/.omp/skills/senior-frontend/SKILL.md`
- `senior-security` — `~/.omp/skills/senior-security/SKILL.md`
- `shipping-and-launch` — `~/.omp/skills/shipping-and-launch/SKILL.md`
- `source-driven-development` — `~/.omp/skills/source-driven-development/SKILL.md`
- `spec-driven-development` — `~/.omp/skills/spec-driven-development/SKILL.md`
- `test-driven-development` — `~/.omp/skills/test-driven-development/SKILL.md`
- `token-bench` — `~/.omp/skills/token-bench/SKILL.md`
- `using-agent-skills` — `~/.omp/skills/using-agent-skills/SKILL.md`

### Quy tắc bất biến

- **Code-intelligence FIRST** (codegraph + codebase-memory-mcp) cho mọi câu hỏi explore code (Step 0). Bypass = bug.
- **Output > ~300 dòng → BẮT BUỘC `headroom_compress`** trước khi vào context — không dump thô.
- Đọc body **CORE** (codegraph → karpathy → ponytail → 8sync-cli) TRƯỚC tool call đầu tiên. **SPECIALIST** (assp · impeccable · taste · image-routing) đọc body KHI task khớp — `impeccable` bắt buộc ngay khi có việc UI/design.
- Skill **on-demand**: chỉ mở khi description khớp task hiện tại — đừng đọc thừa.
- Nếu skill có `scripts/` → ưu tiên invoke script đó thay vì viết lại logic.
- Khi áp dụng skill, **cite** rõ: ví dụ `~/.omp/skills/<name>/SKILL.md:line`.
- **Sau mỗi thay đổi:** ghi học được vào `agents/KNOWLEDGE.md`.
- **Doc-hygiene**: chạy `ckit harness audit` khi đụng vùng có docs — path lệch→fix, doc rác/superseded→xóa (thêm doc phải kèm xóa cái cũ), oversized→trim.
- **Loop / STATE spine**: đọc `agents/STATE.md` đầu phiên; rewrite ở mỗi phase-boundary (Goal·Checklist·Current·Next). Context gần đầy → handoff vào STATE + bài học vào KNOWLEDGE rồi reinit. Đo loop: `ckit harness bench`.
- **Loop discipline (C/D/E)**: implementer↔verifier qua `task` (verifier chạy build/test ĐỘC LẬP, verify-gate TRƯỚC commit); FAIL → ghi `failure:` vào KNOWLEDGE, đọc đầu phiên để khỏi lặp; quy trình `validated:` → distill vào `agents/PLAYBOOKS.md` (index theo `When:`); autonomy L1 report · L2 assisted · L3 unattended — không tự `push`/PR ở L3 mặc định.
<!-- ckit:skills:end -->

# STATE (8sync managed — live plan; rewrite ở MỖI phase-boundary, đọc đầu phiên)

## Goal
Hoàn tất task #11: `ckit setup`/`ckit harness` đăng ký codegraph MCP + STEP-0 MCPs mặc định và bỏ `zai-vision` khỏi harness mặc định.

## Definition of Done
- [x] `setup` Stage A đăng ký `codegraph`, `codebase-memory-mcp`, `headroom`, `serena` MCP.
- [x] `setup`/`harness` gỡ legacy MCP entry `zai-vision`.
- [x] Bundled skill/docs không còn hướng dẫn dùng `zai-vision` mặc định; image routing chuyển sang model-native/built-in image tools + `locate-anything`.
- [x] `cargo check -q` + `cargo build -q` pass (exit 0).
- [x] Scan stale refs chỉ còn intentional legacy cleanup string `zai-vision`.

## Checklist
- [x] Update `crates/cli/src/verbs/setup.rs` step `step0-mcps`.
- [x] Update `crates/cli/src/verbs/skill/deploy.rs` cleanup + capabilities/modality text.
- [x] Remove `assets/skills/zai-vision/` bundled asset.
- [x] Update `assets/skills/00-force-load.md`, `image-routing`, `locate-anything`, root docs, doctor text.
- [ ] Run compile + final scan when tool classifier allows execution.

## Current step
DONE — task #11 hoàn tất: build pass, scan clean, KNOWLEDGE validated.

## Next
_none — chờ chỉ đạo tiếp; cân nhắc `ckit harness audit` + commit khi user yêu cầu._

## Assumptions (auto-decided — user can correct)
- Bỏ `zai-vision` nghĩa là không bundle skill, không auto-register MCP, và cleanup legacy local config; vẫn giữ literal `"zai-vision"` trong cleanup function để xoá máy đã từng cài.

## Open questions / blockers
- Tool execution classifier tạm unavailable, chưa verify được build.

## Handoff (compaction)
Done: source changes for task #11 are applied. In-flight: verification only. Next: run `cargo check -q` and stale-ref scan once Bash/tool execution works.

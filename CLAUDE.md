# CLAUDE.md — guidance for Claude Code

<!-- ckit:skills:begin -->
## 🚨 Rules always-on → `~/.omp/agent/APPEND_SYSTEM.md`

Code-intel-first (codegraph · codebase-memory-mcp · serena qua `xd://` TRƯỚC grep/read) · memory + `agents/STATE.md` · loop C/D/E · doc-hygiene — **định nghĩa đầy đủ, KHÔNG lặp ở đây**: omp nhúng `~/.omp/agent/APPEND_SYSTEM.md` vào MỌI system prompt (không compact). Đọc file đó là luật bất biến.

## 🧩 Skills — CORE đọc ngay · SPECIALIST + on-demand đọc khi task khớp

Mỗi skill = 1 directory (Agent Skills open standard) có `SKILL.md` (frontmatter `name`+`description`). Project-local: `.omp/skills/<name>/`; global: `~/.omp/skills/<name>/`. Mỗi skill liệt kê 1 lần.

### ⛔ CORE always-on — mở `SKILL.md` NGAY, trước tool call đầu tiên (thứ tự = ưu tiên, đọc top-down)

  1. `/Users/tungnt2401/.omp/skills/codegraph/SKILL.md`
  2. `/Users/tungnt2401/.omp/skills/karpathy-guidelines/SKILL.md`
  3. `/Users/tungnt2401/.omp/skills/ponytail/SKILL.md`
  4. `/Users/tungnt2401/.omp/skills/8sync-cli/SKILL.md`

### 🧩 SPECIALIST always-on — biết khả năng, đọc body KHI task khớp

`impeccable` BẮT BUỘC mở body ngay khi có việc UI/design/redesign/audit (kèm `references/house/*`); `assp` copy/offer; `taste` chống slop; `image-routing` ảnh/diff/PDF.

- `assp-skill` — `/Users/tungnt2401/.omp/skills/assp-skill/SKILL.md`
- `impeccable` — `/Users/tungnt2401/.omp/skills/impeccable/SKILL.md`
- `design-taste-frontend` — `/Users/tungnt2401/.omp/skills/taste-skill/SKILL.md`
- `image-routing` — `/Users/tungnt2401/.omp/skills/image-routing/SKILL.md`
- `locate-anything` — `/Users/tungnt2401/.omp/skills/locate-anything/SKILL.md`

### 🔎 On-demand — tên = trigger; mở `SKILL.md` khi description khớp task

- `audit-and-force-omp-code-intel` — `.omp/skills/audit-and-force-omp-code-intel/SKILL.md`
- `audit-omp-codeintel-tool-usage` — `.omp/skills/audit-omp-codeintel-tool-usage/SKILL.md`
- `audit-omp-codeintel-usage` — `.omp/skills/audit-omp-codeintel-usage/SKILL.md`
- `audit-omp-context-files` — `.omp/skills/audit-omp-context-files/SKILL.md`
- `ckit-branding-cleanup` — `.omp/skills/ckit-branding-cleanup/SKILL.md`
- `ckit-bundle-omp-extension` — `.omp/skills/ckit-bundle-omp-extension/SKILL.md`
- `ckit-local-release-commit-tag` — `.omp/skills/ckit-local-release-commit-tag/SKILL.md`
- `ckit-rebrand-stale-scan` — `.omp/skills/ckit-rebrand-stale-scan/SKILL.md`
- `ckit-release-and-harness-config` — `.omp/skills/ckit-release-and-harness-config/SKILL.md`
- `ckit-release-cut` — `.omp/skills/ckit-release-cut/SKILL.md`
- `ckit-release-local-vs-online-update` — `.omp/skills/ckit-release-local-vs-online-update/SKILL.md`
- `ckit-seed-omp-config-key` — `.omp/skills/ckit-seed-omp-config-key/SKILL.md`
- `ckit-selfup-asset-guard` — `.omp/skills/ckit-selfup-asset-guard/SKILL.md`
- `cloudgo-ui-ux` — `~/.omp/skills/cloudgo-ui-ux/SKILL.md`
- `code-review-and-quality` — `~/.omp/skills/code-review-and-quality/SKILL.md`
- `context-engineering` — `~/.omp/skills/context-engineering/SKILL.md`
- `diagnose-ckit-mcp-resource-usage` — `.omp/skills/diagnose-ckit-mcp-resource-usage/SKILL.md`
- `diagnose-omp-mcp-tool-underuse` — `.omp/skills/diagnose-omp-mcp-tool-underuse/SKILL.md`
- `diagnose-omp-startup-context` — `.omp/skills/diagnose-omp-startup-context/SKILL.md`
- `feature` — `~/.omp/skills/feature/SKILL.md`
- `full-flow` — `~/.omp/skills/full-flow/SKILL.md`
- `last30days` — `~/.omp/skills/last30days/SKILL.md`
- `migrate-claude-code-subagents-to-omp` — `.omp/skills/migrate-claude-code-subagents-to-omp/SKILL.md`
- `omp-extension-footer-status` — `.omp/skills/omp-extension-footer-status/SKILL.md`
- `omp-footer-status-extension` — `.omp/skills/omp-footer-status-extension/SKILL.md`
- `omp-mcp-surface-tuning` — `.omp/skills/omp-mcp-surface-tuning/SKILL.md`
- `omp-mcp-tool-visibility` — `.omp/skills/omp-mcp-tool-visibility/SKILL.md`
- `omp-models-yml-api-url-builder` — `.omp/skills/omp-models-yml-api-url-builder/SKILL.md`
- `omp-rule-surface-routing` — `.omp/skills/omp-rule-surface-routing/SKILL.md`
- `omp-rules-engine-routing` — `.omp/skills/omp-rules-engine-routing/SKILL.md`
- `ponytail-audit` — `~/.omp/skills/ponytail-audit/SKILL.md`
- `ponytail-debt` — `~/.omp/skills/ponytail-debt/SKILL.md`
- `ponytail-gain` — `~/.omp/skills/ponytail-gain/SKILL.md`
- `ponytail-help` — `~/.omp/skills/ponytail-help/SKILL.md`
- `ponytail-review` — `~/.omp/skills/ponytail-review/SKILL.md`
- `route-ckit-pack-rules-to-omp-surfaces` — `.omp/skills/route-ckit-pack-rules-to-omp-surfaces/SKILL.md`
- `senior-frontend` — `~/.omp/skills/senior-frontend/SKILL.md`
- `senior-security` — `~/.omp/skills/senior-security/SKILL.md`
- `smoke-test-ckit-engine` — `.omp/skills/smoke-test-ckit-engine/SKILL.md`
- `token-bench` — `~/.omp/skills/token-bench/SKILL.md`
- `tune-omp-mcp-footprint` — `.omp/skills/tune-omp-mcp-footprint/SKILL.md`
- `ui-ux-pro-max-skill` — `~/.omp/skills/ui-ux-pro-max-skill/SKILL.md`
- `using-agent-skills` — `~/.omp/skills/using-agent-skills/SKILL.md`
- `verify-mnemopi-bank-cutover` — `.omp/skills/verify-mnemopi-bank-cutover/SKILL.md`
<!-- ckit:skills:end -->

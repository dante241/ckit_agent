---
name: force-load
description: Session bootstrap for this repo — code-intelligence-first rules, core skill load order (codegraph → karpathy-guidelines → ponytail → ckit-cli), mandatory PHP project rules, and the VTiger/PHP skill routing table. Read at session start, before the first tool call.
---

# Force Load Skills

## 🔴 RULE #0 — CODE INTELLIGENCE FIRST, ALWAYS

Before grep/find/Read for codebase questions, use code intelligence:

- **codegraph**: `codegraph index .` once per session, then `codegraph query/callers/callees/impact/explore` for symbols, flows, impact, route→handler, dead code, architecture.
- **codebase-memory-mcp**: use `search_graph`, `semantic_query`, `trace_path`, `get_architecture`, `detect_changes`, `query_graph`, `get_code_snippet` when available.
- Only `Read` raw files when about to edit them.
- output > ~300 lines must be compressed/summarized before entering context.

## ⛔ CORE — read body before first tool call

1. `.omp/skills/codegraph/SKILL.md` — semantic code intelligence.
2. `.omp/skills/karpathy-guidelines/SKILL.md` — read-before-write, test-before-refactor, small steps.
3. `.omp/skills/ponytail/SKILL.md` — YAGNI, minimum diff, delete over add.
4. `.omp/skills/ckit-cli/SKILL.md` — ckit verbs and VTiger/PHP routing.

## 📐 Project rules (MANDATORY khi code/review PHP)

Convention nền của codebase, deploy ở `.omp/rules/` (per-project, path-scoped):

- `.omp/rules/cloudgo-development-rules.md` — luôn áp dụng (file-separation, error-patterns).
- `.omp/rules/php-conventions.md` — **BẮT BUỘC** trước khi sinh/review PHP (Brace K&R, KHÔNG PSR-12 Allman; class `<Module>_<Component>_<Type>`; type-cast request data; soft-delete check).
- `.omp/rules/security-rules.md` · `performance-rules.md` — load khi chạm `**/*.php`.
- `.omp/rules/error-patterns.md` — catalog bug đã gặp; check trước khi code/review.
- `.omp/rules/{migration,api-connector,javascript,css,language}-rules.md` — path-scoped theo loại file.

## 🔎 On-demand VTiger/PHP skills

Read matching `SKILL.md` before touching code:

- `cook` — orchestrate full VTiger ticket/feature workflow.
- `feature` — new feature flow.
- `fix` — bug/root-cause flow.
- `report` — fixed/chart reports, filters, table structure, pagination.
- `database` — PearDatabase, schema, queries, joins, datetime, safe placeholders.
- `action` — Ajax/actions, JSON, `Vtiger_Response`.
- `view` — pages, Smarty templates, modals, controllers.
- `ui` — buttons, forms, frontend validation, modal UX.
- `field` — uitypes, picklists, EditView/DetailView/QuickCreate.
- `migration` — schema changes, install/migrate scripts.
- `module` — entities, Record_Model, Module_Model, helpers.
- `handler` — save/delete/link/unlink event handlers.
- `cron` — queues, background jobs, supervisord, batches.
- `integration` — webhook/API/inbound/outbound connectors.
- `callcenter` — phone/call log/telephony flows.
- `notification` — email/SMS/Zalo/FCM.
- `inventory` — SalesOrder/Invoice/Quote/line items/tax/currency.
- `language` — labels, `vtranslate`, `jsLanguageStrings`.
- `export` — CSV/Excel/PDF/downloads.
- `testing` — verification/regression.
- `commit`, `gitlab-mr`, `review-pr`, `release-check`, `release-bundle`, `release-core-doc`, `release-doc` — ship/release flow.
- `image-routing` — image/PDF/diff routing.
- `code-review-and-quality`, `senior-security`, `gs` — review/security/team loop support.

## Fast routing

| Task | Skills |
|---|---|
| report/chart/filter/table/BaseFixedReportHandler/pagination | `report` + `database` + `testing` |
| export/excel/csv/pdf/download | `export` + `testing` |
| ajax/json/action/Vtiger_Response | `action` + `error-handling` + `testing` |
| view/page/smarty/tpl/modal/template | `view` + `ui` + `language` |
| button/modal/form validation/frontend ajax | `ui` + `language` |
| field/uitype/picklist/EditView/DetailView/QuickCreate | `field` + `language` |
| SQL/PearDatabase/table/relation/datetime | `database` |
| schema/alter table | `migration` + `database` |
| module/entity/Record_Model/Module_Model/helper | `module` |
| cron/queue/supervisord/background/batch | `cron` |
| handler/aftersave/beforedelete/link/unlink | `handler` |
| integration/webhook/API connector | `integration` |
| callcenter/phone/telephony | `callcenter` |
| notification/email/SMS/Zalo/FCM | `notification` |
| SalesOrder/Invoice/Quote/line item/tax/currency | `inventory` |
| bug/fix/root cause | `fix` |
| feature/ticket workflow | `feature` or `cook` |
| release/MR | `release-check` + `gitlab-mr` + `review-pr` |

## Invariants

- Code intelligence first for code exploration.
- Core skills first: codegraph → karpathy → ponytail → ckit-cli.
- Use smallest working diff; no speculative abstraction.
- Cite code as `path:line` or `path:start-end`.
- Update `CHANGELOG.md` Unreleased + `agents/KNOWLEDGE.md` after meaningful changes.
- Update `agents/STATE.md` at phase boundaries.
- Verify before claiming done.

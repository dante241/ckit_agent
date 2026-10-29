---
name: force-load
description: VTiger/PHP skill routing table for this repo — which project skills to open for report/action/view/field/migration/cron/integration/release work. Bootstrap (core skills, always-on rules) lives in AGENTS.md + ~/.omp/agent/APPEND_SYSTEM.md; use this table after them, before touching code.
---

# Force Load — VTiger/PHP routing

**Bootstrap source of truth = `AGENTS.md`** (core skills `codegraph` → `karpathy-guidelines` → `ponytail` → `8sync-cli`, all global: `skill://<name>`) + `~/.omp/agent/APPEND_SYSTEM.md` (code-intel first, memory, STATE/KNOWLEDGE, doc-hygiene). This skill does not repeat them; it only routes VTiger/PHP tasks to project skills.

## Code intelligence first
- codegraph: `.codegraph/` already exists and the watcher keeps it current — `codegraph status` / `codegraph sync` if it looks stale (`codegraph index .` rebuilds from scratch; only when `.codegraph/` is missing). Query via `xd://mcp__codegraph_explore` or CLI `codegraph callers|callees|impact|node|query`.
- codebase-memory-mcp / serena via `xd://` for search, call paths, references.
- `read` raw files only when about to edit them.

## Project rules
`.omp/rules/` (path-scoped, auto-inject on edit; read them yourself when reviewing). Full list + globs: the rules table in `.omp/rules/cloudgo-development-rules.md` — don't maintain a second copy here. `php-conventions` (K&R braces, NOT PSR-12 Allman) and `error-patterns` are mandatory before writing/reviewing PHP.

## On-demand VTiger/PHP skills
Read the matching `skill://<name>` before touching code:

- `/cook` (command) — orchestrate the full VTiger ticket/feature workflow; `feature` — new feature flow.
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
- `error-handling` — exceptions, logging, permission denial.
- `testing` — verification/regression.
- `commit`, `gitlab-mr`, `review-pr`, `release-check`, `release-bundle`, `release-core-doc`, `release-doc` — ship/release flow.
- `image-routing` — image/PDF/diff routing.
- `code-review-and-quality`, `senior-security` — review/security support.

## Fast routing

| Task | Skills |
|---|---|
| report/chart/filter/table/BaseFixedReportHandler/pagination | `report` + `database` + `testing` |
| export/excel/csv/pdf/download | `export` + `testing` |
| ajax/json/action/Vtiger_Response | `action` + `error-handling` + `testing` |
| view/page/smarty/tpl/modal/template | `view` + `ui` + `language` + `testing` |
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
| feature/ticket workflow | `feature` or `/cook` |
| release/MR | `release-check` + `gitlab-mr` + `review-pr` |

## Invariants
- Smallest working diff; no speculative abstraction.
- Cite code as `path:line` or `path:start-end`.
- `agents/STATE.md`: read first, rewrite at phase boundaries; learnings → `agents/KNOWLEDGE.md` (`validated:` / `failure:`), per APPEND_SYSTEM.
- `CHANGELOG.md` Unreleased entry per change; other docs only when behaviour/setup/contract changes (`rule://documentation-management`).
- Verify before claiming done.

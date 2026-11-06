---
description: "PHP conventions — header, autoload, SQL/quote style, validate-once, blank-line/array style. Always-on cho code + review PHP (quyết định user 2026-09-10). Brace + attribution: cloudgo-development-rules § Style & Comments."
alwaysApply: true
---

# PHP Conventions

## File Header (Required)

Every new PHP file starts with `<?php`, a blank line, then one docblock: `@author Your Name`, `@email your.email@company.vn`, `@create date YYYY.MM.DD`. One header per file — never duplicate the `@author` block (fold tags into the existing class docblock).

## Brace Style

Shared rules: `cloudgo-development-rules` § Brace Style. Multi-line signature: `{` goes on the closing `): type {` line.

## Class Naming & Autoload

- Path: `modules/<Module>/<types>/<Component>.php` (models, views, actions, helpers, crons) — `GeminiAudit.php`, not `GeminiAuditHelper.php`.
- These classes are autoloaded — no `require_once`. `require_once` only for non-module paths (`include/utils/`, `include/Webservice/`, …) and report handlers in `modules/Reports/custom/` (plain class names).

## Database & SQL Conventions

- ALL queries use prepared statements (`pquery` + `?`), no string concatenation. Every JOIN on `vtiger_crmentity` includes `deleted = 0` in its ON clause.
- Plain strings: single quotes. Queries or strings with variables: double quotes, variables as `{$var}`.
- SQL keywords UPPERCASE; long queries: one clause (FROM, JOIN, WHERE, ORDER BY…) per line.
- JOIN conditions on the JOIN line in parentheses; WHERE holds only main-table conditions (except LEFT/RIGHT JOIN null checks). Very long query → split into smaller queries with PHP logic between.

## Type Casting

Cast request/external data when you assign it (`(int)`, `(string)`, `(float)`) — `php83` V3.

## PHP 8.3

Full standard (MUST): `rule://php83` (copy of `docs/php83-coding-rules.md`) — class layout C1-C10, typed params/returns F, checks before calling P, variables V, errors E; cite rule codes in review findings.

## Validate Once, Then Trust (user rule 2026-10-26)

Ensure a variable's type ONCE, where it is assigned (`?? []` / `is_array()` on data from request/DB/API); afterwards use it bare. After `array_map/array_filter/explode/array_keys` or `?? []`, call `count($x)` — never `count($x ?? [])`. Functions taking arrays declare `array $param`.

## Idioms

- `&&` short-circuit for conditional assignment is allowed (`!empty($id) && $x = …;`), still subject to php83 P checks.
- Datetimes: reject `''` and `'0000-00-00 00:00:00'` (`in_array($v, ['', '0000-00-00 00:00:00'], true)`).
- Entity writes via `set('mode', 'edit')` + `save()` or `updateData($data)->save()`; direct `pquery` UPDATE only on hot paths where save handlers are unnecessary.

## Empty Line Rules

Add a blank line: (1) before first method of class, (2) between methods, (3) before control structure (`if`/`switch`/`for`/`while`/`foreach`) when preceded by other statements, (4) after closing `}` when followed by other statements. Never double blank lines.

## Array Style

**NO aligned `=>`** — single space before and after `=>`.

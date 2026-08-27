---
description: "CSS/TPL conventions + file-separation (không inline CSS). Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(**/*.css), tool:write(**/*.css), tool:edit(**/*.tpl), tool:write(**/*.tpl)"
condition: ".*"
---

# CSS Conventions

> Loads only when editing CSS or TPL files.

## File Location

CSS **ALWAYS** lives at `modules/<Module>/resources/<ViewName>.css` — never inline in PHP/TPL. See File Separation Rules in `cloudgo-development-rules.md`.

## Class Naming — kebab-case

```css
.last-campaign-container { }
.last-campaign-link { }
.btn-warranty-check { }
```

## CSS Variables (use existing tokens)

```css
.primary-button {
    background-color: var(--primary-1);
    color: var(--white-1);
}
```

Common tokens: `--primary-1`, `--white-1`, `--gray-*`, `--success-1`, `--danger-1`. Check existing CSS files for the full palette before introducing new colors.

## Text Overflow Pattern

```css
.truncate {
    white-space: nowrap !important;
    overflow: hidden;
    text-overflow: ellipsis;
}
```

## Selectors

- Scope styles to a container class to avoid global leaks: `.cpchatbot-config .header { ... }`
- Avoid `!important` except for utilities (overflow, display:none) and overriding 3rd-party CSS
- No inline `style="..."` in TPL — move to CSS file

## TPL Reminders

- NO `<style>` blocks inside TPL — extract to `<Module>/resources/<View>.css`
- NO `<script>` blocks inside TPL — extract to JS controller and register via `getHeaderScripts()`
- Use `{$VARIABLE|escape}` (HTML escape) when rendering user-controlled data

## Comments & Modification Tracking

Same comment + attribution conventions as `php-conventions.md` (§ Modification Tracking Comments) — applied to CSS/TPL (CSS `/* ... */`, TPL `{* ... *}`):
- **Attribution by ownership.** Others' file → `/* Added by <Name> on <DATE> - <REASON> */` (editing → `Modified by`), close with `/* End <Name> */`; your own file → plain description or none. Owner = file's `Author:` header; no header → resolve via `git blame`, never assume you own it.
- **`<REASON>` in English**, regardless of the file's comment language.
- **≤ 2 lines per comment block; comment what the rule/section does** — not chat decisions/tradeoffs, and NO ticket / plan / phase refs.

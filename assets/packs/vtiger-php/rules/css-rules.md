---
paths:
  - "**/*.css"
  - "**/*.tpl"
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

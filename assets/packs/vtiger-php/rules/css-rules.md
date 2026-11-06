---
description: "CSS/TPL conventions + file-separation (không inline CSS). Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(**/*.css), tool:write(**/*.css), tool:edit(**/*.tpl), tool:write(**/*.tpl)"
condition: ".*"
---

# CSS Conventions

File location + no inline `<style>`/`<script>`: `cloudgo-development-rules.md` § File Separation Rules.

## CSS Variables

Use existing tokens (`--primary-1`, `--white-1`, `--gray-*`, `--success-1`, `--danger-1`); check existing CSS files for the full palette before introducing new colors.

## Selectors

- Scope styles to a container class to avoid global leaks: `.cpchatbot-config .header { ... }`
- Avoid `!important` except for utilities (overflow, display:none) and overriding 3rd-party CSS
- No inline `style="..."` in TPL — move to CSS file
- For filter/action bars using Bootstrap grid columns, size every item for its rendered content in the longest supported language. If buttons are kept on one line (`white-space: nowrap`), the button column must be wide enough at each breakpoint or the buttons must be allowed to wrap/stack.

## TPL

- Use `{$VARIABLE|escape}` (HTML escape) when rendering user-controlled data

## Comments

Attribution, English, ≤ 2 lines: `cloudgo-development-rules.md` § Modification Tracking Comments (CSS `/* */`, TPL `{* *}`).

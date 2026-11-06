---
description: "jQuery/JS conventions — header, controller, app.request, Vue naming, XSS. Always-on cho code + review JS (quyết định user 2026-09-10)."
alwaysApply: true
---

# jQuery / JavaScript Conventions

Brace, attribution (JSDoc) and comment rules: `cloudgo-development-rules` § Style & Comments.

## Block Spacing

One blank line between methods/functions, after the opening `{` of a class or large object literal, and between logical statement groups (setup → logic → return). No blank line right before a closing `}`/`)`.

## File Header

```javascript
/*
    FileName.js
    Author: Your Name
    Date: YYYY-MM-DD
    Purpose: Brief description
*/
```

## Controller Pattern

`Vtiger.Class()` controllers (naming: core § Naming Conventions) extend `Vtiger_List_Js` / `Vtiger_Edit_Js` / `Vtiger_Detail_Js` (core views) or `CustomView_BaseController_Js` (custom views). Parents define no `getContainer()` — the subclass returns its TPL root; cache it and other selectors at the top of `registerEvents()`, never re-query inside loops/callbacks.

## Key JavaScript Objects

AJAX via `app.request.post()` / `.get()` only (never `$.ajax` / `$.post` / `$.get`), callback `(error, data)`; UI via `app.helper.showProgress/hideProgress/showSuccessNotification/showErrorNotification/showModal/hideModal`; strings via `app.vtranslate('JS_…')`.

## Vue.js Data Properties

camelCase for client-only UI state and methods; snake_case only for keys mirroring a backend payload / DB column (requests keep backend casing). Never blanket-snake_case local state.

## Security

Render dynamic content with `.text()`, not `.html()` (XSS).

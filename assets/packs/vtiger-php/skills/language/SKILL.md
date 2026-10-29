---
name: language
description: "VTiger translations — languages/<locale>, vtranslate, jsLanguageStrings, LBL_/JS_ 2 array tách scope (EP-002). Use when: thêm label, nhãn, dịch, translation, ngôn ngữ, key LBL/JS."
user-invocable: false
---

# VTiger Language & Translation Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

- Adding labels for new fields, views, actions
- Creating language files for new modules
- Adding JavaScript translation strings
- Understanding translation loading order
- Working with Settings module translations

## Base rules → `rule://language-rules`

3-tier loading (core → `dev/` → `cus/`), file format with the two arrays, key prefixes (`LBL_`, `JS_`, `SINGLE_`, `LBL_VIEW_*_TITLE`) and the "always write both `en_us` + `vn_vn`" rule live in `rule://language-rules` — read it, don't duplicate here.

**Loader:** `Vtiger_Language_Handler::getModuleStringsFromFile()` in `includes/runtime/LanguageHandler.php`

**MANDATORY:** Check `$developerTeam` in `config.env.php` BEFORE choosing the file path (R&D → `languages/<locale>/X.php`, DEV → `dev/`, Customer → `cus/`). Do NOT assume — read the config value first.

## Quick Reference

### PHP Translation

```php
vtranslate('LBL_ACCOUNT_NAME', 'Accounts')
vtranslate('LBL_SAVE')  // Falls back to common Vtiger strings
vtranslate('LBL_GOAL_REMAINING_USER', $moduleName, ['%type' => $typeLabel])  // replaces %type in the string
```

Pass placeholders as ONE associative array (`['%key' => $value]`). Positional args are risky: `vtranslate()` treats a non-array 3rd argument as the language code (`LanguageHandler.php` `vtranslate()`).

### JavaScript Translation

```javascript
app.vtranslate('JS_SAVE_SUCCESSFULLY')
app.vtranslate('JS_ITEMS_COUNT', count)            // replaces each %s in order
app.vtranslate('JS_RANGE', {from: 1, to: 20})      // replaces %from, %to
```

`app.vtranslate` (`layouts/v7/resources/application.js`) substitutes `%s` positionally or `%key` from an object; `{0}` is NOT substituted.

## Settings Module

Settings submodule files live in a subfolder: `languages/en_us/Settings/Vtiger.php`, `Settings/Workflows.php`, `Settings/PBXManager.php` (dev overrides: `languages/en_us/dev/Settings/Vtiger.php`).

```php
vtranslate('LBL_CONFIG_TITLE', 'Settings:Vtiger')
// File: languages/en_us/Settings/Vtiger.php
```

The loader converts `:` to `.` and `resolveNameToPath()` maps dots to directories, so `Settings:Vtiger` and `Settings.Vtiger` both work; colon is the convention.

## Fallback Chain

1. Module-specific strings (`languages/en_us/ModuleName.php`)
2. Base module strings for submodules (`Settings:Workflows` → `Settings:Vtiger`)
3. Common strings (`languages/en_us/Vtiger.php`)
4. Default language (if user language differs)
5. Return key itself (graceful fallback)

## Critical Pitfalls

1. **Always trailing commas** in array entries — prevents merge conflicts
2. **Define both arrays by convention** — `$languageStrings` AND `$jsLanguageStrings` (the loader defaults a missing one to `[]`, but keep both so keys land in the right scope)
3. **Don't edit Core files** if you're DEV team — use `dev/` directory
4. **Settings module name** — `vtranslate('KEY', 'Settings:Vtiger')` (colon convention; `Settings.Vtiger` also resolves); file is `languages/<locale>/Settings/Vtiger.php`
5. **JS keys use JS_ prefix** — separates PHP and JS scopes
6. **Vietnamese locale** — `vn_vn` mirrors `en_us` structure; MUST write both (`rule://language-rules` § Locales)
7. **Cache per-request** — changes require page reload, not Quick Repair
8. **Insert into correct array** — `LBL_` keys go in `$languageStrings` (read by PHP/TPL `vtranslate()`), `JS_` keys go in `$jsLanguageStrings` (read by `app.vtranslate()`). The two arrays are scope-separated: `vtranslate()` does NOT see `$jsLanguageStrings`, and `app.vtranslate()` does NOT see `$languageStrings`. NEVER blindly append to end of file — the last `);` closes `$jsLanguageStrings`, not `$languageStrings`.

   **Symptom of wrong array:** `vtranslate('LBL_FOO')` in a TPL renders the raw key `LBL_FOO` (not the text) → the `LBL_` key was placed in `$jsLanguageStrings`. Same for a `JS_` key stuck in `$languageStrings` → JS shows the raw key.

   **MANDATORY verify after every language edit** — confirm each new key sits under the matching array:
   ```bash
   awk '/languageStrings = array/{a="LBL-array"} /jsLanguageStrings = array/{a="JS-array"} /YOUR_NEW_KEY/{print a": "$0}' languages/en_us/<File>.php
   ```
   Expected: `LBL_*` → `LBL-array`, `JS_*` → `JS-array`. Mismatch = move it. Run for BOTH locales.

   **A label used by BOTH a TPL and JS needs the key in BOTH arrays** (e.g. `LBL_X` in `$languageStrings` + `JS_X` in `$jsLanguageStrings`) — one key cannot serve both scopes.

## References

- [Language Files](references/language-files.md) — File creation, team routing
- [Translation Patterns](references/translation-patterns.md) — PHP/JS/TPL usage, placeholders, common strings

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- File ngôn ngữ chuẩn 2 array LBL/JS (tung.nguyen 9/9): `languages/vn_vn/CPMasterPlan.php` + bản `languages/en_us/CPMasterPlan.php`

## Verify (EP-002 — BẮT BUỘC sau mỗi lần thêm key)

```bash
for L in en_us vn_vn; do
  awk '/languageStrings = array/{a="LBL-array"} /jsLanguageStrings = array/{a="JS-array"} /<YOUR_KEY>/{print FILENAME" "a": "$0}' languages/$L/<File>.php
done
# Kỳ vọng: LBL_* → LBL-array, JS_* → JS-array. Sai array = bug EP-002.
```

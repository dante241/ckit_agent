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

## 3-Tier Loading Order

```
languages/<locale>/ModuleName.php          → Core (R&D)
languages/<locale>/dev/ModuleName.php      → Dev overrides (merged)
languages/<locale>/cus/ModuleName.php      → Customer overrides (merged)
```

**Merge rule:** Later files override earlier. Dev overrides Core; Customer overrides both.

**Loader:** `Vtiger_Language_Handler::getModuleStringsFromFile()` in `includes/runtime/LanguageHandler.php`

## Quick Reference

### PHP Translation

```php
vtranslate('LBL_ACCOUNT_NAME', 'Accounts')
vtranslate('LBL_SAVE')  // Falls back to common Vtiger strings
```

### JavaScript Translation

```javascript
app.vtranslate('JS_SAVE_SUCCESS')
app.vtranslate('JS_RECORDS_SELECTED')  // Supports {0} placeholders
```

### Language File Structure

```php
<?php

$languageStrings = array(
    'ModuleName' => 'Display Name',
    'SINGLE_ModuleName' => 'Single Record Name',
    'LBL_FIELD_NAME' => 'Field Label',
);

$jsLanguageStrings = array(
    'JS_SAVE_SUCCESS' => 'Saved successfully',
    'JS_DELETE_CONFIRM' => 'Are you sure?',
);
```

## Key Prefixes

| Prefix | Scope | Example |
|--------|-------|---------|
| `LBL_` | HTML/PHP labels | `LBL_ACCOUNT_NAME` |
| `JS_` | JavaScript strings | `JS_SAVE_SUCCESS` |
| `SINGLE_` | Singular module name | `SINGLE_Accounts` |
| `LBL_VIEW_*_TITLE` | View page titles | `LBL_VIEW_CONFIG_TITLE` |

## Dual-Locale Requirement (MANDATORY)

**Always write language strings in BOTH locales:**
- `languages/en_us/...` — English
- `languages/vn_vn/...` — Vietnamese

Both files must have identical keys. English file has English values; Vietnamese file has Vietnamese values.

```php
// R&D team → languages/en_us/CPGoal.php (root, NOT dev/)
$languageStrings = array(
    'LBL_GOAL_NAME' => 'Goal Name',
);

// R&D team → languages/vn_vn/CPGoal.php (root, NOT dev/)
$languageStrings = array(
    'LBL_GOAL_NAME' => 'Tên mục tiêu',
);
```

## File Locations

**MANDATORY:** Check `$developerTeam` in `config.env.php` BEFORE choosing file path. Do NOT assume — read the config value first.

| Team | English Path | Vietnamese Path |
|------|-------------|-----------------|
| **R&D** | `languages/en_us/ModuleName.php` | `languages/vn_vn/ModuleName.php` |
| DEV | `languages/en_us/dev/ModuleName.php` | `languages/vn_vn/dev/ModuleName.php` |
| Customer | `languages/en_us/cus/ModuleName.php` | `languages/vn_vn/cus/ModuleName.php` |

**Current value:** `$developerTeam = 'R&D'` in `config.env.php` → use root path, NOT `dev/`

## Settings Module

Settings submodules use dot notation: `Settings.Vtiger`, `Settings.Workflows`

```php
vtranslate('LBL_CONFIG_TITLE', 'Settings:Vtiger')
// File: languages/en_us/Settings.Vtiger.php
```

## Fallback Chain

1. Module-specific strings (`languages/en_us/ModuleName.php`)
2. Base module strings (for submodules like `Settings.Vtiger` → `Settings`)
3. Common strings (`languages/en_us/Vtiger.php`)
4. Default language (if user language differs)
5. Return key itself (graceful fallback)

## Critical Pitfalls

1. **Always trailing commas** in array entries — prevents merge conflicts
2. **Both arrays required** — file must define both `$languageStrings` AND `$jsLanguageStrings`
3. **Don't edit Core files** if you're DEV team — use `dev/` directory
4. **Settings colon syntax** — `vtranslate('KEY', 'Settings:Vtiger')` not `Settings.Vtiger`
5. **JS keys use JS_ prefix** — separates PHP and JS scopes
6. **Vietnamese locale** — `vn_vn` mirrors `en_us` structure; MUST write both (see Dual-Locale Requirement)
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

- [Language Files](references/language-files.md) — File creation, team routing, migration pattern
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

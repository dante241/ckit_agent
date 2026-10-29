---
description: "Language file 3-tier loading + 2 array languageStrings/jsLanguageStrings. Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(languages/**/*.php), tool:write(languages/**/*.php)"
condition: ".*"
---

# Language Configuration

> Auto-loads when editing language files; also the review checklist for every language-file change.

## 3-Tier Loading Order

| Team | Location | Loading Order |
|------|----------|---------------|
| **R&D** | `languages/<locale>/ModuleName.php` | 1st (base) |
| **DEV** | `languages/<locale>/dev/ModuleName.php` | 2nd (overrides base) |
| **Customer** | `languages/<locale>/cus/ModuleName.php` | 3rd (overrides all) |

Team determined by `$developerTeam` in `config.env.php`.

## File Format

```php
<?php

// HTML / Template labels — use trailing commas
$languageStrings = array(
    // Module basics
    'ModuleName' => 'Module Display Name',
    'SINGLE_ModuleName' => 'Single Record Name',
    'LBL_ADD_RECORD' => 'Add New Record',

    // View titles: LBL_VIEW_<VIEWNAME>_TITLE
    'LBL_VIEW_CONFIG_TITLE' => 'Configuration',

    // Field labels
    'LBL_STATUS' => 'Status',
    'LBL_CPADVERTISINGACCOUNT_CURRENCY_CODE' => 'Currency Code',
    'LBL_LAST_SYNC_DATETIME' => 'Last sync datetime',

    // Picklist values (key === value when no localization needed)
    'Active' => 'Active',
    'Inactive' => 'Inactive',
);

// JavaScript labels
$jsLanguageStrings = array(
    'JS_SAVE_SUCCESSFULLY' => 'Save Successfully',
    'JS_DELETE_CONFIRMATION' => 'Are you sure to delete record?',
    'JS_RECORDS_SELECTED' => '%s records selected',  // app.vtranslate('JS_RECORDS_SELECTED', count): each %s is replaced in order
    'JS_DELETE_WIDGET' => 'Delete dashlet \'%widget_title\'?',  // app.vtranslate(key, {widget_title: name}): %key replaced by name
    'JS_ERROR' => 'Error',
);
```

## Naming Conventions

| Prefix | Purpose | Usage |
|--------|---------|-------|
| `LBL_` | HTML labels (PHP/TPL) | `vtranslate('LBL_KEY', $module)` |
| `LBL_VIEW_*_TITLE` | View page titles | `vtranslate('LBL_VIEW_CONFIG_TITLE', $module)` |
| `JS_` | JavaScript strings | `app.vtranslate('JS_KEY')` |
| `SINGLE_<Module>` | Singular record name | Auto-loaded by VTiger |

## Locales

- `en_us` — English (default)
- `vn_vn` — Vietnamese

Always update **both** locales when adding new strings.

---
description: "Language file 3-tier loading + 2 array languageStrings/jsLanguageStrings. Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(languages/**/*.php), tool:write(languages/**/*.php)"
condition: ".*"
---

# Language Configuration

## 3-Tier Loading Order

| Team | Location | Loading Order |
|------|----------|---------------|
| **R&D** | `languages/<locale>/ModuleName.php` | 1st (base) |
| **DEV** | `languages/<locale>/dev/ModuleName.php` | 2nd (overrides base) |
| **Customer** | `languages/<locale>/cus/ModuleName.php` | 3rd (overrides all) |

Team determined by `$developerTeam` in `config.env.php`.

## File Format

- `LBL_` keys → `$languageStrings`, `JS_` keys → `$jsLanguageStrings`. Add each key inside the right array — never append at end of file (lands in the wrong array).

```php
$languageStrings = array(
    'SINGLE_ModuleName' => 'Single Record Name',   // singular record name, auto-loaded
    'LBL_VIEW_CONFIG_TITLE' => 'Configuration',    // view titles: LBL_VIEW_<VIEWNAME>_TITLE
    'Active' => 'Active',                          // picklist: key === value when no localization needed
);
$jsLanguageStrings = array(
    'JS_RECORDS_SELECTED' => '%s records selected',             // app.vtranslate(key, count): each %s replaced in order
    'JS_DELETE_WIDGET' => 'Delete dashlet \'%widget_title\'?',  // app.vtranslate(key, {widget_title: name})
);
```

Use trailing commas.

## Locales

- `en_us` — English (default)
- `vn_vn` — Vietnamese

Always update **both** locales when adding new strings.

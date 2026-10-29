# Language Files — Creation & Team Routing

> Detailed guide for creating and managing VTiger language files

## File Creation Checklist

When creating a new module or feature, create the language file with both arrays (convention — the loader defaults a missing array to `[]`):

```php
<?php

/**
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.11
 */

$languageStrings = array(
    // Module identity
    'CPGoal' => 'Goals',
    'SINGLE_CPGoal' => 'Goal',

    // View titles
    'LBL_VIEW_CONFIG_TITLE' => 'Goal Configuration',
    'LBL_VIEW_DETAIL_TITLE' => 'Goal Detail',

    // Field labels
    'LBL_GOAL_NAME' => 'Goal Name',
    'LBL_GOAL_TYPE' => 'Goal Type',
    'LBL_TARGET_VALUE' => 'Target Value',
    'LBL_CURRENT_VALUE' => 'Current Value',
    'LBL_PROGRESS' => 'Progress',

    // Picklist values
    'Revenue' => 'Revenue',
    'Quantity' => 'Quantity',
    'Active' => 'Active',
    'Completed' => 'Completed',

    // Action labels
    'LBL_CALCULATE_PROGRESS' => 'Calculate Progress',
    'LBL_EXPORT_REPORT' => 'Export Report',

    // Messages
    'LBL_PROGRESS_CALCULATED' => 'Progress calculated successfully',
    'LBL_NO_RECORDS_FOUND' => 'No records found',
);

$jsLanguageStrings = array(
    'JS_SAVE_SUCCESS' => 'Saved successfully',
    'JS_DELETE_CONFIRM' => 'Are you sure you want to delete this goal?',
    'JS_CALCULATING' => 'Calculating progress...',
    'JS_RECORDS_SELECTED' => '%s records selected',
);
```

## Team Routing Rules

### Which directory to use?

```
config.env.php → $developerTeam = 'DEV' | 'R&D' | 'CUS'
```

| Team | Directory | Rule |
|------|-----------|------|
| R&D | `languages/en_us/ModuleName.php` | Core module labels |
| DEV | `languages/en_us/dev/ModuleName.php` | Customization labels |
| CUS | `languages/en_us/cus/ModuleName.php` | Customer-specific labels |

**Rule:** Never edit Core files if you're DEV team. Create `dev/ModuleName.php` instead.

### Dev Override Example

Only include the keys you want to override:

```php
<?php
// File: languages/en_us/dev/Accounts.php
// Only override specific labels, don't duplicate all

$languageStrings = array(
    'LBL_CUSTOM_FIELD' => 'My Custom Field',
    'LBL_NEW_FEATURE' => 'New Feature Label',
);

$jsLanguageStrings = array(
    'JS_CUSTOM_ACTION' => 'Custom action completed',
);
```

## Settings Module Language Files

Settings submodule files live in the `Settings/` subfolder:

```
languages/en_us/Settings/Vtiger.php        → Settings:Vtiger
languages/en_us/Settings/Workflows.php     → Settings:Workflows
languages/en_us/Settings/PBXManager.php    → Settings:PBXManager
languages/en_us/dev/Settings/Vtiger.php    → dev overrides
```

**PHP usage:** colon notation `Settings:Vtiger` (convention). The loader converts `:` to `.` and `Vtiger_Loader::resolveNameToPath('languages.en_us.Settings.Vtiger')` maps dots to directories, so `Settings.Vtiger` resolves to the same file.

```php
// In PHP/TPL
vtranslate('LBL_CALL_CENTER_CONFIG', 'Settings:Vtiger')

// File: languages/en_us/Settings/Vtiger.php
$languageStrings = array(
    'LBL_CALL_CENTER_CONFIG' => 'Call Center Configuration',
);
```

## Vietnamese Locale

Mirror structure in `vn_vn` directory:

```php
// File: languages/vn_vn/CPGoal.php
$languageStrings = array(
    'CPGoal' => 'Mục tiêu',
    'SINGLE_CPGoal' => 'Mục tiêu',
    'LBL_GOAL_NAME' => 'Tên mục tiêu',
    'LBL_PROGRESS' => 'Tiến độ',
);

$jsLanguageStrings = array(
    'JS_SAVE_SUCCESS' => 'Lưu thành công',
    'JS_DELETE_CONFIRM' => 'Bạn có chắc muốn xóa mục tiêu này?',
);
```

## Common Mistakes

1. **Missing `$jsLanguageStrings`** — define both arrays by convention, even if the JS array is empty (the loader tolerates a missing one)
2. **No trailing comma on last entry** — Causes merge conflicts, always add trailing comma
3. **Editing core files as DEV** — Use `dev/` directory to avoid merge conflicts
4. **Wrong Settings file path** — the file is `languages/<locale>/Settings/Vtiger.php`, not `Settings.Vtiger.php`; call it as `vtranslate('KEY', 'Settings:Vtiger')`
5. **Forgetting Vietnamese file** — Always create `vn_vn` counterpart

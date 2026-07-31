# Translation Patterns — PHP, JS, TPL Usage

> How to use translations across VTiger layers

## PHP Usage

### Basic Translation

```php
// With module scope (preferred)
vtranslate('LBL_ACCOUNT_NAME', 'Accounts')

// Without module — falls back to common Vtiger.php strings
vtranslate('LBL_SAVE')

// Settings module — colon syntax
vtranslate('LBL_CONFIG_TITLE', 'Settings:Vtiger')
```

### In Action/View Controllers

```php
// Action — JSON response message
$response->setResult([
    'success' => true,
    'message' => vtranslate('LBL_PROGRESS_CALCULATED', 'CPGoal'),
]);

// View — assign to template
$viewer->assign('MODULE_LABEL', vtranslate('CPGoal', 'CPGoal'));
$viewer->assign('PAGE_TITLE', vtranslate('LBL_VIEW_CONFIG_TITLE', 'CPGoal'));
```

### In Models/Helpers

```php
// Error messages
throw new AppException(vtranslate('LBL_PERMISSION_DENIED', $moduleName));

// Notification content
$message = vtranslate('LBL_ORDER_SHIPPED', 'SalesOrder');
```

## Smarty TPL Usage

### Basic Label

```smarty
{vtranslate('LBL_SAVE', $MODULE)}
{vtranslate('LBL_CANCEL', $MODULE)}
{vtranslate('SINGLE_CPGoal', $MODULE)}
```

### In HTML Elements

```smarty
<button type="submit" class="btn btn-success">
    {vtranslate('LBL_SAVE', $MODULE)}
</button>

<h3>{vtranslate('LBL_VIEW_CONFIG_TITLE', $MODULE)}</h3>

<input type="text" placeholder="{vtranslate('LBL_SEARCH', $MODULE)}" />
```

### Conditional Labels

```smarty
{if $RECORD_ID}
    {vtranslate('LBL_EDIT_RECORD', $MODULE)}
{else}
    {vtranslate('LBL_CREATE_RECORD', $MODULE)}
{/if}
```

## JavaScript Usage

### Basic JS Translation

```javascript
// Simple label
app.vtranslate('JS_SAVE_SUCCESS')

// With placeholder {0}, {1}
app.vtranslate('JS_RECORDS_SELECTED')  // '{0} records selected'
// Note: Replacement done manually
var msg = app.vtranslate('JS_RECORDS_SELECTED').replace('{0}', count);
```

### In AJAX Callbacks

```javascript
app.request.post({ data: params }).then(function(error, data) {
    if (error) {
        app.helper.showErrorNotification({
            message: app.vtranslate('JS_ERROR_OCCURRED')
        });
        return;
    }

    app.helper.showSuccessNotification({
        message: app.vtranslate('JS_SAVE_SUCCESS')
    });
});
```

### In Confirmation Dialogs

```javascript
app.helper.showConfirmationBox({
    message: app.vtranslate('JS_DELETE_CONFIRM')
}).then(function() {
    // User confirmed
    self.deleteRecord(recordId);
});
```

## Placeholder Patterns

### PHP Placeholders — sprintf style

```php
// Define with %s placeholder
'LBL_WELCOME_USER' => 'Welcome, %s!',

// Usage
sprintf(vtranslate('LBL_WELCOME_USER', 'Vtiger'), $userName)
```

### JS Placeholders — {0} style

```php
// Define in jsLanguageStrings
'JS_ITEMS_COUNT' => '{0} items found',
'JS_RANGE' => 'Showing {0} to {1} of {2}',
```

```javascript
// Replace manually
var msg = app.vtranslate('JS_ITEMS_COUNT').replace('{0}', count);
var range = app.vtranslate('JS_RANGE')
    .replace('{0}', start)
    .replace('{1}', end)
    .replace('{2}', total);
```

## Common Strings (Vtiger.php)

These strings exist in `languages/en_us/Vtiger.php` — no need to redefine:

```
LBL_SAVE, LBL_CANCEL, LBL_DELETE, LBL_EDIT, LBL_BACK,
LBL_SEARCH, LBL_SELECT, LBL_ACTIONS, LBL_STATUS,
LBL_CREATED_ON, LBL_MODIFIED_ON, LBL_ASSIGNED_TO,
LBL_PERMISSION_DENIED, LBL_RECORD_NOT_FOUND,
LBL_YES, LBL_NO, LBL_ALL, LBL_NONE,
JS_SAVE_SUCCESS, JS_DELETE_CONFIRMATION, JS_ERROR_OCCURRED
```

**Rule:** Check `Vtiger.php` before adding common labels — avoid duplication.

## Lookup Order Summary

```
vtranslate('LBL_KEY', 'Settings:PBXManager')
  1. languages/en_us/Settings.PBXManager.php → languageStrings['LBL_KEY']
  2. languages/en_us/dev/Settings.PBXManager.php → merged
  3. languages/en_us/cus/Settings.PBXManager.php → merged
  4. languages/en_us/Settings.Vtiger.php → base module fallback
  5. languages/en_us/Vtiger.php → common strings fallback
  6. Return 'LBL_KEY' as-is → graceful fallback
```

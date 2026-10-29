# VTiger MVC Pattern

## Request Flow

```
User Request (index.php?module=X&action=Y)
    ↓
Router (Vtiger_Request)
    ↓
Controller (Action or View)
    ↓
Model (Record_Model / Module_Model)
    ↓
Helper (Logic_Helper → Data_Helper)
    ↓
Database (global $adb)
    ↓
Response (JSON or HTML via Smarty)
```

## Controller Types

### Action Controller (JSON Response)
- **Purpose**: AJAX endpoints, return JSON
- **Extends**: `Vtiger_Action_Controller`
- **Location**: `modules/{Module}/actions/{ActionName}.php`
- **URL**: `index.php?module=X&action=SaveAjax`
- **Class**: `{Module}_{ActionName}_Action`
- **Response**: `Vtiger_Response` with `setResult()` or `setError()`

### View Controller (HTML Response)
- **Purpose**: Render HTML pages
- **Extends** (per `cloudgo-development-rules` "View Base Classes"):
  - core List / Edit / Detail views → `Vtiger_Index_View` (or `Vtiger_List_View` / `Vtiger_Edit_View` / `Vtiger_Detail_View`); AJAX fragments → `Vtiger_BasicAjax_View`
  - custom pages (config, dashboard, report, landing…) → `CustomView_Base_View`
  - Settings pages → `Settings_Vtiger_Index_View`
- **Location**: `modules/{Module}/views/{ViewName}.php`
- **URL**: `index.php?module=X&view=Detail`
- **Class**: `{Module}_{ViewName}_View`
- **Response**: Smarty template via `$viewer->view()` (core) or `$viewer->display('modules/{Module}/tpls/{ViewName}.tpl')` (custom)

## Model Layer

### Record Model (Instance-Level)
```php
class CPDemo_Record_Model extends Vtiger_Record_Model {
	// CRUD operations on single record
	public function getProgress(): float { }
}
```

**Usage**:
```php
// Get existing record
$record = Vtiger_Record_Model::getInstanceById($id, $module);

// Create new record
$record = Vtiger_Record_Model::getCleanInstance($module);
$record->set('fieldname', $value);
$record->save();

// Update existing
$record->set('mode', 'edit');
$record->set('fieldname', $newValue);
$record->save();
```

### Module Model (Module-Level)
```php
class CPDemo_Module_Model extends Vtiger_Module_Model {
	// Module-wide operations
	public function getActiveItems(): array { }
}
```

**Usage**:
```php
$moduleModel = Vtiger_Module_Model::getInstance($moduleName);
```

## Template Layer (Smarty)

### Template Location
- Core views: `layouts/v7/modules/{Module}/{ViewName}.tpl`
- Custom views (`CustomView_Base_View`): `modules/{Module}/tpls/{ViewName}.tpl`

### Template Syntax
```smarty
{* Variables *}
{$VARIABLE_NAME}

{* Translation *}
{vtranslate('LBL_KEY', $MODULE)}

{* Conditions *}
{if $condition}...{/if}

{* Loops *}
{foreach from=$items item=item}
    {$item.name}
{/foreach}

{* Include partials *}
{include file='modules/{$MODULE}/Partial.tpl'}
```

### Passing Data to Template
```php
// In View controller
$viewer = $this->getViewer($request);
$viewer->assign('RECORD_ID', $recordId);
$viewer->assign('DATA', $data);
$viewer->view('Detail.tpl', $moduleName);
```

## Five Key Rules

### 1. Never Use Database in Action/View
**❌ WRONG:**
```php
class Products_Save_Action extends Vtiger_Action_Controller {
    public function process(Vtiger_Request $request) {
        global $adb;
        $adb->pquery("SELECT * FROM ...", []);  // NEVER!
    }
}
```

**✅ CORRECT:**
```php
class Products_Save_Action extends Vtiger_Action_Controller {
	public function process(Vtiger_Request $request) {
		$result = Products_Logic_Helper::processData($request->get('data'));
	}
}
```

### 2. Never Use HTML in PHP
**❌ WRONG:**
```php
echo '<div class="container">...</div>';
echo '<script>alert("Hi");</script>';
```

**✅ CORRECT:**
```php
$viewer->assign('data', $data);
$viewer->view('Template.tpl', $module);
```

### 3. Always Use Helpers for Business Logic
**❌ WRONG:**
```php
// In Action
$sql = "SELECT * FROM vtiger_products WHERE ...";
$result = $adb->pquery($sql, []);
```

**✅ CORRECT:**
```php
// In Action
$products = Products_Logic_Helper::getActiveProducts();

// In Logic Helper (static methods, no singleton)
public static function getActiveProducts(): array {
	return Products_Data_Helper::fetchActiveProducts();
}
```

### 4. Always Type Cast External Input
```php
$recordId = (int) $request->get('record');
$moduleName = (string) $request->getModule();
$isActive = (bool) $request->get('active');
```

### 5. Always Use Prepared Statements
**❌ WRONG:**
```php
$sql = "SELECT * FROM table WHERE id = " . $id;  // SQL injection!
```

**✅ CORRECT:**
```php
$sql = "SELECT * FROM table WHERE id = ?";
$result = $adb->pquery($sql, [$id]);
```

## Layer Responsibilities

| Layer | Responsibility | Never Do |
|-------|----------------|----------|
| **Action/View** | Request/Response, Permission | DB queries, Business logic |
| **Record Model** | CRUD single record | Module-wide operations |
| **Module Model** | Module-level operations | Single record CRUD |
| **Logic Helper** | Business logic, Orchestration | New SQL (prefer the Data helper) |
| **Data Helper** | Database queries only | Business logic |
| **Template** | HTML presentation | Business logic, DB queries |

# Validation and Permission Patterns

## Request Validation

### validateWriteAccess()

**Purpose:** Validates POST requests and CSRF token

**When to use:** All POST Actions (create, update, delete)

```php
class MyModule_Save_Action extends Vtiger_Action_Controller {
    public function checkPermission(Vtiger_Request $request) {
        $request->validateWriteAccess();
    }
}
```

**What it checks:**
- Request method is POST
- CSRF token is valid
- Throws `AppException` if validation fails

---

### validateReadAccess()

**Purpose:** Validates GET requests and referer

**When to use:** GET Views and Actions that need referer validation

```php
class MyModule_Detail_View extends Vtiger_Index_View {
    public function checkPermission(Vtiger_Request $request) {
        $request->validateReadAccess();
    }
}
```

**What it checks:**
- HTTP referer header
- Request origin matches application domain

---

### validateCSRF()

**Purpose:** Manual CSRF validation

**When to use:** Custom entry points, webhooks (when needed)

```php
if (!csrf_check($request->get('csrf_token'))) {
    throw new AppException('Invalid CSRF token');
}
```

---

## Input Sanitization

### vtlib_purify()

**Purpose:** XSS protection via HTMLPurifier

**Use for:** User input, HTML content, rich text

```php
$accountName = vtlib_purify($request->get('accountname'));
$description = vtlib_purify($request->get('description'));
```

**Behavior:**
- Removes dangerous HTML/JS
- Allows safe HTML tags (if configured)
- Always use for user-submitted content

---

### vtlib_purifyForSql()

**Purpose:** SQL column/table name validation

**Use for:** Dynamic SQL column names, table names

```php
$columnName = vtlib_purifyForSql($request->get('field'));
$sql = "SELECT {$columnName} FROM vtiger_account";
```

**Behavior:**
- Validates alphanumeric + underscore
- Prevents SQL injection in identifiers
- Does NOT escape values (use pquery params)

---

### Request::get() Auto-Purify

**Default behavior:** Auto-purifies with vtlib_purify()

```php
// Auto-purified
$accountName = $request->get('accountname');

// Same as
$accountName = vtlib_purify($request->get('accountname'));
```

---

### Request::getRaw()

**Purpose:** Get raw value without purification

**Use for:** JSON payloads, base64, hashes, API keys

```php
// Raw value
$jsonPayload = $request->getRaw('data');
$apiKey = $request->getRaw('api_key');

// With optional skip purify
$rawValue = $request->get('field', null, true); // true = skip purify
```

**CRITICAL:** Only use for trusted input or when manually sanitizing

---

### Request::getForSql()

**Purpose:** Get SQL-safe value for pquery()

**Use for:** Query parameter values

```php
$recordId = $request->getForSql('record');
$sql = "SELECT * FROM vtiger_account WHERE accountid = ?";
$result = $GLOBALS['adb']->pquery($sql, [$recordId]);
```

**Best Practice:** Use pquery params instead — auto-escapes

```php
// Preferred
$recordId = (int) $request->get('record');
$result = $GLOBALS['adb']->pquery($sql, [$recordId]);
```

---

## Permission Patterns

### hasModulePermission()

**Purpose:** Check if user can access module

**Use in:** checkPermission() for module-level access

```php
public function checkPermission(Vtiger_Request $request) {
    $moduleName = $request->getModule();
    $moduleModel = Vtiger_Module_Model::getInstance($moduleName);
    $userPrivilegesModel = Users_Privileges_Model::getCurrentUserPrivilegesModel();

    if (!$userPrivilegesModel->hasModulePermission($moduleModel->getId())) {
        throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
    }
}
```

**Returns:** boolean

---

### isPermitted()

**Purpose:** Check record-level action permission

**Use in:** checkPermission() for record operations

```php
public function checkPermission(Vtiger_Request $request) {
    $moduleName = $request->getModule();
    $recordId = (int) $request->get('record');

    if (!Users_Privileges_Model::isPermitted($moduleName, 'EditView', $recordId)) {
        throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
    }
}
```

**Actions:**
- `DetailView` - View record
- `EditView` - Edit record
- `Delete` - Delete record
- `CreateView` - Create record

**Returns:** boolean

---

### Permission Check Patterns

#### Pattern 1: Module Access Only

```php
class MyModule_List_View extends Vtiger_Index_View {
    public function checkPermission(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $moduleModel = Vtiger_Module_Model::getInstance($moduleName);
        $userPrivilegesModel = Users_Privileges_Model::getCurrentUserPrivilegesModel();

        if (!$userPrivilegesModel->hasModulePermission($moduleModel->getId())) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }
}
```

---

#### Pattern 2: Record-Level Edit Access

```php
class MyModule_Save_Action extends Vtiger_Save_Action {
    public function checkPermission(Vtiger_Request $request) {
        $request->validateWriteAccess();

        $moduleName = $request->getModule();
        $recordId = (int) $request->get('record');

        if ($recordId) {
            // Edit existing record
            if (!Users_Privileges_Model::isPermitted($moduleName, 'EditView', $recordId)) {
                throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
            }
        } else {
            // Create new record
            $moduleModel = Vtiger_Module_Model::getInstance($moduleName);
            $userPrivilegesModel = Users_Privileges_Model::getCurrentUserPrivilegesModel();

            if (!$userPrivilegesModel->hasModulePermission($moduleModel->getId())) {
                throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
            }
        }
    }
}
```

---

#### Pattern 3: Delete Record Access

```php
class MyModule_Delete_Action extends Vtiger_Delete_Action {
    public function checkPermission(Vtiger_Request $request) {
        $request->validateWriteAccess();

        $moduleName = $request->getModule();
        $recordId = (int) $request->get('record');

        if (!Users_Privileges_Model::isPermitted($moduleName, 'Delete', $recordId)) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }
}
```

---

#### Pattern 4: Admin-Only Access

```php
class MyModule_Config_View extends Vtiger_Index_View {
    public function checkPermission(Vtiger_Request $request) {
        $currentUser = Users_Record_Model::getCurrentUserModel();

        if (!$currentUser->isAdminUser()) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }
}
```

---

## Feature Gating

### isForbiddenFeature()

**Purpose:** Check if feature is disabled by license/package

**Returns:** boolean

**Use in:** Conditional logic, soft feature gating

```php
if (isForbiddenFeature('CustomReports')) {
    // Feature disabled — show upgrade notice
    $viewer->assign('upgradeRequired', true);
} else {
    // Feature enabled — show content
    $reports = $this->getReports();
    $viewer->assign('reports', $reports);
}
```

---

### checkAccessForbiddenFeature()

**Purpose:** Throw exception if feature forbidden

**Throws:** AppException with upgrade message

**Use in:** Hard feature gating, block access completely

```php
class CustomReports_List_View extends Vtiger_Index_View {
    public function checkPermission(Vtiger_Request $request) {
        // Throws AppException if feature forbidden
        checkAccessForbiddenFeature('CustomReports');
    }
}
```

---

### Feature Gate Patterns

#### Pattern 1: View-Level Hard Gate

```php
class MyModule_Premium_View extends Vtiger_Index_View {
    public function checkPermission(Vtiger_Request $request) {
        checkAccessForbiddenFeature('PremiumFeatures');
    }

    public function process(Vtiger_Request $request) {
        // Only reached if feature enabled
        $viewer = $this->getViewer($request);
        $viewer->view('Premium.tpl', $moduleName);
    }
}
```

---

#### Pattern 2: Action-Level Hard Gate

```php
class MyModule_Export_Action extends Vtiger_Action_Controller {
    public function checkPermission(Vtiger_Request $request) {
        checkAccessForbiddenFeature('AdvancedExport');

        // Also check module permission
        $moduleName = $request->getModule();
        $moduleModel = Vtiger_Module_Model::getInstance($moduleName);
        $userPrivilegesModel = Users_Privileges_Model::getCurrentUserPrivilegesModel();

        if (!$userPrivilegesModel->hasModulePermission($moduleModel->getId())) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }
}
```

---

#### Pattern 3: Soft Gate with Upgrade Notice

```php
class MyModule_List_View extends Vtiger_Index_View {
    public function process(Vtiger_Request $request) {
        $viewer = $this->getViewer($request);

        if (isForbiddenFeature('AdvancedFilters')) {
            // Show basic list
            $viewer->assign('showUpgradeNotice', true);
            $viewer->assign('upgradeFeature', 'Advanced Filters');
        } else {
            // Show advanced list with filters
            $filters = $this->getAdvancedFilters();
            $viewer->assign('filters', $filters);
        }

        $viewer->view('List.tpl', $moduleName);
    }
}
```

---

#### Pattern 4: Template-Level Display Control

```php
// In View
public function process(Vtiger_Request $request) {
    $viewer = $this->getViewer($request);
    $viewer->assign('isPremiumEnabled', !isForbiddenFeature('PremiumReports'));
    $viewer->view('List.tpl', $moduleName);
}
```

```smarty
{* In List.tpl *}
{if $isPremiumEnabled}
    <button class="btn btn-primary" id="exportPremium">
        {vtranslate('LBL_EXPORT_PREMIUM', $MODULE)}
    </button>
{else}
    <span class="text-muted">
        {vtranslate('LBL_UPGRADE_REQUIRED', $MODULE)}
    </span>
{/if}
```

---

#### Pattern 5: OperationNotPermitted.tpl Template

```php
class MyModule_Premium_View extends Vtiger_Index_View {
    public function process(Vtiger_Request $request) {
        if (isForbiddenFeature('PremiumDashboard')) {
            $viewer = $this->getViewer($request);
            $viewer->assign('MESSAGE', vtranslate('LBL_UPGRADE_TO_ACCESS_FEATURE', $moduleName));
            $viewer->view('OperationNotPermitted.tpl', $moduleName);
            exit;
        }

        // Normal processing
        $this->showDashboard($request);
    }
}
```

---

## Validation Checklist

### For Actions (AJAX endpoints)

- [ ] Call `validateWriteAccess()` if POST
- [ ] Check module permission with `hasModulePermission()`
- [ ] Check record permission with `isPermitted()` if record operation
- [ ] Use `vtlib_purify()` on user input
- [ ] Use type casting: `(int)`, `(string)`, `(bool)`
- [ ] Use `pquery()` with params, never concatenate SQL
- [ ] Check feature gates with `isForbiddenFeature()`
- [ ] Try-catch around risky operations
- [ ] Return `Vtiger_Response` with proper error codes

### For Views (HTML pages)

- [ ] Check module permission in `checkPermission()`
- [ ] Check record permission if viewing specific record
- [ ] Use feature gates to control UI elements
- [ ] Sanitize output in templates with `{$value|escape}`
- [ ] Use `OperationNotPermitted.tpl` for hard gates

### For Entry Points (Webhooks)

- [ ] Validate webhook signature/token
- [ ] Log all requests with `WebhookUtils::saveLog()`
- [ ] Try-catch with proper HTTP response codes
- [ ] Sanitize external data before saving
- [ ] Use `pquery()` for database operations

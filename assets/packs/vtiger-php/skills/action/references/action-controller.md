# VTiger Action Controller

## Complete Action Template

```php
<?php

/**
 * Action Controller
 * @author Your Name
 * @create date YYYY-MM-DD
 */

class {Module}_{ActionName}_Action extends Vtiger_Action_Controller {

    /**
     * Check user permission before processing
     * @param Vtiger_Request $request
     * @throws AppException if permission denied
     */
    public function checkPermission(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $recordId = (int) $request->get('record');

        // Module-level permission check
        if (!Users_Privileges_Model::isPermitted($moduleName, 'EditView')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED', $moduleName));
        }

        // Record-level permission check (if record ID provided)
        if (!empty($recordId)) {
            if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView', $recordId)) {
                throw new AppException(vtranslate('LBL_PERMISSION_DENIED', $moduleName));
            }
        }

        // Admin-only check
        $currentUser = Users_Record_Model::getCurrentUserModel();
        if (!$currentUser->isAdminUser()) {
            throw new AppException(vtranslate('LBL_ADMIN_ONLY', $moduleName));
        }
    }

    /**
     * Process the action
     * @param Vtiger_Request $request
     */
    public function process(Vtiger_Request $request) {
        $response = new Vtiger_Response();

        try {
            // Type cast request parameters
            $moduleName = (string) $request->getModule();
            $recordId = (int) $request->get('record');
            $action = (string) $request->get('subaction');

            // Early return pattern
            if (empty($recordId)) {
                $response->setError(vtranslate('LBL_RECORD_NOT_FOUND', $moduleName));
                $response->emit();
                return;
            }

            // Get record
            $record = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);

            if (empty($record)) {
                $response->setError(vtranslate('LBL_RECORD_NOT_FOUND', $moduleName));
                $response->emit();
                return;
            }

            // Call business logic helper
            $logic = {Module}_Logic_Helper::getInstance();
            $result = $logic->processAction($record, $action);

            // Success response
            $response->setResult([
                'success' => true,
                'message' => vtranslate('LBL_ACTION_SUCCESS', $moduleName),
                'data' => $result,
                'record_id' => $recordId
            ]);

        } catch (Exception $e) {
            // Error response
            $response->setError($e->getMessage());
        }

        $response->emit();
        return;
    }
}
```

## Type Casting Request Data

```php
// Strings
$moduleName = (string) $request->getModule();
$action = (string) $request->get('action');
$keyword = (string) $request->get('keyword');

// Integers
$recordId = (int) $request->get('record');
$page = (int) $request->get('page');
$limit = (int) $request->get('limit');

// Booleans
$isActive = (bool) $request->get('active');
$force = (bool) $request->get('force');

// Arrays
$items = (array) $request->get('items');
$filters = (array) $request->get('filters');
```

## Early Return Pattern

```php
public function process(Vtiger_Request $request) {
    $response = new Vtiger_Response();

    // Validation checks with early returns
    if (empty($request->get('record'))) {
        $response->setError('Record ID required');
        $response->emit();
        return;
    }

    $recordId = (int) $request->get('record');

    if ($recordId <= 0) {
        $response->setError('Invalid record ID');
        $response->emit();
        return;
    }

    // Main logic
    try {
        $result = $this->performAction($recordId);

        $response->setResult([
            'success' => true,
            'data' => $result
        ]);

    } catch (Exception $e) {
        $response->setError($e->getMessage());
    }

    $response->emit();
    return;
}
```

## Vtiger_Response Methods

```php
$response = new Vtiger_Response();

// Set success result
$response->setResult([
    'success' => true,
    'message' => 'Operation completed',
    'data' => $data
]);

// Set error
$response->setError('Error message');

// Set error with code
$response->setError('Error message', 'ERROR_CODE');

// Emit (send JSON and exit)
$response->emit();
```

## Permission Checks

### Module Permission
```php
// Check any module action
if (!Users_Privileges_Model::isPermitted($moduleName, 'EditView')) {
    throw new AppException('Permission denied');
}

// Check specific record
if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView', $recordId)) {
    throw new AppException('Permission denied');
}
```

### User Role Checks
```php
$currentUser = Users_Record_Model::getCurrentUserModel();

// Admin check
if (!$currentUser->isAdminUser()) {
    throw new AppException('Admin only');
}

// Check user ID
$userId = $currentUser->getId();

// Check if user owns record
$assignedUserId = (int) $record->get('assigned_user_id');
if ($assignedUserId !== $userId && !$currentUser->isAdminUser()) {
    throw new AppException('You can only modify your own records');
}
```

## Error Handling

### Try-Catch Pattern
```php
try {
    // Business logic that may throw
    $logic = CPGoal_Logic_Helper::getInstance();
    $result = $logic->calculateMetrics($recordId);

    $response->setResult([
        'success' => true,
        'data' => $result
    ]);

} catch (Exception $e) {
    // Log error
    error_log('CPGoal Action Error: ' . $e->getMessage());

    // Return error to client
    $response->setError($e->getMessage());
}
```

### Custom Exception Messages
```php
if ($targetValue <= 0) {
    throw new Exception(vtranslate('LBL_INVALID_TARGET_VALUE', 'CPGoal'));
}

if ($this->isDuplicate($goalName)) {
    throw new Exception(vtranslate('LBL_DUPLICATE_GOAL_NAME', 'CPGoal'));
}
```

## Batch Operations

### Process Multiple Records
```php
public function process(Vtiger_Request $request) {
    $response = new Vtiger_Response();

    try {
        $recordIds = (array) $request->get('record_ids');
        $results = [];
        $errors = [];

        foreach ($recordIds as $recordId) {
            try {
                $result = $this->processRecord((int) $recordId);
                $results[] = $result;
            } catch (Exception $e) {
                $errors[$recordId] = $e->getMessage();
            }
        }

        $response->setResult([
            'success' => true,
            'processed' => count($results),
            'failed' => count($errors),
            'results' => $results,
            'errors' => $errors
        ]);

    } catch (Exception $e) {
        $response->setError($e->getMessage());
    }

    $response->emit();
}
```

## File Upload Handling

```php
public function process(Vtiger_Request $request) {
    $response = new Vtiger_Response();

    try {
        if (!isset($_FILES['file'])) {
            throw new Exception('No file uploaded');
        }

        $file = $_FILES['file'];

        // Validate file
        if ($file['error'] !== UPLOAD_ERR_OK) {
            throw new Exception('File upload error');
        }

        // Validate file type
        $allowedTypes = ['image/jpeg', 'image/png', 'application/pdf'];
        if (!in_array($file['type'], $allowedTypes)) {
            throw new Exception('Invalid file type');
        }

        // Process file
        $uploadPath = 'storage/uploads/';
        $fileName = time() . '_' . $file['name'];
        move_uploaded_file($file['tmp_name'], $uploadPath . $fileName);

        $response->setResult([
            'success' => true,
            'file_path' => $uploadPath . $fileName,
            'file_name' => $fileName
        ]);

    } catch (Exception $e) {
        $response->setError($e->getMessage());
    }

    $response->emit();
}
```

## Critical Rules

1. **Always try-catch** in process method
2. **Type cast all input** from $request
3. **checkPermission throws AppException** (not setError)
4. **$response->emit() calls exit** — add return; after for clarity
5. **Use Logic helper** for business logic, not direct DB in Action
6. **Early return** for validation failures
7. **vtranslate** for user-facing messages

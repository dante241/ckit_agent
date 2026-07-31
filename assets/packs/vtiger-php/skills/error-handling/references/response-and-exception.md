# Response and Exception Patterns

## Vtiger_Response

Standard AJAX response object for Actions.

### Key Methods

```php
$response = new Vtiger_Response();

// Success
$response->setResult($data);        // Sets result payload
$response->emit();                  // Outputs JSON and exits

// Error
$response->setError($code, $message);
$response->emit();
```

### Response Format

**Success:**
```json
{
  "success": true,
  "result": { "id": 123, "status": "saved" }
}
```

**Error:**
```json
{
  "success": false,
  "error": {
    "code": 500,
    "message": "Record not found"
  }
}
```

### Important Notes

- `emit()` calls `exit` internally — execution stops
- Still add `return;` after `emit()` for safety
- Always emit at end of Action `process()` method

---

## AppException

Custom exception class with user-facing title field.

### Constructor

```php
throw new AppException($message, $code = 0, $title = '');
```

### Usage Pattern

```php
// Permission denied
throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));

// With title
throw new AppException(
    'Invalid record ID provided',
    400,
    'Validation Error'
);

// In checkPermission
if (!$hasPermission) {
    throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
}
```

---

## Try-Catch Patterns

### 1. Record CRUD

```php
try {
    $recordId = (int) $request->get('record');
    $record = Vtiger_Record_Model::getInstanceById($recordId);
    $record->set('status', 'completed');
    $record->save();
    $response->setResult(['id' => $recordId]);
}
catch (Exception $e) {
    $logger = LoggerManager::getLogger('PLATFORM');
    $logger->error("Record save failed: {$e->getMessage()}");
    $response->setError(500, $e->getMessage());
}
```

**Why:** `getInstanceById()` throws if record deleted/not found. `save()` may throw validation errors.

---

### 2. Record Save with Validation

```php
try {
    $recordModel = Vtiger_Record_Model::getCleanInstance('Accounts');
    $recordModel->set('accountname', $accountName);
    $recordModel->set('email', $email);
    $recordModel->save();

    $response->setResult(['id' => $recordModel->getId()]);
}
catch (Exception $e) {
    if (strpos($e->getMessage(), 'Duplicate') !== false) {
        $response->setError(409, 'Account already exists');
    } else {
        $response->setError(500, $e->getMessage());
    }
}
```

**Why:** Duplicate checks, mandatory field validation, DB constraints can throw.

---

### 3. External Service Call

```php
try {
    $connector = new FacebookAdsConnector();
    $campaigns = $connector->getCampaigns($accountId);

    $response->setResult(['campaigns' => $campaigns]);
}
catch (Exception $e) {
    $logger = LoggerManager::getLogger('PLATFORM');
    $logger->error("Facebook API error: {$e->getMessage()}");
    $response->setError(503, 'External service unavailable');
}
```

**Why:** Network timeout, API rate limits, authentication failures.

---

### 4. Webhook/Integration Handler

```php
public function process(Vtiger_Request $request) {
    try {
        $rawInput = file_get_contents('php://input');
        $data = json_decode($rawInput, true);

        if (empty($data['event_type'])) {
            throw new Exception('Missing event_type');
        }

        $this->handleEvent($data);

        http_response_code(200);
        echo json_encode(['status' => 'success']);
    }
    catch (Exception $e) {
        $logger = LoggerManager::getLogger('PLATFORM');
        $logger->error("Webhook error: {$e->getMessage()}");

        http_response_code(400);
        echo json_encode(['error' => $e->getMessage()]);
    }
}
```

**Why:** External services send unpredictable data formats.

---

### 5. API Handler saveRecord Pattern

```php
protected function saveRecord($module, $data) {
    try {
        $recordId = $data['id'] ?? null;

        if ($recordId) {
            $record = Vtiger_Record_Model::getInstanceById($recordId, $module);
        } else {
            $record = Vtiger_Record_Model::getCleanInstance($module);
        }

        foreach ($data as $field => $value) {
            $record->set($field, $value);
        }

        $record->save();

        return ['success' => true, 'id' => $record->getId()];
    }
    catch (Exception $e) {
        $this->saveLog("Save failed for $module", [
            'data' => $data,
            'error' => $e->getMessage()
        ]);

        return ['success' => false, 'error' => $e->getMessage()];
    }
}
```

**Why:** Upsert pattern with external data — validation, duplicates, missing fields.

---

### 6. Permission Check Re-throw

```php
public function checkPermission(Vtiger_Request $request) {
    try {
        $moduleName = $request->getModule();
        $recordId = (int) $request->get('record');

        $isPermitted = Users_Privileges_Model::isPermitted(
            $moduleName,
            'EditView',
            $recordId
        );

        if (!$isPermitted) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }
    catch (AppException $e) {
        // Let parent handler catch
        throw $e;
    }
}
```

**Why:** `checkPermission()` runs before `process()` — exceptions propagate to global handler.

---

## When to Try-Catch Decision Table

| Operation | Try-Catch? | Catch What? |
|-----------|------------|-------------|
| `getInstanceById()` | **YES** | Exception (record not found) |
| `$record->save()` | **YES** | Exception (validation, duplicate) |
| `$record->delete()` | **YES** | Exception (constraints) |
| External API call | **YES** | Exception, Throwable |
| Webhook JSON decode | **YES** | Exception (malformed JSON) |
| `$request->get()` | NO | Returns null if missing |
| `vtlib_purify()` | NO | Sanitizes silently |
| `pquery()` simple | NO | Check `getAffectedRowCount()` |
| `json_decode()` | **YES** | May return null — check with `json_last_error()` |
| `file_get_contents()` | **YES** | Returns false on failure |

---

## Global Try-Catch (API Entry Points)

```php
public function process(Vtiger_Request $request) {
    $response = new Vtiger_Response();

    try {
        // Business logic
        $result = $this->executeBusinessLogic($request);
        $response->setResult($result);
    }
    catch (AppException $e) {
        // User-facing errors
        $response->setError($e->getCode() ?: 400, $e->getMessage());
    }
    catch (Exception $e) {
        // System errors
        $logger = LoggerManager::getLogger('PLATFORM');
        $logger->error('Unexpected error: ' . $e->getMessage());
        $response->setError(500, 'Internal server error');
    }
    catch (\Throwable $e) {
        // Fatal errors (PHP 7+)
        $logger = LoggerManager::getLogger('PLATFORM');
        $logger->fatal('Fatal error: ' . $e->getMessage());
        $response->setError(500, 'System failure');
    }

    $response->emit();
    return; // Safety — emit() calls exit
}
```

---

## setResponse() Pattern (Legacy)

Some older Actions use `setResponse()` helper:

```php
public function process(Vtiger_Request $request) {
    try {
        $data = $this->getData($request);
        $this->setResponse('success', $data);
    }
    catch (Exception $e) {
        $this->setResponse('error', ['message' => $e->getMessage()]);
    }
    return; // Always add after setResponse
}

protected function setResponse($status, $data) {
    $response = ['status' => $status, 'data' => $data];
    header('Content-Type: application/json');
    echo json_encode($response);
    exit;
}
```

**CRITICAL:** `setResponse()` calls `exit` — still add `return;` after for code clarity.

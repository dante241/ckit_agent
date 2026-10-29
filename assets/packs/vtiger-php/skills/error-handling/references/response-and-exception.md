# Response and Exception Patterns

## Vtiger_Response

Standard AJAX response object for Actions.

### Key Methods

```php
$response = new Vtiger_Response();

// Success
$response->setResult($data);        // Sets result payload
$response->emit();                  // Echoes JSON — does NOT exit

// Error
$response->setError($code, $message = null, $title = null);
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
    "message": "Record not found",
    "title": null
  }
}
```

`setError()` with no message uses `$code` as the message.

### Important Notes

- `emit()` only sends headers + echoes the body — it does NOT `exit`; execution continues
- Add `return;` after `emit()` yourself when more code follows
- Emit once, at the end of the Action `process()` method

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
catch (Throwable $e) {
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
catch (Throwable $e) {
    if (strpos($e->getMessage(), 'Duplicate') !== false) {
        $response->setError(409, 'Account already exists');
    }
    else {
        $response->setError(500, $e->getMessage());
    }
}
```

**Why:** Duplicate checks, mandatory field validation, DB constraints can throw.

---

### 3. External Service Call

Pseudo-code — `MySocialConnector` stands for any HTTP connector class.

```php
try {
    $connector = new MySocialConnector();
    $campaigns = $connector->getCampaigns($accountId);

    $response->setResult(['campaigns' => $campaigns]);
}
catch (Throwable $e) {
    $logger = LoggerManager::getLogger('SOCIAL_INTEGRATION');
    $logger->error("Social API error: {$e->getMessage()}");
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
    catch (Throwable $e) {
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
        }
        else {
            $record = Vtiger_Record_Model::getCleanInstance($module);
        }

        foreach ($data as $field => $value) {
            $record->set($field, $value);
        }

        $record->save();

        return ['success' => true, 'id' => $record->getId()];
    }
    catch (Throwable $e) {
        $this->saveLog("saveRecord {$module}", [ // TraitAPILogger::saveLog($action, $data)
            'data' => $data,
            'error' => $e->getMessage(),
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

## When to Try-Catch

See the decision table in [SKILL.md](../SKILL.md#when-to-try-catch). Extra rows for raw PHP calls:

| Operation | Try-Catch? | Notes |
|-----------|------------|-------|
| `json_decode()` | NO | Returns `null` on bad JSON — check `json_last_error()` |
| `file_get_contents()` | NO | Returns `false` on failure — check `=== false` |

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
        $response->setError($e->getCode() ?: 400, $e->getMessage(), $e->getTitle());
    }
    catch (Throwable $e) {
        // System errors, including TypeError/ValueError
        $logger = LoggerManager::getLogger('PLATFORM');
        $logger->error('Unexpected error: ' . $e->getMessage());
        $response->setError(500, 'Internal server error');
    }

    $response->emit(); // emit() does not exit
    return;
}
```

---

## setResponse() Pattern (API handlers)

`RestfulApiHandler` / `IntegrationApiHandler` subclasses answer with `setResponse($code, $data = '')` — `$code` is the HTTP status, arrays are `json_encode`d:

```php
public function getData() {
    try {
        $data = $this->loadData(); // pseudo-code
        $this->setResponse(200, ['success' => 1, 'data' => $data]);
    }
    catch (Throwable $e) {
        $this->setResponse(500, ['success' => 0, 'message' => $e->getMessage()]);
    }
}
```

**CRITICAL:** `RestfulApiHandler::setResponse()` ends with `exit` — nothing after it runs (so unit tests that call it directly die; assert DB effects instead).

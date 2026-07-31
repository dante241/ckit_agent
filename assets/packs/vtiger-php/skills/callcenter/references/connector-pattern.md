# Call Center Connector Pattern

## Full Connector Skeleton

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('include/utils/CallCenterUtils.php');

class StringeeConnector extends Vtiger_EntryPoint {

    /**
     * Process webhook from Stringee
     */
    public function process(Vtiger_Request $request): void {
        // 1. Check configuration
        CallCenterUtils::checkConfig();

        // 2. Get sanitized request
        $request = CallCenterUtils::getRequest();
        $data = $request->getAllPurified();

        // 3. Validate required fields
        if (empty($data['call_id'])) {
            $this->sendResponse(['success' => 0, 'message' => 'Missing call_id']);
            return;
        }

        try {
            // 4. Normalize phone numbers
            $caller = $this->normalizePhone((string) $data['from']);
            $callee = $this->normalizePhone((string) $data['to']);
            $callId = (string) $data['call_id'];
            $status = (string) ($data['status'] ?? '');
            $duration = (int) ($data['duration'] ?? 0);
            $recordingUrl = (string) ($data['recording_url'] ?? '');

            // 5. Find Contact by phone
            $contactId = CallCenterUtils::findContactByPhone($caller);

            // 6. Create or update call log
            $pbxRecord = $this->findOrCreateCallLog($callId);
            $pbxRecord->set('caller', $caller);
            $pbxRecord->set('callee', $callee);
            $pbxRecord->set('call_status', $this->mapStatus($status));
            $pbxRecord->set('duration', $duration);
            $pbxRecord->set('recording_url', $recordingUrl);

            if (!empty($contactId)) {
                $pbxRecord->set('related_to', $contactId);
            }

            $pbxRecord->save();

            // 7. Send success response
            $this->sendResponse(['success' => 1, 'call_id' => $callId]);

        } catch (\Throwable $th) {
            CallCenterUtils::saveLog('error', $th->getMessage());
            $this->sendResponse(['success' => 0, 'message' => 'Internal error']);
        }
    }

    /**
     * Normalize phone number (Vietnam format)
     */
    protected function normalizePhone(string $phone): string {
        // Remove non-numeric characters except +
        $phone = preg_replace('/[^0-9+]/', '', $phone);

        // Convert 0xxx to 84xxx
        if (substr($phone, 0, 1) === '0') {
            $phone = '84' . substr($phone, 1);
        }

        // Remove + prefix
        $phone = str_replace('+84', '84', $phone);

        return $phone;
    }

    /**
     * Find existing call log or create new one
     */
    protected function findOrCreateCallLog(string $callId): Vtiger_Record_Model {
        $db = PearDatabase::getInstance();
        $sql = 'SELECT crmid FROM vtiger_pbxmanager WHERE call_id = ?';
        $result = $db->pquery($sql, [$callId]);

        if ($db->num_rows($result) > 0) {
            $recordId = (int) $db->query_result($result, 0, 'crmid');
            $record = Vtiger_Record_Model::getInstanceById($recordId, 'PBXManager');
            $record->set('mode', 'edit');
            return $record;
        }

        return Vtiger_Record_Model::getCleanInstance('PBXManager');
    }

    /**
     * Map provider status to CRM status
     */
    protected function mapStatus(string $status): string {
        $statusMapping = [
            'ringing' => 'Ringing',
            'answered' => 'In Progress',
            'ended' => 'Completed',
            'busy' => 'Busy',
            'no-answer' => 'No Answer',
            'failed' => 'Failed',
            'missed' => 'Missed',
        ];

        return $statusMapping[strtolower($status)] ?? 'Unknown';
    }

    /**
     * Send JSON response and exit
     */
    protected function sendResponse(array $data): void {
        header('Content-Type: application/json');
        echo json_encode($data);
        exit;
    }
}
```

## Key Patterns Explained

### 1. Webhook Validation

**Always validate required fields early:**
```php
if (empty($data['call_id'])) {
    $this->sendResponse(['success' => 0, 'message' => 'Missing call_id']);
    return;
}

if (empty($data['from']) || empty($data['to'])) {
    $this->sendResponse(['success' => 0, 'message' => 'Missing phone numbers']);
    return;
}
```

### 2. Type Casting (Security)

**Cast all external data:**
```php
$callId = (string) $data['call_id'];
$duration = (int) ($data['duration'] ?? 0);
$recordingUrl = (string) ($data['recording_url'] ?? '');
```

### 3. Phone Number Normalization

**Vietnam standard: 84xxxxxxxxx (no + or 0 prefix)**
```php
protected function normalizePhone(string $phone): string {
    $phone = preg_replace('/[^0-9+]/', '', $phone);
    if (substr($phone, 0, 1) === '0') $phone = '84' . substr($phone, 1);
    $phone = str_replace('+84', '84', $phone);
    return $phone;
}
```

### 4. Find or Create Pattern

**Upsert call log by external call_id:**
```php
protected function findOrCreateCallLog(string $callId): Vtiger_Record_Model {
    $db = PearDatabase::getInstance();
    $sql = 'SELECT crmid FROM vtiger_pbxmanager WHERE call_id = ?';
    $result = $db->pquery($sql, [$callId]);

    if ($db->num_rows($result) > 0) {
        $recordId = (int) $db->query_result($result, 0, 'crmid');
        $record = Vtiger_Record_Model::getInstanceById($recordId, 'PBXManager');
        $record->set('mode', 'edit');
        return $record;
    }

    return Vtiger_Record_Model::getCleanInstance('PBXManager');
}
```

### 5. Status Mapping

**Provider-specific → CRM standard:**
```php
protected function mapStatus(string $status): string {
    $statusMapping = [
        'ringing' => 'Ringing',
        'answered' => 'In Progress',
        'ended' => 'Completed',
        'missed' => 'Missed',
    ];
    return $statusMapping[strtolower($status)] ?? 'Unknown';
}
```

### 6. Error Handling

**Catch all exceptions, log, respond gracefully:**
```php
try {
    // Process webhook
} catch (\Throwable $th) {
    CallCenterUtils::saveLog('error', $th->getMessage());
    $this->sendResponse(['success' => 0, 'message' => 'Internal error']);
}
```

### 7. Response Pattern

**Always JSON, always exit:**
```php
protected function sendResponse(array $data): void {
    header('Content-Type: application/json');
    echo json_encode($data);
    exit;
}
```

## Provider-Specific Variations

### CloudFone - Nested Data
```php
$caller = (string) ($data['call']['from'] ?? '');
$callee = (string) ($data['call']['to'] ?? '');
```

### OmiCall - Event Types
```php
$eventType = (string) ($data['event'] ?? '');
if ($eventType === 'call.ended') {
    // Handle ended call
}
```

### FreePBX - Direction
```php
$direction = (string) ($data['direction'] ?? 'inbound');
if ($direction === 'outbound') {
    $caller = $callee;
    $callee = $data['destination'];
}
```

## Common Pitfalls

### ❌ Don't concatenate SQL
```php
$sql = "SELECT * WHERE call_id = '$callId'"; // VULNERABLE
```

### ✅ Use prepared statements
```php
$sql = 'SELECT * WHERE call_id = ?';
$result = $db->pquery($sql, [$callId]);
```

### ❌ Don't skip normalization
```php
$phone = $data['from']; // May be +84, 0, or 84 format
```

### ✅ Always normalize
```php
$phone = $this->normalizePhone((string) $data['from']);
```

### ❌ Don't hardcode status values
```php
$pbxRecord->set('call_status', 'ended'); // Won't match picklist
```

### ✅ Use status mapping
```php
$pbxRecord->set('call_status', $this->mapStatus($status));
```

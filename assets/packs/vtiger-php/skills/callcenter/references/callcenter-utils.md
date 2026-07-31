# CallCenter Utils

## CallCenterUtils Class

**Location:** `include/utils/CallCenterUtils.php`

Utility class for call center operations: phone lookup, call log creation, configuration management.

## Core Methods

### 1. Configuration Check

```php
CallCenterUtils::checkConfig(): void
```

**Purpose:** Validate call center module is enabled and configured

**Usage:**
```php
CallCenterUtils::checkConfig();
```

**Throws:** Exception if PBXManager module disabled or missing config

### 2. Request Sanitization

```php
CallCenterUtils::getRequest(): Vtiger_Request
```

**Purpose:** Get sanitized request object with purified data

**Usage:**
```php
$request = CallCenterUtils::getRequest();
$data = $request->getAllPurified();
```

### 3. Error Logging

```php
CallCenterUtils::saveLog(string $level, string $message): void
```

**Purpose:** Write call center logs to system

**Parameters:**
- `$level` - Log level: 'info', 'warning', 'error'
- `$message` - Log message

**Usage:**
```php
CallCenterUtils::saveLog('error', 'Failed to process webhook: ' . $e->getMessage());
CallCenterUtils::saveLog('info', 'Call log created for call_id: ' . $callId);
```

## Phone Lookup Methods

### Find Contact by Phone

```php
CallCenterUtils::findContactByPhone(string $phone): int
```

**Purpose:** Search Contacts module for matching phone number

**Returns:** Contact record ID or 0 if not found

**Usage:**
```php
$phone = '84909123456';
$contactId = CallCenterUtils::findContactByPhone($phone);

if ($contactId > 0) {
    $contact = Vtiger_Record_Model::getInstanceById($contactId, 'Contacts');
}
```

**Search Fields:**
- `phone` - Primary phone
- `mobile` - Mobile phone
- `homephone` - Home phone
- `otherphone` - Other phone

### Find Lead by Phone

```php
CallCenterUtils::findLeadByPhone(string $phone): int
```

**Purpose:** Search Leads module for matching phone number

**Returns:** Lead record ID or 0 if not found

**Usage:**
```php
$leadId = CallCenterUtils::findLeadByPhone($phone);
```

### Find Any Record by Phone

```php
CallCenterUtils::findRecordByPhone(string $phone): array
```

**Purpose:** Search multiple modules for phone number

**Returns:** `['module' => 'Contacts', 'id' => 123]` or empty array

**Search Order:**
1. Contacts
2. Leads
3. Accounts (if enabled)

**Usage:**
```php
$result = CallCenterUtils::findRecordByPhone($phone);
if (!empty($result)) {
    $moduleName = $result['module'];
    $recordId = $result['id'];
}
```

## Call Log Creation Pattern

### Basic Call Log

```php
$pbxRecord = Vtiger_Record_Model::getCleanInstance('PBXManager');
$pbxRecord->set('call_id', $callId);
$pbxRecord->set('caller', $normalizedCaller);
$pbxRecord->set('callee', $normalizedCallee);
$pbxRecord->set('call_status', 'Ringing');
$pbxRecord->set('callstarttime', date('Y-m-d H:i:s'));
$pbxRecord->save();
```

### Link to Contact

```php
$contactId = CallCenterUtils::findContactByPhone($caller);
if ($contactId > 0) {
    $pbxRecord->set('related_to', $contactId);
}
```

### Update with Call Result

```php
$pbxRecord->set('mode', 'edit');
$pbxRecord->set('call_status', 'Completed');
$pbxRecord->set('duration', $duration);
$pbxRecord->set('recording_url', $recordingUrl);
$pbxRecord->set('callendtime', date('Y-m-d H:i:s'));
$pbxRecord->save();
```

## PBXManager Module Fields

### Standard Fields

| Field | Type | Description |
|-------|------|-------------|
| `call_id` | string | External provider call ID (unique) |
| `caller` | string | Calling phone number (normalized) |
| `callee` | string | Called phone number (normalized) |
| `call_status` | picklist | Call status (Ringing, Completed, etc.) |
| `direction` | picklist | Inbound/Outbound |
| `duration` | integer | Call duration in seconds |
| `recording_url` | url | Link to call recording |
| `callstarttime` | datetime | When call started |
| `callendtime` | datetime | When call ended |
| `related_to` | reference | Link to Contact/Lead/Account |
| `assigned_user_id` | reference | CRM user handling call |

### Call Status Values

```php
$callStatuses = [
    'Ringing',
    'In Progress',
    'Completed',
    'No Answer',
    'Busy',
    'Failed',
    'Missed',
    'Voicemail',
];
```

### Direction Values

```php
$directions = [
    'Inbound',
    'Outbound',
    'Internal',
];
```

## Complete Integration Example

```php
class StringeeConnector extends Vtiger_EntryPoint {

    public function process(Vtiger_Request $request): void {
        // 1. Validate configuration
        CallCenterUtils::checkConfig();

        // 2. Get sanitized request
        $request = CallCenterUtils::getRequest();
        $data = $request->getAllPurified();

        try {
            // 3. Parse webhook data
            $callId = (string) $data['call_id'];
            $caller = $this->normalizePhone((string) $data['from']);
            $callee = $this->normalizePhone((string) $data['to']);
            $status = (string) $data['status'];

            // 4. Find related contact
            $contactId = CallCenterUtils::findContactByPhone($caller);

            // 5. Create/update call log
            $pbxRecord = $this->findOrCreateCallLog($callId);
            $pbxRecord->set('caller', $caller);
            $pbxRecord->set('callee', $callee);
            $pbxRecord->set('call_status', $this->mapStatus($status));

            if ($contactId > 0) {
                $pbxRecord->set('related_to', $contactId);
            }

            $pbxRecord->save();

            // 6. Log success
            CallCenterUtils::saveLog('info', "Call log created: $callId");

            // 7. Respond to webhook
            $this->sendResponse(['success' => 1]);

        } catch (\Throwable $th) {
            CallCenterUtils::saveLog('error', $th->getMessage());
            $this->sendResponse(['success' => 0]);
        }
    }
}
```

## Advanced Patterns

### Inbound Call Notification

```php
// Create call log
$pbxRecord->save();

// Send real-time notification to assigned user
$userId = $pbxRecord->get('assigned_user_id');
CPNotifications_Model::sendNotification([
    'user_id' => $userId,
    'title' => 'Incoming Call',
    'message' => "Call from $caller",
    'module' => 'PBXManager',
    'record_id' => $pbxRecord->getId(),
]);
```

### Create Callback Task for Missed Call

```php
if ($status === 'missed') {
    $task = Vtiger_Record_Model::getCleanInstance('Calendar');
    $task->set('subject', "Callback: $caller");
    $task->set('activitytype', 'Task');
    $task->set('taskstatus', 'Planned');
    $task->set('parent_id', $contactId);
    $task->set('date_start', date('Y-m-d'));
    $task->set('due_date', date('Y-m-d'));
    $task->save();
}
```

### Link Call to Opportunity

```php
// Find open opportunity for contact
$db = PearDatabase::getInstance();
$sql = 'SELECT potentialid FROM vtiger_potential
        INNER JOIN vtiger_crmentity ON crmid = potentialid
        WHERE related_to = ? AND sales_stage != "Closed Won"
        AND deleted = 0 LIMIT 1';
$result = $db->pquery($sql, [$contactId]);

if ($db->num_rows($result) > 0) {
    $opportunityId = (int) $db->query_result($result, 0, 'potentialid');
    $pbxRecord->set('related_to', $opportunityId);
}
```

## Performance Considerations

### Cache Phone Lookups

```php
// Static cache for single webhook batch
protected static $phoneCache = [];

protected function findContact(string $phone): int {
    if (!isset(self::$phoneCache[$phone])) {
        self::$phoneCache[$phone] = CallCenterUtils::findContactByPhone($phone);
    }
    return self::$phoneCache[$phone];
}
```

### Batch Update Call Logs

```php
// Use direct SQL for bulk status updates
$db = PearDatabase::getInstance();
$sql = 'UPDATE vtiger_pbxmanager SET call_status = ? WHERE call_id IN (' .
       generateQuestionMarks($callIds) . ')';
$db->pquery($sql, array_merge(['Completed'], $callIds));
```

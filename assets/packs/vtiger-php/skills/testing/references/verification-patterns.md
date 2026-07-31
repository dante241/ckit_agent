# Verification Patterns

> Common verification methods for VTiger test scripts

## Database Verification

### Verify Record Created

```php
// Check if record exists in crmentity
function verifyRecordExists(int $recordId): bool {
    global $adb;

    $sql = "SELECT crmid FROM vtiger_crmentity WHERE crmid = ? AND deleted = 0";
    $result = $adb->pquery($sql, [$recordId]);

    return $adb->num_rows($result) > 0;
}

// Usage in test
$recordModel = Vtiger_Record_Model::getCleanInstance('Accounts');
$recordModel->set('accountname', 'Test Account');
$recordModel->save();

$recordId = $recordModel->getId();
assertResult('Record should exist in database', verifyRecordExists($recordId));
```

### Verify Field Value Saved

```php
// Verify specific field value
function verifyFieldValue(int $recordId, string $module, string $fieldName, $expectedValue): bool {
    global $adb;

    $recordModel = Vtiger_Record_Model::getInstanceById($recordId, $module);
    $actualValue = $recordModel->get($fieldName);

    return ($actualValue === $expectedValue);
}

// Usage in test
$recordModel->set('accountname', 'Updated Name');
$recordModel->set('mode', 'edit');
$recordModel->save();

assertResult(
    'Account name should be updated',
    verifyFieldValue($recordId, 'Accounts', 'accountname', 'Updated Name')
);
```

### Verify Relation Created

```php
// Check if two records are related
function verifyRelationExists(int $sourceRecordId, int $targetRecordId): bool {
    global $adb;

    $sql = "SELECT crmid FROM vtiger_crmentityrel
            WHERE (crmid = ? AND relcrmid = ?)
               OR (crmid = ? AND relcrmid = ?)";

    $result = $adb->pquery($sql, [
        $sourceRecordId, $targetRecordId,
        $targetRecordId, $sourceRecordId
    ]);

    return $adb->num_rows($result) > 0;
}

// Usage in test
$contactId = 123;
$accountId = 456;

// Create relation
$contactModel = Vtiger_Record_Model::getInstanceById($contactId, 'Contacts');
$contactModel->set('account_id', $accountId);
$contactModel->set('mode', 'edit');
$contactModel->save();

assertResult('Relation should exist', verifyRelationExists($contactId, $accountId));
```

### Verify Record Count

```php
// Count records matching criteria
function getRecordCount(string $module, array $conditions = []): int {
    global $adb;

    $moduleModel = Vtiger_Module_Model::getInstance($module);
    $tableName = $moduleModel->get('basetable');
    $tableIndex = $moduleModel->get('basetableid');

    $sql = "SELECT COUNT(*) as count
            FROM {$tableName}
            INNER JOIN vtiger_crmentity ON crmid = {$tableIndex} AND deleted = 0";

    $params = [];
    if (!empty($conditions)) {
        $whereClauses = [];
        foreach ($conditions as $field => $value) {
            $whereClauses[] = "{$field} = ?";
            $params[] = $value;
        }
        $sql .= " WHERE " . implode(' AND ', $whereClauses);
    }

    $result = $adb->pquery($sql, $params);
    $row = $adb->fetchByAssoc($result);

    return (int) $row['count'];
}

// Usage in test
$countBefore = getRecordCount('Campaigns', ['campaignstatus' => 'Active']);

// Create new active campaign
$campaign = Vtiger_Record_Model::getCleanInstance('Campaigns');
$campaign->set('campaignname', 'Test Campaign');
$campaign->set('campaignstatus', 'Active');
$campaign->save();

$countAfter = getRecordCount('Campaigns', ['campaignstatus' => 'Active']);

assertEquals('Active campaign count should increase by 1', $countBefore + 1, $countAfter);
```

### Verify Data Integrity

```php
// Verify foreign key integrity
function verifyForeignKeyIntegrity(int $recordId, string $relatedModule, string $relatedField): bool {
    global $adb;

    $recordModel = Vtiger_Record_Model::getInstanceById($recordId);
    $relatedRecordId = (int) $recordModel->get($relatedField);

    if (empty($relatedRecordId)) {
        return true; // Optional field
    }

    // Check if related record exists
    $sql = "SELECT crmid FROM vtiger_crmentity
            WHERE crmid = ? AND setype = ? AND deleted = 0";

    $result = $adb->pquery($sql, [$relatedRecordId, $relatedModule]);

    return $adb->num_rows($result) > 0;
}

// Usage in test
assertResult(
    'Related account should exist',
    verifyForeignKeyIntegrity($contactId, 'Accounts', 'account_id')
);
```

## API Response Verification

### Test API Call with cURL

```php
// Make API call and verify response
function testApiCall(string $url, string $method, array $data = [], array $headers = []): array {
    $ch = curl_init();

    curl_setopt($ch, CURLOPT_URL, $url);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
    curl_setopt($ch, CURLOPT_CUSTOMREQUEST, $method);

    if (!empty($headers)) {
        curl_setopt($ch, CURLOPT_HTTPHEADER, $headers);
    }

    if (!empty($data) && in_array($method, ['POST', 'PUT', 'PATCH'])) {
        curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode($data));
    }

    $response = curl_exec($ch);
    $httpCode = curl_getinfo($ch, CURLINFO_HTTP_CODE);
    curl_close($ch);

    return [
        'http_code' => $httpCode,
        'body' => json_decode($response, true),
        'raw_body' => $response,
    ];
}

// Usage in test
$apiUrl = 'https://localhost/api/IntegrationAPI/Zalo/CreateOrder';
$headers = ['Authorization: Bearer test_token_123'];
$payload = [
    'order_id' => 'ORDER_123',
    'customer_name' => 'Nguyễn Văn A',
];

$response = testApiCall($apiUrl, 'POST', $payload, $headers);

assertEquals('HTTP status should be 200', 200, $response['http_code']);
assertEquals('Response should be success', true, $response['body']['success'] ?? false);
assertNotEmpty('Order ID should be returned', $response['body']['result']['id'] ?? '');
```

### Verify JSON Structure

```php
// Check if JSON response has required fields
function verifyJsonStructure(array $json, array $requiredFields): bool {
    foreach ($requiredFields as $field) {
        if (!isset($json[$field])) {
            echo "    Missing field: {$field}\n";
            return false;
        }
    }
    return true;
}

// Usage in test
$response = ['success' => true, 'result' => ['id' => 123, 'name' => 'Test']];

assertResult(
    'Response should have required fields',
    verifyJsonStructure($response, ['success', 'result'])
);

assertResult(
    'Result should have required fields',
    verifyJsonStructure($response['result'], ['id', 'name'])
);
```

## Log Verification

### Read Last Lines from Log File

```php
// Read last N lines from log file
function readLastLogLines(string $logFile, int $lines = 10): array {
    if (!file_exists($logFile)) {
        return [];
    }

    $file = new SplFileObject($logFile, 'r');
    $file->seek(PHP_INT_MAX);
    $lastLine = $file->key();
    $startLine = max(0, $lastLine - $lines);

    $result = [];
    $file->seek($startLine);
    while (!$file->eof()) {
        $result[] = $file->current();
        $file->next();
    }

    return $result;
}

// Usage in test
$logFile = 'logs/vtigercrm.log';
$logLines = readLastLogLines($logFile, 20);

$found = false;
foreach ($logLines as $line) {
    if (strpos($line, 'Goal progress calculated') !== false) {
        $found = true;
        break;
    }
}

assertResult('Log should contain progress calculation message', $found);
```

### Check for Error in Logs

```php
// Check if log contains error message
function logContainsError(string $logFile, string $errorPattern): bool {
    if (!file_exists($logFile)) {
        return false;
    }

    $lines = readLastLogLines($logFile, 50);

    foreach ($lines as $line) {
        if (preg_match("/{$errorPattern}/i", $line)) {
            return true;
        }
    }

    return false;
}

// Usage in test
assertResult(
    'Log should not contain SQL errors',
    !logContainsError('logs/vtigercrm.log', 'SQL.*error|mysql.*error')
);
```

## Cleanup Pattern

### Clean Up Test Records

```php
// Clean up test records at end of test
function cleanupTestRecords(array $recordIds): void {
    if (empty($recordIds)) {
        return;
    }

    echo "Cleaning up test records...\n";

    foreach ($recordIds as $id) {
        try {
            $recordModel = Vtiger_Record_Model::getInstanceById((int) $id);
            $recordModel->delete();
            echo "  ✓ Deleted record ID: {$id}\n";
        } catch (Exception $e) {
            echo "  ✗ Failed to delete record ID: {$id} - {$e->getMessage()}\n";
        }
    }
}

// Track records during test
$testRecordIds = [];

// Create records
$record1 = Vtiger_Record_Model::getCleanInstance('Accounts');
$record1->set('accountname', 'Test 1');
$record1->save();
$testRecordIds[] = $record1->getId();

$record2 = Vtiger_Record_Model::getCleanInstance('Accounts');
$record2->set('accountname', 'Test 2');
$record2->save();
$testRecordIds[] = $record2->getId();

// Run tests...

// Cleanup at end
cleanupTestRecords($testRecordIds);
```

### Clean Up by Module and Criteria

```php
// Delete records matching criteria
function cleanupRecordsByCriteria(string $module, array $conditions): void {
    global $adb;

    $moduleModel = Vtiger_Module_Model::getInstance($module);
    $tableName = $moduleModel->get('basetable');
    $tableIndex = $moduleModel->get('basetableid');

    $whereClauses = [];
    $params = [];
    foreach ($conditions as $field => $value) {
        $whereClauses[] = "{$field} = ?";
        $params[] = $value;
    }

    $sql = "SELECT {$tableIndex} FROM {$tableName}
            INNER JOIN vtiger_crmentity ON crmid = {$tableIndex} AND deleted = 0
            WHERE " . implode(' AND ', $whereClauses);

    $result = $adb->pquery($sql, $params);
    $recordIds = [];

    while ($row = $adb->fetchByAssoc($result)) {
        $recordIds[] = $row[$tableIndex];
    }

    cleanupTestRecords($recordIds);
}

// Usage in test - cleanup all test accounts created today
cleanupRecordsByCriteria('Accounts', [
    'accountname' => 'Test Account%'
]);
```

## File System Verification

### Verify File Created

```php
// Check if file exists and has content
function verifyFileCreated(string $filePath, int $minSize = 0): bool {
    if (!file_exists($filePath)) {
        echo "    File does not exist: {$filePath}\n";
        return false;
    }

    $fileSize = filesize($filePath);
    if ($fileSize < $minSize) {
        echo "    File too small: {$fileSize} bytes (min: {$minSize})\n";
        return false;
    }

    return true;
}

// Usage in test - verify export file created
$exportFile = 'test/storage/export_' . time() . '.xlsx';

// Run export
$exporter->exportToExcel($exportFile);

assertResult('Export file should be created', verifyFileCreated($exportFile, 100));

// Cleanup
if (file_exists($exportFile)) {
    unlink($exportFile);
}
```

### Verify CSV Content

```php
// Check CSV file structure
function verifyCsvContent(string $csvFile, array $expectedHeaders): bool {
    if (!file_exists($csvFile)) {
        return false;
    }

    $handle = fopen($csvFile, 'r');
    $headers = fgetcsv($handle);
    fclose($handle);

    foreach ($expectedHeaders as $header) {
        if (!in_array($header, $headers)) {
            echo "    Missing header: {$header}\n";
            return false;
        }
    }

    return true;
}

// Usage in test
assertResult(
    'CSV should have correct headers',
    verifyCsvContent('export.csv', ['ID', 'Name', 'Status'])
);
```

## Performance Verification

### Measure Execution Time

```php
// Measure function execution time
function measureExecutionTime(callable $callable): float {
    $start = microtime(true);
    $callable();
    $end = microtime(true);

    return round(($end - $start) * 1000, 2); // milliseconds
}

// Usage in test
$executionTime = measureExecutionTime(function() {
    // Code to measure
    $helper->processLargeDataset();
});

echo "Execution time: {$executionTime}ms\n";
assertResult('Should execute in under 1 second', $executionTime < 1000, $executionTime);
```

# Standalone Test Script

> VTiger has NO PHPUnit. All tests are standalone PHP scripts with VTiger bootstrap.

## Full Test Script Template

```php
<?php
/**
 * Test: {Feature Name}
 * @author {Your Name}
 * @email {your.email@company.vn}
 * @create date {YYYY.MM.DD}
 */

// Bootstrap VTiger
chdir(dirname(__FILE__) . '/../');
require_once('config.php');
require_once('include/utils/VtlibUtils.php');
require_once('includes/runtime/EntryPoint.php');

// Test assertion functions
function assertResult(string $testName, bool $condition, $actual = null): bool {
    global $passed, $failed, $total;
    $total++;

    if ($condition) {
        $passed++;
        echo "[✓] PASS: {$testName}\n";
        return true;
    } else {
        $failed++;
        echo "[✗] FAIL: {$testName}\n";
        if ($actual !== null) {
            echo "    Actual: " . var_export($actual, true) . "\n";
        }
        return false;
    }
}

function assertEquals(string $testName, $expected, $actual): bool {
    $condition = ($expected === $actual);
    if (!$condition) {
        echo "    Expected: " . var_export($expected, true) . "\n";
        echo "    Actual: " . var_export($actual, true) . "\n";
    }
    return assertResult($testName, $condition, $actual);
}

function assertNotEmpty(string $testName, $value): bool {
    return assertResult($testName, !empty($value), $value);
}

function assertEmpty(string $testName, $value): bool {
    return assertResult($testName, empty($value), $value);
}

function assertException(string $testName, callable $callable): bool {
    global $passed, $failed, $total;
    $total++;

    try {
        $callable();
        $failed++;
        echo "[✗] FAIL: {$testName} - No exception thrown\n";
        return false;
    } catch (Exception $e) {
        $passed++;
        echo "[✓] PASS: {$testName} - Exception: {$e->getMessage()}\n";
        return true;
    }
}

// Initialize test environment
global $adb, $current_user;
$current_user = CRMEntity::getInstance('Users');
$current_user->retrieveCurrentUserInfoFromFile(1); // Admin user

// Test counters
$passed = 0;
$failed = 0;
$total = 0;

// Test records cleanup tracking
$testRecordIds = [];

echo "\n========================================\n";
echo "Test: {Feature Name}\n";
echo "========================================\n\n";

// ============================================
// TEST CASES
// ============================================

// TC-01: Test description
echo "TC-01: Test description\n";
// Test code here
assertEquals('TC-01: Expected result', $expected, $actual);
echo "\n";

// TC-02: Another test
echo "TC-02: Another test description\n";
// Test code here
assertNotEmpty('TC-02: Result should not be empty', $result);
echo "\n";

// ============================================
// CLEANUP
// ============================================

if (!empty($testRecordIds)) {
    echo "Cleaning up test records...\n";
    foreach ($testRecordIds as $id) {
        try {
            $recordModel = Vtiger_Record_Model::getInstanceById((int) $id);
            $recordModel->delete();
            echo "  Deleted record ID: {$id}\n";
        } catch (Exception $e) {
            echo "  Failed to delete record ID: {$id} - {$e->getMessage()}\n";
        }
    }
}

// ============================================
// SUMMARY
// ============================================

echo "\n========================================\n";
echo "Test Summary\n";
echo "========================================\n";
echo "Total: {$total}\n";
echo "Passed: {$passed}\n";
echo "Failed: {$failed}\n";
echo "Success Rate: " . ($total > 0 ? round(($passed / $total) * 100, 2) : 0) . "%\n";
echo "========================================\n\n";

// Exit with proper code
exit($failed > 0 ? 1 : 0);
```

## Component-Specific Patterns

### Test Helper/Model Function

```php
// TC-01: Test helper function with normal input
echo "TC-01: Calculate goal progress - normal input\n";

require_once('modules/CPGoal/helpers/CPGoal_Data_Helper.php');
$helper = CPGoal_Data_Helper::getInstance();

$actual = 50;
$target = 100;
$progress = $helper->calculateProgress($actual, $target);

assertEquals('TC-01: Progress should be 50%', 50, $progress);
echo "\n";

// TC-02: Test with empty input
echo "TC-02: Calculate goal progress - empty input\n";

$progress = $helper->calculateProgress(0, 0);
assertEquals('TC-02: Progress should be 0 for empty input', 0, $progress);
echo "\n";

// TC-03: Test with boundary case
echo "TC-03: Calculate goal progress - division by zero\n";

$progress = $helper->calculateProgress(100, 0);
assertEquals('TC-03: Should handle division by zero', 0, $progress);
echo "\n";
```

### Test Action Controller (Simulate Request)

```php
// TC-04: Test Action controller - happy path
echo "TC-04: Save action - happy path\n";

// Create mock request
require_once('include/Webservices/Utils.php');
$_REQUEST = [
    'module' => 'Accounts',
    'action' => 'Save',
    'accountname' => 'Test Account ' . time(),
    'assigned_user_id' => 1,
];

// Mock CSRF token
$_SESSION['vtiger_authenticated_user_id'] = 1;

// Execute action
try {
    $controller = new Accounts_Save_Action();

    // Create request object
    $request = new Vtiger_Request($_REQUEST);

    // Mock response object
    ob_start();
    $controller->process($request);
    $output = ob_get_clean();

    // Parse JSON response
    $response = json_decode($output, true);

    assertNotEmpty('TC-04: Response should not be empty', $response);
    assertEquals('TC-04: Response should be success', true, $response['success'] ?? false);

    // Track for cleanup
    if (!empty($response['result']['id'])) {
        $testRecordIds[] = $response['result']['id'];
    }
} catch (Exception $e) {
    assertResult('TC-04: Should not throw exception', false, $e->getMessage());
}
echo "\n";
```

### Test Database Query

```php
// TC-05: Test database query
echo "TC-05: Query campaigns by status\n";

global $adb;

$sql = "SELECT campaignid, campaignname
        FROM vtiger_campaign
        INNER JOIN vtiger_crmentity ON crmid = campaignid AND deleted = 0
        WHERE campaignstatus = ?
        LIMIT 5";

$result = $adb->pquery($sql, ['Active']);
$count = $adb->num_rows($result);

assertResult('TC-05: Should find active campaigns', $count > 0, $count);

// Test with decode
while ($row = $adb->fetchByAssoc($result)) {
    $row = decodeUTF8($row);
    assertNotEmpty('TC-05: Campaign name should not be empty', $row['campaignname']);
}
echo "\n";
```

### Test Record Model CRUD

```php
// TC-06: Create record
echo "TC-06: Create CPGoal record\n";

$recordModel = Vtiger_Record_Model::getCleanInstance('CPGoal');
$recordModel->set('goalname', 'Test Goal ' . time());
$recordModel->set('goaltype', 'Revenue');
$recordModel->set('target_value', 100000);
$recordModel->set('assigned_user_id', 1);
$recordModel->save();

$recordId = $recordModel->getId();
assertNotEmpty('TC-06: Record ID should not be empty', $recordId);
$testRecordIds[] = $recordId;
echo "\n";

// TC-07: Retrieve and update record
echo "TC-07: Update CPGoal record\n";

$retrievedRecord = Vtiger_Record_Model::getInstanceById($recordId, 'CPGoal');
assertEquals('TC-07: Retrieved record should match', 'Revenue', $retrievedRecord->get('goaltype'));

$retrievedRecord->set('mode', 'edit');
$retrievedRecord->set('target_value', 150000);
$retrievedRecord->save();

$updatedRecord = Vtiger_Record_Model::getInstanceById($recordId, 'CPGoal');
assertEquals('TC-07: Updated value should match', '150000', $updatedRecord->get('target_value'));
echo "\n";
```

### Test API Handler (Mock Request)

```php
// TC-08: Test API handler
echo "TC-08: API handler - valid request\n";

require_once('api/IntegrationAPI/Zalo.php');

// Mock request
$_SERVER['REQUEST_METHOD'] = 'POST';
$_SERVER['HTTP_AUTHORIZATION'] = 'Bearer test_token_12345';

$requestBody = json_encode([
    'order_id' => 'ORDER_' . time(),
    'customer_name' => 'Nguyễn Văn A',
    'total_amount' => 500000,
]);

// Simulate file_get_contents('php://input')
$GLOBALS['HTTP_RAW_POST_DATA'] = $requestBody;

ob_start();
try {
    $handler = new ZaloApiHandler();
    $handler->process('CreateOrder');
    $output = ob_get_clean();

    $response = json_decode($output, true);
    assertNotEmpty('TC-08: API response should not be empty', $response);
} catch (Exception $e) {
    ob_get_clean();
    assertResult('TC-08: Should handle request', false, $e->getMessage());
}
echo "\n";
```

### Test Event Handler

```php
// TC-09: Test event handler
echo "TC-09: AfterSave event handler\n";

require_once('modules/CPGoal/handlers/CPGoalHandler.php');

$handler = new CPGoalHandler();

// Create test record
$recordModel = Vtiger_Record_Model::getCleanInstance('CPGoal');
$recordModel->set('goalname', 'Handler Test ' . time());
$recordModel->set('goaltype', 'Revenue');
$recordModel->set('assigned_user_id', 1);

// Trigger handler manually
try {
    $handler->handleAfterSaveEvent($recordModel);
    assertResult('TC-09: Handler should execute without error', true);

    // Save record for cleanup
    $recordModel->save();
    $testRecordIds[] = $recordModel->getId();
} catch (Exception $e) {
    assertResult('TC-09: Handler failed', false, $e->getMessage());
}
echo "\n";
```

## File Naming Convention

**Pattern:** `test-{component}-{module}-{feature}.php`

**Examples:**
- `test/test-action-accounts-save.php`
- `test/test-model-cpgoal-calculation.php`
- `test/test-helper-cpgoal-progress.php`
- `test/test-api-zalo-webhook.php`
- `test/test-handler-cpgoal-aftersave.php`
- `test/test-connector-stringee-webhook.php`
- `test/test-view-cpgoal-config.php`

## Running Tests

```bash
# Run single test
php test/test-action-accounts-save.php

# Run multiple tests
php test/test-model-cpgoal-*.php

# Run with output redirect
php test/test-action-accounts-save.php > results.txt 2>&1
```

## Best Practices

1. **Always bootstrap VTiger** with correct path
2. **Use global $current_user** for permission context
3. **Track test records** in `$testRecordIds` array
4. **Clean up records** in cleanup section
5. **Use type casting** for security: `(int)`, `(string)`
6. **Test Vietnamese text** with actual Unicode characters
7. **Mock requests** with `$_REQUEST`, `$_SERVER`, `$_SESSION`
8. **Capture output** with `ob_start()` / `ob_get_clean()`
9. **Exit with proper code** (`0` = success, `1` = failure)
10. **Print clear test names** for debugging

# Standalone Test Script

> Standalone PHP scripts with the VTiger bootstrap are for legacy code (`modules/`, `include/`, `api/`…). Code under `domains/*/tests` uses PHPUnit (`phpunit.xml.dist`; see `rule://domain-tests` and skill `vtiger-phpunit-unit-test`).

## Full Test Script Template

```php
<?php

/**
 * Test: {Feature Name}
 * @author {Your Name}
 * @email {your.email@company.vn}
 * @create date {YYYY.MM.DD}
 */

// Bootstrap VTiger (same as the scripts in test/)
chdir(dirname(__DIR__));
$_SERVER['HTTP_HOST'] = 'localhost';
$_SERVER['REQUEST_METHOD'] = 'POST';
require_once('includes/main/WebUI.php');

// Test assertion functions
function assertResult(string $testName, bool $condition, $actual = null): bool {
	global $passed, $failed, $total;
	$total++;

	if ($condition) {
		$passed++;
		echo "[✓] PASS: {$testName}\n";
		return true;
	}

	$failed++;
	echo "[✗] FAIL: {$testName}\n";

	if ($actual !== null) {
		echo "    Actual: " . var_export($actual, true) . "\n";
	}

	return false;
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
	}
	catch (Throwable $e) {
		$passed++;
		echo "[✓] PASS: {$testName} - " . get_class($e) . ": {$e->getMessage()}\n";
		return true;
	}
}

// Initialize test environment
global $adb, $current_user;
$current_user = Users::getActiveAdminUser();

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
		}
		catch (Throwable $e) {
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

`<Module>_<Component>_<Type>` classes are autoloaded — no `require_once`.

```php
// TC-01: Year picklist starts at the current year
echo "TC-01: Year picklist - normal case\n";

$years = CPGoal_Data_Helper::getYearPickList();

assertEquals('TC-01: Should return 5 years', 5, count($years));
assertEquals('TC-01: First year should be the current year', date('Y'), (string) $years[0]);
echo "\n";

// TC-02: Implement-time picklist has 12 months and 4 quarters
echo "TC-02: Implement-time picklist - web variant\n";

$implementTimes = CPGoal_Data_Helper::getImplementTimePickList();

assertEquals('TC-02: Should have 12 months', 12, count($implementTimes['month']));
assertEquals('TC-02: Should have 4 quarters', 4, count($implementTimes['quarter']));
echo "\n";

// TC-03: Empty period is never a valid implement time
echo "TC-03: Implement-time validation - empty input\n";

assertEquals('TC-03: Empty period should be rejected', false, CPGoal_Data_Helper::isValidImplementTime(''));
echo "\n";
```

### Test Action Controller (Simulate Request)

Resolve the class via `Vtiger_Loader::getComponentClassName()` (falls back to `Vtiger_*` when the module has no override). Use `SaveAjax`: it emits JSON through `Vtiger_Response` (`{"success":true,"result":{…,"_recordId":…}}` or `{"success":false,"error":…}`); `Save` redirects with `header('Location: …')` and prints nothing.

```php
// TC-04: SaveAjax action - happy path
echo "TC-04: SaveAjax action - happy path\n";

$params = [
	'module' => 'Accounts',
	'action' => 'SaveAjax',
	'accountname' => 'Test Account ' . time(),
	'assigned_user_id' => $current_user->id,
];
$request = new Vtiger_Request($params, $params);

try {
	$className = Vtiger_Loader::getComponentClassName('Action', 'SaveAjax', 'Accounts');
	$controller = new $className();

	ob_start();
	$controller->process($request);
	$output = ob_get_clean();

	$response = json_decode($output, true);
	assertResult('TC-04: Response should be JSON', is_array($response), $output);
	assertEquals('TC-04: Response should be success', true, $response['success'] ?? false);

	$recordId = (int) ($response['result']['_recordId'] ?? 0);
	assertResult('TC-04: Record ID should be returned', $recordId > 0, $recordId);

	// Track for cleanup
	if ($recordId > 0) {
		$testRecordIds[] = $recordId;
	}
}
catch (Throwable $e) {
	assertResult('TC-04: Should not throw', false, $e->getMessage());
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
	INNER JOIN vtiger_crmentity ON (crmid = campaignid AND deleted = 0)
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

Use the real fieldnames (`vtiger_field.fieldname`, see `modules/CPGoal/BlocksAndFieldsRegister.php`); picklist values must exist in the picklist table (`vtiger_cpgoal_goal_type`).

```php
// TC-06: Create record
echo "TC-06: Create CPGoal record\n";

$recordModel = Vtiger_Record_Model::getCleanInstance('CPGoal');
$recordModel->set('name', 'Test Goal ' . time());
$recordModel->set('cpgoal_goal_type', 'personal');
$recordModel->set('goal_value', 100000);
$recordModel->set('assigned_user_id', $current_user->id);
$recordModel->save();

$recordId = (int) $recordModel->getId();
assertResult('TC-06: Record ID should be set', $recordId > 0, $recordId);
$testRecordIds[] = $recordId;
echo "\n";

// TC-07: Retrieve and update record
echo "TC-07: Update CPGoal record\n";

$retrievedRecord = Vtiger_Record_Model::getInstanceById($recordId, 'CPGoal');
assertEquals('TC-07: Retrieved goal type should match', 'personal', $retrievedRecord->get('cpgoal_goal_type'));

$retrievedRecord->set('mode', 'edit');
$retrievedRecord->set('goal_value', 150000);
$retrievedRecord->save();

$updatedRecord = Vtiger_Record_Model::getInstanceById($recordId, 'CPGoal');
assertEquals('TC-07: Updated value should match', 150000, (int) $updatedRecord->get('goal_value'));
echo "\n";
```

### Test Integration API (HTTP)

`api/IntegrationAPI/Endpoint.php` dispatches `?platform=<X>` to `include/Webservice/<X>ApiHandler.php`, and every `setResponse()` calls `exit` — so test it over HTTP, not in-process. To unit-test a handler method in-process, see skill `vtiger-unit-test-integration-api-handler`.

```php
// TC-08: Integration API rejects calls without a token
echo "TC-08: Integration API - missing token\n";

global $site_URL;

function callEndpoint(string $url, array $data = [], array $headers = []): array {
	$ch = curl_init($url);
	curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
	curl_setopt($ch, CURLOPT_TIMEOUT, 30);
	curl_setopt($ch, CURLOPT_POST, true);
	curl_setopt($ch, CURLOPT_POSTFIELDS, json_encode($data));
	curl_setopt($ch, CURLOPT_HTTPHEADER, array_merge(['Content-Type: application/json'], $headers));
	$body = curl_exec($ch);
	$httpCode = (int) curl_getinfo($ch, CURLINFO_HTTP_CODE);
	curl_close($ch);

	return ['http_code' => $httpCode, 'body' => json_decode((string) $body, true)];
}

$endpoint = rtrim($site_URL, '/') . '/api/IntegrationAPI/Endpoint.php';

$response = callEndpoint("{$endpoint}?platform=CloudBot&action=getTicketList");
assertEquals('TC-08: Missing token should return 401', 401, $response['http_code']);

$response = callEndpoint("{$endpoint}?platform=NoSuchPlatform");
assertEquals('TC-08: Unknown platform should return 404', 404, $response['http_code']);
echo "\n";
```

### Test Event Handler

Handler classes such as `CPGoalHandler` do not follow `<Module>_<Component>_<Type>`, so they are not autoloaded: `require_once` the handler file. Handlers expose `handleEvent($eventName, $entityData)` with a `VTEntityData`.

```php
// TC-09: Test event handler
echo "TC-09: CPGoal aftersave handler\n";

require_once('include/events/include.inc');
require_once('modules/CPGoal/handlers/CPGoalHandler.php');

// Save a record first (save() already fires the registered handlers once)
$recordModel = Vtiger_Record_Model::getCleanInstance('CPGoal');
$recordModel->set('name', 'Handler Test ' . time());
$recordModel->set('cpgoal_goal_type', 'personal');
$recordModel->set('goal_value', 100);
$recordModel->set('assigned_user_id', $current_user->id);
$recordModel->save();
$testRecordIds[] = (int) $recordModel->getId();

// Trigger the handler manually on the saved record
try {
	$entityData = VTEntityData::fromEntityId($adb, (int) $recordModel->getId(), 'CPGoal');
	$handler = new CPGoalHandler();
	$handler->handleEvent('vtiger.entity.aftersave', $entityData);
	assertResult('TC-09: Handler should execute without error', true);
}
catch (Throwable $e) {
	assertResult('TC-09: Handler failed', false, $e->getMessage());
}
echo "\n";
```

## File Naming Convention

Scripts live in `test/`. Follow one of the two naming styles already used there:

- `test/test-#<ticket>-<feature>.php` — e.g. `test/test-#10432-save-integration.php`, `test/test-#10432-saveajax-regression.php`
- `test/test-<component>-<module>-<feature>.php` — e.g. `test/test-model-cpnotifications-muted-modules.php`, `test/test-model-reports-chart-date-formatting.php`

## Running Tests

```bash
# Run single test
php 'test/test-#10432-save-integration.php'

# Run multiple tests
for f in test/test-model-*.php; do php "$f" || echo "FAILED: $f"; done

# Run with output redirect
php test/test-model-cpnotifications-muted-modules.php > results.txt 2>&1
```

## Best Practices

1. **Always bootstrap VTiger** with correct path
2. **Use global $current_user** for permission context
3. **Track test records** in `$testRecordIds` array
4. **Clean up records** in cleanup section
5. **Use type casting** for security: `(int)`, `(string)`
6. **Test Vietnamese text** with actual Unicode characters
7. **Mock requests** with `Vtiger_Request($params, $params)`; set `$_SERVER` only when the code reads it
8. **Capture output** with `ob_start()` / `ob_get_clean()`
9. **Exit with proper code** (`0` = success, `1` = failure)
10. **Print clear test names** for debugging

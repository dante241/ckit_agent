# VTiger Cron Pattern

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md` + `.omp/rules/php-conventions.md`

## Architecture Overview

The cron pattern uses **2 files** for separation of concerns:

```
cron/modules/<Module>/<Name>.service       → Thin wrapper (3-5 lines), required by vtigercron.php
modules/<Module>/crons/<Component>.php     → Logic class <Module>_<Component>_Cron (autoloaded)
```

**Why this separation?**
- Service file is lightweight, `require_once`d by `vtigercron.php` (root) for each due task
- Logic class contains all business logic, easier to test
- Prevents memory bloat in cron runner process

Alternative seen in the repo (and in the exemplar `cron/modules/CPSocialIntegration/SyncZaloGroupInfo.service`): the service calls a static method on `<Module>_Service_Model` (`modules/<Module>/models/Service.php`). Both are fine; pick the `crons/` class for new single-purpose jobs.

## Service File Template

**Location:** `cron/modules/<Module>/<Name>.service`

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

vimport('includes.runtime.Globals');

$cronService = new Project_CalculateProjectCost_Cron();
$cronService->process();
```

**Rules:**
- Keep to 3-5 lines only
- Always `vimport('includes.runtime.Globals')` first
- No `require_once` for the logic class: `vtigercron.php` registers the vtiger autoloader, which maps `Project_CalculateProjectCost_Cron` → `modules/Project/crons/CalculateProjectCost.php`
- Instantiate and call `process()` method
- NO business logic here

> The autoloader resolves the file name from the class name, so the file MUST be named `<Component>.php` (PascalCase). The legacy `modules/Project/crons/calculateProjectCost.php` is lowercase and only works because its service `require_once`s it — don't copy that.

## Logic Class Template

**Location:** `modules/<Module>/crons/<Component>.php`

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

class Project_CalculateProjectCost_Cron {

	// Main entry point for cron execution
	public function process(): void {
		$records = $this->getRecords();

		foreach ($records as $recordId) {
			try {
				$this->processRecord($recordId);
			}
			catch (Throwable $e) {
				// One failed record must not stop the batch
				error_log("Failed to process record {$recordId}: " . $e->getMessage());
			}
		}
	}

	protected function processRecord(int $recordId): void {
		$recordModel = Vtiger_Record_Model::getInstanceById($recordId, 'Project');
		$cost = $this->calculateCost($recordModel);

		$recordModel->set('mode', 'edit');
		$recordModel->set('actual_cost', $cost);
		$recordModel->save();
	}

	protected function getRecords(): array {
		global $adb;

		$sql = "SELECT projectid
			FROM vtiger_project
			INNER JOIN vtiger_crmentity ON (crmid = projectid AND deleted = 0)
			WHERE projectstatus = ?";
		$result = $adb->pquery($sql, ['Open']);

		$ids = [];

		while ($row = $adb->fetchByAssoc($result)) {
			$row = decodeUTF8($row);
			$ids[] = (int) $row['projectid'];
		}

		return $ids;
	}

	protected function calculateCost(Vtiger_Record_Model $record): float {
		// Calculation logic
		return 0.0;
	}
}
```

## Class Naming Convention

Format: `{Module}_{Component}_Cron` in file `modules/{Module}/crons/{Component}.php`

**Examples:**
- `Project_CalculateProjectCost_Cron` → `modules/Project/crons/CalculateProjectCost.php`
- `Accounts_SyncExternalData_Cron` → `modules/Accounts/crons/SyncExternalData.php`
- `CPNotifications_Cleanup_Cron` → `modules/CPNotifications/crons/Cleanup.php`

## Critical Patterns

### 1. Error Isolation Per Record

**ALWAYS** wrap individual record processing in try-catch:

```php
foreach ($records as $recordId) {
	try {
		$this->processRecord($recordId);
	}
	catch (Throwable $e) {
		error_log("Error processing {$recordId}: " . $e->getMessage());
	}
}
```

**Why?** One failed record shouldn't stop entire batch. `vtigercron.php` itself only catches `Exception` around each task, so an uncaught `Error` aborts the whole cron run.

### 2. Global $adb for Database

```php
global $adb;

$sql = "SELECT field FROM table WHERE condition = ?";
$result = $adb->pquery($sql, [$param]);

while ($row = $adb->fetchByAssoc($result)) {
	$row = decodeUTF8($row);
	// Process row
}
```

**Rules:**
- Always use `pquery()` with params (never concatenate SQL)
- Always call `decodeUTF8()` on `fetchByAssoc()` results
- Type cast extracted values: `(int)`, `(string)`

### 3. Record Model Pattern

```php
try {
	$recordModel = Vtiger_Record_Model::getInstanceById($recordId);
}
catch (Throwable $e) {
	// Record may not exist or be deleted
	return;
}

$recordModel->set('mode', 'edit');
$recordModel->set('field_name', $value);
$recordModel->save();
```

### 4. Batch Processing for Large Datasets

```php
protected function getRecords(): array {
	global $adb;

	// Limit batch size to prevent memory issues; the next run picks up the rest
	$sql = "SELECT id FROM table
		WHERE status = ?
		LIMIT 1000";

	// Process in chunks
}
```

## Common Use Cases

### 1. Data Synchronization

```php
class Accounts_SyncExternalData_Cron {

	public function process(): void {
		$accounts = $this->getAccountsToSync();

		foreach ($accounts as $accountId) {
			try {
				$externalData = $this->fetchExternalData($accountId);
				$this->updateRecord($accountId, $externalData);
			}
			catch (Throwable $e) {
				error_log("Sync failed for {$accountId}: " . $e->getMessage());
			}
		}
	}
}
```

### 2. Cleanup Old Records

```php
class CPNotifications_Cleanup_Cron {

	public function process(): void {
		global $adb;

		$sql = "DELETE FROM vtiger_notifications
			WHERE created_time < DATE_SUB(NOW(), INTERVAL 90 DAY)";
		$adb->pquery($sql, []);
	}
}
```

### 3. Periodic Calculations

```php
class Accounts_CalculateMetrics_Cron {

	public function process(): void {
		$accounts = $this->getRecords();

		foreach ($accounts as $accountId) {
			try {
				$metrics = $this->calculateMetrics($accountId);
				$this->saveMetrics($accountId, $metrics);
			}
			catch (Throwable $e) {
				error_log("Metrics failed for {$accountId}: " . $e->getMessage());
			}
		}
	}
}
```

## Testing Locally

`vtigercron.php` lives at the repo root and ignores CLI arguments; the only task selector is the `service` request param (= `vtiger_cron_task.name`).

```bash
# Run all due cron tasks (what cron/vtigercron.sh does)
php vtigercron.php

# Run one task (by vtiger_cron_task.name) through the real runner
php -r '$_REQUEST["service"] = "CalculateProjectCost"; require "vtigercron.php";'

# The runner still applies isRunnable() (frequency since lastend) and skips run_by='Supervisor' rows;
# "not ready to run" → reset laststart/lastend of that row in vtiger_cron_task to force it.
# Over HTTP the same selector is <site_URL>/vtigercron.php?service=<name> (admin session required).

# Check output for errors
tail -f logs/vtigercrm.log
```

## Common Pitfalls

1. **Service File Bloat**: Never put logic in .service file
2. **No Error Isolation**: One error shouldn't stop entire batch
3. **Missing decodeUTF8()**: Character encoding issues
4. **SQL Concatenation**: Security risk, use pquery()
5. **No Batch Limit**: Memory exhaustion on large datasets
6. **Missing Type Casts**: Type safety for request/DB data
7. **No Logging**: Silent failures are hard to debug

## Next Steps

1. Create service wrapper in `cron/modules/<Module>/`
2. Create logic class in `modules/<Module>/crons/<Component>.php`
3. Register via migration (see `references/registration.md`)
4. Test locally before deploying
5. Monitor logs after deployment

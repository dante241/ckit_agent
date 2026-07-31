# VTiger Cron Pattern

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## Architecture Overview

The cron pattern uses **2 files** for separation of concerns:

```
cron/modules/<Module>/<Name>.service    → Thin wrapper (3-5 lines)
modules/<Module>/crons/<name>.php       → Actual logic class
```

**Why this separation?**
- Service file is lightweight, loaded by vtigercron.php
- Logic class contains all business logic, easier to test
- Prevents memory bloat in cron runner process

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
require_once('modules/Project/crons/calculateProjectCost.php');

$cronService = new Project_CalculateProjectCost_Cron();
$cronService->process();
```

**Rules:**
- Keep to 3-5 lines only
- Always `vimport('includes.runtime.Globals')` first
- Single require_once for logic class
- Instantiate and call `process()` method
- NO business logic here

## Logic Class Template

**Location:** `modules/<Module>/crons/<name>.php`

```php
<?php
/**
 * @name Project_CalculateProjectCost_Cron
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

class Project_CalculateProjectCost_Cron {

    /**
     * Main entry point for cron execution
     */
    public function process(): void {
        $records = $this->getRecords();

        foreach ($records as $recordId) {
            try {
                $this->processRecord($recordId);
            } catch (\Throwable $th) {
                // Log error but continue processing other records
                error_log("Failed to process record {$recordId}: " . $th->getMessage());
            }
        }
    }

    /**
     * Process individual record
     */
    protected function processRecord(int $recordId): void {
        try {
            $recordModel = Vtiger_Record_Model::getInstanceById($recordId);

            // Business logic here
            $cost = $this->calculateCost($recordModel);

            $recordModel->set('mode', 'edit');
            $recordModel->set('total_cost', $cost);
            $recordModel->save();

        } catch (\Throwable $th) {
            throw $th; // Re-throw for outer catch
        }
    }

    /**
     * Get records to process
     */
    protected function getRecords(): array {
        global $adb;

        $sql = "SELECT projectid
                FROM vtiger_project
                INNER JOIN vtiger_crmentity ON (crmid = projectid AND deleted = 0)
                WHERE projectstatus = ?";

        $result = $adb->pquery($sql, ['Open']);

        $ids = [];
        while ($row = $adb->fetchByAssoc($result)) {
            $row = decodeUTF8($row); // CRITICAL: decode UTF-8
            $ids[] = (int) $row['projectid'];
        }

        return $ids;
    }

    /**
     * Calculate project cost
     */
    protected function calculateCost(Vtiger_Record_Model $record): float {
        // Calculation logic
        return 0.0;
    }
}
```

## Class Naming Convention

Format: `{Module}_{Name}_Cron`

**Examples:**
- `Project_CalculateProjectCost_Cron`
- `Accounts_SyncExternalData_Cron`
- `CPNotifications_CleanupOldNotifications_Cron`

## Critical Patterns

### 1. Error Isolation Per Record

**ALWAYS** wrap individual record processing in try-catch:

```php
foreach ($records as $recordId) {
    try {
        $this->processRecord($recordId);
    } catch (\Throwable $th) {
        // Log but continue with next record
        error_log("Error processing {$recordId}: " . $th->getMessage());
    }
}
```

**Why?** One failed record shouldn't stop entire batch.

### 2. Global $adb for Database

```php
global $adb;

$sql = "SELECT field FROM table WHERE condition = ?";
$result = $adb->pquery($sql, [$param]);

while ($row = $adb->fetchByAssoc($result)) {
    $row = decodeUTF8($row); // CRITICAL
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
} catch (\Throwable $th) {
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

    // LIMIT batch size to prevent memory issues
    $sql = "SELECT id FROM table
            WHERE status = ?
            LIMIT 1000"; // Batch limit

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
            } catch (\Throwable $th) {
                error_log("Sync failed for {$accountId}: " . $th->getMessage());
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

        $sql = "DELETE FROM vtiger_cpnotifications
                WHERE created_date < DATE_SUB(NOW(), INTERVAL 90 DAY)";

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
            } catch (\Throwable $th) {
                // Continue
            }
        }
    }
}
```

## Testing Locally

```bash
# Run all cron tasks
php cron/vtigercron.php

# Run specific task by name
php cron/vtigercron.php -m "CalculateProjectCost"

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
2. Create logic class in `modules/<Module>/crons/`
3. Register via migration (see `references/registration.md`)
4. Test locally before deploying
5. Monitor logs after deployment

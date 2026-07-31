# VTiger Cron Task Registration

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## Registration Methods

There are two ways to register cron tasks:

1. **Via Migration (Recommended)** - Versioned, tracked, repeatable
2. **Manual SQL** - Quick testing, not recommended for production

## Method 1: Via Migration (Recommended)

**Location:** `modules/CPMigration/migrations/YYYY.MM.DD.HH.mm.ss_RegisterCronTask.php`

```php
<?php
/**
 * @name RegisterCalculateProjectCost
 * @create date 2025-02-11 10:30:00
 */

return new class extends CPMigration_Base_Model {

    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        // Register cron task
        $this->createCronjob(
            'CalculateProjectCost',                          // Task name
            3600,                                             // Frequency (seconds)
            'Project',                                        // Module
            'cron/modules/Project/CalculateProjectCost.service',  // Handler file
            'Cronjob'                                         // run_by
        );

        return self::UP_SUCCESS;
    }

    public function down(): int {
        // Remove cron task
        $GLOBALS['adb']->pquery("DELETE FROM vtiger_cron_task WHERE name = ?", ['CalculateProjectCost']);

        return self::DOWN_SUCCESS;
    }
};
```

### Multiple Cron Registration

```php
public function up(): int {
    // Register multiple tasks
    $cronTasks = [
        [
            'name' => 'SyncAccounts',
            'frequency' => 3600,
            'module' => 'Accounts',
            'handler' => 'cron/modules/Accounts/SyncAccounts.service',
            'run_by' => 'Cronjob'
        ],
        [
            'name' => 'CleanupOldNotifications',
            'frequency' => 86400,
            'module' => 'CPNotifications',
            'handler' => 'cron/modules/CPNotifications/Cleanup.service',
            'run_by' => 'Cronjob'
        ],
        [
            'name' => 'ProcessNotifications',
            'frequency' => 0,
            'module' => 'CPNotifications',
            'handler' => 'cron/modules/CPNotifications/ProcessNotifications.service',
            'run_by' => 'Supervisor'  // Continuous queue processing
        ]
    ];

    foreach ($cronTasks as $task) {
        $this->createCronjob(
            $task['name'],
            $task['frequency'],
            $task['module'],
            $task['handler'],
            $task['run_by']
        );
    }

    return self::UP_SUCCESS;
}
```

## createCronjob() Method Signature

From `CPMigration_Base_Model`:

```php
protected function createCronjob(
    string $name,              // Unique task name
    int $frequency,            // Seconds between runs (0 for supervisor)
    string $module,            // Module name
    string $handlerFile,       // Path to .service file
    string $runBy = 'Cronjob', // 'Cronjob' or 'Supervisor'
    string $description = '',  // Optional description
    int $status = 1,           // 1=enabled, 0=disabled
    int $sequence = null       // Execution order (auto-increment if null)
): void
```

## vtiger_cron_task Table Structure

```sql
CREATE TABLE `vtiger_cron_task` (
  `id` int(11) NOT NULL AUTO_INCREMENT,
  `name` varchar(100) NOT NULL,
  `handler_file` varchar(255) NOT NULL,
  `frequency` int(11) NOT NULL COMMENT 'Seconds between runs',
  `laststart` int(11) DEFAULT NULL COMMENT 'Unix timestamp',
  `lastend` int(11) DEFAULT NULL COMMENT 'Unix timestamp',
  `status` int(11) DEFAULT 1 COMMENT '0=disabled, 1=enabled, 2=running',
  `sequence` int(11) NOT NULL,
  `module` varchar(100) DEFAULT NULL,
  `description` text,
  `run_by` varchar(20) DEFAULT 'Cronjob' COMMENT 'Cronjob or Supervisor',
  PRIMARY KEY (`id`),
  UNIQUE KEY `name` (`name`)
);
```

**Key Fields:**

| Field | Type | Purpose |
|-------|------|---------|
| `name` | VARCHAR(100) | Unique identifier for task |
| `handler_file` | VARCHAR(255) | Path to .service file |
| `frequency` | INT | Seconds between runs (0 for supervisor) |
| `laststart` | INT | Unix timestamp of last execution start |
| `lastend` | INT | Unix timestamp of last execution end |
| `status` | INT | 0=disabled, 1=enabled, 2=running |
| `sequence` | INT | Execution order (lower = earlier) |
| `module` | VARCHAR(100) | Module name for organization |
| `description` | TEXT | Human-readable description |
| `run_by` | VARCHAR(20) | 'Cronjob' or 'Supervisor' |

## Frequency Values Reference

| Interval | Seconds | Constant | Use Case |
|----------|---------|----------|----------|
| 1 minute | 60 | | Real-time monitoring |
| 5 minutes | 300 | `Vtiger_Cron::FREQUENCY_5MIN` | Frequent sync |
| 15 minutes | 900 | `Vtiger_Cron::FREQUENCY_15MIN` | Regular updates |
| 30 minutes | 1800 | | Moderate frequency |
| 1 hour | 3600 | `Vtiger_Cron::FREQUENCY_1HOUR` | Hourly tasks |
| 12 hours | 43200 | `Vtiger_Cron::FREQUENCY_12HOURS` | Twice daily |
| 1 day | 86400 | `Vtiger_Cron::FREQUENCY_1DAY` | Daily cleanup |
| 7 days | 604800 | `Vtiger_Cron::FREQUENCY_1WEEK` | Weekly reports |
| 0 (continuous) | 0 | | Supervisor queue processing |

**Using constants:**

```php
$this->createCronjob(
    'HourlySync',
    Vtiger_Cron::FREQUENCY_1HOUR,
    'Accounts',
    'cron/modules/Accounts/HourlySync.service'
);
```

## run_by Values

### 'Cronjob' (Default)

```php
$this->createCronjob(
    'TaskName',
    3600,
    'Module',
    'cron/modules/Module/Task.service',
    'Cronjob'  // Periodic execution via vtigercron.php
);
```

**Characteristics:**
- Runs on schedule via `cron/vtigercron.php`
- Executes once per frequency interval
- Stops after completion
- Best for periodic tasks

### 'Supervisor'

```php
$this->createCronjob(
    'ProcessQueue',
    0,  // Frequency ignored for supervisor tasks
    'CPNotifications',
    'cron/modules/CPNotifications/ProcessQueue.service',
    'Supervisor'  // Continuous execution via supervisord
);
```

**Characteristics:**
- Runs continuously via supervisord
- Never stops (infinite loop)
- Frequency should be 0
- Best for queue processing
- Requires supervisord configuration

## Delete Cronjob in down()

**WARNING:** `CPMigration_Base_Model` does NOT have a `deleteCronjob()` method.
Use direct SQL delete instead:

```php
public function down(): int {
    $GLOBALS['adb']->pquery("DELETE FROM vtiger_cron_task WHERE name = ?", ['CalculateProjectCost']);
    return self::DOWN_SUCCESS;
}
```

## Method 2: Manual SQL (Testing Only)

**Not recommended for production**, but useful for quick testing:

```sql
INSERT INTO vtiger_cron_task (name, handler_file, frequency, status, sequence, module, description, run_by)
VALUES (
    'TestTask',
    'cron/modules/Project/TestTask.service',
    3600,
    1,
    (SELECT IFNULL(MAX(sequence), 0) + 1 FROM vtiger_cron_task AS t),
    'Project',
    'Testing cron task',
    'Cronjob'
);
```

**Delete:**

```sql
DELETE FROM vtiger_cron_task WHERE name = 'TestTask';
```

## Running Cron Tasks

### Run All Enabled Tasks

```bash
php cron/vtigercron.php
```

### Run Specific Task by Name

```bash
php cron/vtigercron.php -m "CalculateProjectCost"
```

### Run with Debug Output

```bash
php cron/vtigercron.php --debug
```

### System Cron Setup

Add to system crontab (run every minute):

```bash
* * * * * cd /path/to/vtiger && php cron/vtigercron.php > /dev/null 2>&1
```

**VTiger's cron runner** checks each task's frequency and `lastend` to determine if it should run.

## Checking Task Status

### Via SQL

```sql
-- View all tasks
SELECT id, name, frequency, status, module, run_by, FROM_UNIXTIME(laststart) as last_run
FROM vtiger_cron_task
ORDER BY sequence;

-- Check specific task
SELECT * FROM vtiger_cron_task WHERE name = 'CalculateProjectCost';

-- Find running tasks (potential stuck tasks)
SELECT name, FROM_UNIXTIME(laststart) as started_at
FROM vtiger_cron_task
WHERE status = 2;

-- Calculate next run time
SELECT
    name,
    FROM_UNIXTIME(lastend) as last_completed,
    FROM_UNIXTIME(lastend + frequency) as next_run,
    frequency / 60 as frequency_minutes
FROM vtiger_cron_task
WHERE status = 1 AND run_by = 'Cronjob'
ORDER BY next_run;
```

### Via VTiger UI

Navigate to: **Settings → CRM Settings → Scheduler**

View:
- Task name and status
- Last execution time
- Next scheduled run
- Enable/disable tasks

## Common Patterns

### 1. Standard Cron Task

```php
// Hourly sync
$this->createCronjob(
    'SyncExternalData',
    3600,
    'Accounts',
    'cron/modules/Accounts/SyncExternalData.service',
    'Cronjob'
);
```

### 2. Daily Cleanup

```php
// Daily at midnight (via system cron)
$this->createCronjob(
    'CleanupOldRecords',
    86400,
    'Settings',
    'cron/modules/Settings/CleanupOldRecords.service',
    'Cronjob'
);
```

### 3. Queue Processor

```php
// Continuous supervisor process
$this->createCronjob(
    'ProcessNotifications',
    0,  // Frequency ignored
    'CPNotifications',
    'cron/modules/CPNotifications/ProcessNotifications.service',
    'Supervisor'  // Requires supervisord config
);
```

## Troubleshooting

### Task Not Running

1. Check status: `SELECT status FROM vtiger_cron_task WHERE name = 'TaskName'`
2. Enable if disabled: `UPDATE vtiger_cron_task SET status = 1 WHERE name = 'TaskName'`
3. Check system cron: `crontab -l`
4. Check laststart/lastend: may be stuck in "running" status

### Task Stuck in "Running" Status

```sql
-- Reset stuck task
UPDATE vtiger_cron_task
SET status = 1, lastend = UNIX_TIMESTAMP()
WHERE name = 'TaskName' AND status = 2;
```

### Supervisor Task Not Starting

1. Check supervisord config exists
2. Reload supervisor: `sudo supervisorctl reread && sudo supervisorctl update`
3. Check supervisor status: `sudo supervisorctl status`
4. View logs: `tail -f /var/log/supervisor/vtiger-*.log`

## Best Practices

1. **Always use migrations** for registration (versioned, tracked)
2. **Use descriptive names** (no spaces, CamelCase)
3. **Set appropriate frequency** (don't over-poll)
4. **Add descriptions** for maintenance clarity
5. **Test manually first** (`php cron/vtigercron.php -m "TaskName"`)
6. **Monitor execution time** (lastend - laststart)
7. **Handle errors gracefully** in task logic
8. **Log task output** for debugging
9. **Use sequence** to control execution order if needed
10. **Supervisor tasks need config** in `/etc/supervisor/conf.d/`

## Complete Example

**Migration:** `modules/CPMigration/migrations/2025.02.11.10.30.00_RegisterProjectCrons.php`

```php
<?php
return new class extends CPMigration_Base_Model {

    public function up(): int {
        // Hourly calculation
        $this->createCronjob(
            'CalculateProjectCost',
            Vtiger_Cron::FREQUENCY_1HOUR,
            'Project',
            'cron/modules/Project/CalculateProjectCost.service',
            'Cronjob',
            'Calculate total project costs hourly'
        );

        // Daily report
        $this->createCronjob(
            'GenerateProjectReport',
            Vtiger_Cron::FREQUENCY_1DAY,
            'Project',
            'cron/modules/Project/GenerateProjectReport.service',
            'Cronjob',
            'Generate daily project status reports'
        );

        return self::UP_SUCCESS;
    }

    public function down(): int {
        $GLOBALS['adb']->pquery("DELETE FROM vtiger_cron_task WHERE name = ?", ['CalculateProjectCost']);
        $GLOBALS['adb']->pquery("DELETE FROM vtiger_cron_task WHERE name = ?", ['GenerateProjectReport']);

        return self::DOWN_SUCCESS;
    }
};
```

## Next Steps

1. Create cron service file (see `references/cron-pattern.md`)
2. Create logic class
3. Create registration migration
4. Test manually: `php cron/vtigercron.php -m "TaskName"`
5. Verify in UI: Settings → Scheduler
6. Monitor execution logs

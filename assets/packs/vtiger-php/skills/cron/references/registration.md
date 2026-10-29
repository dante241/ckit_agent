# VTiger Cron Task Registration

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md` + `.omp/rules/migration-rules.md`
>
> This file is the canonical reference for cron registration (`createCronjob`, `vtiger_cron_task`, running/troubleshooting). Generic migration mechanics live in the `migration` skill.

## Registration Methods

1. **Via Migration (Recommended)** - Versioned, tracked, repeatable
2. **Manual SQL** - Quick local testing only

## Method 1: Via Migration (Recommended)

**Location:** `modules/CPMigration/migrations/YYYY.MM.DD.HH.mm.ss_RegisterCronTask.php`

Real exemplar: `modules/CPMigration/migrations/2026.06.22.12.35.00_SyncZaloGroupInfoCron.php`.

```php
<?php

/**
 * @name RegisterCalculateProjectCost
 * @create date 2025-02-11 10:30:00
 */

return new class extends CPMigration_Base_Model {

	protected $isRunBeforeQuickRepair = false;

	public function up(): int {
		$this->createCronjob(
			'CalculateProjectCost',
			3600,
			'Project',
			'cron/modules/Project/CalculateProjectCost.service'
		);

		return self::UP_SUCCESS;
	}

	public function down(): int {
		$this->pquery("DELETE FROM vtiger_cron_task WHERE name = ?", ['CalculateProjectCost']);
		return self::DOWN_SUCCESS;
	}
};
```

### Multiple Cron Registration

```php
public function up(): int {
	$cronTasks = [
		['SyncAccounts', 3600, 'Accounts', 'cron/modules/Accounts/SyncAccounts.service', 'Cronjob'],
		['CleanupOldNotifications', 86400, 'CPNotifications', 'cron/modules/CPNotifications/Cleanup.service', 'Cronjob'],
		['SendWebhookEvent', 60, 'CPWebhook', 'cron/modules/CPWebhook/ProcessWebhookQueue.service', 'Supervisor'],
	];

	foreach ($cronTasks as [$name, $frequency, $moduleName, $handlerFile, $runBy]) {
		$this->createCronjob($name, $frequency, $moduleName, $handlerFile, $runBy);
	}

	return self::UP_SUCCESS;
}
```

## createCronjob() Method Signature

From `modules/CPMigration/models/Base.php`:

```php
protected function createCronjob(string $name, int $frequencySeconds, string $moduleName, string $handlerFile, string $runBy = 'Cronjob')
```

What it does:
- `DELETE FROM vtiger_cron_task WHERE name = ?` first → re-running the migration re-creates the row (idempotent by name)
- `sequence` = `MAX(sequence) + 1`, `status` = 1 (enabled)
- `description` = `"Recommended frequency for <name> is <N> mins"` — there is no description parameter; to set a custom (e.g. Vietnamese) description, `UPDATE vtiger_cron_task SET description = ? WHERE name = ?` afterwards (see `2026.04.29.10.00.00_UpdateCronTaskDescriptions.php`)

## vtiger_cron_task Table Structure

Live DDL (`SHOW CREATE TABLE vtiger_cron_task`):

```sql
CREATE TABLE `vtiger_cron_task` (
  `id` int NOT NULL AUTO_INCREMENT,
  `name` varchar(100) DEFAULT NULL,
  `handler_file` varchar(100) DEFAULT NULL,
  `frequency` int DEFAULT NULL,
  `laststart` int unsigned DEFAULT NULL,
  `lastend` int unsigned DEFAULT NULL,
  `status` int DEFAULT NULL,
  `module` varchar(100) DEFAULT NULL,
  `sequence` int DEFAULT NULL,
  `run_by` text,
  `description` text,
  PRIMARY KEY (`id`) USING BTREE,
  UNIQUE KEY `name` (`name`) USING BTREE,
  UNIQUE KEY `handler_file` (`handler_file`) USING BTREE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb3;
```

**Key Fields:**

| Field | Type | Purpose |
|-------|------|---------|
| `name` | VARCHAR(100), unique | Task identifier (= queue name for Supervisor tasks) |
| `handler_file` | VARCHAR(100), **unique** | Path to .service file — one handler file cannot back two tasks; keep the path ≤ 100 chars |
| `frequency` | INT | Seconds between runs (Supervisor rows: 60) |
| `laststart` / `lastend` | INT UNSIGNED | Unix timestamps of last start/end |
| `status` | INT | `Vtiger_Cron::$STATUS_*`: 0 disabled, 1 enabled, 2 running, 3 completed |
| `sequence` | INT | Execution order (lower = earlier) |
| `module` | VARCHAR(100) | Module name for organization |
| `run_by` | TEXT | `'Cronjob'` or `'Supervisor'` |
| `description` | TEXT | Shown in Settings > Scheduler |

## Frequency Values Reference

`Vtiger_Cron` has no frequency constants — pass integer seconds.

| Interval | Seconds | Use Case |
|----------|---------|----------|
| 1 minute | 60 | Real-time monitoring; all Supervisor rows |
| 5 minutes | 300 | Frequent sync |
| 15 minutes | 900 | Regular updates |
| 30 minutes | 1800 | Moderate frequency |
| 1 hour | 3600 | Hourly tasks |
| 12 hours | 43200 | Twice daily |
| 1 day | 86400 | Daily cleanup |
| 7 days | 604800 | Weekly reports |

A `Cronjob` task becomes runnable when `time() - lastend >= frequency - 60` (`Vtiger_Cron::isRunnable()`), and never while it is still running (`lastend < laststart`).

## run_by Values

### 'Cronjob' (Default)

```php
$this->createCronjob('TaskName', 3600, 'Module', 'cron/modules/Module/Task.service');
```

**Characteristics:**
- Run by `vtigercron.php` (repo root) from the system crontab
- Executes once per frequency interval, then stops
- Best for periodic tasks

### 'Supervisor'

```php
$this->createCronjob('SendNotification', 60, 'CPNotifications', 'cron/modules/CPNotifications/processNotificationQueue.service', 'Supervisor');
```

**Characteristics:**
- Skipped by `vtigercron.php`; run by a Supervisor program that restarts the service after every batch
- `frequency` = 60 by convention (all existing rows); it does not drive scheduling
- The Supervisor program is created through `CPQueue_Supervisor_Provider` when the task is saved in Settings > Scheduler or on QuickRepair — see `references/queue-pattern.md`

## Delete Cronjob in down()

`CPMigration_Base_Model` has no `deleteCronjob()` method. Use direct SQL:

```php
public function down(): int {
	$this->pquery("DELETE FROM vtiger_cron_task WHERE name = ?", ['CalculateProjectCost']);
	return self::DOWN_SUCCESS;
}
```

## Method 2: Manual SQL (Local Testing Only)

```sql
INSERT INTO vtiger_cron_task (name, handler_file, frequency, status, sequence, module, description, run_by)
SELECT 'TestTask', 'cron/modules/Project/TestTask.service', 3600, 1, IFNULL(MAX(sequence), 0) + 1, 'Project', 'Testing cron task', 'Cronjob'
FROM vtiger_cron_task;

DELETE FROM vtiger_cron_task WHERE name = 'TestTask';
```

## Running Cron Tasks

`vtigercron.php` is at the repo root; it ignores CLI arguments.

```bash
# Run all due Cronjob tasks
php vtigercron.php

# Run one task by name (still subject to isRunnable(); unknown name → fatal)
php -r '$_REQUEST["service"] = "CalculateProjectCost"; require "vtigercron.php";'
```

Over HTTP the selector is `<site_URL>/vtigercron.php?service=<name>` (requires an admin session).

### System Cron Setup

`cron/vtigercron.sh` cds to the repo root and runs `php -f vtigercron.php`:

```bash
* * * * * sh /path/to/vtiger/cron/vtigercron.sh > /dev/null 2>&1
```

## Checking Task Status

### Via SQL

```sql
-- View all tasks
SELECT id, name, frequency, status, module, run_by, FROM_UNIXTIME(laststart) AS last_run
FROM vtiger_cron_task
ORDER BY sequence;

-- Find running tasks (potential stuck tasks)
SELECT name, FROM_UNIXTIME(laststart) AS started_at
FROM vtiger_cron_task
WHERE status = 2;

-- Next run time of Cronjob tasks
SELECT name,
	FROM_UNIXTIME(lastend) AS last_completed,
	FROM_UNIXTIME(lastend + frequency) AS next_run,
	frequency / 60 AS frequency_minutes
FROM vtiger_cron_task
WHERE status = 1 AND run_by = 'Cronjob'
ORDER BY next_run;
```

### Via VTiger UI

**Settings → Scheduler** (`modules/Settings/CronTasks`): status, last start/end, frequency, enable/disable, Supervisor process control.

## Troubleshooting

### Task Not Running

1. Check status: `SELECT status, laststart, lastend FROM vtiger_cron_task WHERE name = 'TaskName'`
2. Enable if disabled: `UPDATE vtiger_cron_task SET status = 1 WHERE name = 'TaskName'`
3. Check system cron: `crontab -l`
4. `run_by = 'Supervisor'` rows never run from `vtigercron.php`

### Task Stuck in "Running" Status

`vtigercron.php` calls `Vtiger_Cron::resetStuckServices()` on every run: rows with `status = 2` started more than 30 minutes ago get `laststart = lastend = 0`. Manual reset:

```sql
UPDATE vtiger_cron_task
SET status = 1, lastend = UNIX_TIMESTAMP()
WHERE name = 'TaskName' AND status = 2;
```

### Supervisor Task Not Starting

1. Row exists, `status = 1`, `run_by = 'Supervisor'`
2. Re-save the task in Settings → Scheduler (or run QuickRepair) to (re)create the Supervisor process
3. On the server: `sudo supervisorctl status | grep <domain>_<TaskName>`

## Best Practices

1. **Always use migrations** for registration (versioned, tracked)
2. **Use descriptive names** (no spaces, PascalCase)
3. **Set appropriate frequency** (don't over-poll)
4. **Unique handler file per task** (DB unique key)
5. **Test manually first** (`php -r '$_REQUEST["service"] = "TaskName"; require "vtigercron.php";'`)
6. **Monitor execution time** (lastend - laststart)
7. **Handle errors gracefully** in task logic
8. **Use sequence** to control execution order if needed

## Next Steps

1. Create cron service file (see `references/cron-pattern.md`)
2. Create logic class
3. Create registration migration
4. Test manually (see Running Cron Tasks)
5. Verify in UI: Settings → Scheduler
6. Monitor execution logs

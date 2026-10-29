# VTiger Queue Pattern

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## Architecture Overview

A queue is identified by ONE name used in three places: the job class name, `queue_tasks.task_name`, and `vtiger_cron_task.name` of its consumer (e.g. `SendNotification`).

```
modules/CPQueue/Jobs/<JobName>.php            → Job class: static run(Task $task) = the real work per task
cron/modules/<Module>/<Name>.service          → Thin consumer wrapper (3-5 lines), run by Supervisor
modules/<Module>/services/<ProcessName>.php   → Consumer class extends BaseProcess (optional hooks)
```

Producers enqueue with `CPQueue_Server_Model::addTask('<JobName>', $data)`; the consumer pulls one batch of `<JobName>` tasks, runs `<JobName>::run()` for each, exits; Supervisor restarts it.

**Why this pattern?**
- Near-real-time processing of queued tasks
- Runs under Supervisor (not the periodic `vtigercron.php`, which skips `run_by='Supervisor'` rows)
- Batch limiting and per-task status tracking come from CPQueue (php-queue library)

## When to Use Queue Pattern

| Use Queue For | Don't Use For |
|---------------|---------------|
| Real-time notifications | Hourly/daily reports |
| Webhook processing | Data cleanup |
| Chat message delivery | Periodic sync |
| Push notifications | Batch calculations |
| SMS sending | Analytics updates |
| Email queues | Backup tasks |

**Rule of thumb:** If it needs to happen "now" (within seconds), use queue pattern.

## 1. Job Class (the work)

**Location:** `modules/CPQueue/Jobs/<JobName>.php` — the class name MUST equal the queue/task name. `CPQueue_Server_Model`'s constructor registers `modules/CPQueue/Jobs` with the php-queue autoloader, so no require is needed by the consumer.

Real example: `modules/CPQueue/Jobs/SendNotification.php`, `modules/CPQueue/Jobs/ProcessEmailQueue.php`.

```php
<?php

/*
	SendWebhookEvent
	Author: Your Name
	Date: YYYY-MM-DD
*/

use PhpQueue\Task;

require_once 'modules/CPQueue/include.php';

class SendWebhookEvent extends Vtiger_Base_Model implements PhpQueue\Interfaces\IJob {

	public static function run(Task $task) {
		$data = $task->get_request_data();

		if (empty($data)) {
			return;
		}

		// Business logic for ONE task, e.g. call a helper of the owning module
		CPWebhook_Logic_Helper::send($data);
	}
}
```

`CPWebhook_Logic_Helper` is a placeholder for your module's helper.

## 2. Enqueue Tasks (producer)

```php
// $data is stored as queue_tasks.request_data
$queue = CPQueue_Server_Model::getCleanInstance();
$queue->addTask('SendWebhookEvent', ['record_id' => $recordId, 'event' => 'updated'], 3);	// 3 = max retries
```

Signature: `addTask($job = "", $data = [], $maxRetries = 0, $callBack = false, $priority = 0, $group_id = 0, $subTask = [], $date = null)` (`modules/CPQueue/models/Server.php`). Returns the task id or `false` (failure is logged via `saveLog('QUEUE', …)`). On a hot path that enqueues many tasks per request, `CPQueue_Server_Model::getInstance()` reuses one instance/PDO connection (only for queues not listed in `$queueConfig['session_manager_queue']`).

## 3. Consumer Service File

**Location:** `cron/modules/<Module>/<Name>.service`

```php
<?php

require_once('modules/CPNotifications/services/ProcessSendNotification.php');
$process = new ProcessSendNotification('SendNotification', 200);
$process->processQueue();
```

(Verbatim: `cron/modules/CPNotifications/processNotificationQueue.service`, registered as `vtiger_cron_task.name = 'SendNotification'`.)

**Parameters:**
- First param: queue name — must equal the job class name AND the `vtiger_cron_task.name` row
- Second param: batch limit (default 200; keep it moderate, 500 is a sensible upper bound — nothing enforces a cap)

## 4. Consumer Process Class

**Location:** `modules/<Module>/services/<ProcessName>.php`

```php
<?php

/*
	ProcessSendNotification Service
	Author: Your Name
	Date: YYYY-MM-DD
*/

include_once('modules/CPQueue/models/BaseProcess.php');

class ProcessSendNotification extends BaseProcess {

	protected function beforeExecuteTasks() {
		// Optional: load config once per batch
	}

	protected function afterExecuteTasks() {
		// Optional: cleanup, batch stats
	}
}
```

Both hooks are optional (the base class has empty implementations); the task logic does NOT go here — it lives in the job's `run()`.

## BaseProcess Framework (real code)

`modules/CPQueue/models/BaseProcess.php` (abstract):

```php
abstract class BaseProcess {
	protected $service;					// CPQueue_Server_Model::getCleanInstance()
	protected $queueName;
	protected $queueProcessBatchLimit;

	public function __construct($queueName, $queueProcessBatchLimit = 200) { … }
	protected function beforeExecuteTasks() { }
	protected function afterExecuteTasks() { }
	protected function preProcess($cronTask) { … }		// setBulkMode + markRunning
	protected function endProcess($queueName) { sleep(2); }
	public function processQueue() { … }
}
```

`processQueue()` runs **one batch** and returns:
1. `Vtiger_Cron::getInstance($queueName)`; missing row or disabled Supervisor row → `endProcess()` and return
2. Supervisor row → `markRunning()`
3. `beforeExecuteTasks()`
4. `$this->service->executeTasks($batchLimit, $queueName)` — claims up to the limit of `task_name = $queueName` tasks; each runs through php-queue's `TaskPerformer` (→ `<JobName>::run()`), exceptions are caught and echoed per task
5. `resetSkipThisTimeType()` + `completeProcessingTask()` (closes this run's claimed IN_PROCESS tasks and ones started > 1 hour ago)
6. `afterExecuteTasks()`, `markFinished()`, then `endProcess()` sleeps 2 s

There is no `while (true)` loop: the process exits after each batch and Supervisor (autorestart) starts it again.

## Queue Table: `queue_tasks`

Key columns (live DDL, `SHOW CREATE TABLE queue_tasks`):

| Column | Meaning |
|--------|---------|
| `id`, `uniqid` | Task identity |
| `task_name` | Queue / job class name |
| `type` | 1 = TASK, 0 = SKIP_IN_THIS_TIME (`PhpQueue\TaskConst`) |
| `status` | 1 NEW, 2 IN_PROCESS, 3 DONE, 4 ERROR, 5 CANCEL, 6 CALLBACK_ERROR, 7 DONE_WITH_ERROR (`PhpQueue\Task::STATUS_*`) |
| `request_data` / `response_data` | Payload / result (serialized by php-queue) |
| `priority`, `task_group_id`, `execution_date` | Ordering / scheduling |
| `create_date`, `start_date`, `done_date` | Timing |
| `session_id`, `user_id`, `target_record`, `target_module` | Set only for queues in `$queueConfig['session_manager_queue']` |

Index `idx_queue_claim_order (task_name, type, status, priority DESC, id)` backs the claim query.

## Run By Supervisor

Register the consumer row with `run_by = 'Supervisor'` (see `references/registration.md`):

```php
$this->createCronjob(
	'SendNotification',
	60,
	'CPNotifications',
	'cron/modules/CPNotifications/processNotificationQueue.service',
	'Supervisor'
);
```

All existing Supervisor rows use `frequency = 60` (the value is not used to schedule the process).

The Supervisor program itself is NOT a hand-written conf file: `CPQueue_Supervisor_Provider` (`modules/CPQueue/providers/Supervisor.php`) registers/starts/stops a process named `<domain>_<queueName>` through the local supervisor-manager API (`http://127.0.0.1:5000`, `/save_process`, `/control_process`). It is called from Settings > Scheduler (`modules/Settings/CronTasks/…`) and from QuickRepair (`modules/Vtiger/actions/QuickRepair.php`).

## Common Use Cases

Every use case is a job class in `modules/CPQueue/Jobs/` + a consumer; look at the existing jobs before writing a new one:

| Need | Existing job |
|------|--------------|
| Notifications | `SendNotification.php` |
| Email queue | `ProcessEmailQueue.php` (sets `Vtiger_Mailer` properties, then calls `Send()`) |
| SMS / OTT messages | `SMSAndOTTMessageQueueProcessing.php` |
| Export | `ProcessExportQueue.php` |
| Import | `ScheduleImport.php` |

Sending mail from a job: `Vtiger_Mailer::Send($sync = false, $linktoid = false, int $emailId = 0)` takes no to/subject/body — set `$mailer->From`, `Subject`, `Body`, `AddAddress(…)` first (see `ProcessEmailQueue::run()`).

## Monitoring & Debugging

### Check Queue Status

```sql
-- Pending tasks
SELECT COUNT(*) FROM queue_tasks WHERE task_name = 'SendNotification' AND status = 1;

-- Failed tasks
SELECT id, task_name, response_data, create_date FROM queue_tasks
WHERE status IN (4, 6, 7) ORDER BY id DESC LIMIT 10;

-- Processing time
SELECT AVG(TIMESTAMPDIFF(SECOND, create_date, done_date)) AS avg_seconds
FROM queue_tasks
WHERE task_name = 'SendNotification' AND status = 3;
```

### Run One Batch Locally

```bash
# Same as one Supervisor iteration (processes up to the batch limit, then sleeps 2s and exits)
php -r 'include_once "vtlib/Vtiger/Cron.php"; require_once "config.inc.php"; require_once "includes/Loader.php"; vimport("includes.runtime.EntryPoint"); require "cron/modules/CPNotifications/processNotificationQueue.service";'
```

### Check Supervisor Status (server)

```bash
sudo supervisorctl status | grep <domain>_SendNotification
```

## Common Pitfalls

1. **Name mismatch**: job class, `addTask()` name and `vtiger_cron_task.name` must be identical
2. **Logic in the process class**: `BaseProcess` never calls an `executeTask()` hook — put the work in `<JobName>::run()`
3. **Wrong run_by**: Must be 'Supervisor', not 'Cronjob' (`vtigercron.php` skips Supervisor rows, and vice versa)
4. **No batch limit**: memory exhaustion on high traffic
5. **Swallowed errors**: an exception in `run()` is caught per task and only echoed — log what you need
6. **No monitoring**: watch the pending count per `task_name`

## Performance Tips

1. **Archive old tasks** (DONE/ERROR > 30 days) — `queue_tasks` grows fast
2. **Use priority** (`addTask(..., $priority)`) for urgent tasks
3. **Monitor queue depth** (pending count per `task_name`)
4. **Reuse the server instance** (`CPQueue_Server_Model::getInstance()`) when enqueuing in a loop

## Next Steps

1. Create the job class in `modules/CPQueue/Jobs/<JobName>.php`
2. Enqueue with `CPQueue_Server_Model::getCleanInstance()->addTask('<JobName>', $data)`
3. Create consumer service wrapper in `cron/modules/<Module>/` (+ process class in `modules/<Module>/services/` if hooks are needed)
4. Register via migration with `run_by='Supervisor'`, frequency 60
5. Save the task in Settings > Scheduler (or run QuickRepair) so the Supervisor process is created
6. Monitor `queue_tasks` depth

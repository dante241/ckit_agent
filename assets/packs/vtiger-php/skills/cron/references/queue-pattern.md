# VTiger Queue Pattern

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## Architecture Overview

The queue pattern uses **2 files** for continuous processing:

```
cron/modules/<Module>/<Name>.service          → Thin wrapper (3-5 lines)
modules/<Module>/services/<ProcessName>.php   → Extends BaseProcess
```

**Why this pattern?**
- Real-time processing of queued tasks
- Runs continuously via supervisord (not periodic cron)
- Built-in batch limiting, error handling, sleep intervals
- Uses CPQueue module for task management

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

## Service File Template

**Location:** `cron/modules/<Module>/<Name>.service`

```php
<?php
/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('modules/CPNotifications/services/ProcessSendNotification.php');

$process = new ProcessSendNotification('SendNotification', 200);
$process->processQueue();
```

**Parameters:**
- First param: Queue name (must match CPQueue record)
- Second param: Batch limit (default 200, max 500)

## Process Class Template

**Location:** `modules/<Module>/services/<ProcessName>.php`

```php
<?php
/**
 * @name ProcessSendNotification
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

include_once('modules/CPQueue/models/BaseProcess.php');

class ProcessSendNotification extends BaseProcess {

    /**
     * Execute before processing tasks
     * Use for initialization, config loading
     */
    protected function beforeExecuteTasks(): void {
        // Optional: load config, initialize services
        $this->initializeNotificationService();
    }

    /**
     * Execute after processing tasks
     * Use for cleanup, logging
     */
    protected function afterExecuteTasks(): void {
        // Optional: cleanup resources, log stats
        $this->logProcessingStats();
    }

    /**
     * Initialize notification service
     */
    private function initializeNotificationService(): void {
        // Service initialization
    }

    /**
     * Log processing statistics
     */
    private function logProcessingStats(): void {
        // Logging logic
    }
}
```

## BaseProcess Framework

The parent class `CPQueue/models/BaseProcess.php` provides:

```php
class BaseProcess {
    protected $queueName;           // Queue identifier
    protected $batchLimit = 200;    // Tasks per batch
    protected $sleepTime = 2;       // Seconds between batches

    public function __construct(string $queueName, int $batchLimit = 200) {
        $this->queueName = $queueName;
        $this->batchLimit = $batchLimit;
    }

    public function processQueue(): void {
        while (true) {
            $this->beforeExecuteTasks();

            $tasks = $this->getTasksFromQueue();

            foreach ($tasks as $task) {
                try {
                    $this->executeTask($task);
                    $this->markTaskComplete($task);
                } catch (\Throwable $th) {
                    $this->markTaskFailed($task, $th->getMessage());
                }
            }

            $this->afterExecuteTasks();

            // Prevent CPU spike
            sleep($this->sleepTime);
        }
    }
}
```

**Framework handles:**
- Infinite loop for continuous processing
- Batch limiting (prevents memory exhaustion)
- Error isolation per task
- Sleep intervals (prevents CPU spike)
- Task status tracking

## CPQueue Integration

Queue tasks are stored in `vtiger_cpqueue` table:

```sql
CREATE TABLE vtiger_cpqueue (
    id INT PRIMARY KEY AUTO_INCREMENT,
    queue_name VARCHAR(100),
    task_data TEXT,           -- JSON payload
    status VARCHAR(20),        -- pending|processing|completed|failed
    priority INT DEFAULT 0,
    created_at DATETIME,
    processed_at DATETIME,
    error_message TEXT
);
```

### Adding Tasks to Queue

```php
// Create queue task
$queueModel = new CPQueue_Queue_Model();
$queueModel->addTask('SendNotification', [
    'user_id' => 123,
    'message' => 'Hello world',
    'type' => 'push'
]);
```

## Real-World Example: Notification Processing

**Service:** `cron/modules/CPNotifications/ProcessNotifications.service`

```php
<?php
require_once('modules/CPNotifications/services/ProcessSendNotification.php');

$process = new ProcessSendNotification('SendNotification', 200);
$process->processQueue();
```

**Process Class:** `modules/CPNotifications/services/ProcessSendNotification.php`

```php
<?php
include_once('modules/CPQueue/models/BaseProcess.php');

class ProcessSendNotification extends BaseProcess {

    private $fcmService;
    private $smsService;

    protected function beforeExecuteTasks(): void {
        // Initialize services once per batch
        $this->fcmService = new FCM_Service();
        $this->smsService = new SMS_Service();
    }

    /**
     * Override parent method to add custom task execution
     */
    protected function executeTask($task): void {
        $data = json_decode($task['task_data'], true);

        switch ($data['type']) {
            case 'push':
                $this->sendPushNotification($data);
                break;
            case 'sms':
                $this->sendSMS($data);
                break;
            case 'email':
                $this->sendEmail($data);
                break;
        }
    }

    protected function afterExecuteTasks(): void {
        // Log batch completion
        error_log("Processed batch at " . date('Y-m-d H:i:s'));
    }

    private function sendPushNotification(array $data): void {
        $this->fcmService->send($data['user_id'], $data['message']);
    }

    private function sendSMS(array $data): void {
        $this->smsService->send($data['phone'], $data['message']);
    }

    private function sendEmail(array $data): void {
        // Email logic
    }
}
```

## Supervisord Configuration

Queue processes run continuously via supervisord.

**Config file:** `/etc/supervisor/conf.d/vtiger-notifications.conf`

```ini
[program:vtiger-notifications]
command=php /path/to/vtiger/cron/modules/CPNotifications/ProcessNotifications.service
directory=/path/to/vtiger
user=www-data
autostart=true
autorestart=true
redirect_stderr=true
stdout_logfile=/var/log/supervisor/vtiger-notifications.log
```

**Supervisor commands:**

```bash
# Reload config
sudo supervisorctl reread
sudo supervisorctl update

# Start process
sudo supervisorctl start vtiger-notifications

# Check status
sudo supervisorctl status vtiger-notifications

# View logs
tail -f /var/log/supervisor/vtiger-notifications.log

# Restart process
sudo supervisorctl restart vtiger-notifications
```

## Critical Configuration

### 1. Batch Limit

```php
// Low traffic: 50-100 tasks per batch
$process = new ProcessName('QueueName', 50);

// Medium traffic: 200 tasks per batch (default)
$process = new ProcessName('QueueName', 200);

// High traffic: 500 tasks per batch (max recommended)
$process = new ProcessName('QueueName', 500);
```

**Why limit?** Prevents memory exhaustion from processing too many tasks at once.

### 2. Sleep Interval

```php
protected $sleepTime = 2; // seconds

// Override in constructor if needed
public function __construct(string $queueName, int $batchLimit = 200) {
    parent::__construct($queueName, $batchLimit);
    $this->sleepTime = 5; // Custom sleep time
}
```

**Why sleep?** Prevents CPU spike at 100% usage.

### 3. Run By Supervisor

In cron registration (see `references/registration.md`):

```php
$this->createCronjob(
    'ProcessNotifications',
    0,                          // Frequency: 0 for continuous
    'CPNotifications',
    'cron/modules/CPNotifications/ProcessNotifications.service',
    'Supervisor'                // CRITICAL: Must be 'Supervisor', not 'Cronjob'
);
```

## Common Use Cases

### 1. Webhook Processing

```php
class ProcessWebhooks extends BaseProcess {
    protected function executeTask($task): void {
        $data = json_decode($task['task_data'], true);
        $response = $this->callExternalAPI($data['url'], $data['payload']);

        // Store response
        $this->saveWebhookResponse($task['id'], $response);
    }
}
```

### 2. Chat Message Delivery

```php
class ProcessChatMessages extends BaseProcess {
    protected function executeTask($task): void {
        $data = json_decode($task['task_data'], true);

        // Send via WebSocket or external service
        $this->sendToWebSocket($data['channel'], $data['message']);
    }
}
```

### 3. Email Queue

```php
class ProcessEmailQueue extends BaseProcess {
    protected function executeTask($task): void {
        $data = json_decode($task['task_data'], true);

        $mailer = new Vtiger_Mailer();
        $mailer->send($data['to'], $data['subject'], $data['body']);
    }
}
```

## Monitoring & Debugging

### Check Queue Status

```sql
-- Pending tasks
SELECT COUNT(*) FROM vtiger_cpqueue WHERE status = 'pending' AND queue_name = 'SendNotification';

-- Failed tasks
SELECT * FROM vtiger_cpqueue WHERE status = 'failed' ORDER BY created_at DESC LIMIT 10;

-- Processing time
SELECT AVG(TIMESTAMPDIFF(SECOND, created_at, processed_at)) as avg_seconds
FROM vtiger_cpqueue
WHERE status = 'completed' AND queue_name = 'SendNotification';
```

### Check Supervisor Status

```bash
# Is process running?
sudo supervisorctl status vtiger-notifications

# View recent logs
tail -n 100 /var/log/supervisor/vtiger-notifications.log

# Check CPU usage
top -u www-data
```

## Common Pitfalls

1. **No Batch Limit**: Memory exhaustion on high traffic
2. **No Sleep**: CPU at 100%, server slowdown
3. **Wrong run_by**: Must be 'Supervisor', not 'Cronjob'
4. **No Supervisord Config**: Process won't auto-restart
5. **No Error Handling**: Failed tasks block queue
6. **Synchronous Processing**: Use async for external APIs
7. **No Monitoring**: Can't detect queue backlog

## Performance Tips

1. **Index queue_name and status** in vtiger_cpqueue
2. **Archive old tasks** (completed/failed > 30 days)
3. **Use priority field** for urgent tasks
4. **Monitor queue depth** (pending count)
5. **Scale horizontally** (multiple supervisor processes)

## Next Steps

1. Create service wrapper in `cron/modules/<Module>/`
2. Create process class in `modules/<Module>/services/`
3. Register via migration with `run_by='Supervisor'`
4. Create supervisord config
5. Test queue processing
6. Monitor logs and queue depth

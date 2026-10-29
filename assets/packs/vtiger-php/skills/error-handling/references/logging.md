# Logging Patterns

## LoggerManager (custom lightweight logger)

`libraries/log4php/LoggerManager.php` is a small in-house logger that only borrows log4php's config file format — it is NOT the Apache Log4PHP library.

### Basic Usage

```php
$logger = LoggerManager::getLogger('PLATFORM');
$logger->info('Operation completed successfully');
$logger->error('Failed to save record: ' . $errorMessage);
$logger->debug('Request data: ' . json_encode($data));
$logger->fatal('Critical system failure');
```

Each level method takes ONLY `$message` (non-strings are `var_export`ed). A second "context" argument is silently dropped — put the context into the message string (e.g. `json_encode`).

### When Does a Line Actually Get Written?

Both gates must pass:

1. **`$loggerConfig`** in `config_override.php` — the per-level switch (`ERROR`/`FATAL`/`INFO`/`WARN`/`DEBUG`). All are `false` by default, so nothing is written until you enable a level.
2. **Category threshold** in `log4php.properties` — `log4php.logger.PLATFORM=INFO,A6` writes INFO and above (INFO, WARN, ERROR, FATAL) for that category.

A category missing from `log4php.properties` falls back to the root logger (`log4php.rootLogger.VTIGERCRM=FATAL,A1` → `logs/vtigercrm.log`, FATAL only).

### Log Levels

| Level | Method | Use For |
|-------|--------|---------|
| `DEBUG` | `debug()` | Development debugging, verbose data |
| `INFO` | `info()` | Normal operations, webhook success |
| `WARN` | `warn()` | Recoverable issues, deprecation notices |
| `ERROR` | `error()` | Failures, exceptions, API errors |
| `FATAL` | `fatal()` | System crashes, unrecoverable errors |

---

## Category → Log File Mapping

Categories are defined in `log4php.properties`. Each category writes to a specific log file.

### Common Categories

| Category | Log File | Use For |
|----------|----------|---------|
| `PLATFORM` | `platform.log` | General system logs, webhooks, utils |
| `WEBSERVICE` | `webservice.log` | REST API calls (Webservices) |
| `OPEN_API` | `open_api.log` | OpenAPI endpoints |
| `CALLCENTER` | `callcenter.log` | Call center integrations, phone logs |
| `WORKFLOW` | `workflow.log` | Workflow task execution |
| `NOTIFICATIONS` | `notifications.log` | FCM push notifications |
| `ADMIN_AUDIT` | `admin_audit.log` | Admin actions, user changes |
| `BANKHUB_INTEGRATION` | `bankhub_integration.log` | Payment gateway webhooks |
| `SOCIAL_INTEGRATION` | `social_integration.log` | Facebook / Zalo / social channels |
| `SYNC_ZNS` | `sync_zns.log` | Zalo ZNS sync |
| `SMS_OTT` | `sms_ott.log` | SMS / OTT messages |
| `GOOGLE_INTEGRATION` | `google_integration.log` | Google Ads, Analytics |
| `TIKTOKSHOP_INTEGRATION` | `tiktokshop_integration.log` | TikTok Shop |
| `QUEUE` | `queue.log` | Queue processing |
| `SQL_ERRORS` | `sql_errors.log` | Failed SQL (written by `PearDatabase::checkError()`) |

Full list: `grep '^log4php.logger' log4php.properties`.

### Adding New Category

**CRITICAL:** New categories MUST be added to `log4php.properties` before use.

```properties
# In log4php.properties — copy an existing block
log4php.logger.MY_NEW_INTEGRATION=INFO,A46
log4php.appender.A46=LoggerAppenderRollingFile
log4php.appender.A46.MaxFileSize=10MB
log4php.appender.A46.MaxBackupIndex=10
log4php.appender.A46.layout=LoggerLayoutTTCC
log4php.appender.A46.layout.ContextPrinting="true"
log4php.appender.A46.layout.DateFormat="%c"
log4php.appender.A46.File=logs/my_new_integration.log
```

**Appender ID:** Use the next free one (currently max is A45, so use A46). The file key must be `File` (capital F) — the logger reads `File`; any other spelling logs to `vtigercrm.log`.

---

## saveLog Variants Decision Matrix

Different contexts use different `saveLog()` functions. Choose the right one.

### 1. Webhook Logging (WebhookUtils)

```php
WebhookUtils::saveLog($description, $headers, $input, $response);
```

**Category:** `PLATFORM`

**File:** `logs/platform.log`

**Use For:**
- Webhook receivers (Entry Points)
- External service callbacks
- Payment gateway webhooks

**Example:**
```php
$headers = getallheaders();
$input = file_get_contents('php://input');

WebhookUtils::saveLog(
    'BankHub payment webhook received',
    $headers,
    $input,
    json_encode(['status' => 'processed'])
);
```

---

### 2. REST API Logging (CloudGoApiUtils)

```php
CloudGoApiUtils::saveLog($description, $info);
```

**Category:** `WEBSERVICE`

**File:** `logs/webservice.log`

**Use For:**
- REST API endpoints (Webservices module)
- External API calls from VTiger
- API authentication failures

**Example:**
```php
CloudGoApiUtils::saveLog(
    'Facebook Ads API call',
    [
        'url' => $apiUrl,
        'method' => 'GET',
        'response_code' => 200,
        'response' => $responseData
    ]
);
```

---

### 3. IntegrationAPI Logging (TraitAPILogger)

```php
$this->saveLog($action, $data);
```

**Signature:** `saveLog($action, $data)` — a no-op unless `$this->enableLog` is true (`IntegrationApiHandler` defaults it to `true`).

**Storage:** `CPLogAPI_Logger_Helper::saveLogApi()` — the CPLogAPI store (`vtiger_cpcallapilog` in the SQLite3 backend by default, Elastic when configured), not a log file.

**Lifecycle:** `saveLog()` records the INBOUND request and keeps `$this->logId`; the trait's `setResponse($code, $data)` then writes status (`success`/`error` from `$data['success']`), response body and HTTP code onto the same entry before calling `parent::setResponse()` (which `exit`s).

**Use For:**
- IntegrationAPI handlers (inbound API) — `IntegrationApiHandler` already uses the trait
- Audit trail for API operations (view via the CPLogAPI module)

**Example:**
```php
class MyChannelApiHandler extends IntegrationApiHandler {

    protected $channel = 'MY_CHANNEL';

    public function createOrder() {
        $body = $this->getRequestBody(); // pseudo-code: read the decoded request body
        $this->saveLog('createOrder', $body);

        $this->setResponse(200, ['success' => 1, 'data' => []]); // completes the log entry, then exits
    }
}
```

---

### 4. Workflow Task Logging

There is no `VTTask::saveLog()`. Log through the `WORKFLOW` category:

```php
$logger = LoggerManager::getLogger('WORKFLOW');
```

**File:** `logs/workflow.log`

**Use For:**
- Workflow task execution
- Scheduled workflow actions
- Workflow debugging

**Example (custom task class):**
```php
class MyModuleSampleTask extends VTTask {

    public function doTask($entity) {
        $logger = LoggerManager::getLogger('WORKFLOW');
        $logger->info("Running workflow task: {$this->id}");

        try {
            $this->sendEmail($entity); // pseudo-code
            $logger->info('Email sent successfully');
        }
        catch (Throwable $e) {
            $logger->error("Email failed: {$e->getMessage()}");
        }
    }
}
```

---

### 5. Admin Audit Logging

```php
Vtiger_AdminAudit_Helper::saveLog($category, $description, $logData = null);
```

**Category:** `ADMIN_AUDIT` (the helper forces `$loggerConfig['INFO'] = true`, so it writes even when INFO is globally off)

**File:** `logs/admin_audit.log` only (no DB table). Each line also carries user, IP, request URL and the full POST data.

**Use For:**
- Admin panel changes
- User role modifications
- System configuration updates
- Security-sensitive operations

**Example:**
```php
Vtiger_AdminAudit_Helper::saveLog(
    'USER_MANAGEMENT',
    'User role changed',
    [
        'user_id' => $userId,
        'old_role' => 'Sales Manager',
        'new_role' => 'Admin',
    ]
);
```

---

### 6. General Logging (Global saveLog)

```php
saveLog($logger, $description, $data = null);
```

Global function loaded through `include/utils/utils.php` on every request. `$logger` is the category name; writes at INFO level with user + IP, `$data` is `json_encode`d.

**File:** Depends on category

**Use For:**
- Custom module logging
- Background jobs
- General purpose logging

**Example:**
```php
saveLog(
    'SOCIAL_INTEGRATION',
    'Campaign sync completed',
    [
        'account_id' => $accountId,
        'campaigns_synced' => count($campaigns),
        'duration_ms' => $duration,
    ]
);
```

---

## Logging Patterns by Context

### Action Controller Error Logging

```php
class MyModule_Process_Action extends Vtiger_Action_Controller {
    public function process(Vtiger_Request $request) {
        $response = new Vtiger_Response();
        $logger = LoggerManager::getLogger('PLATFORM');

        try {
            $result = $this->executeLogic($request);
            $logger->info('Process completed successfully');
            $response->setResult($result);
        }
        catch (Throwable $e) {
            $context = json_encode([
                'module' => $request->getModule(),
                'user' => Users_Record_Model::getCurrentUserModel()->getId(),
            ]);
            $logger->error('Process failed: ' . $e->getMessage() . ' ' . $context);
            $response->setError(500, $e->getMessage());
        }

        $response->emit();
    }
}
```

---

### Webhook Receiver Logging

```php
class BankHubWebhook extends Vtiger_EntryPoint {
    public function process(Vtiger_Request $request) {
        $headers = getallheaders();
        $rawInput = file_get_contents('php://input');

        try {
            $data = json_decode($rawInput, true);

            WebhookUtils::saveLog(
                'BankHub webhook received',
                $headers,
                $rawInput,
                null
            );

            $this->processPayment($data);

            WebhookUtils::saveLog(
                'BankHub webhook processed',
                $headers,
                $rawInput,
                json_encode(['status' => 'success'])
            );

            http_response_code(200);
            echo json_encode(['status' => 'ok']);
        }
        catch (Throwable $e) {
            WebhookUtils::saveLog(
                'BankHub webhook error',
                $headers,
                $rawInput,
                json_encode(['error' => $e->getMessage()])
            );

            http_response_code(400);
            echo json_encode(['error' => $e->getMessage()]);
        }
    }
}
```

---

### Cron Job Logging

Cron classes are plain `<Module>_<Component>_Cron` classes with a `process()` method (no base class — see `modules/Project/crons/calculateProjectCost.php`).

```php
class MyModule_SyncData_Cron {

    public function process() {
        $logger = LoggerManager::getLogger('QUEUE');
        $logger->info('Cron job started');

        try {
            $records = $this->getRecordsToProcess();
            $logger->info('Processing ' . count($records) . ' records');

            foreach ($records as $record) {
                $this->processRecord($record);
            }

            $logger->info('Cron job completed successfully');
        }
        catch (Throwable $e) {
            $logger->error('Cron job failed: ' . $e->getMessage());
            throw $e;
        }
    }
}
```

---

### External API Call Logging

Pseudo-code — the connector class and `$this->httpClient` are illustrative.

```php
class MySocialConnector {

    protected function makeGetRequest($endpoint, $params) {
        $logger = LoggerManager::getLogger('SOCIAL_INTEGRATION');

        $url = $this->buildUrl($endpoint, $params);
        $logger->debug("API Request: GET {$url}");

        try {
            $response = $this->httpClient->get($url);
            $logger->info("API call successful: {$endpoint}");
            return json_decode($response, true);
        }
        catch (Throwable $e) {
            $logger->error("API call failed: {$e->getMessage()} - " . json_encode(['endpoint' => $endpoint, 'params' => $params]));
            throw $e;
        }
    }
}
```

---

## Best Practices

1. **Always log category**: Use appropriate category for log file routing
2. **Include context**: Add relevant data (IDs, user, module) to log messages
3. **Log before/after**: Log entry and exit of critical operations
4. **Log exceptions**: Always log caught exceptions with stack trace
5. **Avoid sensitive data**: Never log passwords, tokens, credit cards
6. **Use appropriate level**: INFO for normal, ERROR for failures, DEBUG for development
7. **Check category exists**: Verify category in `log4php.properties` before first use

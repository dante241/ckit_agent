# Logging Patterns

## LoggerManager (Log4PHP)

VTiger uses Log4PHP for structured logging across modules.

### Basic Usage

```php
$logger = LoggerManager::getLogger('PLATFORM');
$logger->info('Operation completed successfully');
$logger->error('Failed to save record: ' . $errorMessage);
$logger->debug('Request data: ' . json_encode($data));
$logger->fatal('Critical system failure');
```

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

Categories are defined in `log4php.properties`. Each category writes to specific log file.

### Common Categories

| Category | Log File | Use For |
|----------|----------|---------|
| `PLATFORM` | `platform.log` | General system logs, webhooks, utils |
| `WEBSERVICE` | `webservice.log` | REST API calls (Webservices) |
| `CALLCENTER` | `callcenter.log` | Call center integrations, phone logs |
| `WORKFLOW` | `workflow.log` | Workflow task execution |
| `NOTIFICATIONS` | `notifications.log` | FCM push notifications |
| `ADMIN_AUDIT` | `admin_audit.log` | Admin actions, user changes |
| `BANKHUB_INTEGRATION` | `bankhub_integration.log` | Payment gateway webhooks |
| `ZALO_INTEGRATION` | `zalo_integration.log` | Zalo ZNS, Zalo API |
| `FACEBOOK_INTEGRATION` | `facebook_integration.log` | Facebook Ads, Messenger |
| `GOOGLE_INTEGRATION` | `google_integration.log` | Google Ads, Analytics |
| `TIKTOK_INTEGRATION` | `tiktok_integration.log` | TikTok Ads |
| `BACKGROUND_JOBS` | `background_jobs.log` | Cron jobs, queue processing |

### Adding New Category

**CRITICAL:** New categories MUST be added to `log4php.properties` before use.

```properties
# In log4php.properties
log4php.appender.A42 = LoggerAppenderRollingFile
log4php.appender.A42.file = logs/my_new_integration.log
log4php.appender.A42.MaxFileSize = 5MB
log4php.appender.A42.MaxBackupIndex = 10
log4php.appender.A42.layout = LoggerLayoutPattern
log4php.appender.A42.layout.ConversionPattern = %d{Y-m-d H:i:s} %c %5p %m%n

log4php.logger.MY_NEW_INTEGRATION = DEBUG, A42
```

**Appender ID:** Use next available (currently max is A41, so use A42, A43, etc.)

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
$this->saveLog($description, $data, $status);
```

**Category:** N/A (writes to `vtiger_cp_export_log` table)

**File:** Database table, not log file

**Use For:**
- IntegrationAPI handlers (inbound API)
- External system integration logs
- Audit trail for API operations

**Example:**
```php
class ShopifyApiHandler extends IntegrationApiHandler {
    use TraitAPILogger;

    protected function processOrder($data) {
        try {
            $orderId = $this->createSalesOrder($data);
            $this->saveLog('Order created', ['order_id' => $orderId], 'success');
        }
        catch (Exception $e) {
            $this->saveLog('Order creation failed', ['error' => $e->getMessage()], 'error');
        }
    }
}
```

---

### 4. Workflow Task Logging

```php
VTTask::saveLog($description, $data, $taskId);
```

**Category:** `WORKFLOW`

**File:** `logs/workflow.log`

**Use For:**
- Workflow task execution
- Scheduled workflow actions
- Workflow debugging

**Example:**
```php
class VTSendEmailTask extends VTTask {
    public function doTask($entity) {
        $logger = LoggerManager::getLogger('WORKFLOW');
        $logger->info("Sending email for workflow: {$this->id}");

        try {
            $this->sendEmail($entity);
            $logger->info('Email sent successfully');
        }
        catch (Exception $e) {
            $logger->error("Email failed: {$e->getMessage()}");
        }
    }
}
```

---

### 5. Admin Audit Logging

```php
AdminAudit_Helper::saveLog($category, $description, $data);
```

**Category:** `ADMIN_AUDIT`

**File:** `logs/admin_audit.log` + `vtiger_admin_audit` table

**Use For:**
- Admin panel changes
- User role modifications
- System configuration updates
- Security-sensitive operations

**Example:**
```php
AdminAudit_Helper::saveLog(
    'USER_MANAGEMENT',
    'User role changed',
    [
        'user_id' => $userId,
        'old_role' => 'Sales Manager',
        'new_role' => 'Admin',
        'changed_by' => $currentUserId
    ]
);
```

---

### 6. General Logging (Global saveLog)

```php
saveLog($category, $description, $data);
```

**Category:** Parameter-specified

**File:** Depends on category

**Use For:**
- Custom module logging
- Background jobs
- General purpose logging

**Example:**
```php
saveLog(
    'FACEBOOK_INTEGRATION',
    'Campaign sync completed',
    [
        'account_id' => $accountId,
        'campaigns_synced' => count($campaigns),
        'duration_ms' => $duration
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
        catch (Exception $e) {
            $logger->error('Process failed: ' . $e->getMessage(), [
                'module' => $request->getModule(),
                'user' => Users_Record_Model::getCurrentUserModel()->getId()
            ]);
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
        catch (Exception $e) {
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

```php
class MyModule_CronHandler extends Vtiger_CronHandler {
    public function execute() {
        $logger = LoggerManager::getLogger('BACKGROUND_JOBS');

        $logger->info('Cron job started');

        try {
            $records = $this->getRecordsToProcess();
            $logger->info("Processing {count($records)} records");

            foreach ($records as $record) {
                $this->processRecord($record);
            }

            $logger->info('Cron job completed successfully');
        }
        catch (Exception $e) {
            $logger->error('Cron job failed: ' . $e->getMessage());
            throw $e;
        }
    }
}
```

---

### External API Call Logging

```php
class FacebookAdsConnector {
    protected function makeGetRequest($endpoint, $params) {
        $logger = LoggerManager::getLogger('FACEBOOK_INTEGRATION');

        $url = $this->buildUrl($endpoint, $params);
        $logger->debug("API Request: GET {$url}");

        try {
            $response = $this->httpClient->get($url);
            $logger->info('API call successful', ['endpoint' => $endpoint]);
            return json_decode($response, true);
        }
        catch (Exception $e) {
            $logger->error("API call failed: {$e->getMessage()}", [
                'endpoint' => $endpoint,
                'params' => $params
            ]);
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

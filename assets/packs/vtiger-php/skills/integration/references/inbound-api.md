# Inbound API - IntegrationApiHandler

## Simple Inbound API Pattern

For straightforward APIs with few endpoints, extend `IntegrationApiHandler` directly.

### 3-File Architecture

1. **Handler**: `include/Webservice/CloudBotApi/{Platform}ApiHandler.php`
2. **API Definitions**: `include/Webservice/CloudBotApi/inbound_apis/{Platform}.php`
3. **Enums**: `include/Webservice/CloudBotApi/enums/{Platform}.php`

### Handler Skeleton

```php
<?php

/**
 * {Platform} inbound API handler
 * Location: include/Webservice/CloudBotApi/{Platform}ApiHandler.php
 */

require_once 'include/Webservice/CloudBotApi/IntegrationApiHandler.php';
require_once 'include/Webservice/CloudBotApi/traits/JWTTrait.php';
require_once 'include/Webservice/CloudBotApi/traits/LoggerTrait.php';

class ZaloApiHandler extends IntegrationApiHandler {

    use JWTTrait;
    use LoggerTrait;

    protected $platform = 'Zalo';

    // CRITICAL: Read from config, don't hardcode true
    protected function isEnabled(): bool {
        global $zaloConfig;
        return !empty($zaloConfig['api_enabled']) && $zaloConfig['api_enabled'];
    }

    public function create_customer($request, $user) {
        try {
            // Validate required fields
            $phone = $request['phone'] ?? '';
            if (empty($phone)) {
                return $this->setResponse(['error' => 'Phone is required'], 400);
            }

            // Create or update customer
            $recordId = $this->syncRecord('Contacts', $request);

            return $this->setResponse([
                'success' => true,
                'recordId' => $recordId,
            ]);
        }
        catch (Exception $e) {
            $this->logError(__FUNCTION__, $e->getMessage());
            return $this->setResponse(['error' => $e->getMessage()], 500);
        }
    }

    public function get_customer($request, $user) {
        try {
            $phone = $request['phone'] ?? '';
            if (empty($phone)) {
                return $this->setResponse(['error' => 'Phone is required'], 400);
            }

            // Find customer
            $sql = "SELECT contactid, firstname, lastname, phone, email
                    FROM vtiger_contactdetails
                    INNER JOIN vtiger_crmentity ON crmid = contactid
                    WHERE phone = ? AND deleted = 0
                    LIMIT 1";
            $result = $GLOBALS['adb']->pquery($sql, [$phone]);

            if ($GLOBALS['adb']->num_rows($result) == 0) {
                return $this->setResponse(['error' => 'Customer not found'], 404);
            }

            $customer = decodeUTF8($GLOBALS['adb']->fetchByAssoc($result));

            return $this->setResponse([
                'success' => true,
                'customer' => $customer,
            ]);
        }
        catch (Exception $e) {
            $this->logError(__FUNCTION__, $e->getMessage());
            return $this->setResponse(['error' => $e->getMessage()], 500);
        }
    }
}
```

### API Definitions File

```php
<?php

/**
 * Zalo API definitions
 * Location: include/Webservice/CloudBotApi/inbound_apis/Zalo.php
 */

// TYPO ALERT: Variable name has typo but MUST keep for compatibility
$intergrationAPIs = [
    'create_customer' => [
        'method' => 'POST',
        'description' => 'Create or update customer',
        'requireAuth' => true,
    ],
    'get_customer' => [
        'method' => 'GET',
        'description' => 'Get customer by phone',
        'requireAuth' => true,
    ],
];
```

### Enums File

```php
<?php

/**
 * Zalo enums and constants
 * Location: include/Webservice/CloudBotApi/enums/Zalo.php
 */

class ZaloEnum {
    const CUSTOMER_STATUS_ACTIVE = 'Active';
    const CUSTOMER_STATUS_INACTIVE = 'Inactive';

    const ORDER_STATUS_PENDING = 'Pending';
    const ORDER_STATUS_CONFIRMED = 'Confirmed';
}
```

## SAVE_UNSUPPORTED_FIELDS

**Critical:** These fields are silently stripped before save:

```php
const SAVE_UNSUPPORTED_FIELDS = [
    'createdtime',
    'modifiedtime',
    'deleted',
    'items',      // Inventory line items
    'tags',
];
```

Don't try to set these via API - they're auto-managed.

## setResponse() Pattern

```php
// setResponse() calls exit() internally - still add return for clarity
return $this->setResponse(['success' => true], 200);

// After this line, code never executes
$this->logInfo("Never logged");  // Won't run
```

## DRY Pattern: syncRecord()

```php
// Define lookup fields for upsert
const LOOKUP_FIELDS = [
    'Contacts' => ['phone', 'email'],
    'Accounts' => ['accountname', 'phone'],
    'Products' => ['productcode'],
];

private function syncRecord(string $module, array $data): int {
    $lookupFields = self::LOOKUP_FIELDS[$module] ?? [];

    // Find existing record
    $recordId = 0;
    foreach ($lookupFields as $field) {
        if (!empty($data[$field])) {
            $recordId = $this->findRecordByField($module, $field, $data[$field]);
            if ($recordId > 0) break;
        }
    }

    if ($recordId > 0) {
        // Update existing
        $record = Vtiger_Record_Model::getInstanceById($recordId, $module);
        $record->set('mode', 'edit');
    }
    else {
        // Create new
        $record = Vtiger_Record_Model::getCleanInstance($module);
    }

    // Set data
    foreach ($data as $field => $value) {
        if (!in_array($field, self::SAVE_UNSUPPORTED_FIELDS)) {
            $record->set($field, $value);
        }
    }

    $record->save();
    return (int) $record->getId();
}
```

## Complex Inbound: Subhandler Pattern

For APIs with many endpoints, use subhandlers:

```php
<?php

/**
 * CloudBot main handler with subhandler routing
 * Location: include/Webservice/CloudBotApi/CloudBotApiHandler.php
 */

class CloudBotApiHandler extends IntegrationApiHandler {

    use JWTTrait;
    use LoggerTrait;

    protected $platform = 'CloudBot';

    protected function isEnabled(): bool {
        global $cloudBotConfig;
        return !empty($cloudBotConfig['api_enabled']);
    }

    // Route to subhandlers by action
    protected function getAPIHandlerByAction(string $action) {
        $handlers = [
            'create_customer' => 'CloudBotApi_Customer',
            'create_order' => 'CloudBotApi_Order',
            'sync_product' => 'CloudBotApi_Product',
        ];

        if (isset($handlers[$action])) {
            require_once "include/Webservice/CloudBotApi/subhandlers/{$handlers[$action]}.php";
            return new $handlers[$action]();
        }

        return null;
    }

    public function __call($method, $args) {
        $handler = $this->getAPIHandlerByAction($method);

        if (!$handler) {
            return $this->setResponse(['error' => 'Invalid action'], 404);
        }

        return call_user_func_array([$handler, 'handle'], $args);
    }
}
```

**Subhandler:**

```php
<?php

/**
 * Customer subhandler
 * Location: include/Webservice/CloudBotApi/subhandlers/CloudBotApi_Customer.php
 */

require_once 'include/Webservice/CloudBotApi/AbstractCloudBotApi.php';

class CloudBotApi_Customer extends AbstractCloudBotApi {

    public function handle($request, $user) {
        // Shared utilities from AbstractCloudBotApi:
        // - checkSession()
        // - doSave()
        // - searchModule()
        // - syncCustomerInfo()
        // - cacheManager

        $phone = $request['phone'] ?? '';
        if (empty($phone)) {
            return $this->setResponse(['error' => 'Phone required'], 400);
        }

        $recordId = $this->syncCustomerInfo($request);

        return $this->setResponse(['success' => true, 'recordId' => $recordId]);
    }
}
```

## Critical Pitfalls

1. **Extend IntegrationApiHandler** - NOT standalone class
2. **$intergrationAPIs typo** - MUST keep (compatibility)
3. **SAVE_UNSUPPORTED_FIELDS** - silently stripped, don't try to set
4. **setResponse() calls exit** - still add `return;` for clarity
5. **isEnabled() from config** - don't hardcode `true`
6. **Use syncRecord()** - DRY pattern for upsert
7. **LOOKUP_FIELDS constant** - define module-specific lookup fields

## URL Format

```
POST https://crm.domain.com/api/IntegrationAPI/Zalo/create_customer

Headers:
  Authorization: Bearer {JWT_TOKEN}
  Content-Type: application/json

Body:
{
  "phone": "0901234567",
  "firstname": "Nguyen",
  "lastname": "Van A",
  "email": "a@example.com"
}
```

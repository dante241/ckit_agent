# Inbound API - IntegrationApiHandler

## Simple Inbound API Pattern

For straightforward APIs with few endpoints, extend `IntegrationApiHandler` directly.

### 3-File Architecture

1. **Handler**: `include/Webservice/{Platform}ApiHandler.php` — class `{Platform}ApiHandler` (Endpoint.php only looks at this top-level path)
2. **API Definitions**: `modules/CPAPIIntegration/inbound_apis/{channel}.php` (sample: `Inbound.sample.php`)
3. **Enums (picklist mapping)**: `modules/CPAPIIntegration/enums/{channel}.php` — class `CPAPIIntegration_{channel}_Enum`; falls back to `enums/Base.php`

`{channel}` = the handler's `$channel` property. Base class `include/Webservice/IntegrationApiHandler.php` extends `RestfulApiHandler` and uses traits `TraitAuthenticationApiHandler` + `TraitAPILogger` (`include/WebserviceTraits/`) — they come with the base class, do not `use` them again.

### Handler Skeleton

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('include/Webservice/IntegrationApiHandler.php');

class ZaloApiHandler extends IntegrationApiHandler {

	// Selects inbound_apis/Zalo.php + enums/Zalo.php; constructor copies it into $logger and $source
	protected $channel = 'Zalo';

	// Actions callable without the token header
	protected $skipCheckToken = [];

	// CRITICAL: read from config, don't hardcode true
	public function isEnabled(): bool {
		global $zaloConfig;
		return !empty($zaloConfig['api_enabled']);
	}

	// API /createCustomer
	public function createCustomer(Vtiger_Request $request) {
		$data = $this->getProcessedRequestData(__FUNCTION__, $request);
		$result = $this->saveRecord('Contacts', $data, $this->findContactIdByPhone((string) ($data['mobile'] ?? '')));

		if (empty($result['success'])) {
			$this->setResponse(200, ['success' => 0, 'message' => $result['message']]);
		}

		$this->setResponse(200, ['success' => 1, 'id' => $result['record']->getId()]);
	}

	// API /getCustomer
	public function getCustomer(Vtiger_Request $request) {
		global $adb;
		$phone = (string) $request->get('phone');

		if ($phone === '') {
			$this->setResponse(400, ['success' => 0, 'message' => 'Phone is required']);
		}

		$sql = "SELECT c.contactid, c.firstname, c.lastname, c.mobile, c.email
			FROM vtiger_contactdetails AS c
			INNER JOIN vtiger_crmentity AS e ON (e.crmid = c.contactid AND e.deleted = 0)
			WHERE c.mobile = ?
			LIMIT 1";
		$result = $adb->pquery($sql, [$phone]);

		if ($adb->num_rows($result) == 0) {
			$this->setResponse(404, ['success' => 0, 'message' => 'Customer not found']);
		}

		$this->setResponse(200, ['success' => 1, 'customer' => $adb->fetchByAssoc($result)]);
	}

	// Upsert helper: 0 means create, otherwise saveRecord() updates this id
	private function findContactIdByPhone(string $phone): int {
		global $adb;
		if ($phone === '') return 0;

		$sql = "SELECT c.contactid
			FROM vtiger_contactdetails AS c
			INNER JOIN vtiger_crmentity AS e ON (e.crmid = c.contactid AND e.deleted = 0)
			WHERE c.mobile = ?
			LIMIT 1";

		return (int) $adb->getOne($sql, [$phone]);
	}
}
```

- Handler methods receive ONE argument, a `Vtiger_Request` (Endpoint.php calls `$instance->$method($request)`). It is NOT an array: use `$request->get('x')` / `$request->getAll()`, never `$request['x']`.
- Method name = action with the first letter lowercased (`.../Zalo/CreateCustomer` → `createCustomer`). Unknown method → 400 `Method not found`.
- `getProcessedRequestData(__FUNCTION__, $request)` looks up the definition keyed by the method name: 404 if missing, 403 if not `enabled`, checks HTTP `method`, validates/maps `params`, then returns CRM-keyed data.

### API Definitions File

```php
<?php

// modules/CPAPIIntegration/inbound_apis/Zalo.php
// TYPO ALERT: variable name is read by IntegrationApiHandler::getAPIDefination() — MUST keep
$intergrationAPIs = [
	'createCustomer' => [
		'enabled' => true,
		'method' => 'POST',
		'params' => [
			'phone' => [
				'internal_key' => 'mobile',
				'is_required' => true,
			],
			'name' => [
				'internal_key' => 'lastname',
				'is_required' => true,
			],
			'source' => [
				'internal_key' => 'leadsource',
				'type' => 'picklist',
			],
			'birthday' => [
				'internal_key' => 'birthday',
				'type' => 'date',
			],
			'company_code' => [
				'type' => 'reference',
				'internal_key' => 'account_id',
				'extra_data' => ['module' => 'Accounts', 'search_field' => 'customer_code_1'],
			],
		],
	],
];
```

Keys per API: `enabled`, `method`, `params` (+ optional `group_params`). Per param: `internal_key` (CRM field; empty = keep name), `is_required`, `type` (`picklist`, `date`, `reference`…), `extra_data`. Full example: `modules/CPAPIIntegration/inbound_apis/Inbound.sample.php`.

### Enums File

```php
<?php

// modules/CPAPIIntegration/enums/Zalo.php
require_once('modules/CPAPIIntegration/enums/Base.php');

class CPAPIIntegration_Zalo_Enum extends CPAPIIntegration_Base_Enum {

	// [crm_field => [external_value => internal_value]]
	protected const INTEGRATION_MAPPING = [
		'leadsource' => [
			'zalo_oa' => 'Zalo',
		],
	];
}
```

## SAVE_UNSUPPORTED_FIELDS

**Critical:** `saveRecord()` silently strips these keys before save (`IntegrationApiHandler::SAVE_UNSUPPORTED_FIELDS`):

```php
const SAVE_UNSUPPORTED_FIELDS = ['starred', 'tags', 'createdtime', 'createdby', 'modifiedtime', 'modifiedby', 'deleted', 'items'];
```

Don't try to set these via API — they're auto-managed. Inventory line items go through the `$itemGroupKey` (default `items`) argument of `saveRecord()`, which calls `saveWithItems()`.

## setResponse() Pattern

```php
// Signature: setResponse($code, $data = '') — code FIRST; it echoes JSON and calls exit
$this->setResponse(200, ['success' => 1, 'id' => $recordId]);

// After this line, code never executes
```

`TraitAPILogger::setResponse()` also writes the response into the API log when `saveLog()` opened one.

## Complex Inbound: Sub-handler Pattern (CloudBot)

For APIs with many endpoints, define `processActionBySubHandler()` on the router; Endpoint.php calls it instead of `$instance->$method()`. Real implementation: `include/Webservice/CloudBotApiHandler.php`.

```php
<?php

// include/Webservice/CloudBotApiHandler.php (abridged)
require_once('include/Webservice/CloudBotApi/AbstractCloudBotApi.php');

class CloudBotApiHandler extends AbstractCloudBotApi {

	// Actions that bypass checkSession()
	protected $skipCheckToken = ['authenticate'];

	protected function getAPIHandlerByAction(string $action): AbstractCloudBotApi {
		$handlerClass = '';

		switch ($action) {
			case 'saveTicket':
			case 'validateCustomer': {
				$handlerClass = 'CustomerServiceApiHandler';
				break;
			}

			default: {
				$handlerClass = 'CloudBotApiHandler';
				break;
			}
		}

		if ($handlerClass == 'CloudBotApiHandler') {
			return $this;
		}

		if (!file_exists("include/Webservice/CloudBotApi/{$handlerClass}.php")) {
			$this->setResponse(200, ['success' => 0, 'message' => 'Invalid action']);
		}

		require_once("include/Webservice/CloudBotApi/{$handlerClass}.php");
		$apiHandler = new $handlerClass();
		$apiHandler->setLogId($this->logId);

		return $apiHandler;
	}

	/** @return void */
	public function processActionBySubHandler(string $action, Vtiger_Request $request) {
		$apiHandler = $this->getAPIHandlerByAction($action);

		if (!method_exists($apiHandler, $action)) {
			$this->setResponse(200, ['success' => 0, 'message' => 'Invalid action']);
		}

		$apiHandler->$action($request);
	}
}
```

**Sub-handler** — `include/Webservice/CloudBotApi/{Class}.php`, extends `AbstractCloudBotApi` (which sets `$channel = 'CloudBot'`, `isEnabled()`, its own `checkSession()`, cache manager). Add the new action to the router's `switch`.

```php
<?php

// include/Webservice/CloudBotApi/CustomerServiceApiHandler.php (shape)
require_once('include/Webservice/CloudBotApi/AbstractCloudBotApi.php');

class CustomerServiceApiHandler extends AbstractCloudBotApi {

	// API /validateCustomer
	public function validateCustomer(Vtiger_Request $request) {
		// Shared utilities from AbstractCloudBotApi: doSave(), searchModule(), getRecordDetail(), syncCustomerInfo(), $cacheManager
		$data = $this->getProcessedRequestData(__FUNCTION__, $request);

		$this->setResponse(200, ['success' => 1, 'data' => $data]);
	}
}
```

## Critical Pitfalls

1. **Extend IntegrationApiHandler** - NOT standalone class; file must be `include/Webservice/{Platform}ApiHandler.php`
2. **$intergrationAPIs typo** - MUST keep (read by `getAPIDefination()`)
3. **SAVE_UNSUPPORTED_FIELDS** - silently stripped by `saveRecord()`, don't try to set
4. **setResponse($code, $data) calls exit** - code first, data second
5. **`public function isEnabled(): bool`** - parent is `abstract public`; read from config, don't hardcode `true`
6. **`$request` is `Vtiger_Request`** - use `$request->get()`, not array access
7. **Upsert** - resolve the existing id yourself, then pass it as `saveRecord()`'s 3rd argument

## URL Format

```
# 1. Get a token (GET only)
GET https://crm.domain.com/api/IntegrationAPI/Zalo/auth?username=admin&access_key_md5={md5(user accesskey)}
→ {"success":1,"access_token":"...","created_time":...,"expire_time":...}

# 2. Call an action with the token header
POST https://crm.domain.com/api/IntegrationAPI/Zalo/createCustomer

Headers:
  token: {access_token}
  Content-Type: application/json

Body:
{
  "phone": "0901234567",
  "name": "Nguyen Van A"
}
```

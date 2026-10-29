# Outbound API - CPAPIIntegration_Channel

## Overview

`CPAPIIntegration_Channel` (`modules/CPAPIIntegration/Channel.php`) provides a consistent pattern for integrating VTiger with external platforms (Facebook, TikTok, Zalo, etc.). Real concrete channel to copy: `modules/CPAPIIntegration/channels/TMS.php`.

## Channel Skeleton

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

// modules/CPAPIIntegration/channels/Facebook.php
require_once('modules/CPAPIIntegration/Channel.php');

class CPAPIIntegration_Facebook_Channel extends CPAPIIntegration_Channel {

	// Non-empty logger name => makeRequest() logs every call via CPLogAPI_Logger_Helper
	protected $logger = 'Facebook';
	protected $enableLog = true;

	public function __construct($configs = []) {
		global $facebookAdsConfig;

		parent::__construct(array_merge($configs, (array) $facebookAdsConfig));
		$this->uri = 'https://graph.facebook.com/' . $this->getParameters('api_version');
	}

	// CRITICAL: read from config, don't hardcode
	public function isEnabled(): bool {
		return !empty($this->getParameters('enabled'));
	}

	// Show "Sync to Facebook" button on the record detail view
	public function isEnabledButtonSyncToExternalSystems($recordModel): bool {
		return false;
	}

	public function getModuleSyncToExternalSystems(): array {
		return ['Campaigns', 'Leads', 'Contacts'];
	}

	// Field name storing sync status
	protected function getStatusKey(): string {
		return 'status';
	}

	protected function getAccessToken(): string {
		return (string) $this->getParameters('access_token');
	}

	public function getCampaigns(string $accountId, string $since = ''): array {
		if (empty($accountId)) return [];

		$params = [
			'access_token' => $this->getAccessToken(),
			'fields' => 'id,name,status,objective,created_time,updated_time',
			'limit' => 100,
		];

		if (!empty($since)) {
			$params['filtering'] = json_encode([
				['field' => 'updated_time', 'operator' => 'GREATER_THAN', 'value' => $since],
			]);
		}

		return $this->makeGetRequest($this->getServiceUrl("{$accountId}/campaigns"), __FUNCTION__, $params);
	}

	public function createCampaign(string $accountId, array $campaignData): array {
		if (empty($accountId)) return [];

		return $this->makePostRequest($this->getServiceUrl("{$accountId}/campaigns"), __FUNCTION__, $campaignData);
	}

	public function updateCampaign(string $campaignId, array $campaignData): array {
		if (empty($campaignId)) return [];

		return $this->makePutRequest($this->getServiceUrl($campaignId), __FUNCTION__, $campaignData);
	}

	public function deleteCampaign(string $campaignId): array {
		if (empty($campaignId)) return [];

		return $this->makeDeleteRequest($this->getServiceUrl($campaignId), __FUNCTION__);
	}
}
```

## Abstract Methods (MUST Implement)

Copied from `modules/CPAPIIntegration/Channel.php` — overrides must keep this visibility and return type:

```php
// Field name storing sync status
abstract protected function getStatusKey() : string;

// Access token used to call the API (no parameter — read it from configs/properties)
abstract protected function getAccessToken() : string;

// Modules that can sync to the external system
abstract public function getModuleSyncToExternalSystems() : array;

// Show "Sync to {Platform}" button on detail view
abstract public function isEnabledButtonSyncToExternalSystems($recordModel) : bool;

// Integration on/off
abstract public function isEnabled() : bool;
```

A subclass may widen `protected` to `public` (TMS does for `getStatusKey()`/`getAccessToken()`), never narrow `public` to `protected`.

## Built-in HTTP Methods

```php
public function makeGetRequest(string $serviceUrl, string $action, array $params = [], array $headers = []): array;
public function makePostRequest(string $serviceUrl, string $action, array $params = [], array $headers = []): array;
public function makePutRequest(string $serviceUrl, string $action, array $params = [], array $headers = []): array;
public function makeDeleteRequest(string $serviceUrl, string $action, array $params = [], array $headers = [], bool $isParamsHeader = false): array;

// Helper (protected): getServiceUrl(string $extendService = ''): string
// = rtrim($this->uri, '/') . '/' . ltrim($extendService, '/')
```

All four go through `makeRequest()`, which returns `[]` without calling when `isEnabled()` is false, and sends JSON via `CPAPIIntegration_CURLClient` (GET/DELETE params become the query string).

**Logging:** only when `$this->logger` is non-empty (base default is `''`) and `$enableLog` is true. Then `CPLogAPI_Logger_Helper::saveLogApi($this->logger, …)` stores the request (direction `OUTBOUND`) and `setResponseLog()` stores the response — in the SQLite3 or Elastic backend chosen by `CPLogAPI_Logger_Helper::getHandleClassName()`, not a `vtiger_*` table. Override `getResponseStatus($apiResponse)` to return `'success'` for successful responses, otherwise every logged call is marked `error`.

## Channel Naming Convention

```
modules/CPAPIIntegration/channels/{Name}.php  →  class CPAPIIntegration_{Name}_Channel
```

Examples:
- `CPAPIIntegration_TMS_Channel` (real, `channels/TMS.php`)
- `CPAPIIntegration_Facebook_Channel`
- `CPAPIIntegration_TikTok_Channel`

Picklist mappings for a channel live in class `CPAPIIntegration_{Name}_Enum` (see `modules/CPAPIIntegration/enums/Base.php`); `getPicklistMappings()` resolves it from `getName()` = `$configs['name']`.

## Config Pattern

```php
// In config.inc.php or config.env.php — merged into $configs by the channel constructor
$facebookAdsConfig = [
	'name' => 'Facebook',
	'enabled' => true,
	'api_version' => 'v18.0',
	'app_id' => 'YOUR_APP_ID',
	'app_secret' => 'YOUR_APP_SECRET',
];

$tiktokAdsConfig = [
	'name' => 'TikTok',
	'enabled' => true,
	'api_version' => 'v1.3',
	'app_id' => 'YOUR_APP_ID',
	'secret' => 'YOUR_SECRET',
];
```

## Data Formatting Helpers

The base class already has `protected function processFormatData($entityData, &$params)` (CRM → external: picklist, boolean, datetime). Do NOT reuse that name for external → CRM mapping — a different signature is a fatal at class load. Use a separate public helper:

```php
// Map one external record into CRM field values
public function mapExternalData(array $externalData, string $module): array {
	$crmData = [];

	foreach ($this->getFieldMapping($module) as $externalField => $crmField) {
		$crmData[$crmField] = $externalData[$externalField] ?? '';
	}

	if (!empty($externalData['status'])) {
		$crmData['campaignstatus'] = $this->getStatusMapping()[$externalData['status']] ?? '';
	}

	return $crmData;
}

private function getFieldMapping(string $module): array {
	$mappings = [
		'Campaigns' => [
			'id' => 'social_campaign_id',
			'name' => 'campaignname',
			'objective' => 'campaigntype',
			'budget' => 'budgetcost',
			'spend' => 'actualcost',
		],
		'Leads' => [
			'id' => 'external_lead_id',
			'email' => 'email',
			'phone' => 'phone',
			'first_name' => 'firstname',
			'last_name' => 'lastname',
		],
	];

	return $mappings[$module] ?? [];
}

private function getStatusMapping(): array {
	return [
		'ACTIVE' => 'Active',
		'PAUSED' => 'Inactive',
		'ARCHIVED' => 'Inactive',
		'DELETED' => 'Cancelled',
	];
}
```

## Complete Example: TikTok Channel

```php
<?php

// modules/CPAPIIntegration/channels/TikTok.php
require_once('modules/CPAPIIntegration/Channel.php');

class CPAPIIntegration_TikTok_Channel extends CPAPIIntegration_Channel {

	protected $logger = 'TikTok';

	public function __construct($configs = []) {
		global $tiktokAdsConfig;

		parent::__construct(array_merge($configs, (array) $tiktokAdsConfig));
		$this->uri = 'https://business-api.tiktok.com/open_api/' . $this->getParameters('api_version');
	}

	public function isEnabled(): bool {
		return !empty($this->getParameters('enabled'));
	}

	public function isEnabledButtonSyncToExternalSystems($recordModel): bool {
		return false;
	}

	public function getModuleSyncToExternalSystems(): array {
		return ['Campaigns', 'CPAdvertisers'];
	}

	protected function getStatusKey(): string {
		return 'status';
	}

	// Token of the ads account passed in $configs['access_token']
	protected function getAccessToken(): string {
		return (string) $this->getParameters('access_token');
	}

	protected function getResponseStatus($apiResponse) {
		return isset($apiResponse['code']) && $apiResponse['code'] == 0 ? 'success' : 'error';
	}

	public function getCampaigns(string $advertiserId, array $filters = []): array {
		if (empty($advertiserId)) return [];

		$params = [
			'advertiser_id' => $advertiserId,
			'page_size' => 100,
			'page' => $filters['page'] ?? 1,
		];

		if (!empty($filters['campaign_ids'])) {
			$params['campaign_ids'] = $filters['campaign_ids'];
		}

		$response = $this->makeGetRequest($this->getServiceUrl('campaign/get/'), __FUNCTION__, $params, ['Access-Token' => $this->getAccessToken()]);

		return $response['data']['list'] ?? [];
	}

	public function createCampaign(string $advertiserId, array $campaignData): array {
		if (empty($advertiserId)) return [];

		$params = array_merge(['advertiser_id' => $advertiserId], $campaignData);

		return $this->makePostRequest($this->getServiceUrl('campaign/create/'), __FUNCTION__, $params, ['Access-Token' => $this->getAccessToken()]);
	}

	// Refresh the token stored on the ads account record when it has expired
	public function refreshAccessToken(Vtiger_Record_Model $adsAccount): void {
		$expiryDate = (string) $adsAccount->get('token_expired_date');
		if (empty($expiryDate) || strtotime($expiryDate) > time()) return;

		$params = [
			'app_id' => $this->getParameters('app_id'),
			'secret' => $this->getParameters('secret'),
			'refresh_token' => $adsAccount->get('refresh_token'),
		];
		$result = $this->makePostRequest($this->getServiceUrl('oauth2/refresh_token/'), __FUNCTION__, $params);

		if (!empty($result['data']['access_token'])) {
			$adsAccount->set('mode', 'edit');
			$adsAccount->set('access_token', $result['data']['access_token']);
			$adsAccount->set('token_expired_date', date('Y-m-d H:i:s', time() + (int) $result['data']['expires_in']));
			$adsAccount->save();

			$this->configs['access_token'] = $result['data']['access_token'];
		}
	}
}
```

## Usage from Cron/Handler

```php
require_once('modules/CPAPIIntegration/channels/Facebook.php');

$adsAccountRecord = Vtiger_Record_Model::getInstanceById($adsAccountId, 'CPAdvertisingAccount');

// Constructor requires $configs (merged with the global config inside the channel)
$channel = new CPAPIIntegration_Facebook_Channel(['access_token' => $adsAccountRecord->get('access_token')]);

if (!$channel->isEnabled()) {
	return;
}

$campaigns = $channel->getCampaigns((string) $adsAccountRecord->get('account_id'), $lastSyncDateTime);

foreach ($campaigns['data'] ?? [] as $campaignData) {
	$crmData = $channel->mapExternalData($campaignData, 'Campaigns');
	// Save to CRM
}
```

## Critical Rules

1. **Logging is opt-in** - set `protected $logger = '<Name>'`; the base default `''` logs nothing
2. **isEnabled() from config** - Never hardcode `true`; `makeRequest()` returns `[]` when disabled
3. **Channel naming** - `modules/CPAPIIntegration/channels/{Name}.php` → `CPAPIIntegration_{Name}_Channel`
4. **Abstract methods** - implement all 5 with the exact visibility/return type of `Channel.php`
5. **Constructor** - base `__construct($configs)` requires configs; call `parent::__construct()`
6. **Token refresh** - `getAccessToken()` takes no argument; refresh outside it and update `$this->configs`
7. **Guard clauses** - Check empty params before API calls
8. **Error handling** - a failed call returns the decoded error body or `[]`; check the payload, not exceptions
9. **Don't override `processFormatData()`** with another signature - name mapping helpers differently

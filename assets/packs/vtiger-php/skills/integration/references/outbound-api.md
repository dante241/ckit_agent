# Outbound API - CPAPIIntegration_Channel

## Overview

`CPAPIIntegration_Channel` provides consistent pattern for integrating VTiger with external platforms (Facebook, TikTok, Zalo, etc.).

## Channel Skeleton

```php
<?php

/**
 * Facebook Ads Channel
 * Location: include/CPAPIIntegration/Facebook_Channel.php
 */

require_once 'include/CPAPIIntegration/CPAPIIntegration_Channel.php';

class Facebook_Channel extends CPAPIIntegration_Channel {

    protected $channel = 'facebook';

    // CRITICAL: Read from config, don't hardcode
    protected function isEnabled(): bool {
        global $facebookAdsConfig;
        return !empty($facebookAdsConfig['enabled']) && $facebookAdsConfig['enabled'];
    }

    protected function getAccessToken(Record_Model $record): string {
        return (string) $record->get('access_token');
    }

    protected function getStatusKey(): string {
        return 'status'; // Field name storing sync status
    }

    protected function getModuleSyncToExternalSystems(): array {
        return ['Campaigns', 'Leads', 'Contacts'];
    }

    protected function isEnabledButtonSyncToExternalSystems(): bool {
        return true; // Show "Sync to Facebook" button in UI
    }

    // Business methods - auto-logged via CPLogAPI_Logger_Helper
    public function getCampaigns(string $accountId, string $since = ''): array {
        if (empty($accountId)) return [];

        $url = "https://graph.facebook.com/v18.0/{$accountId}/campaigns";
        $params = [
            'fields' => 'id,name,status,objective,created_time,updated_time',
            'limit' => 100,
        ];

        if (!empty($since)) {
            $params['filtering'] = json_encode([
                ['field' => 'updated_time', 'operator' => 'GREATER_THAN', 'value' => $since],
            ]);
        }

        // makeGetRequest auto-logs to CPLogAPI
        return $this->makeGetRequest($url, __FUNCTION__, $params);
    }

    public function createCampaign(string $accountId, array $campaignData): array {
        if (empty($accountId)) return [];

        $url = "https://graph.facebook.com/v18.0/{$accountId}/campaigns";

        return $this->makePostRequest($url, __FUNCTION__, $campaignData);
    }

    public function updateCampaign(string $campaignId, array $campaignData): array {
        if (empty($campaignId)) return [];

        $url = "https://graph.facebook.com/v18.0/{$campaignId}";

        return $this->makePutRequest($url, __FUNCTION__, $campaignData);
    }

    public function deleteCampaign(string $campaignId): array {
        if (empty($campaignId)) return [];

        $url = "https://graph.facebook.com/v18.0/{$campaignId}";

        return $this->makeDeleteRequest($url, __FUNCTION__, []);
    }
}
```

## Abstract Methods (MUST Implement)

```php
// Check if integration is enabled
protected function isEnabled(): bool;

// Get access token for API calls
protected function getAccessToken(Record_Model $record): string;

// Field name storing sync status
protected function getStatusKey(): string;

// Modules that can sync to external system
protected function getModuleSyncToExternalSystems(): array;

// Show "Sync to {Platform}" button in UI
protected function isEnabledButtonSyncToExternalSystems(): bool;
```

## Built-in HTTP Methods

All methods auto-log via `CPLogAPI_Logger_Helper`:

```php
// GET request
protected function makeGetRequest(
    string $url,
    string $function,
    array $params = []
): array;

// POST request
protected function makePostRequest(
    string $url,
    string $function,
    array $params = []
): array;

// PUT request
protected function makePutRequest(
    string $url,
    string $function,
    array $params = []
): array;

// DELETE request
protected function makeDeleteRequest(
    string $url,
    string $function,
    array $params = []
): array;
```

**Logging:** Logs automatically stored in `vtiger_cplogapi` table.

## Channel Naming Convention

```
{Name}_Channel
```

Examples:
- `Facebook_Channel`
- `TikTok_Channel`
- `Zalo_Channel`
- `GoogleAds_Channel`

## Config Pattern

```php
// In config.inc.php or config.env.php
$facebookAdsConfig = [
    'enabled' => true,
    'api_version' => 'v18.0',
    'app_id' => 'YOUR_APP_ID',
    'app_secret' => 'YOUR_APP_SECRET',
];

$tiktokAdsConfig = [
    'enabled' => true,
    'api_version' => 'v1.3',
    'app_id' => 'YOUR_APP_ID',
    'secret' => 'YOUR_SECRET',
];
```

## Data Formatting Helpers

```php
// Process external data before saving to CRM
protected function processFormatData(array $externalData, string $module): array {
    $crmData = [];

    // Field mapping
    $fieldMapping = $this->getFieldMapping($module);
    foreach ($fieldMapping as $externalField => $crmField) {
        $crmData[$crmField] = $externalData[$externalField] ?? '';
    }

    // Status mapping
    if (!empty($externalData['status'])) {
        $statusMapping = $this->getStatusMapping();
        $crmData['status'] = $statusMapping[$externalData['status']] ?? '';
    }

    // Date conversion
    if (!empty($externalData['created_time'])) {
        $crmData['createdtime'] = $this->formatDateTime($externalData['created_time']);
    }

    return $crmData;
}

private function getFieldMapping(string $module): array {
    $mappings = [
        'Campaigns' => [
            'id' => 'social_campaign_id',
            'name' => 'campaignname',
            'status' => 'campaignstatus',
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

private function formatDateTime(string $datetime): string {
    // External: ISO 8601 (2025-02-10T14:30:00+0000)
    // CRM: Y-m-d H:i:s (2025-02-10 14:30:00)
    return date('Y-m-d H:i:s', strtotime($datetime));
}
```

## Complete Example: TikTok Channel

```php
<?php

require_once 'include/CPAPIIntegration/CPAPIIntegration_Channel.php';

class TikTok_Channel extends CPAPIIntegration_Channel {

    protected $channel = 'tiktok';
    protected $apiVersion = 'v1.3';
    protected $baseUrl = 'https://business-api.tiktok.com/open_api';

    protected function isEnabled(): bool {
        global $tiktokAdsConfig;
        return !empty($tiktokAdsConfig['enabled']);
    }

    protected function getAccessToken(Record_Model $record): string {
        // Check token expiry
        $expiryDate = (string) $record->get('token_expired_date');
        if (!empty($expiryDate) && strtotime($expiryDate) < time()) {
            // Token expired - refresh
            $this->refreshAccessToken($record);
        }

        return (string) $record->get('access_token');
    }

    protected function getStatusKey(): string {
        return 'status';
    }

    protected function getModuleSyncToExternalSystems(): array {
        return ['Campaigns', 'CPAdvertisers'];
    }

    protected function isEnabledButtonSyncToExternalSystems(): bool {
        return true;
    }

    public function getCampaigns(string $advertiserId, array $filters = []): array {
        if (empty($advertiserId)) return [];

        $url = "{$this->baseUrl}/{$this->apiVersion}/campaign/get/";
        $params = [
            'advertiser_id' => $advertiserId,
            'page_size' => 100,
            'page' => $filters['page'] ?? 1,
        ];

        if (!empty($filters['campaign_ids'])) {
            $params['campaign_ids'] = $filters['campaign_ids'];
        }

        $response = $this->makeGetRequest($url, __FUNCTION__, $params);

        return $response['data']['list'] ?? [];
    }

    public function createCampaign(string $advertiserId, array $campaignData): array {
        if (empty($advertiserId)) return [];

        $url = "{$this->baseUrl}/{$this->apiVersion}/campaign/create/";
        $params = array_merge(['advertiser_id' => $advertiserId], $campaignData);

        return $this->makePostRequest($url, __FUNCTION__, $params);
    }

    private function refreshAccessToken(Record_Model $record): void {
        global $tiktokAdsConfig;

        $url = 'https://business-api.tiktok.com/open_api/v1.3/oauth2/refresh_token/';
        $params = [
            'app_id' => $tiktokAdsConfig['app_id'],
            'secret' => $tiktokAdsConfig['secret'],
            'refresh_token' => $record->get('refresh_token'),
        ];

        $result = $this->makePostRequest($url, __FUNCTION__, $params);

        if (!empty($result['data']['access_token'])) {
            $record->set('access_token', $result['data']['access_token']);
            $record->set('token_expired_date', date('Y-m-d H:i:s', time() + $result['data']['expires_in']));
            $record->set('mode', 'edit');
            $record->save();
        }
    }
}
```

## Usage from Cron/Handler

```php
// Get channel instance
$channel = new Facebook_Channel();

if (!$channel->isEnabled()) {
    return;
}

// Get ads account record
$adsAccountRecord = Vtiger_Record_Model::getInstanceById($adsAccountId, 'CPAdvertisingAccount');

// Fetch campaigns
$campaigns = $channel->getCampaigns(
    $adsAccountRecord->get('account_id'),
    $lastSyncDateTime
);

// Process campaigns
foreach ($campaigns as $campaignData) {
    $crmData = $channel->processFormatData($campaignData, 'Campaigns');
    // Save to CRM
}
```

## Critical Rules

1. **Auto-logging** - All HTTP methods log to CPLogAPI automatically
2. **isEnabled() from config** - Never hardcode `true`
3. **Channel naming** - `{Name}_Channel` pattern
4. **Abstract methods** - MUST implement all 5 abstract methods
5. **Token refresh** - Check expiry in getAccessToken()
6. **Guard clauses** - Check empty params before API calls
7. **Error handling** - HTTP methods return empty array on error
8. **Data formatting** - Use helpers for field/status/date mapping

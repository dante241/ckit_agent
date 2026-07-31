# Webhook Connector Pattern

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## Architecture

Webhook connectors receive callbacks from external services (callcenter, social, IoT).

- **Base class:** extends `Vtiger_EntryPoint`
- **URL:** `entrypoint.php?name=<ConnectorName>`
- **File:** `include/Webhooks/<Provider>Connector.php`

## Connector Pattern

```php
<?php

/**
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 */

require_once('include/utils/CallCenterUtils.php');

class StringeeConnector extends Vtiger_EntryPoint {

    public function process(Vtiger_Request $request): void {
        CallCenterUtils::checkConfig();

        $request = CallCenterUtils::getRequest();
        $data = $request->getAllPurified();

        // Route by action/event type
        if ($data['action'] == 'GetIVRRouting') {
            $response = PBXManager_Stringee_Connector::getIvrRouting(...);
            header('Content-Type: application/json');
            echo json_encode($response);
            exit;
        }

        // Process webhook event
        // Log, validate, transform, save to CRM
    }
}
```

## 38 Connectors in `include/Webhooks/`

**Call Center (16):**
StringeeConnector, CloudFoneConnector, CloudCALLConnector, OmiCallConnector, VoIP24HConnector, Tel4VNConnector, SouthTelecomConnector, FPTTelecomConnector, CMCTelecomConnector, FreePBXConnector, GrandStreamConnector, VoiceCloudConnector, XorcomConnector, YeaStarConnector, VCSConnector, MiTekConnector

**Social/Chat (5):**
FacebookConnector, ZaloConnector, IndividualZaloConnector, TelegramConnector, TawkConnector

**Other (7):**
AbenlaConnector, BBHConnector, BankHubConnector, ChatBotAIConnector, HanaConnector, MauticConnector, SunOceanConnector

**IoT/Camera (3):**
CMCCloudCameraConnector, HanetAICameraConnector, IoTDeviceConnector

**Infrastructure (4):**
BaseBSConnector, DataWarehouseConnector, WebsiteWidgetConnector, OTTCallback

## Common Patterns

- `CallCenterUtils::checkConfig()` -- validate call center config
- `CallCenterUtils::getRequest()` -- parse webhook request body
- `CallCenterUtils::saveLog()` -- log webhook data
- Route by `$data['action']` or event type
- Delegate to `PBXManager_{Provider}_Connector` for provider-specific logic
- Always `exit` after response (webhook expects timely reply)

## Pitfalls

1. **Extends Vtiger_EntryPoint** -- NOT standalone or Action
2. **Public URL** -- validate all requests (API key, signature)
3. **Always exit** after JSON response
4. **Log all payloads** for debugging
5. **Handle retries** -- webhooks may retry, implement idempotency
6. **Phone normalization** -- Vietnamese numbers have multiple formats (0xx, +84xx, 84xx)
7. **Call recordings** -- URLs may expire, store or download promptly

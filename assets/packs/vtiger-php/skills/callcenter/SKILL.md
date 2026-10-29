---
name: callcenter
description: "VTiger call center — PBXManager connector, webhook tổng đài, phone normalize, call log, popup cuộc gọi. Use when: tích hợp tổng đài, callcenter, VoIP, cuộc gọi, click-to-call; keywords: PBX, call, tổng đài."
user-invocable: false
---

# VTiger Call Center Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Integrating telephony providers (PBX, VoIP, cloud call centers)
- Creating webhook receivers for call events
- Phone number cleanup / customer lookup by phone
- Creating call logs (PBXManager) and the real-time call popup
- Supporting call recording URLs
- Building custom call center connectors

## Connector Architecture (two layers)

```
Provider → webhook.php?name=<Provider>Connector&secret_key=<key>
         → include/Webhooks/<Provider>Connector.php   (class <Provider>Connector extends Vtiger_EntryPoint)
             CallCenterUtils::checkConfig() → getRequest()->getAllPurified() → saveLog()
             → PBXManager_<Provider>_Connector::forwardToCallCenterBridge($msg)   (popup)
             → PBXManager_<Provider>_Connector::handleCallEvent($data)             (call log)
         → modules/PBXManager/connectors/<Provider>.php (class PBXManager_<Provider>_Connector extends PBXManager_Base_Connector)
             settings, makeCall, recordings, handleCallEvent → PBXManager_Data_Model::handleStartupCall/updateCallStatus/handleHangupCall
```

- `webhook.php` checks `secret_key` (except `VCSConnector`), loads `include/Webhooks/<name>.php`, runs `process()`, and logs uncaught `Throwable` via `WebhookUtils::saveLog`.
- `CallCenterUtils::checkConfig()` `die()`s unless the active connector (`PBXManager_Server_Model::getActiveConnector()`) matches `<name>` minus `Connector`.

## Available Call Center Connectors (19)

Abenla, BaseBS, CMCTelecom, CloudCALL, CloudFone, FPTTelecom, FreePBX, GrandStream, MiTek, OmiCall, SouthTelecom, Stringee, SunOcean, Tel4VN, VCS, VoIP24H, VoiceCloud, Xorcom, YeaStar — each has both `include/Webhooks/<P>Connector.php` and `modules/PBXManager/connectors/<P>.php`. Other `include/Webhooks/*Connector.php` (Facebook, Zalo, Telegram, Tawk, …) are social/other integrations, not call center.

## Critical Patterns

### 1. Phone numbers — reuse helpers, don't write a normalizer
- `CallCenterUtils::getCustomerPhoneNumber($caller, $receiver, $direction)` / `getAgentExtNumber(...)` pick the right side by direction.
- `PBXManager_Logic_Helper::cleanupPhoneNumber()`, `addLeadingZeroToPhoneNumber()`, `addVnCountryCodeToPhoneNumber()` for format changes.

### 2. Customer / agent lookup
```php
$customer = PBXManager_Data_Model::findCustomerByPhoneNumber($customerPhoneNumber, $isOutbound, $agentExtNumber, true);
$agent = PBXManager_Data_Model::findAgentByExtNumber($agentExtNumber);
```

### 3. Call log (table `vtiger_pbxmanager`, keyed by `sourceuuid` = provider call id)
```php
PBXManager_Data_Model::handleStartupCall([
	'direction' => $direction,
	'callstatus' => 'ringing',
	'starttime' => date('Y-m-d H:i:s'),
	'sourceuuid' => $callId,
	'gateway' => 'CloudFone',
	'user' => $agent['id'],
	'customer' => $customer['id'],
	'customernumber' => $customerPhoneNumber,
	'customertype' => $customer['type'],
	'assigned_user_id' => $agent['id'],
]);
PBXManager_Data_Model::updateCallStatus($callId, ['callstatus' => 'answered']);
PBXManager_Data_Model::handleHangupCall($callId, ['callstatus' => 'completed', 'endtime' => date('Y-m-d H:i:s'), 'totalduration' => $total, 'billduration' => $bill, 'recordingurl' => $url]);
```
`callstatus` picklist values: `ringing`, `answered`, `hangup`, `completed`.

## References

- [Call Center Overview](references/callcenter-overview.md) — architecture, request flow, popup states, integration points
- [Connector Pattern](references/connector-pattern.md) — skeleton for both layers (based on CloudFone)
- [CallCenter Utils](references/callcenter-utils.md) — real API list (CallCenterUtils, WebhookUtils, PBXManager_Data_Model, PBXManager_Logic_Helper), fields, statuses

## Exemplars

- Webhook entry: `include/Webhooks/CloudFoneConnector.php` (simple), `include/Webhooks/StringeeConnector.php` (IVR routing, transfer, missed call)
- Connector: `modules/PBXManager/connectors/CloudFone.php`; base class `modules/PBXManager/BaseConnector.php`
- Data layer: `modules/PBXManager/models/Data.php`

## Verify

```bash
php -l <file>
# Replay a real provider payload (from old webhook logs) to
# webhook.php?name=<Provider>Connector&secret_key=<key>
# then check the vtiger_pbxmanager row (sourceuuid, callstatus) + popup event
```

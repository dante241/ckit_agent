# Call Center Overview

## Architecture

**Provider webhook → `webhook.php` → `include/Webhooks/<P>Connector` (entry) → `PBXManager_<P>_Connector` (provider logic) → `PBXManager_Data_Model` (call log) + Call Center Bridge (popup)**

```
External Provider          VTiger System
┌─────────────┐   POST    ┌───────────────────────────────────────────┐
│   PBX/VoIP  │──────────▶│ webhook.php?name=<P>Connector&secret_key= │
│   Provider  │           └──────────────────┬────────────────────────┘
└─────────────┘                              │ checkSecretKey (skip: VCSConnector)
                                             ▼
                         include/Webhooks/<P>Connector.php  (extends Vtiger_EntryPoint)
                           ├─ CallCenterUtils::checkConfig()        → active connector must be <P>
                           ├─ CallCenterUtils::getRequest()->getAllPurified()
                           ├─ CallCenterUtils::saveLog('[<P>] Webhook data', null, $data)
                           ├─ PBXManager_<P>_Connector::isExists($callId, $status)  → drop retries
                           ├─ find agent (findAgentByExtNumber) + customer (findCustomerByPhoneNumber)
                           ├─ forwardToCallCenterBridge($msg)       → call popup (RINGING/ANSWERED/HANGUP…)
                           └─ PBXManager_<P>_Connector::handleCallEvent($data) → vtiger_pbxmanager row
```

## Two layers

| Layer | Path | Class | Responsibility |
|-------|------|-------|----------------|
| Webhook entry | `include/Webhooks/<P>Connector.php` | `<P>Connector extends Vtiger_EntryPoint` | Parse provider payload, filter noise/duplicates, resolve agent/customer, push popup message, call `handleCallEvent` |
| Provider connector | `modules/PBXManager/connectors/<P>.php` | `PBXManager_<P>_Connector extends PBXManager_Base_Connector` | `getGatewayName()`, settings (`$SETTINGS_REQUIRED_PARAMETERS`, `setServerParameters($serverModel)`), `makeCall()`, recordings/report APIs, `isExists()`, `handleCallEvent()` |

`webhook.php` runs `process()` inside `try/catch (\Throwable)` and logs failures with `WebhookUtils::saveLog('Webhook error', null, $_REQUEST, $th->getMessage())`. Unknown `name` → prints `Webhook not found!`.

## Webhook URL

```
POST {site_URL}/webhook.php?name=CloudFoneConnector&secret_key=<webhook secret>
Content-Type: application/json
```

Payload shape is provider-specific (CloudFone: `KeyRinging`, `CallNumber`, `ReceiptNumber`, `Direction`, `Status`=`Ringing|Up|Down`, `Data{TotalTimeCall,RealTimeCall,LinkFile}`).

## Popup message (Call Center Bridge)

```php
$msg = [
	'call_id' => $callId,          // Required
	'receiver_id' => $receiverId,  // Required (CRM user id)
	'state' => 'RINGING',          // RINGING/REJECTED/ANSWERED/HANGUP/TRANSFERRED/COMPLETED/CUSTOMER_INFO
];
// RINGING only: direction (INBOUND/OUTBOUND), then customer fields
CallCenterUtils::fillMsgDataForRingingEvent($msg, $customerPhoneNumber, $customer);
PBXManager_CloudFone_Connector::forwardToCallCenterBridge($msg);
```

`forwardToCallCenterBridge()` uses the hub bridge when `$callCenterConfig['use_hub_bridge']` is set, otherwise socket.io.

## Call Center Connectors (19)

Each provider below has both layers.

| Provider | Entry | Connector |
|----------|-------|-----------|
| Abenla | `AbenlaConnector` | `PBXManager_Abenla_Connector` |
| BaseBS | `BaseBSConnector` | `PBXManager_BaseBS_Connector` |
| CMCTelecom | `CMCTelecomConnector` | `PBXManager_CMCTelecom_Connector` |
| CloudCALL | `CloudCALLConnector` | `PBXManager_CloudCALL_Connector` |
| CloudFone | `CloudFoneConnector` | `PBXManager_CloudFone_Connector` |
| FPTTelecom | `FPTTelecomConnector` | `PBXManager_FPTTelecom_Connector` |
| FreePBX | `FreePBXConnector` | `PBXManager_FreePBX_Connector` |
| GrandStream | `GrandStreamConnector` | `PBXManager_GrandStream_Connector` |
| MiTek | `MiTekConnector` | `PBXManager_MiTek_Connector` |
| OmiCall | `OmiCallConnector` | `PBXManager_OmiCall_Connector` |
| SouthTelecom | `SouthTelecomConnector` | `PBXManager_SouthTelecom_Connector` |
| Stringee | `StringeeConnector` | `PBXManager_Stringee_Connector` |
| SunOcean | `SunOceanConnector` | `PBXManager_SunOcean_Connector` |
| Tel4VN | `Tel4VNConnector` | `PBXManager_Tel4VN_Connector` |
| VCS | `VCSConnector` (no secret_key check) | `PBXManager_VCS_Connector` |
| VoIP24H | `VoIP24HConnector` | `PBXManager_VoIP24H_Connector` |
| VoiceCloud | `VoiceCloudConnector` | `PBXManager_VoiceCloud_Connector` |
| Xorcom | `XorcomConnector` | `PBXManager_Xorcom_Connector` |
| YeaStar | `YeaStarConnector` | `PBXManager_YeaStar_Connector` |

Other `include/Webhooks/*Connector.php` files (Facebook, Zalo, IndividualZalo, Telegram, Tawk, Mautic, BankHub, …) are not call center connectors.

## Common Use Cases

### Inbound call ringing
1. Entry resolves agent from extension (`getAgentExtNumber` → `findAgentByExtNumber`)
2. Customer from caller number (`getCustomerPhoneNumber` → `findCustomerByPhoneNumber`)
3. Popup `RINGING` via `forwardToCallCenterBridge`
4. `handleCallEvent` → `PBXManager_Data_Model::handleStartupCall` (`callstatus = ringing`)

### Call answered / ended
1. `updateCallStatus($callId, ['callstatus' => 'answered'])`
2. On end: `handleHangupCall($callId, [...'callstatus' => 'completed', 'endtime', 'totalduration', 'billduration', 'recordingurl'])`

### Missed call (see `PBXManager_Stringee_Connector::handleMissedCallEvent`)
1. Inbound call ends while status is still `ringing`
2. `updateCallStatus($callId, ['callstatus' => 'hangup'])`
3. `CallCenterUtils::getMissedCallAlertUsers($customer)` → `saveMissedCallLog(...)` → `sendMissedCallNotification(...)`

## Integration Points

- **PBXManager module**: call logs (`vtiger_pbxmanager`)
- **Customer lookup**: Contacts / Leads / CPTarget / Accounts via `PBXManager_Data_Model::findCustomerByPhoneNumber`
- **CallCenterUtils** (`include/utils/CallCenterUtils.php`, extends `WebhookUtils`): config check, request, logging, missed-call alerts
- **Active connector + credentials**: `PBXManager_Server_Model::getActiveConnector()` → `setServerParameters($serverModel)`
- **Runtime config**: `$callCenterConfig` (`config_override.php`; `debug` enables `saveDebugLog`)

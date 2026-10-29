# CallCenter Utils — real API reference

Skeletons using these APIs: `connector-pattern.md`. All signatures below are copied from the source files.

## WebhookUtils — `include/utils/WebhookUtils.php`

| Method | Notes |
|--------|-------|
| `getRequest($isPost = true)` | `$isPost` → `RestfulApiUtils::getRequest()` (JSON/body) merged with query string; else `new Vtiger_Request($_REQUEST, $_REQUEST, true)`. Use `->getAllPurified()`. |
| `getAuthenticatedUser(&$webhookInstance)` | For webhooks that authenticate a CRM user |
| `saveLog($description, $headers = null, $input = null, $response = null)` | Writes the webhook log entry |

## CallCenterUtils — `include/utils/CallCenterUtils.php` (`extends WebhookUtils`)

| Method | Notes |
|--------|-------|
| `checkConfig()` | `checkAccessForbiddenFeature('CallCenterIntegration')`; `die('This provider is not enabled yet!')` unless `PBXManager_Server_Model::getActiveConnector()->getGatewayName()` equals `$_REQUEST['name']` minus `Connector`. Returns the active connector instance. |
| `getCustomerPhoneNumber($callerNumber, $receiverNumber, $direction)` | INBOUND → caller, else receiver |
| `getAgentExtNumber($callerNumber, $receiverNumber, $direction)` | INBOUND → receiver, else caller |
| `fillMsgDataForRingingEvent(&$msg, $customerPhoneNumber, $customerInfo)` | Adds `customer_number/id/name/type/avatar`… to the RINGING popup message |
| `getMissedCallAlertUsers($customerInfo)` | User ids to alert (reads `callcenter_config`) |
| `sendMissedCallNotification($customerInfo, $customerNumber, $userIds)` | In-app notification for a missed call |
| `sendMissedAlertEmail($customerInfo, $customerNumber, $hotline, $callTime, array $userIds)` | Missed-call email |
| `saveMissedCallLog(array $customerInfo, $customerNumber, $callId, $callTime, array $userIds)` | Missed-call history |
| `saveDebugLog($description, $headers = null, $input = null, $response = null)` | `saveLog` only when `$callCenterConfig['debug']` |
| `notifyStateDataChangedToClient($entityData)` | Push state change to clients |
| `saveLog(...)`, `getRequest(...)` | Inherited from `WebhookUtils` |

## PBXManager_Data_Model — `modules/PBXManager/models/Data.php`

Lookup:

| Method | Notes |
|--------|-------|
| `findCustomerByPhoneNumber(&$phoneNumber, $forOutbound = false, $agentExtNumber = '', $getAvatar = false, $returnMatchedRecord = false)` | Contacts/Leads/CPTarget/Accounts; outbound uses click-to-call cache. Returns customer info array (`id`, `type`, `name`, `assigned_user_*`, optional `avatar`, `call_log_id`). |
| `findCustomerById($customerId, $getAvatar = false)` | |
| `findAgentByExtNumber($extNumber)` | Agent by `phone_crm_extension` → array with `id` |
| `getRoutingByCustomerNumber($customerNumber, $hotline, $mode = 'ext')` | Skill routing |

Call state (all keyed by `sourceuuid` = provider call id):

| Method | Notes |
|--------|-------|
| `isExists($callId, $status)` | Row with that `sourceuuid` + `callstatus` (non-deleted) |
| `isTransferredCall($callId)`, `markCallAsTransferred($callId, $curAgentExt, $destAgentExt)` | Transfers |
| `handleStartupCall($data)` | Creates the PBXManager record from field => value pairs (+ popup addition info) |
| `updateCall($callId, $data, $skipCompletedCall = true)` | Generic column update |
| `updateCallStatus($callId, $data)` | Sets `callstatus` (skips completed calls) |
| `handleHangupCall($callId, $data)` | `completed` → sets `endtime`, durations (+ minutes), `recordingurl`; otherwise status only |
| `saveMissedCall(array $data, $customer, $agent, array $userIds)` | Inbound + `hangup` record |
| `getCallStatus($callId)`, `getCallDirection($callId)`, `getAgentUserIdFromCall($callId)`, `getCustomerFromCall($callId)`, `getExtraDataFromCall($callId)`, `getCallData($callId, $isParentCallId = false)`, `deleteCall($callId)` | Read/cleanup helpers |
| `saveAutoCallLog(...)`, `updateAutoCallStatus($callId, $status, $duration = 0, $responseKey = null)` | Auto-call campaigns |

## PBXManager_Logic_Helper — `modules/PBXManager/helpers/Logic.php`

| Method | Notes |
|--------|-------|
| `cleanupPhoneNumber($phoneNumber)` | Strips `( ) space + -` |
| `addLeadingZeroToPhoneNumber($phoneNumber)` | Prepends `0` unless it already starts with `0` or a known country code (`$countryCodes`) |
| `addVnCountryCodeToPhoneNumber($phoneNumber)` | `0xxx` → `84xxx`; prepends `84` if missing |
| `getGatewayName()`, `isClick2CallEnabled($moduleName)`, `canMakeCall()`, `canTransferCall()` | Click-to-call checks |
| `saveOutboundCache(...)`, `getOutboundCache($phoneNumber)` | Click-to-call → customer mapping used by outbound lookup |

## PBXManager_Base_Connector — `modules/PBXManager/BaseConnector.php`

`getGatewayName()`, `getServiceUrl($action)`, `setServerParameters($serverModel)`, `getParameters()`, `static getSettingFields()`, `getRestClient($serviceURL, $headers = [], $noHeaders = false)`, `callRestApi($curl, $method, $params, $sendAsJSON = true, $responseAsJSON = true)`, `static forwardToCallCenterBridge($msg)`, `static forwardToCallCenterHubBridge($msg)`.

## Table `vtiger_pbxmanager`

Columns: `pbxmanagerid, direction, callstatus, starttime, endtime, totalduration, billduration, recordingurl, sourceuuid, gateway, customer, user, customernumber, customertype, tags, hotline, parent_call_id, transferred, extra_data, total_duration_minutes, bill_duration_minutes, users_department`.

| Column | Meaning |
|--------|---------|
| `sourceuuid` | Provider call id (lookup key for every update) |
| `direction` | Free text (uitype 1, no picklist table); connectors store `inbound` / `outbound` |
| `callstatus` | Picklist `vtiger_callstatus`: `ringing`, `answered`, `hangup`, `completed` |
| `starttime` / `endtime` | Datetime |
| `totalduration` / `billduration` | Seconds (`*_minutes` filled by `handleHangupCall`) |
| `recordingurl` | Recording link (played via connector `getRecordingData`) |
| `gateway` | Provider name (`getGatewayName()`) |
| `user` | Agent user id |
| `customer`, `customertype`, `customernumber` | Matched CRM record id, its module, the customer phone |
| `hotline`, `parent_call_id`, `transferred`, `extra_data` | Hotline, transfer chain, JSON extra info |

## Missed call flow (real: `PBXManager_Stringee_Connector::handleMissedCallEvent`)

```php
$customer = PBXManager_Data_Model::findCustomerByPhoneNumber($customerPhoneNumber);
$userIds = CallCenterUtils::getMissedCallAlertUsers($customer);
PBXManager_Data_Model::updateCallStatus($callId, ['callstatus' => 'hangup']);
CallCenterUtils::saveMissedCallLog($customer, $customerPhoneNumber, $callId, $callTime, $userIds);
CallCenterUtils::sendMissedCallNotification($customer, $customerPhoneNumber, $userIds);
```

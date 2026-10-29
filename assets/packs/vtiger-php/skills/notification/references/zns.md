# Zalo ZNS / OTT Reference

Module `CPOTTIntegration` sends OTT messages (Zalo ZNS, Viber, Telegram) from `CPSMSTemplate` templates.

## Channels and Config

- `CPOTTIntegration_Config_Helper::getChannels()` (`helpers/Config.php:25`) → `Zalo` (Zalo ZNS), `Viber`, `Telegram`; each removed when its license feature is forbidden (`ZaloZNSIntegration`, `ViberOTTIntegration`, `TelegramOTTIntegration`).
- Per-channel config: `CPOTTIntegration_Config_Helper::getConfig($channel)` → `Settings_Vtiger_Config_Model` category `<channel>_ott_config` (`active_gateway`, `service_url`, `zalo_oa_id`, …).

## Check Availability

```php
if (!CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
	// Not licensed (SendMessageViaZaloZNS) or no active Zalo gateway
}
```

Siblings: `canSendViberSMsg()` (`SendMessageViaViber`), `canSendTelegramSMsg()` (`SendMessageTelegram`).

## Gateways

`CPOTTIntegration_Gateway_Model` (`models/Gateway.php`) has exactly:

- `getGatewayList($channel)` — every gateway in `modules/CPOTTIntegration/gateways/*.php` supporting the channel
- `getActiveGateway($channel)` — gateway named by `active_gateway` in the channel config; `null` when none
- `getGateway($gatewayName, $channel, array $config = [])`

Gateway classes: `CPOTTIntegration_<Name>_Gateway extends CPOTTIntegration_Base_Gateway` (`BaseGateway.php`, itself extends `SMSNotifier_Base_Provider`). Files: `ZaloZNS.php`, `ESMS.php`, `Mitek.php`, `SouthTelecom.php`. `$channelMethodsMapping`: `Zalo` → `sendZaloMsg`, `Viber` → `sendViberMsg`, `Telegram` → `sendTelegramMsg`.

## sendZaloMsg()

Real signature (`gateways/ZaloZNS.php:60`; ESMS/Mitek/SouthTelecom have the same params without `$sendTo`):

```php
public function sendZaloMsg($message, array $toNumbers, $templateId, array $variablesMapping, $recordId = null, $sender = '', $sendTo = 'phone')
```

| Param | Meaning |
|---|---|
| `$message` | ignored for sending; per recipient it is rebuilt from the template for the log |
| `$toNumbers` | `['0901234567' => $customerId]` (or `[$zaloUid => $customerId]` when `$sendTo = 'uid'`) |
| `$templateId` | `CPSMSTemplate` record id; mapped to the Zalo template id by `CPOTTIntegration_Data_Model::getOTTTemplateId()` |
| `$variablesMapping` | `['$var$' => 'fieldname', ...]` — template var → CRM field, NOT literal values |
| `$recordId` | when set and different from the customer id, variables are filled from this record instead |
| `$sender` | Zalo OA id; `ZaloUtils::retrieveAccessToken($sender)` gets the OA token. Valid OAs: `CPOTTIntegration_Data_Model::getZNSSenders()` |
| `$sendTo` | `'phone'` (business ZNS API) or `'uid'` (OA `message/template` by Zalo user id) |

**Return:** list of `['to', 'id' => tracking id, 'message', 'status' => 'success'|'failed', 'statusmessage', 'customer_id', 'error' => bool]`. It does NOT write `CPSMSOTTMessageLog` — the caller must (or use `SMSNotifier::sendMessage('Zalo', ...)`, which logs).

## Template Data (`CPOTTIntegration_Data_Model`, `models/Data.php`)

- Template content uses `$var$` placeholders (`parseVariables()`).
- `fillDataForVariables(array $variableMapping, $recordId)` (static) — input `['$var1$' => 'field1']`, output `['$var1$' => 'field1 value']` via `populateTemplateWithRecordData()`; `record_id` is replaced by the record id.
- `getTemplateContent($templateId, array $variablesData = [])`, `getPreviewContent()`, `getAllTemplates($channel)`, `getZNSTemplatesByOAId($oaId)`, `getOTTTemplateId($templateId)`.

## Example — ZNS with SMS fallback

```php
require_once('modules/SMSNotifier/SMSNotifier.php');

$variablesMapping = ['$customer_name$' => 'lastname', '$order_no$' => 'salesorder_no'];
$responses = [];

if (CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
	// Logs to CPSMSOTTMessageLog; variables are filled from the customer record
	$responses = SMSNotifier::sendMessage('Zalo', '', [$phone => $customerId], $znsTemplateId, $variablesMapping, $oaId, $ownerId, [$recordId]);
}

if ((empty($responses) || !empty($responses[0]['error'])) && SMSNotifier_Logic_Helper::canSendSMSMsg()) {
	SMSNotifier::sendMessage('SMS', $smsText, [$phone => $customerId], '', [], '', $ownerId, [$recordId]);
}
```

Direct gateway call (caller logs itself), as in `modules/CPAutomaticTaskList/helpers/Logic.php:102-117`:

```php
$ottGateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');

if ($ottGateway) {
	$result = $ottGateway->sendZaloMsg('', [$phone => $targetRecord], $templateId, $variablesMapping, $targetRecord, $oaId);
}
```

Real callers: `modules/CPAutomaticTaskList/helpers/Logic.php:116`, `modules/CPEventPRMessageLog/helpers/Logic.php:214`, `modules/CPOTTIntegration/workflow/VTZaloOTTMessageTask.php:140`, `modules/SMSNotifier/actions/MassSaveAjax.php:99` (via `sendMessage`).

## Workflow Task — `VTZaloOTTMessageTask`

`modules/CPOTTIntegration/workflow/VTZaloOTTMessageTask.php`, fields: `sender`, `template_id`, `send_to`, `phone_fields`, `uid_fallback_to_phone`, `variables_mapping`, `send_sms_when_workflow_zns_failed`, `sms_template_id`, `sms_variables_mapping`. Core of `doTask($entity)` (`:38`):

```php
if (!CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
	VTTask::saveLog('[VTZaloOTTMessageTask::doTask] No active gateway for Zalo ZNS or sending Zalo ZNS is not supported in this package!');
	return;
}

$activeGateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');
// ... build $toNumbers from phone_fields (or Zalo UID with phone fallback) ...
$result = $activeGateway->sendZaloMsg('', $toNumbers, $this->template_id, $this->variables_mapping, $recordId, $this->sender, $sendToForGateway);

foreach ($result as $item) {
	CPSMSOTTMessageLog_Record_Model::addNew($logData);

	if ($sendToForGateway == 'phone' && $this->send_sms_when_workflow_zns_failed && $item['status'] == $activeGateway::MSG_STATUS_FAILED) {
		$this->sendSMSFallback($item['to'], $item['customer_id'], $this->sms_template_id, $this->sms_variables_mapping, $recordId, $recordModule);
	}
}
```

`sendSMSFallback()` checks `isForbiddenFeature('SMSWorkflows')`, `SMSNotifier_Logic_Helper::hasActiveGateway()`, `SMSNotifier::checkServer()`, fills the SMS template via `fillDataForVariables()` + `getTemplateContent()`, then calls `SMSNotifier::sendMessage('SMS', $message, [$phoneNumber => $customerId], $templateId, $variablesMapping, '', $current_user->id, [$customerId])` (`:198`).

## Critical Pitfalls

1. **`$templateId` is the CRM `CPSMSTemplate` id**, not the Zalo template id
2. **`$variablesMapping` maps `'$var$'` → field name** — values come from the record
3. **`$sender` is the OA id** — wrong/empty OA → no access token → every recipient fails
4. **Return is a result list** — check `status`/`error` per item; direct gateway calls must log themselves
5. **Phone format** — pass `0xx`; the gateway converts to `84xx`
6. **Channels are only `Zalo`/`Viber`/`Telegram`**

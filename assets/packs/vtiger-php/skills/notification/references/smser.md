# SMS Reference

`SMSNotifier` is really an SMS **and** OTT notifier (`modules/SMSNotifier/SMSNotifier.php:15`). Not autoloaded: `require_once('modules/SMSNotifier/SMSNotifier.php');`

## SMSNotifier::sendMessage()

Real signature (`modules/SMSNotifier/SMSNotifier.php:41`):

```php
static function sendMessage($channel, $message, array $toNumbers, $templateId = '', array $variablesMapping = [], $sender = '', $ownerId = '', array $linkToIds = [], $linktoModule = '', $sendTo = 'phone')
```

| Param | Meaning |
|---|---|
| `$channel` | `'SMS'` → active SMS provider. Anything else is an **OTT channel** (`'Zalo'`, `'Viber'`, `'Telegram'`) routed to `CPOTTIntegration_Gateway_Model::getActiveGateway($channel)`. A provider name (`'Stringee'`, `'SpeedSMS'`…) is NOT a channel — it hits the OTT branch, finds no gateway and returns `false` |
| `$message` | SMS: text sent as-is after the provider fills record merge tags. OTT: ignored — replaced by the template content (`CPOTTIntegration_Data_Model::getTemplateContent($templateId)`) |
| `$toNumbers` | `['0901234567' => $customerId, ...]` — the customer id is used to fill merge tags/variables and is written to the message log. With `$sendTo = 'uid'` (Zalo ZNS) keys are Zalo user ids |
| `$templateId` | `CPSMSTemplate` record id (required for OTT; for SMS only stored in the log) |
| `$variablesMapping` | OTT only: `['$var$' => 'fieldname', ...]`, filled per customer by `CPOTTIntegration_Data_Model::fillDataForVariables()`; `record_id` maps to the record id |
| `$sender` | OTT only: sender account (Zalo ZNS = OA id used for the access token) |
| `$ownerId` | user assigned to the `SMSNotifier` record (default current user, else 1) |
| `$linkToIds` / `$linktoModule` | records related to the `SMSNotifier` record; module looked up per id when `$linktoModule` is empty |
| `$sendTo` | `'phone'` (default) or `'uid'` |

**Return:** the provider/gateway response list, or `false` when no active provider/gateway. Each item: `['to', 'id', 'message', 'status' => 'success'|'failed', 'statusmessage', 'customer_id', 'error' => bool]`.

## Internal Flow

1. OTT channel → `$message = CPOTTIntegration_Data_Model::getTemplateContent($templateId)`
2. Save an `SMSNotifier` record (`message`, `assigned_user_id`) and relate `$linkToIds`
3. `fireSendMessage()` (`:210`):
   - `'SMS'` → `SMSNotifierManager::getActiveProviderInstance()->send($message, $toNumbers)`
   - OTT → `$gateway->{channelMethodsMapping[$channel]}($message, $toNumbers, $templateId, $variablesMapping, null, $sender, $sendTo)`
4. `saveMessageLog()` → one `CPSMSOTTMessageLog` record per response (table `vtiger_cpsmsottmessagelog`, `extra_data` keeps template id + variables mapping)

## Provider Architecture

- Providers: `modules/SMSNotifier/providers/<Name>.php`, class `SMSNotifier_<Name>_Provider extends SMSNotifier_Base_Provider` (`modules/SMSNotifier/BaseProvider.php`, implements `SMSNotifier_ISMSProvider_Model`). Files renamed `*._php` (Twilio, ClickATell, …) are disabled; enabled ones include Stringee, SpeedSMS, VHT, VMG, VietGuys, FPTTelecom, CMCTelecom, SouthTelecom, Mitek, ESMS, AbenlaSOAP.
- Loader: `SMSProvider::getInstance($providerName)` (`modules/SMSNotifier/ext/SMSProvider.php`).
- Active provider: `SMSNotifierManager::getActiveProviderInstance()` (no args) reads the `vtiger_smsnotifier_servers` row with `isactive = 1`; cached wrapper `SMSNotifier_Provider_Model::getActiveGateway()`.
- A provider overrides `getName()`, `getServiceURL()`, `prepareParameters()`, `send($message, $toNumbers)` and optionally `query($messageId)`. `send()` calls `correctPhoneNumber()` (strips `( ) + - space`, leading `0` → `84`), `populateTemplateWithRecordData($message, $customerId)`, and most providers `unUnicode()` the text.

## Check Availability

```php
// License feature + active SMS provider
if (!SMSNotifier_Logic_Helper::canSendSMSMsg()) {
	return false;
}
```

- `SMSNotifier_Logic_Helper::canSendSMSMsg()` = `!isForbiddenFeature('SendMessageViaSMS')` && `hasActiveGateway()`
- `SMSNotifier_Logic_Helper::hasActiveGateway()` / `SMSNotifier::checkServer()` — an active provider exists
- Other SMS feature keys: `SMSIntegration`, `SMSCampaign`, `SMSWorkflows` (workflow tasks check `isForbiddenFeature('SMSWorkflows')`)

## Message Content

- `sendMessage('SMS', ...)`: the provider runs `populateTemplateWithRecordData($message, $customerId)`, so the text may contain workflow merge tags of the customer record (`$lastname`, `$(account_id : (Accounts) accountname)`).
- `SMSer::send($toNumber, $templateId, $variables, $relatedRecordId, $relatedModuleName, $senderId = '')` (`include/SMSer.php:21`): loads a `CPSMSTemplate`, replaces `%key%` from `$variables`, then calls `sendMessage('SMS', ...)`.

## Example

```php
require_once('modules/SMSNotifier/SMSNotifier.php');

if (!SMSNotifier_Logic_Helper::canSendSMSMsg()) {
	return false;
}

$message = 'Don hang $salesorder_no da duoc xac nhan';
$responses = SMSNotifier::sendMessage('SMS', $message, [$phone => $recordId], '', [], '', $ownerId, [$recordId], 'SalesOrder');

if (empty($responses) || !empty($responses[0]['error'])) {
	saveLog('SMS_OTT', '[SalesOrder] Send SMS failed', $responses);
}
```

Real callers: `include/SMSer.php:46`, `modules/SMSNotifier/workflow/VTSMSTask.php:84` (`'SMS'`), `modules/SMSNotifier/actions/MassSaveAjax.php:99,108` (SMS or OTT channel from the UI, `'uid'`/`'phone'`), `modules/CPOTTIntegration/workflow/VTZaloOTTMessageTask.php:198` (ZNS → SMS fallback).

## Critical Pitfalls

1. **`$channel` is `'SMS'` or an OTT channel** — never a provider name
2. **Return is an array or `false`** — check `$responses[n]['error']`/`['status']`
3. **Arg order** — `$sender` (6th) is the OTT sender, `$ownerId` (7th) is the user id; pass `''` for `$sender` on SMS
4. **Unicode** — most providers strip Vietnamese diacritics; keep SMS text short
5. **Feature gate** — `canSendSMSMsg()` (`SendMessageViaSMS`), workflows `SMSWorkflows`
6. **Phone format** — pass local `0xx`; providers convert to `84xx`

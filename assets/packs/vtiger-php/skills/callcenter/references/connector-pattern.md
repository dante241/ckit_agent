# Call Center Connector Pattern

A provider needs two files. Skeleton below follows the real CloudFone pair (`include/Webhooks/CloudFoneConnector.php` + `modules/PBXManager/connectors/CloudFone.php`); replace `Acme` and the payload keys with the provider's.

## 1. Provider connector — `modules/PBXManager/connectors/Acme.php`

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('modules/PBXManager/BaseConnector.php');
require_once('include/utils/CallCenterUtils.php');

class PBXManager_Acme_Connector extends PBXManager_Base_Connector {

	protected static $SETTINGS_REQUIRED_PARAMETERS = [
		'webservice_url' => 'text',
		'api_key' => 'password',
	];
	protected $webserviceUrl;
	protected $apiKey;

	// Gateway name must equal the webhook name minus "Connector" (checked by CallCenterUtils::checkConfig)
	public function getGatewayName() {
		return 'Acme';
	}

	public function setServerParameters($serverModel) {
		$this->webserviceUrl = $serverModel->get('webservice_url');
		$this->apiKey = $serverModel->get('api_key');
	}

	// Ignore provider retries of an event already stored
	static function isExists($callId, $status) {
		$statusMapping = [
			'Ringing' => 'ringing',
			'Up' => 'answered',
			'Down' => 'hangup',
		];

		return PBXManager_Data_Model::isExists($callId, $statusMapping[$status] ?? '');
	}

	// Persist the call log for every webhook event
	static function handleCallEvent($data) {
		$callId = $data['KeyRinging'];

		if ($data['Status'] == 'Ringing') {
			$customerPhoneNumber = CallCenterUtils::getCustomerPhoneNumber($data['CallNumber'], $data['ReceiptNumber'], $data['Direction']);
			$agent = $GLOBALS['agent'];
			$customer = $GLOBALS['customer'];

			PBXManager_Data_Model::handleStartupCall([
				'direction' => strtolower($data['Direction']),
				'callstatus' => 'ringing',
				'starttime' => date('Y-m-d H:i:s'),
				'sourceuuid' => $callId,
				'gateway' => 'Acme',
				'user' => $agent['id'],
				'customer' => $customer['id'],
				'customernumber' => $customerPhoneNumber,
				'customertype' => $customer['type'],
				'hotline' => '',
				'assigned_user_id' => $agent['id'],
			]);

			return;
		}

		if ($data['Status'] == 'Up') {
			PBXManager_Data_Model::updateCallStatus($callId, ['callstatus' => 'answered']);

			return;
		}

		if ($data['Status'] == 'Down') {
			$params = [
				'callstatus' => 'completed',
				'endtime' => date('Y-m-d H:i:s'),
			];

			if (!empty($data['Data'])) {
				$params['totalduration'] = (int) $data['Data']['TotalTimeCall'];
				$params['billduration'] = (int) $data['Data']['RealTimeCall'];
				$params['recordingurl'] = (string) $data['Data']['LinkFile'];
			}

			PBXManager_Data_Model::handleHangupCall($callId, $params);
		}
	}
}
```

Optional overrides (see `CloudFone.php`, `Stringee.php`): `makeCall($receiverNumber, $parentId)`, `getHistoryReport($headers, $params)`, `getRecordingData($callRecordModel)`; REST calls via `$this->getRestClient()` + `$this->callRestApi()`.

## 2. Webhook entry — `include/Webhooks/AcmeConnector.php`

URL: `webhook.php?name=AcmeConnector&secret_key=<key>`.

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('include/utils/CallCenterUtils.php');

class AcmeConnector extends Vtiger_EntryPoint {

	function process(Vtiger_Request $request) {
		CallCenterUtils::checkConfig();

		$request = CallCenterUtils::getRequest();
		$data = $request->getAllPurified();
		CallCenterUtils::saveLog('[Acme] Webhook data', null, $data);

		// Provider health check / empty signal
		if (empty($data['CallNumber']) || empty($data['ReceiptNumber'])) {
			echo 'Listening!';
			exit;
		}

		if (PBXManager_Acme_Connector::isExists($data['KeyRinging'], $data['Status'])) {
			exit;
		}

		$callId = $data['KeyRinging'];
		$receiverId = '';
		$agentExtNumber = '';

		if ($data['Status'] == 'Ringing') {
			$agentExtNumber = CallCenterUtils::getAgentExtNumber($data['CallNumber'], $data['ReceiptNumber'], $data['Direction']);
			$agent = PBXManager_Data_Model::findAgentByExtNumber($agentExtNumber);

			if (!empty($agent)) {
				$receiverId = $agent['id'];
			}
		}
		else {
			$receiverId = PBXManager_Data_Model::getAgentUserIdFromCall($callId);
		}

		if (!empty($receiverId)) {
			$stateMapping = [
				'Ringing' => 'RINGING',
				'Up' => 'ANSWERED',
				'Down' => 'HANGUP',
			];

			$msg = [
				'call_id' => $callId,
				'receiver_id' => $receiverId,
				'state' => $stateMapping[$data['Status']],
			];

			if ($data['Status'] == 'Ringing') {
				$msg['direction'] = strtoupper($data['Direction']);
				$customerPhoneNumber = CallCenterUtils::getCustomerPhoneNumber($data['CallNumber'], $data['ReceiptNumber'], $data['Direction']);
				$customer = PBXManager_Data_Model::findCustomerByPhoneNumber($customerPhoneNumber, $msg['direction'] == 'OUTBOUND', $agentExtNumber, true);
				CallCenterUtils::fillMsgDataForRingingEvent($msg, $customerPhoneNumber, $customer);

				// Reused by handleCallEvent() in the same request
				$GLOBALS['agent'] = $agent;
				$GLOBALS['customer'] = $customer;

				if (!empty($customer['call_log_id'])) {
					$msg['call_log_id'] = $customer['call_log_id'];
				}
			}

			PBXManager_Acme_Connector::forwardToCallCenterBridge($msg);
			CallCenterUtils::saveDebugLog('[Acme] Data sent to call popup for ' . $agentExtNumber, null, $msg);
		}

		// Skip ringing events that no CRM agent can receive
		if ($data['Status'] != 'Ringing' || !empty($receiverId)) {
			PBXManager_Acme_Connector::handleCallEvent($data);
		}
	}
}
```

Outbound calls also mask the customer number for agents without permission: see the `[PhoneNumberMasked]` block in `CloudFoneConnector.php` (`canCurrentUserSeePhoneNumber`, `maskedPhoneNumber`).

## Key Patterns

- **Signature**: `process(Vtiger_Request $request)` matches abstract `Vtiger_EntryPoint::process` — no return type.
- **Errors**: `webhook.php` already wraps `process()` in `try/catch (\Throwable)` and logs via `WebhookUtils::saveLog`. Catch locally only when you must reply to the provider; `catch` goes on its own line:
  ```php
  try {
  	$response = PBXManager_Acme_Connector::getIvrRouting($data['fromNumber']);
  }
  catch (Throwable $e) {
  	CallCenterUtils::saveLog('[Acme] IVR routing error', null, $data, $e->getMessage());
  	$response = [];
  }
  ```
- **Duplicates**: providers retry on timeout — always guard with `isExists($callId, $status)` (`PBXManager_Data_Model::isExists` checks `sourceuuid` + `callstatus`).
- **Statuses**: map provider events to `ringing` / `answered` / `hangup` / `completed` only (picklist `vtiger_callstatus`). `direction` is free text (uitype 1); existing connectors store `inbound` / `outbound`.
- **Phones**: no custom normalizer — use `CallCenterUtils::getCustomerPhoneNumber()` and `PBXManager_Logic_Helper::cleanupPhoneNumber()` / `addLeadingZeroToPhoneNumber()` / `addVnCountryCodeToPhoneNumber()`.
- **SQL**: go through `PBXManager_Data_Model` (prepared statements keyed by `sourceuuid`); never concatenate payload values.

## Common Pitfalls

- `getGatewayName()` ≠ webhook name minus `Connector` → `checkConfig()` dies with `This provider is not enabled yet!`.
- Forgetting `secret_key` in the provider's webhook URL → rejected by `checkSecretKey()` (only `VCSConnector` is exempt).
- Calling `handleStartupCall()` again on a retried RINGING event → duplicate call logs; guard with `isExists()` first.

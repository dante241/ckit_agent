---
name: integration
description: "VTiger API integration — inbound IntegrationApiHandler, outbound CPAPIIntegration, entry points, webhook connector, CloudBotApi. Use when: tích hợp API ngoài, webhook, kết nối hệ thống thứ 3; keywords: integration, webhook, connector, API."
user-invocable: false
---

# VTiger Integration Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Creating inbound API endpoints for external systems
- Building outbound API integrations to third-party services
- Implementing webhook connectors
- Creating public entry points (no auth required)
- Syncing VTiger records with external platforms

## 4 Integration Patterns

| Pattern | Direction | Auth | Use Case |
|---------|-----------|------|----------|
| **Inbound API** | External → VTiger | JWT/API Key | Receive data from external systems |
| **Outbound API** | VTiger → External | OAuth/API Key | Send data to external platforms |
| **Entry Point** | External → VTiger | None | Public webhooks, callbacks |
| **Webhook Connector** | External → VTiger | None/Custom | Call center, IoT, social media hooks |

## Inbound API Routing

URL: `api/IntegrationAPI/{Platform}/{action}` (rewritten by `api/IntegrationAPI/.htaccess` to `Endpoint.php?platform=…&action=…`)

Example: `api/IntegrationAPI/CloudBot/saveTicket`

**Dispatch (`api/IntegrationAPI/Endpoint.php`):**
1. Loads `include/Webservice/{Platform}ApiHandler.php` (top level, not a subdir) and instantiates `{Platform}ApiHandler`; it must be an `IntegrationApiHandler`.
2. `saveLog()` → `handleDuplicateRequest()` → `isEnabled()` (503 if false) → `auth` action handled by the trait → otherwise `checkSession()` with the `token` header (skipped for actions in `$skipCheckToken`).
3. Lowercases the first letter of the action and calls `$instance->$method($request)` with ONE argument (`Vtiger_Request`); if the handler defines `processActionBySubHandler($method, $request)`, that is called instead.

## Outbound Channel Pattern

Extend `CPAPIIntegration_Channel` for consistent outbound integrations:

```php
require_once('modules/CPAPIIntegration/Channel.php');

class CPAPIIntegration_Facebook_Channel extends CPAPIIntegration_Channel {

	protected $logger = 'Facebook';	// non-empty => requests are logged via CPLogAPI_Logger_Helper

	public function isEnabled(): bool { }
	public function isEnabledButtonSyncToExternalSystems($recordModel): bool { }
	public function getModuleSyncToExternalSystems(): array { }
	protected function getStatusKey(): string { }
	protected function getAccessToken(): string { }

	public function getCampaigns(string $url, array $params): array {
		return $this->makeGetRequest($url, __FUNCTION__, $params);
	}
}
```

## Decision Tree

**Need to receive data from external system?**
- Public webhook → Use Entry Point or Webhook Connector
- Authenticated API → Use Inbound API Handler

**Need to send data to external platform?**
- Use Outbound Channel (extends CPAPIIntegration_Channel)

**Need public callback URL (OAuth, payment gateway)?**
- Use Entry Point (extends Vtiger_EntryPoint)

## References

- [Inbound API](references/inbound-api.md) - IntegrationApiHandler, token auth, CloudBot sub-handlers, SAVE_UNSUPPORTED_FIELDS
- [Outbound API](references/outbound-api.md) - CPAPIIntegration_Channel, HTTP methods, `$logger` logging
- [Entry Point](references/entry-point.md) - Vtiger_EntryPoint, `include/EntryPoints/`, bypass auth
- [Webhook Connector](references/webhook-connector.md) - Call center, social, IoT connectors in `include/Webhooks/`

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn)

- API connector base ĐỌC TRƯỚC: `include/Webservice/CloudBotApi/AbstractCloudBotApi.php`
- Handler mẫu: `include/Webservice/CloudBotApi/CustomerServiceApiHandler.php`
- Router mẫu (sub-handler): `include/Webservice/CloudBotApiHandler.php`
- Channel mẫu: `modules/CPAPIIntegration/channels/TMS.php`

## Verify

```bash
php -l <file>
# Smoke API với payload thật (xem api-connector-rules.md cho auth):
curl -s -X POST 'http://localhost/vtiger/api/IntegrationAPI/<Platform>/<action>' -H 'token: <access_token>' -H 'Content-Type: application/json' --data '<payload>' | head -c 500
# Kỳ vọng: JSON đúng shape, error case trả structured error không phải 500 HTML
```

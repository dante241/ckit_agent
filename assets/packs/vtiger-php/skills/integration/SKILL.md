---
name: integration
description: "VTiger API integration — inbound IntegrationApiHandler, outbound CPAPIIntegration, entry points, webhook connector, CloudbotApi. Use when: tích hợp API ngoài, webhook, kết nối hệ thống thứ 3; keywords: integration, webhook, connector, API."
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

URL: `api/IntegrationAPI/{Platform}/{action}`

Example: `api/IntegrationAPI/Zalo/create_customer`

**Auto-discovery:**
1. Request: `POST /api/IntegrationAPI/Zalo/create_customer`
2. System loads: `include/Webservice/CloudBotApi/ZaloApiHandler.php`
3. Calls: `ZaloApiHandler->create_customer($request, $user)`

## Outbound Channel Pattern

Extend `CPAPIIntegration_Channel` for consistent outbound integrations:

```php
class Facebook_Channel extends CPAPIIntegration_Channel {
    protected function isEnabled(): bool { }
    protected function getAccessToken(Record_Model $record): string { }
    protected function getModuleSyncToExternalSystems(): array { }

    // Auto-logging HTTP methods
    public function getCampaigns(array $params): array {
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

- [Inbound API](references/inbound-api.md) - IntegrationApiHandler, JWT, subhandlers, SAVE_UNSUPPORTED_FIELDS
- [Outbound API](references/outbound-api.md) - CPAPIIntegration_Channel, HTTP methods, auto-logging
- [Entry Point](references/entry-point.md) - Vtiger_EntryPoint, 22 existing entry points, bypass auth
- [Webhook Connector](references/webhook-connector.md) - Call center, social, IoT, 38 connectors

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- API connector base ĐỌC TRƯỚC: `include/Webservice/CloudbotApi/AbstractCloudBotApi.php`
- Handler mẫu: `include/Webservice/CloudbotApi/CustomerServiceApiHandler.php`

## Verify

```bash
php -l <file>
# Smoke API với payload thật (xem api-connector-rules.md cho auth):
curl -s -X POST 'http://localhost/vtiger/<endpoint>' -H 'Content-Type: application/json' --data '<payload>' | head -c 500
# Kỳ vọng: JSON đúng shape, error case trả structured error không phải 500 HTML
```

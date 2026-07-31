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
- Implementing phone number normalization/lookup
- Creating call logs and linking to CRM records
- Supporting call recording URLs
- Building custom call center connectors

## Connector Architecture

Call center connectors extend `Vtiger_EntryPoint` and receive webhooks from external telephony providers.

**Flow:**
```
External Provider → Webhook URL → Connector.process() → Create/Update CRM Record
```

**Location:** `include/Webhooks/{Provider}Connector.php`

**URL Pattern:** `{vtiger_url}/index.php?webhook={provider}&action=process`

## Available Connectors

### Call Center Providers (16)
- Stringee, CloudFone, CloudCALL, OmiCall
- VoIP24H, Tel4VN, SouthTelecom, FPTTelecom
- CMCTelecom, FreePBX, GrandStream, VoiceCloud
- Xorcom, YeaStar, VCS, MiTek

### Social/Chat Providers (5)
- Facebook, Zalo, IndividualZalo, Telegram, Tawk

## Critical Patterns

### 1. Phone Normalization
```php
protected function normalizePhone(string $phone): string {
    $phone = preg_replace('/[^0-9+]/', '', $phone);
    if (substr($phone, 0, 1) === '0') $phone = '84' . substr($phone, 1);
    $phone = str_replace('+84', '84', $phone);
    return $phone;
}
```

### 2. Contact Lookup by Phone
```php
$contactId = CallCenterUtils::findContactByPhone($normalizedPhone);
```

### 3. Call Log Creation
```php
$pbxRecord = Vtiger_Record_Model::getCleanInstance('PBXManager');
$pbxRecord->set('caller', $caller);
$pbxRecord->set('callee', $callee);
$pbxRecord->set('call_id', $callId);
$pbxRecord->save();
```

## References

- [Call Center Overview](references/callcenter-overview.md) - Architecture, 21 connectors, webhook flow
- [Connector Pattern](references/connector-pattern.md) - Full skeleton, validation, response handling
- [CallCenter Utils](references/callcenter-utils.md) - Phone lookup, call log creation, PBXManager integration

## Exemplars (PENDING REVIEW by user)

> ⚠️ Chưa tìm được exemplar thuần Tín Bùi/Tùng Nguyễn cho domain này — file dưới là code tác giả khác, dùng tạm đến khi user chỉ định file chuẩn.

- Connector base ĐỌC TRƯỚC: `modules/PBXManager/BaseConnector.php`
- Connector mẫu trong: `modules/PBXManager/connectors/`
- Callback flow: `modules/PBXManager/callbacks/`

## Verify

```bash
php -l <file>
# Simulate callback từ tổng đài bằng curl với payload mẫu thật (lấy từ logs cũ)
# Check record cuộc gọi tạo đúng + popup event
```

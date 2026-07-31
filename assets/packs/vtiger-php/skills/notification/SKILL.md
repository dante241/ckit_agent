---
name: notification
description: "VTiger notifications — email Mailer, SMS SMSNotifier, Zalo ZNS, FCM push, CPNotifications. Use when: gửi thông báo, email, SMS, Zalo, push notification; keywords: notification, gửi mail, ZNS."
user-invocable: false
---

# VTiger Notification Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

Use this skill when:
- Sending emails (transactional, marketing, alerts)
- Sending SMS messages
- Sending Zalo ZNS (OTT) messages
- Sending push notifications (FCM)
- Building notification workflows
- Creating message templates
- Implementing multi-channel messaging

## Channel Comparison

| Channel | Use Case | Template | Queue | Cost |
|---------|----------|----------|-------|------|
| **Email** | Formal, detailed, attachments | Yes | Yes | Low |
| **SMS** | Urgent, 160 chars, wide reach | Yes | No | Medium |
| **Zalo ZNS** | Vietnam market, rich content | Yes | No | Low |
| **Push** | Real-time, in-app alerts | No | No | Free |

## Email Quick Reference

### Mailer::send() — Template-based

```php
Mailer::send(
    bool $immediately,           // false = queue, true = send now
    array $receivers,            // ['email' => 'contactId']
    int $templateId,             // Email template ID
    array $variables = [],       // ['key' => 'value'] for %key%
    string $cc = '',             // CC emails (comma-separated)
    string $bcc = '',            // BCC emails (comma-separated)
    array $attachments = [],     // File paths
    array $parentIds = [],       // Related record IDs for tracking
    string $sender = ''          // Override sender email
): bool
```

### Mailer::sendEmail() — Custom content

```php
Mailer::sendEmail(
    bool $immediately,
    array $receivers,
    string $subject,
    string $body,                // HTML content
    array $variables = [],
    string $cc = '',
    string $bcc = '',
    array $attachments = [],
    array $parentIds = [],
    string $sender = '',
    string $scheduleSendTime = '' // 'Y-m-d H:i:s' for scheduled
): bool
```

## SMS Quick Reference

```php
SMSNotifier::sendMessage(
    string $channel,             // SMS provider ('Stringee', 'SMSAPI', etc.)
    string $message,             // Message content (160 char limit)
    array $toNumbers,            // ['phone' => 'customerId']
    int $templateId = 0,         // SMS template ID (optional)
    array $variablesMapping = [],// Template variables
    string $sender = '',         // Sender name/number
    int $ownerId = 0,            // Owner user ID
    array $linkToIds = []        // Related record IDs
): bool
```

## Zalo ZNS Quick Reference

### Check availability

```php
if (!CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
    // Fallback to SMS
}
```

### Get active gateway

```php
$gateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');
if (empty($gateway)) {
    // No gateway configured
}
```

### Send ZNS message

```php
$gateway->sendZaloMsg(
    string $msg,                 // Message content
    array $toNumbers,            // ['0901234567' => 'contactId']
    string $templateId,          // ZNS template ID
    array $variablesMapping,     // Template variables
    int $recordId,               // Related record ID
    string $sender = ''          // Sender name
): bool
```

## Push Notification Quick Reference

```php
NotificationHelper::sendNotification(
    array $data,                 // Notification data
    bool $store = true           // false = flash only, true = save to DB
): bool
```

**Data structure:**
```php
$data = [
    'receiver_id' => 123,                    // User ID
    'message' => 'New order received',       // Notification text
    'image' => 'path/to/image.jpg',         // Optional image
    'type' => 'info',                        // info/warning/error/success
    'related_record_id' => 456,             // Related record
    'related_record_name' => 'SO-001',      // Record display name
    'related_module_name' => 'SalesOrder',  // Module name
    'extra_data' => ['key' => 'value'],     // Additional data
];
```

## Critical Pitfalls

### Email
1. **send(false) queues, send(true) sends now** — check server load
2. **%key% format not $key$** — variable replacement
3. **decodeUTF8 template content** — encoding issues
4. **SMTP from vtiger_systems** — don't hardcode config
5. **parentIds for tracking** — link emails to records

### SMS
1. **160 char limit** — split or truncate long messages
2. **Phone normalization** — 0xx → 84xx for Vietnam
3. **Check feature availability** — `SMSNotifier::hasActiveGateway()`
4. **Channel parameter required** — provider name
5. **toNumbers format** — `['phone' => 'customerId']` for logging

### Zalo ZNS
1. **Requires active gateway** — check before sending
2. **Template ID required** — ZNS uses pre-approved templates
3. **Variable mapping strict** — match template placeholders
4. **ZNS→SMS fallback** — implement fallback if ZNS unavailable
5. **Phone format** — 0xx format for Vietnam

### Push Notifications
1. **receiver_id required** — push to specific user
2. **store=false for flash** — transient notifications
3. **Check app installed** — user must have CloudGO app
4. **FCM token must exist** — device registered

## Reference Files

- [Mailer](references/mailer.md) — Email system architecture
- [SMS](references/smser.md) — SMS provider integration
- [Zalo ZNS](references/zns.md) — OTT messaging system
- [Push Notification](references/push-notification.md) — FCM integration

## Quick Example

```php
// Send email with template
Mailer::send(
    false,                                    // Queue
    ['user@example.com' => 123],             // Receivers
    5,                                        // Template ID
    ['customer_name' => 'John Doe'],         // Variables
    '',                                       // CC
    '',                                       // BCC
    ['/path/to/invoice.pdf'],                // Attachments
    [456],                                    // Related to SO-456
    'no-reply@company.com'                   // Sender
);

// Send SMS
SMSNotifier::sendMessage(
    'Stringee',
    'Your order #SO-456 has been shipped',
    ['0901234567' => 123],
    0,
    [],
    'Company',
    1,
    [456]
);

// Send push notification
NotificationHelper::sendNotification([
    'receiver_id' => 123,
    'message' => 'New order received',
    'type' => 'info',
    'related_record_id' => 456,
    'related_module_name' => 'SalesOrder',
], true);
```

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Notification frontend + flow (tin/tung nhiều commit): `modules/CPNotifications/resources/Notifications.js` + `modules/CPNotifications/tpls/Notifications.tpl`
- Auto notification helper (đối chiếu — chưa có bản thuần Tín/Tùng): `modules/HelpDesk/helpers/AutoNotificationUtils.php`

## Verify

```bash
php -l <file>
# Trigger notification thật trên dev, rồi check:
# - Bảng notification/queue tương ứng có row mới
# - logs/ không có error gửi
# KHÔNG gửi thật ra Zalo/SMS/FCM production từ máy dev — dùng config test.
```

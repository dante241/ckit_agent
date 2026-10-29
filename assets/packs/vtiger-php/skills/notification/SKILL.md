---
name: notification
description: "VTiger notifications — email Mailer, SMS SMSNotifier, Zalo ZNS, FCM push, CPNotifications. Use when: gửi thông báo, email, SMS, Zalo, push notification; keywords: notification, gửi mail, ZNS."
user-invocable: false
---

# VTiger Notification Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

Use this skill when:
- Sending emails (transactional, alerts)
- Sending SMS or OTT (Zalo ZNS / Viber / Telegram) messages
- Sending in-app + push notifications (FCM)
- Building notification workflows or message templates

## Channels

| Channel | Entry point (load with) | Queue | Reference |
|---------|-------------------------|-------|-----------|
| **Email** | `Mailer::send()` / `Mailer::sendEmail()` (`require_once('include/Mailer.php')`) | `$immediately = false` → `vtiger_mailer_queue` + CPQueue `ProcessEmailQueue` | [mailer.md](references/mailer.md) |
| **SMS** | `SMSNotifier::sendMessage('SMS', ...)` (`require_once('modules/SMSNotifier/SMSNotifier.php')`) | No (sent in the request) | [smser.md](references/smser.md) |
| **Zalo ZNS / OTT** | `SMSNotifier::sendMessage('Zalo', ...)` or `CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo')->sendZaloMsg(...)` | No | [zns.md](references/zns.md) |
| **In-app + push** | `NotificationHelper::sendNotification($data, $store = true)` (`require_once('include/utils/NotificationHelper.php')`) | Row saved synchronously; FCM push deferred to CPQueue `SendNotification` | [push-notification.md](references/push-notification.md) |

Read the reference file for the exact signature before writing a call — every signature there is copied from source.

## Essentials

### Email
- Receivers / CC / BCC: `[['name' => ..., 'email' => ...]]` — entries without `name` are silently dropped
- Attachments: `[['name' => ..., 'path' => ...]]`; sender: `['email' => ..., 'name' => ...]` (empty → system address)
- Variables replace `%key%`; returns `['success', 'email_log_id']` (or `false` from `send()` when the template is missing)

### SMS / OTT
- `$channel` is `'SMS'` or an OTT channel (`Zalo`/`Viber`/`Telegram`) — never a provider name
- `$toNumbers` = `['0901234567' => $customerId]`; returns the response list or `false`
- Gate: `SMSNotifier_Logic_Helper::canSendSMSMsg()`, `CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()`
- OTT `$variablesMapping` = `['$var$' => 'fieldname']`; `$templateId` = `CPSMSTemplate` id; Zalo `$sender` = OA id

### Push
- Returns nothing; `message` is only the push body (in-app text is rendered from `extra_data['action']`)
- Stored in `vtiger_notifications`; FCM tokens are user preferences `push_client_tokens`

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn)

- Notification frontend + flow (tin/tung nhiều commit): `modules/CPNotifications/resources/Notifications.js` + `modules/CPNotifications/tpls/Notifications.tpl`
- Auto notification helper (đối chiếu — chưa có bản thuần Tín/Tùng): `modules/HelpDesk/helpers/AutoNotificationUtils.php`

## Verify

```bash
php -l <file>
# Trigger notification thật trên dev, rồi check:
# - Email: vtiger_mailer_queue / Emails record (khi có parentIds)
# - SMS/OTT: SMSNotifier record + vtiger_cpsmsottmessagelog
# - Push: vtiger_notifications + CPQueue task SendNotification
# - logs/ không có error gửi
# KHÔNG gửi thật ra Zalo/SMS/FCM production từ máy dev — dùng config test.
```

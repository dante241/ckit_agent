# Mailer Reference

## Two-Layer Architecture

- **Layer 1:** `include/Mailer.php` — static facade `Mailer::send()` / `Mailer::sendEmail()` (not autoloaded: `require_once('include/Mailer.php');`)
- **Layer 2:** `vtlib/Vtiger/Mailer.php` — `Vtiger_Mailer` (PHPMailer wrapper): SMTP config from `vtiger_systems` (`server_type = 'email'`), queueing, `dispatchQueue()`

## Mailer::send() — Template-based Email

Real signature (`include/Mailer.php:21`):

```php
static function send($immediately, $mainReceivers, $templateId, $variables = [], $ccReceivers = [], $bccReceivers = [], $attachments = [], $parentIds = [], $sender = [])
```

| Param | Shape |
|---|---|
| `$immediately` | `true` = send now over SMTP, `false` = queue (`vtiger_mailer_queue` + CPQueue task) |
| `$mainReceivers` / `$ccReceivers` / `$bccReceivers` | list of `['name' => 'Full Name', 'email' => 'a@b.vn']` — a receiver with empty `name` or invalid `email` is **silently skipped** (`Mailer.php:76`) |
| `$templateId` | `EmailTemplates` record id; subject/body are `decodeUTF8`-ed from it |
| `$variables` | `['key' => 'value']` → replaces `%key%` in subject and body |
| `$attachments` | list of `['name' => 'Report.xls', 'path' => '/abs/or/root-relative/path']` |
| `$parentIds` | CRM record ids; when non-empty an `Emails` log record is saved and a tracking pixel is appended |
| `$sender` | `['email' => ..., 'name' => ...]`; empty → `getSystemEmailAddress()` (org name + system from-address) |

Returns `sendEmail()`'s array, or `false` when the template is missing or an exception is thrown.

## Mailer::sendEmail() — Custom Content

Real signature (`include/Mailer.php:58`):

```php
static function sendEmail($immediately, $mainReceivers, $subject, $body, $variables = [], $ccReceivers = [], $bccReceivers = [], $attachments = [], $parentIds = [], $sender = [], $scheduleSendTime = null, $inReplyTo = '', $references = '')
```

- Same receiver/attachment/sender shapes as `send()`.
- `$scheduleSendTime` (`'Y-m-d H:i:s'`) is stored on the queue row; only honoured when `$immediately = false` (the `ProcessEmailQueue` job skips rows whose `scheduled_send_time` is in the future).
- `$inReplyTo` / `$references` set the `In-Reply-To` / `References` headers (threading) and are saved on the email log.
- Returns `['success' => bool|null, 'email_log_id' => int|null]`. `email_log_id` is only set when `$parentIds` is non-empty; `success` is `null` when no SMTP server is configured (`Vtiger_Mailer::Send()` returns early).

## Examples

Template email, sent now (pattern from `forgotPassword.php:65-78`):

```php
require_once('include/Mailer.php');

$mainReceivers = [
	['name' => $contactName, 'email' => $contactEmail],
];
$variables = [
	'customer_name' => $contactName,
	'order_no' => $orderNo,
];
$result = Mailer::send(true, $mainReceivers, $templateId, $variables);

if (empty($result['success'])) {
	saveLog('PLATFORM', '[Order] Send confirmation email failed', ['record_id' => $recordId]);
}
```

Custom content, queued, with CC, attachment, record log and custom sender:

```php
require_once('include/Mailer.php');

$mainReceivers = [['name' => $contactName, 'email' => $contactEmail]];
$ccReceivers = [['name' => $managerName, 'email' => $managerEmail]];
$attachments = [['name' => 'Invoice.pdf', 'path' => $pdfPath]];
$sender = ['email' => 'sales@company.vn', 'name' => 'Sales Team'];

$result = Mailer::sendEmail(false, $mainReceivers, 'Order %order_no% shipped', $body, ['order_no' => $orderNo], $ccReceivers, [], $attachments, [$recordId], $sender);
```

Real callers: `forgotPassword.php:76` (`send`, immediate), `modules/CPLeaving/helpers/Logic.php:94,176` (`send`, queued + CC), `modules/CPAutomaticTaskList/helpers/Logic.php:37-53` (`sendEmail` + document attachments + parentIds), `modules/CPEventTicket/helpers/Logic.php:24` (`sendEmail` with cc/bcc/attachments/parentIds/sender).

## Internal Flow (`sendEmail`)

1. `new Vtiger_Mailer()` (loads SMTP from `vtiger_systems`), `IsHTML(true)`
2. Sender: `$sender` or `getSystemEmailAddress()` → `ConfigSenderInfo($sender['email'], $sender['name'])`
3. Add TO/CC/BCC (skips entries without `name` or with invalid `email`)
4. Replace `%key%` in subject (tags stripped) and body
5. If `$parentIds`: `saveEmailLog()` creates an `Emails` record + `Vtiger_Functions::getTrackImageContent()` pixel
6. Attachments, inline base64 images → CID, threading headers
7. `$mail->Send($immediately, false, $emailLogId)`

## Queue

`Vtiger_Mailer::Send(false, ...)` → `__AddToQueue()` (`vtlib/Vtiger/Mailer.php:239`):

- `vtiger_mailer_queue` — id, fromname, fromemail, mailer, content_type, subject, body, relcrmid, failed, failreason, attempt_count, scheduled_send_time, in_reply_to, reference_ids, email_id
- `vtiger_mailer_queueinfo` — id, name, email, type (`TO`/`CC`/`BCC`/`RPLYTO`)
- `vtiger_mailer_queueattachments` — id, path, name, encoding, type
- Adds a CPQueue task `ProcessEmailQueue` with `$emailQueueConfig['max_attempts']` (`config_override.php`, default 5)

Dispatch: `modules/CPQueue/Jobs/ProcessEmailQueue.php` sends one queued row (driven by `cron/modules/Vtiger/ProcessEmailQueue.service` → `EmailQueue` process). On success the row is removed (`Vtiger_Mailer::removeQueue()`); on failure `failed=1`, `failreason`, `attempt_count+1` until `attempt_count + 1 >= max_attempts`, then the row is removed. There is no backoff schedule — retries happen on the next queue run.

`Vtiger_Mailer::dispatchQueue(Vtiger_Mailer_Listener $listener = null)` (`vtlib/Vtiger/Mailer.php:314`) is the legacy batch sender (`batch_limit` rows, same `max_attempts` rule); still called by Import (`modules/Import/actions/Data.php:1334`).

## Critical Pitfalls

1. **Receiver shape** — `[['name' => ..., 'email' => ...]]`; `['email' => id]` or a missing `name` sends to nobody without error
2. **Sender shape** — array `['email', 'name']`, never a string
3. **Return is an array** — check `$result['success']`, not `$result === true`
4. **`%key%` format** — not `$key$` or `{key}`
5. **Queued mail needs the CPQueue worker / cron** running
6. **Attachments** — each item needs `path` (readable from the web root) and `name`
7. **SMTP comes from `vtiger_systems`** — never hardcode SMTP config

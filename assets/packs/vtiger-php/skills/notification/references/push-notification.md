# Push Notification Reference

## NotificationHelper::sendNotification()

Real signature (`include/utils/NotificationHelper.php:12`); not autoloaded — `require_once('include/utils/NotificationHelper.php');` first:

```php
static function sendNotification($data, $store = true)
```

Returns nothing. It silently returns early when the receiver user does not exist or the user muted `related_module_name` (`CPNotifications_Data_Model::isModuleMutedForUser()`).

## $data

| Key | Use |
|---|---|
| `receiver_id` | user id (required) |
| `message` | push body only — **not stored** |
| `type` | push payload type, default `'notification'` |
| `category` | stored category; default `'update'`, or `'checkin'` when `extra_data['action'] == 'employee_checkin'` |
| `image` | stored + pushed |
| `related_record_id`, `related_record_name`, `related_module_name` | stored + pushed |
| `extra_data` | array, stored as JSON; `extra_data['action']` drives the text rendered by `translateNotificationMessage()` |

## Internal Flow

1. User exists + module not muted, else return
2. `$store = true` → `CPNotifications_Data_Model::saveNotification($data)` inserts into `vtiger_notifications` (id, receiver_id, category, image, related_record_id, related_record_name, related_module_name, created_time, read, extra_data, pin) and returns the id
3. Build push payload: title `'New notification!'` (or `LBL_NOTIFICATION_TITLE_CLOUGO` for modules in `$cloudGoAppConfig[...]['supported_modules']`), `id` when stored; `$store = false` sets `extra_data['is_flash_msg'] = true`
4. FCM push is **deferred**: `CPQueue_Server_Model::getInstance()->addTask('SendNotification', ['type' => 'deliver_fcm_push', ...])` → `modules/CPQueue/Jobs/SendNotification.php` → `FCMHelper::sendNotification($title, $message, $userId, $data, $forPortalUser)`

The in-app list text is NOT the `message` you passed: `CPNotifications_Data_Model::populateRowData()` re-renders it with `translateNotificationMessage($row)` from the stored row.

## $store

- `true` (default): row in `vtiger_notifications` (bell list, read/unread, pin) + push
- `false`: push only, flagged `is_flash_msg`; no history

## FCM Tokens

- Stored as user preferences (`Users_Preferences_Model`): `push_client_tokens` and per app `<app>push_client_tokens` (e.g. `cloudgopush_client_tokens`)
- `CPNotifications_Data_Model::getClientTokens($userId, $app = '')`, `saveClientToken($token, $app = '')` (current user), `removeClientToken($token, $app = '')`
- Registered from `CPNotifications` `HandleAjax` action and the mobile APIs (`include/Webservice/RestfulApiHandler.php:288`, `include/utils/CloudGoApiUtils.php:762`); expired tokens dropped by `FCMHelper::removeExpiredTokens()`
- No token → the in-app row is still saved; only the push is skipped

## Related API

- `CPNotifications_Data_Model::loadNotifications($type, $subType, $offset, $includeModules = [], $keyword = '')`
- `CPNotifications_Data_Model::markAsRead($target, $status = null, $includeModules = [])`
- `CPNotifications_Data_Model::getNotificationCount($userId)`
- `NotificationHelper::computeRelatedRecordName($entityData, string $moduleName, CRMEntity $focus): string`
- `FCMHelper::sendNotification($title, $message, $userId, $data = null, $forPortalUser = false)` — low-level; prefer `NotificationHelper`

## Example

Pattern from `modules/CPThankYouCard/handlers/CPThankYouCardHandler.php:57-83`:

```php
require_once('include/utils/NotificationHelper.php');

$userLanguage = getUserData('language', $receiverId);
$userTimezone = getUserData('time_zone', $receiverId);

$data = [
	'receiver_id' => $receiverId,
	'type' => 'notification',
	'related_record_id' => $recordId,
	'related_record_name' => $recordName,
	'related_module_name' => 'CPThankYouCard',
	'extra_data' => [
		'action' => 'new_thank_you_card',
		'creator' => $creatorId,
	],
];

$data['message'] = translateNotificationMessage($data, $userLanguage, $userTimezone);
NotificationHelper::sendNotification($data);
```

Other real callers: `modules/CPOTRegister/handlers/CPOTRegisterHandler.php:110`, `include/utils/CallCenterUtils.php`, `include/EntryPoints/PushNotificationTest.php`. Bulk sends go through CPQueue `SendNotification` task types (`send_mass_notification`, `send_update_notification_to_group`, …).

## Critical Pitfalls

1. **No return value** — do not branch on the result
2. **`message` is push-only** — in-app text comes from `extra_data['action']` via `translateNotificationMessage()`; add a translation case for new actions
3. **Push is async** — needs the CPQueue worker for `SendNotification`
4. **Muted modules** — users can mute `related_module_name`; nothing is stored or sent
5. **`extra_data` must be an array** — it is `json_encode`d
6. **Push opt-out / no project** — `FCMHelper` skips users with `notification_config->receive_notifications == 0`, no tokens, or empty `$googleConfig['firebase']['project_id']`

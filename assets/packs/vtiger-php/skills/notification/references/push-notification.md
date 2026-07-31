# Push Notification Reference

## NotificationHelper::sendNotification()

```php
NotificationHelper::sendNotification(
    array $data,                    // Notification data
    bool $store = true              // false = flash only, true = save to DB
): bool
```

## Data Structure

**Required fields:**
```php
$data = [
    'receiver_id' => 123,                       // User ID (required)
    'message' => 'New order received',          // Notification text (required)
    'image' => 'path/to/image.jpg',            // Optional image
    'type' => 'info',                           // info/warning/error/success
    'related_record_id' => 456,                // Related record ID
    'related_record_name' => 'SO-001',         // Record display name
    'related_module_name' => 'SalesOrder',     // Module name
    'extra_data' => ['key' => 'value'],        // Additional JSON data
];
```

## Internal Flow

1. **Validate user** — check receiver_id exists
2. **Save notification** to `vtiger_cpnotifications` (if $store=true)
3. **Determine app** — check user's CloudGO app installation
4. **Send FCM** — push to Firebase Cloud Messaging
5. **Return status** — true if sent, false if failed

## CloudGO App Module Routing

**Notification opens specific module in app:**

| Module | App Route |
|--------|-----------|
| `SalesOrder` | `/salesorder/detail/456` |
| `Invoice` | `/invoice/detail/456` |
| `Contacts` | `/contacts/detail/456` |
| `Accounts` | `/accounts/detail/456` |
| `Leads` | `/leads/detail/456` |
| `HelpDesk` | `/helpdesk/detail/456` |

**Routing handled automatically based on `related_module_name` and `related_record_id`.**

## Key Classes

### NotificationHelper

**Location:** `include/utils/NotificationHelper.php`

**Methods:**
- `sendNotification(array $data, bool $store = true): bool`
- `getNotifications(int $userId, int $limit = 20): array`
- `markAsRead(int $notificationId): bool`
- `deleteNotification(int $notificationId): bool`

### FCMHelper

**Location:** `include/utils/FCMHelper.php`

**Methods:**
- `sendPush(string $token, array $data): bool`
- `sendPushToUser(int $userId, array $data): bool`
- `sendPushToMultipleUsers(array $userIds, array $data): bool`

### CPNotifications_Data_Model

**Location:** `modules/CPNotifications/models/Data.php`

**Methods:**
- `createNotification(array $data): int`
- `getUserNotifications(int $userId, int $limit = 20): array`
- `markAsRead(int $notificationId): bool`
- `deleteNotification(int $notificationId): bool`

## $store Parameter

### $store = true (default)

**Persistent notification:**
- Saved to `vtiger_cpnotifications` table
- User can view in notification center
- Shows in notification history
- Can be marked as read/unread

### $store = false

**Flash notification:**
- Sent via FCM immediately
- NOT saved to database
- User sees once
- No history/tracking
- Use for real-time alerts (chat, live updates)

## Real Codebase Examples

### Example 1: Order Assignment

```php
// modules/SalesOrder/actions/Assign.php
public function notifyAssignee(int $recordId, int $newOwnerId): void {
    $record = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');

    NotificationHelper::sendNotification([
        'receiver_id' => $newOwnerId,
        'message' => 'New order assigned to you: ' . $record->get('subject'),
        'type' => 'info',
        'related_record_id' => $recordId,
        'related_record_name' => $record->get('subject'),
        'related_module_name' => 'SalesOrder',
        'extra_data' => [
            'grand_total' => $record->get('hdnGrandTotal'),
            'customer_name' => $record->getDisplayValue('account_id'),
        ],
    ], true);  // Save to DB
}
```

### Example 2: Payment Received

```php
// modules/Invoice/actions/ReceivePayment.php
public function notifyPaymentReceived(int $invoiceId, float $amount): void {
    $invoice = Vtiger_Record_Model::getInstanceById($invoiceId, 'Invoice');
    $salesPersonId = (int) $invoice->get('assigned_user_id');

    NotificationHelper::sendNotification([
        'receiver_id' => $salesPersonId,
        'message' => "Payment received: $amount for " . $invoice->get('subject'),
        'type' => 'success',
        'related_record_id' => $invoiceId,
        'related_record_name' => $invoice->get('subject'),
        'related_module_name' => 'Invoice',
        'extra_data' => [
            'payment_amount' => $amount,
            'remaining_balance' => $invoice->get('balance'),
        ],
    ], true);
}
```

### Example 3: Real-time Chat Message (Flash)

```php
// modules/CPChat/actions/SendMessage.php
public function notifyNewMessage(int $recipientId, string $message, string $senderName): void {
    NotificationHelper::sendNotification([
        'receiver_id' => $recipientId,
        'message' => $senderName . ': ' . substr($message, 0, 50),
        'type' => 'info',
        'extra_data' => [
            'chat_id' => $this->chatId,
            'message_id' => $this->messageId,
        ],
    ], false);  // Flash only, don't store
}
```

### Example 4: Multi-user Notification

```php
// modules/SalesOrder/actions/Approve.php
public function notifyTeam(int $recordId, array $teamMemberIds): void {
    $record = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');

    foreach ($teamMemberIds as $userId) {
        NotificationHelper::sendNotification([
            'receiver_id' => (int) $userId,
            'message' => 'Order approved: ' . $record->get('subject'),
            'type' => 'success',
            'related_record_id' => $recordId,
            'related_record_name' => $record->get('subject'),
            'related_module_name' => 'SalesOrder',
        ], true);
    }
}
```

## Notification Types

| Type | Badge Color | Use Case |
|------|-------------|----------|
| `info` | Blue | General updates, assignments |
| `success` | Green | Completed actions, approvals |
| `warning` | Yellow | Alerts, pending actions |
| `error` | Red | Errors, rejections, critical alerts |

## FCM Token Management

**User must have FCM token registered:**

```php
// Check if user has FCM token
$sql = "SELECT fcm_token FROM vtiger_users WHERE id = ?";
global $adb;
$result = $adb->pquery($sql, [$userId]);
$token = $adb->query_result($result, 0, 'fcm_token');

if (empty($token)) {
    // User hasn't installed CloudGO app or not logged in
    return false;
}
```

**Token registration:**
- User installs CloudGO mobile app
- App requests FCM token from Firebase
- Token saved to `vtiger_users.fcm_token`
- Token refreshed periodically

## Database Schema

**vtiger_cpnotifications table:**
```sql
CREATE TABLE vtiger_cpnotifications (
    cpnotificationsid INT PRIMARY KEY,
    receiver_id INT NOT NULL,
    message TEXT NOT NULL,
    image VARCHAR(255),
    type VARCHAR(50),
    related_record_id INT,
    related_record_name VARCHAR(255),
    related_module_name VARCHAR(100),
    extra_data TEXT,
    is_read TINYINT DEFAULT 0,
    created_at DATETIME
);
```

## Critical Pitfalls

1. **receiver_id required** — push to specific user
2. **store=false for flash** — transient notifications
3. **Check app installed** — user must have CloudGO app
4. **FCM token must exist** — device registered
5. **Type affects display** — use appropriate type
6. **Message length** — keep under 100 chars for mobile display
7. **Image path** — full URL for remote access
8. **extra_data JSON** — must be valid JSON array

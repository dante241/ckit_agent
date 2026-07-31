# SMS Reference

## SMSNotifier::sendMessage()

```php
SMSNotifier::sendMessage(
    string $channel,                // SMS provider name ('Stringee', 'SMSAPI', 'Twilio')
    string $message,                // Message content (160 char limit)
    array $toNumbers,               // ['0901234567' => 'contactId']
    int $templateId = 0,            // SMS template ID (optional)
    array $variablesMapping = [],   // Template variables ['name' => 'John']
    string $sender = '',            // Sender name/number (brand name or shortcode)
    int $ownerId = 0,               // Owner user ID for tracking
    array $linkToIds = []           // Related record IDs [123, 456]
): bool
```

## $toNumbers Format

**Critical:** Array format `['phone' => 'customerId']`

**Examples:**
```php
// Single recipient
['0901234567' => 123]

// Multiple recipients
[
    '0901234567' => 123,
    '0912345678' => 456,
    '84987654321' => 789,
]

// Phone normalization
'0901234567'   → '84901234567'  // Vietnam
'+84901234567' → '84901234567'  // Strip +
```

## Internal Flow

1. **getActiveProviderInstance($channel)** — Load provider plugin
2. **Save log entry** to `vtiger_cpsmsmessagelog`
3. **fireSendMessage** event (pre-send hooks)
4. **provider->send($message, $toNumbers)** — Call provider API
5. **Update log with result** (success/failed)
6. **Create CPSMSOTTMessageLog record** if needed

## Provider Architecture

**Interface:** `include/SMSNotifier/ISMSProvider.php`

```php
interface ISMSProvider {
    public function send(string $message, array $toNumbers): bool;
    public function getProviderName(): string;
    public function isConfigured(): bool;
}
```

**Provider discovery:** `include/SMSNotifier/ext/`

**Built-in providers:**
- `ext/Stringee.php` — Stringee SMS gateway
- `ext/SMSAPI.php` — SMS API gateway
- `ext/Twilio.php` — Twilio SMS

## Check Availability

### isForbiddenFeature

```php
if (isForbiddenFeature('SMS')) {
    // SMS feature disabled in license
    return false;
}
```

### hasActiveGateway

```php
if (!SMSNotifier::hasActiveGateway()) {
    // No SMS provider configured
    return false;
}
```

### checkServer

```php
// Check specific provider configured
$provider = SMSNotifier::getActiveProviderInstance('Stringee');
if (!$provider->isConfigured()) {
    // Provider not configured
    return false;
}
```

## Real Codebase Examples

### Example 1: Order Alert

```php
// modules/SalesOrder/actions/SendSMS.php
public function sendOrderAlert(int $recordId): bool {
    $record = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');
    $contactId = (int) $record->get('contact_id');

    if (empty($contactId)) return false;

    $contact = Vtiger_Record_Model::getInstanceById($contactId, 'Contacts');
    $phone = (string) $contact->get('mobile');

    if (empty($phone)) return false;

    $message = "Your order {$record->get('subject')} has been confirmed. Total: {$record->get('hdnGrandTotal')}";

    return SMSNotifier::sendMessage(
        'Stringee',
        $message,
        [$phone => $contactId],
        0,
        [],
        'Company',
        Users_Record_Model::getCurrentUserModel()->getId(),
        [$recordId]
    );
}
```

### Example 2: OTP Verification

```php
// modules/Users/actions/SendOTP.php
public function sendOTP(int $userId): bool {
    $user = Users_Record_Model::getInstanceById($userId);
    $phone = (string) $user->get('phone_mobile');

    if (empty($phone)) return false;

    $otp = $this->generateOTP(6);
    $this->saveOTP($userId, $otp);

    $message = "Your verification code is: {$otp}. Valid for 5 minutes.";

    return SMSNotifier::sendMessage(
        'Stringee',
        $message,
        [$phone => $userId],
        0,
        [],
        'Company',
        $userId,
        []
    );
}

protected function generateOTP(int $length): string {
    return str_pad((string) mt_rand(0, pow(10, $length) - 1), $length, '0', STR_PAD_LEFT);
}
```

### Example 3: Template-based SMS

```php
// With template
$templateId = 5;  // "Order confirmed" template
$variables = [
    'customer_name' => 'John Doe',
    'order_number' => 'SO-001',
];

SMSNotifier::sendMessage(
    'Stringee',
    '',  // Empty, will load from template
    ['0901234567' => 123],
    $templateId,
    $variables,
    'Company',
    1,
    [456]
);
```

## Phone Normalization

**Vietnam format:**
```php
// Normalize to 84xxx format
function normalizePhone(string $phone): string {
    $phone = preg_replace('/[^0-9]/', '', $phone);

    if (substr($phone, 0, 2) === '84') {
        return $phone;
    }

    if (substr($phone, 0, 1) === '0') {
        return '84' . substr($phone, 1);
    }

    return '84' . $phone;
}
```

## SMS Template Variables

**Format:** Same as email `%variable_name%`

**Example template:**
```
Hello %customer_name%,
Your order %order_number% has been confirmed.
Total: %total_amount%
Thank you for your business!
```

**Send with variables:**
```php
SMSNotifier::sendMessage(
    'Stringee',
    '',
    [$phone => $contactId],
    $templateId,
    [
        'customer_name' => 'John Doe',
        'order_number' => 'SO-001',
        'total_amount' => '$1,250.00',
    ],
    'Company',
    1,
    []
);
```

## Critical Pitfalls

1. **160 char limit** — split or truncate long messages
2. **Phone normalization** — 0xx → 84xx for Vietnam
3. **Check feature availability** — `isForbiddenFeature('SMS')`
4. **Channel parameter required** — provider name
5. **toNumbers format** — `['phone' => 'customerId']` for logging
6. **Empty message with template** — pass '' when using templateId
7. **Provider configured** — check `isConfigured()` before sending
8. **Cost tracking** — SMS costs money, log all sends

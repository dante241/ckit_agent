# Zalo ZNS Reference

## Zalo ZNS (Zalo Notification Service)

**Use case:** Vietnam market, rich content notifications via Zalo OTT platform

## Check Availability

### canSendZaloZNSMsg()

```php
if (!CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
    // Fallback to SMS
    SMSNotifier::sendMessage(...);
    return;
}
```

**Checks:**
1. Zalo OTT feature enabled in license
2. Active Zalo gateway configured
3. Server configuration valid

## Get Active Gateway

```php
$gateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');

if (empty($gateway)) {
    // No gateway configured, fallback to SMS
    return false;
}
```

**Gateway model methods:**
- `getActiveGateway(string $channel)` — Get active gateway for channel
- `sendZaloMsg(...)` — Send ZNS message
- `getTemplates()` — Get available ZNS templates
- `isConfigured()` — Check if gateway configured

## Send ZNS Message

```php
$gateway->sendZaloMsg(
    string $msg,                    // Message content (fallback if template fails)
    array $toNumbers,               // ['0901234567' => 'contactId']
    string $templateId,             // ZNS template ID from Zalo
    array $variablesMapping,        // Template variables
    int $recordId,                  // Related record ID
    string $sender = ''             // Sender name (optional)
): bool
```

**Example:**
```php
$gateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');

$result = $gateway->sendZaloMsg(
    'Fallback SMS message',                         // Fallback
    ['0901234567' => 123],                         // Recipients
    '123456',                                       // ZNS template ID
    [
        'customer_name' => 'John Doe',
        'order_number' => 'SO-001',
        'order_date' => '2026-02-10',
        'total_amount' => '1,250,000đ',
    ],
    456,                                            // Related to SO-456
    'Company Store'                                 // Sender
);
```

## Template Variables

### fillDataForVariables()

```php
$template = CPOTTIntegration_Template_Model::getInstanceById($templateId);
$content = $template->getTemplateContent();

// Fill variables
$filledContent = $template->fillDataForVariables($content, [
    'customer_name' => 'John Doe',
    'order_number' => 'SO-001',
]);
```

### getTemplateContent()

```php
public function getTemplateContent(): string {
    return (string) $this->get('template_content');
}
```

**Template format (Zalo approved):**
```
Xin chào {{customer_name}},

Đơn hàng {{order_number}} của bạn đã được xác nhận.
Ngày đặt: {{order_date}}
Tổng tiền: {{total_amount}}

Cảm ơn bạn đã mua hàng!
```

## ZNS → SMS Fallback Pattern

```php
public function sendOrderNotification(int $recordId): bool {
    $record = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');
    $contactId = (int) $record->get('contact_id');

    if (empty($contactId)) return false;

    $contact = Vtiger_Record_Model::getInstanceById($contactId, 'Contacts');
    $phone = (string) $contact->get('mobile');

    if (empty($phone)) return false;

    $variables = [
        'customer_name' => $contact->getName(),
        'order_number' => $record->get('subject'),
        'order_date' => date('d/m/Y'),
        'total_amount' => number_format($record->get('hdnGrandTotal')) . 'đ',
    ];

    // Try ZNS first
    if (CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
        $gateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');

        if (!empty($gateway)) {
            $result = $gateway->sendZaloMsg(
                '',  // Empty, will use fallback SMS if ZNS fails
                [$phone => $contactId],
                '123456',  // ZNS template ID
                $variables,
                $recordId,
                'Company'
            );

            if ($result) return true;
        }
    }

    // Fallback to SMS
    $message = "Xin chào {$variables['customer_name']}, đơn hàng {$variables['order_number']} đã xác nhận. Tổng: {$variables['total_amount']}";

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

## OTT Channel Routing

**CPOTTIntegration channels:**
- `Zalo` — Zalo ZNS
- `Viber` — Viber messaging
- `WhatsApp` — WhatsApp Business
- `Telegram` — Telegram Bot API

**Channel selection logic:**
```php
public function selectBestChannel(string $phone): string {
    // Check user preference
    $preference = $this->getUserChannelPreference($phone);
    if (!empty($preference)) return $preference;

    // Default priority: Zalo > SMS
    if (CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
        return 'Zalo';
    }

    return 'SMS';
}
```

## Workflow Task Pattern

**VTZaloOTTMessageTask extends VTTask:**

```php
class VTZaloOTTMessageTask extends VTTask {

    public function doTask(Vtiger_Record_Model $record): void {
        $phoneField = $this->phoneField;
        $phone = (string) $record->get($phoneField);

        if (empty($phone)) return;

        $gateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');
        if (empty($gateway)) return;

        $variables = $this->getVariables($record);

        $gateway->sendZaloMsg(
            $this->fallbackMessage,
            [$phone => $record->getId()],
            $this->templateId,
            $variables,
            $record->getId(),
            $this->sender
        );
    }

    protected function getVariables(Vtiger_Record_Model $record): array {
        $vars = [];

        foreach ($this->variableMapping as $templateVar => $fieldName) {
            $vars[$templateVar] = (string) $record->get($fieldName);
        }

        return $vars;
    }
}
```

## Real Codebase Example

### Order Confirmation with ZNS

```php
// modules/SalesOrder/actions/Confirm.php
public function sendZNSConfirmation(int $recordId): bool {
    // Check ZNS available
    if (!CPOTTIntegration_Logic_Helper::canSendZaloZNSMsg()) {
        return $this->sendSMSConfirmation($recordId);
    }

    $record = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');
    $contactId = (int) $record->get('contact_id');

    if (empty($contactId)) return false;

    $contact = Vtiger_Record_Model::getInstanceById($contactId, 'Contacts');
    $phone = (string) $contact->get('mobile');

    if (empty($phone)) return false;

    // Phone format for Vietnam: 0xxx
    $phone = $this->normalizePhone($phone);

    $gateway = CPOTTIntegration_Gateway_Model::getActiveGateway('Zalo');
    if (empty($gateway)) {
        return $this->sendSMSConfirmation($recordId);
    }

    // Prepare variables
    $variables = [
        'customer_name' => $contact->getName(),
        'order_number' => $record->get('subject'),
        'order_date' => date('d/m/Y', strtotime($record->get('createdtime'))),
        'total_amount' => number_format($record->get('hdnGrandTotal'), 0, ',', '.') . 'đ',
        'delivery_date' => date('d/m/Y', strtotime($record->get('delivery_date'))),
    ];

    // Send ZNS
    $result = $gateway->sendZaloMsg(
        '',  // Fallback handled internally
        [$phone => $contactId],
        '234567',  // ZNS template "Order Confirmation"
        $variables,
        $recordId,
        'Company Store'
    );

    // Fallback to SMS if ZNS fails
    if (!$result) {
        return $this->sendSMSConfirmation($recordId);
    }

    return true;
}

protected function normalizePhone(string $phone): string {
    $phone = preg_replace('/[^0-9]/', '', $phone);

    // Convert 84xxx to 0xxx for Zalo
    if (substr($phone, 0, 2) === '84') {
        return '0' . substr($phone, 2);
    }

    return $phone;
}
```

## Critical Pitfalls

1. **Requires active gateway** — check before sending
2. **Template ID required** — ZNS uses pre-approved templates from Zalo
3. **Variable mapping strict** — must match template placeholders exactly
4. **ZNS→SMS fallback** — always implement fallback
5. **Phone format** — 0xx format for Vietnam (different from SMS 84xx)
6. **Template approval** — templates must be approved by Zalo before use
7. **Rich content** — ZNS supports buttons, images (unlike SMS)
8. **Cost tracking** — ZNS cheaper than SMS but still costs money

# Mailer Reference

## Two-Layer Architecture

**Layer 1:** `include/Mailer.php` — Static facade with clean API
**Layer 2:** `include/utils/Vtiger_Mailer.php` — PHPMailer wrapper with queue

## Mailer::send() — Template-based Email

```php
Mailer::send(
    bool $immediately = false,      // false = queue, true = send now
    array $receivers = [],          // ['email' => 'contactId']
    int $templateId = 0,            // Email template ID from vtiger_emailtemplates
    array $variables = [],          // ['customer_name' => 'John'] for %customer_name%
    string $cc = '',                // 'cc1@ex.com,cc2@ex.com'
    string $bcc = '',               // 'bcc1@ex.com,bcc2@ex.com'
    array $attachments = [],        // ['/full/path/file.pdf']
    array $parentIds = [],          // [123, 456] related record IDs
    string $sender = ''             // Override sender (from template otherwise)
): bool
```

**Example:**
```php
Mailer::send(
    false,                                          // Queue for later
    ['customer@example.com' => 123],               // Send to contact 123
    5,                                              // Template ID 5
    ['customer_name' => 'John', 'order_id' => 'SO-001'], // Variables
    'manager@company.com',                          // CC
    '',                                             // No BCC
    ['/var/www/invoices/INV-001.pdf'],             // Attachment
    [456],                                          // Related to record 456
    'sales@company.com'                             // Custom sender
);
```

## Mailer::sendEmail() — Custom Content

```php
Mailer::sendEmail(
    bool $immediately = false,
    array $receivers = [],
    string $subject = '',           // Email subject
    string $body = '',              // HTML body
    array $variables = [],          // Variable replacement in subject/body
    string $cc = '',
    string $bcc = '',
    array $attachments = [],
    array $parentIds = [],
    string $sender = '',
    string $scheduleSendTime = ''   // 'Y-m-d H:i:s' for scheduled send
): bool
```

**Example:**
```php
Mailer::sendEmail(
    true,                                           // Send immediately
    ['customer@example.com' => 123],
    'Your order has been shipped',                  // Subject
    '<p>Hello %customer_name%,</p><p>Order %order_id% shipped.</p>', // HTML body
    ['customer_name' => 'John', 'order_id' => 'SO-001'],
    '',
    '',
    [],
    [456],
    'no-reply@company.com',
    '2026-02-10 14:30:00'                          // Schedule for later
);
```

## Internal Flow

1. **Create Vtiger_Mailer instance**
2. **Load SMTP configuration** from `vtiger_systems` table
3. **Set recipients** (TO, CC, BCC)
4. **Variable replacement** — `%key%` → value in subject/body
5. **decodeUTF8** — handle Vietnamese characters
6. **Tracking pixel** — inject if tracking enabled
7. **Queue or send immediately**
8. **Log to vtiger_mailer_queue**

## Queue System

**Tables:**
- `vtiger_mailer_queue` — pending emails
- `vtiger_mailer_queue_log` — sent/failed log

**Queue processing:**
```php
// Cron job calls
Mailer::dispatchQueue(int $limit = 100): void
```

**Retry logic:**
- Max 3 attempts
- Exponential backoff (5min, 15min, 1hr)
- Failed after 3 attempts → mark as failed

## Template Variables

**Format:** `%variable_name%`

**Common variables:**
- `%customer_name%` — Customer name
- `%order_id%` — Order reference
- `%invoice_number%` — Invoice number
- `%company_name%` — Company name
- `%user_name%` — User name
- `%current_date%` — Current date

**In template body:**
```html
<p>Dear %customer_name%,</p>
<p>Your order %order_id% has been confirmed.</p>
<p>Total: %total_amount%</p>
```

**Pass variables:**
```php
Mailer::send(false, $receivers, $templateId, [
    'customer_name' => 'John Doe',
    'order_id' => 'SO-001',
    'total_amount' => '$1,250.00',
]);
```

## Real Codebase Examples

### Example 1: Order Confirmation

```php
// modules/SalesOrder/actions/Confirm.php
public function sendConfirmationEmail(int $recordId): bool {
    $record = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');
    $contactId = (int) $record->get('contact_id');

    if (empty($contactId)) return false;

    $contact = Vtiger_Record_Model::getInstanceById($contactId, 'Contacts');
    $email = (string) $contact->get('email');

    if (empty($email)) return false;

    return Mailer::send(
        false,
        [$email => $contactId],
        7,  // Order confirmation template
        [
            'customer_name' => $contact->get('firstname') . ' ' . $contact->get('lastname'),
            'order_number' => $record->get('subject'),
            'order_date' => $record->get('createdtime'),
            'total_amount' => $record->get('hdnGrandTotal'),
        ],
        '',
        '',
        [],
        [$recordId],
        ''
    );
}
```

### Example 2: Workflow Task

```php
// modules/Workflow/tasks/VTEmailTask.php
public function doTask(Vtiger_Record_Model $record): void {
    $emailField = $this->emailfield;
    $email = (string) $record->get($emailField);

    if (empty($email)) return;

    Mailer::send(
        false,
        [$email => $record->getId()],
        (int) $this->template,
        $this->getVariables($record),
        '',
        '',
        $this->getAttachments($record),
        [$record->getId()],
        ''
    );
}

protected function getVariables(Vtiger_Record_Model $record): array {
    $vars = [];
    foreach ($record->getData() as $field => $value) {
        $vars[$field] = (string) $value;
    }
    return $vars;
}
```

## Email Log and Tracking

**vtiger_mailer_queue table:**
- `id` — queue entry ID
- `receivers` — JSON array of receivers
- `subject` — email subject
- `body` — HTML body
- `status` — pending/sent/failed
- `created_at` — queue time
- `sent_at` — sent time
- `attempts` — retry count

**Tracking pixel:**
```html
<!-- Injected automatically if tracking enabled -->
<img src="https://crm.example.com/track.php?id=123" width="1" height="1">
```

## Critical Pitfalls

1. **send(false) queues / send(true) immediate** — check server load
2. **%key% format** — not $key$ or {key}
3. **decodeUTF8 template** — Vietnamese/UTF-8 encoding
4. **SMTP from vtiger_systems** — never hardcode SMTP config
5. **parentIds for tracking** — link emails to records
6. **Attachments full path** — relative paths fail
7. **Queue processing** — ensure cron job running
8. **receivers format** — `['email' => customerId]` for logging

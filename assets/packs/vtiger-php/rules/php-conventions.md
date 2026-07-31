---
paths:
  - "**/*.php"
---

# PHP Conventions

> Loads only when editing PHP files. Pairs with `cloudgo-development-rules.md` (always loaded).

## File Header (Required)

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */
```

## Brace Style (Required — codebase is K&R, NOT PSR-12)

> **Subagent trap:** Fresh LLM output defaults to PSR-12 (Allman — `{` on its own line for class/method). That is **WRONG** for this codebase. Match the K&R files next to you.

| Construct | Brace position | Example |
|-----------|----------------|---------|
| **Class** | K&R — same line | `class Foo_Bar_Model extends Vtiger_Base_Model {` |
| **Method / function** | K&R — same line | `public function getName(): string {` |
| **Control (if/for/foreach/while/switch)** | K&R — same line | `if ($id > 0) {` |
| **`else`/`elseif`/`catch`/`finally`** | Stroustrup — OWN line after `}` | see below |

```php
// CORRECT (this codebase)
class V1_RecordController extends V1_BaseHandler {

    public function list(array $params): array {
        if (empty($params)) {
            return [];
        }
        try {
            return $this->run($params);
        }
        catch (Throwable $e) {        // catch on its OWN line (Stroustrup)
            return [];
        }
    }
}

// WRONG (PSR-12 Allman — do NOT use)
class V1_RecordController extends V1_BaseHandler
{
    public function list(array $params): array
    {
        ...
    }
}
```

Multi-line method signatures: put `{` on the line with the closing `) : type`:
```php
private function serializeRow(
    Vtiger_Record_Model $record,
    array $fields
): array {
```

Verified against 18/22 V1 classes (`Response.php`, `Router.php`, `BaseHandler.php`, `AuthController.php`, `MeController.php`, …). One file header per file — never duplicate the `@author` block (fold tags into the existing class docblock).

## Class Naming: `<Module>_<Component>_<Type>`

VTiger autoloads classes — class name maps to file path:
`<Module>_<Component>_<Type>` → `modules/<Module>/<types>/<Component>.php`

```php
// Models → modules/<Module>/models/<Component>.php
class Accounts_Record_Model extends Vtiger_Record_Model { }
class Accounts_Module_Model extends Vtiger_Module_Model { }

// Views → modules/<Module>/views/<Component>.php
class Accounts_Detail_View extends Vtiger_Detail_View { }
class Products_CheckWarranty_View extends Vtiger_Index_View { }

// Actions → modules/<Module>/actions/<Component>.php
class Accounts_Save_Action extends Vtiger_Save_Action { }
class Products_CheckWarrantyAjax_Action extends Vtiger_Action_Controller { }

// Helpers → modules/<Module>/helpers/<Component>.php
class CPBranch_Logic_Helper { }

// Crons → modules/<Module>/crons/<Component>.php
class CPChatbotConfig_BotChatAudit_Cron { }
```

**File name = `<Component>` part only.** Do NOT include type suffix (e.g., `GeminiAudit.php` not `GeminiAuditHelper.php`).

## Autoload Rules

- Classes following `<Module>_<Component>_<Type>` convention are **autoloaded** — do NOT use `require_once`
- Only use `require_once` for files in `include/utils/`, `include/Webservice/`, or other non-module paths
- Exception: Report handlers in `modules/Reports/custom/` use plain class names and need `require_once`

## Method Naming: camelCase with verb prefix

```php
public function getAccountName(): string { }   // Getters: get*
public function setMode(string $mode): self { } // Setters: set*
public function createNewRecord(): int { }      // Actions: verb prefix
public function isEnabled(): bool { }           // Boolean: is*/has*/can*
public function up(): int { }                   // Migration: up/down
```

## Naming — fields, picklists, files, HTML (nguồn: DevKit nội bộ)

| Element | Rule | Example |
|---------|------|---------|
| Module name | Danh từ số ít, tiếng Anh, PascalCase | `Tour`, `TourBooking` |
| Picklist field name | lowercase + `_`, prefix tên module, kết thúc danh từ số nhiều | `tour_types` |
| Picklist value key / array key | snake_case toàn thường | `new_customer`, `existing_customer` |
| Field / DB column | danh từ hoặc tính từ, snake_case | `module_name`, `is_primary`, `can_delete` |
| CSS/JS/TPL filename | danh từ, PascalCase | `EditView.js`, `BusinessType.tpl` |
| HTML id/class | kebab-case (khuyến khích) | `btn-submit`, `form-register` |
| input name | được dùng `_` (trùng tên field) | `account_name` |

## String Quotes & SQL Style (nguồn: DevKit nội bộ)

- Chuỗi thường: **nháy đơn**. Query hoặc chuỗi chèn biến: **nháy đôi**, biến bọc `{$var}`.
- SQL keywords viết **HOA** (SELECT, FROM, WHERE, JOIN, ORDER BY, GROUP BY...).
- Query dài hơn màn hình: mỗi statement (FROM, JOIN, WHERE, ORDER BY...) xuống dòng riêng.
- Điều kiện JOIN nằm cùng hàng statement JOIN, bọc ngoặc tròn; WHERE chỉ chứa điều kiện bảng chính — điều kiện JOIN KHÔNG để trong WHERE (trừ null-check của LEFT/RIGHT JOIN).
- Query quá dài → tách nhiều query nhỏ xen kẽ PHP logic.

## Type Casting (Security — REQUIRED for request/external data)

```php
$moduleName = (string) $request->getModule();
$recordId = (int) $request->get('record');
$adsAccountId = (string) $adsAccount->get('account_id');
$expiresIn = (int) $token['expires_in'];
```

## Type Declarations (PHP 7+)

```php
public function isValid(): bool { }
public function getId(): int { }
public function getInstance(): self { }
public function process(string $module, int $recordId, bool $force = false): bool { }
```

## Early Return Pattern

Guard clauses at function start; never deep-nest:

```php
public function processRequest(Vtiger_Request $request): bool {
    if (!$this->isEnabled()) return false;
    if (!$this->hasPermission()) return false;

    $recordId = (int) $request->get('record');
    if (empty($recordId)) return false;

    try {
        return $this->processRecord($recordId);
    }
    catch (Exception $e) {
        error_log('Error: ' . $e->getMessage());
        return false;
    }
}

// Empty / short-circuit guards
if (empty($adsAccountId)) return [];
if (empty($connector) || !$connector instanceof BaseConnector) return [];
if ($id == 0) return;
```

## Logical AND Short-Circuit (codebase convention)

```php
// Conditional assignment
!empty($campaignId) && $recordModel = Vtiger_Record_Model::getInstanceById($campaignId);

// Conditional record retrieval
$campaignId > 0 && $sourceCampaign = Vtiger_Record_Model::getInstanceByConditions('Campaigns', ['crmid' => $lastCampaignId]);

// Short inline operations
!empty($data['start_time']) && $data['start_time'] = date('Y-m-d', strtotime($data['start_time']));
```

## Null Coalescing

```php
$campaign[$crmField] = $socialCampaign[$externalField] ?? '';
$insights = $campaignInsights[$campaignId] ?? [];
$campaign['ads'] = !empty($socialCampaign['ads']['data']) ? $socialCampaign['ads']['data'] : [];
```

## Date / Time Formatting

```php
date('Y-m-d H:i:s')
date('Y-m-d H:i:s', strtotime("+ $expiresIn seconds"))
date('Y-m-d', strtotime($dateString))

// Always reject invalid datetimes
!in_array($lastSyncDateTime, ['', '0000-00-00 00:00:00'])
```

## Instance Check Pattern

```php
if (empty($adsAccountRecord) || !$adsAccountRecord instanceof CPAdvertisingAccount_Record_Model) return;
if ($sourceCampaign && $sourceCampaign instanceof Campaigns_Record_Model) { /* ... */ }
```

## Modification Tracking Comments (ownership-based, decided 2026-07-07 — supersedes 2026-07-03 ticket-ID rule)

**No ticket numbers in comments, ever** (default — the 2026-07-03 "ticket ID required" rule is reversed). `/feature` (GSD) workflow always follows this. Other workflows (`/cook`, `/fix`) may still require a ticket ID per their own skill reference — check there first.

Which comment to write depends on **whose class/function you're touching** and **whether the function is new**:

| Situation | Comment |
|---|---|
| New function, class you (the file's `@author`) own | Plain 1-line description of what it does. No `Added by <name>`. |
| New function, class owned by someone else | `// Added by <Name> on <DATE> - <REASON>` |
| Editing an existing function someone else originally wrote | `// Modified by <Name> on <DATE> - <REASON>` |
| Editing an existing function **you** originally wrote | No comment at all — not even a summary line. |

"Owner" of a class = the file's `@author`/`Author:` header. "Originally wrote a function" = check `git blame`/`git log` on that function if the file has multiple contributors — don't assume file-level ownership extends to every function in a multi-author file.

Close attribution blocks with `// End <Name>` (only when the block has a name to close — own-class new functions and own-function edits have no block to close).

```php
// Modified by Vu Mai on 2025-04-28 - support filter by modules
public function getFilteredData(array $modules = []): array {
    // Implementation
}
// End Vu Mai

// Get all active social campaign ids
public function getActiveSocialCampaignIds(): array {
    // Implementation — this is a new function in a class I own, so: summary only, no author
}
```

## Class Structure Pattern

```php
class ModuleName_ClassName_Model extends Vtiger_Base_Model {

    // 1. Constants first
    public const STATUS_ACTIVE = 'ACTIVE';

    // 2. Singleton (if needed)
    public static function getInstance(): self {
        static $instance = null;
        return $instance ?: $instance = new self();
    }

    // 3. Public methods
    public function isEnabled(): bool { }

    // 4. Protected / private methods
    protected function internalMethod(): void { }
}
```

Order: Constants → Singleton `getInstance()` → Public methods → Protected/private methods.

## Empty Line Rules

Add a blank line: (1) before first method of class, (2) between methods, (3) before control structure (`if`/`switch`/`for`/`while`/`foreach`) when preceded by other statements, (4) after closing `}` when followed by other statements. Never double blank lines.

## Array Style

**NO aligned `=>`** — single space before and after `=>`. Aligned arrows create noisy diffs and are inconsistent.

```php
// CORRECT
return [
    'msg_id' => (string) ($data['message']['msg_id'] ?? ''),
    'icon' => $this->rawToCrm($rawIcon),
    'action' => $action,
    'sender_type' => $isPage ? 'oa' : 'user',
    'timestamp' => (int) ($data['timestamp'] ?? 0),
];

// WRONG — do NOT align =>
return [
    'msg_id'      => ...,
    'icon'        => ...,
    'sender_type' => ...,
];
```

## Data Transformation Patterns

### Field Mapping (external → CRM)

```php
$fieldMapping = [
    'id' => 'social_campaign_id',
    'status' => 'campaignstatus',
    'name' => 'campaignname',
    'start_time' => 'start_date',
    'stop_time' => 'closingdate',
    'reach' => 'reach_count',
    'spend' => 'actualcost',
];

foreach ($fieldMapping as $externalField => $crmField) {
    $campaign[$crmField] = $socialCampaign[$externalField] ?? '';
}
```

### Status Mapping

```php
$campaignStatusMapping = [
    'ACTIVE' => 'Active',
    'PAUSED' => 'Inactive',
    'ARCHIVED' => 'Inactive',
    'DELETED' => 'Cancelled',
    'DRAFT' => 'Planning',
    'ENDED' => 'Completed',
];
$socialCampaign['status'] = $campaignStatusMapping[$socialCampaign['status']] ?? '';
```

### Array Merge for Record Data

```php
$campaign = array_merge($campaign, [
    'campaigntype'                => capitalizeFirstLetter($channel) . ' Ads',
    'source'                      => capitalizeFirstLetter($channel) . ' Ads',
    'related_cpadvertisingaccount' => $adsAccountRecord->getId(),
    'assigned_user_id'            => $accountMainOwner,
]);
```

## Database & SQL Conventions

**ALL queries MUST use prepared statements** — NO string concatenation:

```php
$sql = "UPDATE vtiger_cpadvertisingaccount SET last_sync_datetime = ? WHERE cpadvertisingaccountid = ?";
$GLOBALS['adb']->pquery($sql, [date('Y-m-d H:i:s'), $id]);
```

### SQL with COALESCE (handle nullable fields)

```php
WHERE c.social_campaign_id IS NOT NULL
  AND (COALESCE(c.closingdate, '') = '' OR c.closingdate > NOW())
```

### JOIN with Soft Delete Check (REQUIRED)

Always include `deleted = 0` when joining `vtiger_crmentity`:

```php
INNER JOIN vtiger_crmentity ON crmid = cpsocialfeedbackid AND deleted = 0
INNER JOIN vtiger_crmentity AS e ON e.crmid = c.campaignid AND e.deleted = 0
```

## Record Model Patterns

### Standard Update

```php
$record->set('mode', 'edit');
$record->set('field_name', $value);
$record->save();
```

### Bulk / Update Data Pattern

```php
$adsAccount['last_sync_datetime'] = date('Y-m-d H:i:s');
$recordModel->updateData($adsAccount)->save();
```

### Direct SQL Update (Performance — skips event handlers)

Use when triggering save handlers is expensive or unnecessary:

```php
public function updateLastSyncDateTime() {
    $id = (int) $this->getId();
    if ($id == 0) return;

    $sql = "UPDATE vtiger_table SET last_sync_datetime = ? WHERE id = ?";
    $GLOBALS['adb']->pquery($sql, [date('Y-m-d H:i:s'), $id]);
}
```

## Common Patterns

| Task | Pattern |
|------|---------|
| Get record | `Vtiger_Record_Model::getInstanceById($id)` |
| Create record | `Vtiger_Record_Model::getCleanInstance($module)` |
| Update record | `$record->set('mode', 'edit'); $record->save();` |
| Update record data | `$record->updateData($data)->save();` |
| Translation (PHP) | `vtranslate('LBL_KEY', $module)` |
| SQL query | `$GLOBALS['adb']->pquery($sql, $params)` |
| Field mapping | `foreach ($map as $ext => $crm) { $data[$crm] = $src[$ext] ?? ''; }` |
| Status mapping | `$status = $statusMap[$externalStatus] ?? '';` |

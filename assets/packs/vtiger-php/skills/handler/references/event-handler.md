# Event Handler - VTEventHandler

## Handler Skeleton

```php
<?php

/**
 * @author Your Name
 * @create date 2025-02-10
 */

class Accounts_UpdateRelated_Handler extends VTEventHandler {

    public function handleEvent(string $eventName, VTEntityData $entityData): void {
        // CRITICAL: Guard clause - handler fires for ALL modules
        $moduleName = $entityData->getModuleName();
        if ($moduleName !== 'Accounts') {
            return;
        }

        try {
            switch ($eventName) {
                case 'vtiger.entity.aftersave':
                    $this->handleAfterSave($entityData);
                    break;

                case 'vtiger.entity.beforesave':
                    $this->handleBeforeSave($entityData);
                    break;

                case 'vtiger.entity.beforedelete':
                    $this->handleBeforeDelete($entityData);
                    break;

                case 'vtiger.entity.afterdelete':
                    $this->handleAfterDelete($entityData);
                    break;

                case 'vtiger.entity.link':
                    $this->handleLink($entityData);
                    break;

                case 'vtiger.entity.unlink':
                    $this->handleUnlink($entityData);
                    break;
            }
        }
        catch (Throwable $e) {
            error_log("Handler error [{$moduleName}][{$eventName}]: " . $e->getMessage());
            // DON'T throw - will block record save/delete
        }
    }

    private function handleAfterSave(VTEntityData $entityData): void {
        $recordId = (int) $entityData->getId();
        $isNew = $entityData->isNew();

        if ($isNew) {
            // New record logic
            $this->onNewRecord($recordId, $entityData);
        }
        else {
            // Edit record logic
            $this->onEditRecord($recordId, $entityData);
        }
    }

    private function handleBeforeSave(VTEntityData $entityData): void {
        // Validate/modify data before save
        $data = $entityData->getData();

        // Example: Auto-calculate field
        if (empty($data['accountname'])) {
            throw new Exception('Account name is required');
        }
    }

    private function handleBeforeDelete(VTEntityData $entityData): void {
        $recordId = (int) $entityData->getId();

        // Check if deletion is allowed
        $hasRelatedRecords = $this->hasActiveContracts($recordId);
        if ($hasRelatedRecords) {
            throw new Exception('Cannot delete account with active contracts');
        }
    }

    private function handleAfterDelete(VTEntityData $entityData): void {
        $recordId = (int) $entityData->getId();

        // Cleanup related data
        $this->cleanupRelatedData($recordId);
    }

    private function handleLink(VTEntityData $entityData): void {
        // Called after relationship created
        $sourceId = (int) $entityData->getId();
        $targetId = (int) $entityData->get('destinationRecordId');

        // Update aggregate fields, etc.
    }

    private function handleUnlink(VTEntityData $entityData): void {
        // Called after relationship removed
        $sourceId = (int) $entityData->getId();
        $targetId = (int) $entityData->get('destinationRecordId');

        // Recalculate totals, etc.
    }
}
```

## EntityData Methods

### Core Methods

```php
// Get record ID
$recordId = $entityData->getId(); // Returns string
$recordId = (int) $entityData->getId(); // Type cast to int

// Get module name
$moduleName = $entityData->getModuleName(); // 'Accounts', 'Contacts', etc.

// Check if new record (only works in aftersave)
$isNew = $entityData->isNew(); // true/false

// Get all data (DB format)
$data = $entityData->getData();
// ['accountname' => 'Acme Corp', 'phone' => '123-456', ...]

// Get single field value
$accountName = $entityData->get('accountname');
$phone = $entityData->get('phone');

// Get focus object (legacy, avoid if possible)
$focus = $entityData->getFocus();
```

### Field Access Patterns

```php
// Get field with default
$status = $entityData->get('accountstatus') ?: 'Active';

// Get related field (lookup)
$accountId = $entityData->get('account_id');

// Get custom field
$customField = $entityData->get('cf_custom_field');

// Check field changed (requires custom tracking)
$data = $entityData->getData();
if (isset($data['amount'])) {
    // Field was modified
}
```

## Complete Examples

### Example 1: Update Related Records on Save

```php
class Accounts_SyncContacts_Handler extends VTEventHandler {

    public function handleEvent(string $eventName, VTEntityData $entityData): void {
        $moduleName = $entityData->getModuleName();
        if ($moduleName !== 'Accounts' || $eventName !== 'vtiger.entity.aftersave') {
            return;
        }

        try {
            $this->syncContactAddresses($entityData);
        }
        catch (Throwable $e) {
            error_log("SyncContacts error: " . $e->getMessage());
        }
    }

    private function syncContactAddresses(VTEntityData $entityData): void {
        $accountId = (int) $entityData->getId();
        $mailingStreet = $entityData->get('bill_street');
        $mailingCity = $entityData->get('bill_city');

        // Update all related contacts
        $sql = "UPDATE vtiger_contactaddress ca
                INNER JOIN vtiger_contactdetails cd ON ca.contactaddressid = cd.contactid
                INNER JOIN vtiger_crmentity ce ON ce.crmid = cd.contactid
                SET ca.mailingstreet = ?, ca.mailingcity = ?
                WHERE cd.accountid = ? AND ce.deleted = 0";

        $GLOBALS['adb']->pquery($sql, [$mailingStreet, $mailingCity, $accountId]);
    }
}
```

### Example 2: Prevent Deletion with Business Logic

```php
class Invoice_PreventDelete_Handler extends VTEventHandler {

    public function handleEvent(string $eventName, VTEntityData $entityData): void {
        $moduleName = $entityData->getModuleName();
        if ($moduleName !== 'Invoice' || $eventName !== 'vtiger.entity.beforedelete') {
            return;
        }

        $this->checkDeletionAllowed($entityData);
    }

    private function checkDeletionAllowed(VTEntityData $entityData): void {
        $invoiceId = (int) $entityData->getId();

        // Check if invoice is paid
        $status = $entityData->get('invoicestatus');
        if ($status === 'Paid') {
            throw new Exception('Cannot delete paid invoices');
        }

        // Check if invoice has payments
        $sql = "SELECT COUNT(*) FROM vtiger_payment
                WHERE related_invoice = ? AND deleted = 0";
        $result = $GLOBALS['adb']->pquery($sql, [$invoiceId]);
        $paymentCount = (int) $GLOBALS['adb']->query_result($result, 0, 0);

        if ($paymentCount > 0) {
            throw new Exception('Cannot delete invoice with payments');
        }
    }
}
```

### Example 3: Queue Heavy Work for Scheduler

```php
class Accounts_NotifyExternal_Handler extends VTEventHandler {

    public function handleEvent(string $eventName, VTEntityData $entityData): void {
        $moduleName = $entityData->getModuleName();
        if ($moduleName !== 'Accounts' || $eventName !== 'vtiger.entity.aftersave') {
            return;
        }

        try {
            // DON'T call external API here - too slow
            // Queue for background processing instead
            $this->queueExternalNotification($entityData);
        }
        catch (Throwable $e) {
            error_log("NotifyExternal error: " . $e->getMessage());
        }
    }

    private function queueExternalNotification(VTEntityData $entityData): void {
        $accountId = (int) $entityData->getId();

        // Insert into queue table
        $sql = "INSERT INTO vtiger_sync_queue (module, recordid, action, status, created_at)
                VALUES (?, ?, ?, ?, ?)";

        $GLOBALS['adb']->pquery($sql, [
            'Accounts',
            $accountId,
            'notify_external',
            'pending',
            date('Y-m-d H:i:s'),
        ]);

        // Scheduler will process queue in background
    }
}
```

## Handler Locations

### Module-Specific

```
modules/{Module}/handlers/{Handler}.php
```

Example:
```
modules/Accounts/handlers/UpdateRelated.php
modules/Invoice/handlers/PreventDelete.php
```

### Custom/Shared

```
custom/include/EventHandlers/{Handler}.php
```

Example:
```
custom/include/EventHandlers/GlobalSync.php
custom/include/EventHandlers/AuditLog.php
```

## Critical Rules

1. **ALWAYS check module name** - Handler fires for ALL modules
2. **ALWAYS try-catch** - Uncaught exceptions block save/delete
3. **Keep lightweight** - No slow operations (API calls, heavy queries)
4. **Queue heavy work** - Use scheduler for slow tasks
5. **Type cast IDs** - `(int) $entityData->getId()`
6. **isNew() only in aftersave** - Not available in other events
7. **beforedelete throws = blocks deletion** - Use for validation
8. **beforesave throws = blocks save** - Use for validation
9. **Don't modify entityData** - Read-only access
10. **Quick Repair** required after registration changes

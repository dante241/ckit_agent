# Event Handler - VTEventHandler

Skeleton + method table: see `SKILL.md` (single copy). This file covers event semantics and worked examples.

## Event semantics

- Base class `include/events/VTEventHandler.inc`: `public abstract function handleEvent($name, $data);` — overrides MUST stay untyped (`handleEvent($eventName, $entityData)`), otherwise PHP fatals "must be compatible".
- Save triggers (`data/CRMEntity.php`): `vtiger.entity.beforesave.modifiable` → `beforesave` → `beforesave.final` → (DB write) → `aftersave` → `aftersave.final`. All receive the SAME `VTEntityData` built by `VTEntityData::fromCRMEntity($this)`, so `isNew()` is valid in before AND after save, and `set()` in beforesave changes the values that get written.
- Bulk save (import) only fires entity events when `$batchSaveConfig['trigger_entity_events']` is on; otherwise only `vtiger.batchevent.save` fires, with an ARRAY of `VTEntityData`.
- Handlers fire for EVERY module that triggers the event → first line is always the module guard.
- Throwing from a before* event aborts the save/delete (exception propagates to the action). Do NOT wrap before* logic in `try/catch` — that swallows the block.

## EntityData Access (`include/events/VTEntityData.inc`)

```php
$recordId = (int) $entityData->getId();          // '' for a new record in beforesave
$moduleName = $entityData->getModuleName();
$isNew = $entityData->isNew();                    // before + after save
$data = $entityData->getData();                   // column_fields array
$accountName = $entityData->get('accountname');
$entityData->set('implementation_year', date('Y')); // beforesave: value is saved
$focus = $entityData->focus;                      // legacy CRMEntity (property, no getter)
```

Field changes vs the stored value are not tracked by `VTEntityData`; read the old value yourself in beforesave (e.g. `Vtiger_Record_Model::getInstanceById($recordId)`) when you need a diff.

## Complete Examples

### Example 1: Update Related Records on Save

```php
private function syncContactAddresses(VTEntityData $entityData): void {
	$accountId = (int) $entityData->getId();
	$mailingStreet = $entityData->get('bill_street');
	$mailingCity = $entityData->get('bill_city');

	$sql = "UPDATE vtiger_contactaddress AS ca
		INNER JOIN vtiger_contactdetails AS cd ON (ca.contactaddressid = cd.contactid)
		INNER JOIN vtiger_crmentity AS ce ON (ce.crmid = cd.contactid AND ce.deleted = 0)
		SET ca.mailingstreet = ?, ca.mailingcity = ?
		WHERE cd.accountid = ?";

	$GLOBALS['adb']->pquery($sql, [$mailingStreet, $mailingCity, $accountId]);
}
```

Called from the `vtiger.entity.aftersave` branch (inside its `try/catch (Throwable)`).

### Example 2: Prevent Deletion with Business Logic

```php
// Called from the vtiger.entity.beforedelete branch WITHOUT try/catch
private function checkDeletionAllowed(VTEntityData $entityData): void {
	$invoiceId = (int) $entityData->getId();

	if ($entityData->get('invoicestatus') === 'Paid') {
		throw new Exception('Cannot delete paid invoices');
	}

	// vtiger_payment is a hypothetical table — substitute the real related table
	$sql = "SELECT COUNT(*) FROM vtiger_payment WHERE related_invoice = ? AND deleted = 0";
	$paymentCount = (int) $GLOBALS['adb']->getOne($sql, [$invoiceId]);

	if ($paymentCount > 0) {
		throw new Exception('Cannot delete invoice with payments');
	}
}
```

### Example 3: Queue Heavy Work for Scheduler

```php
// aftersave: don't call external APIs inline — enqueue for a cron instead
private function queueExternalNotification(VTEntityData $entityData): void {
	// vtiger_sync_queue is a hypothetical table — create it via migration first
	$sql = "INSERT INTO vtiger_sync_queue (module, recordid, action, status, created_at) VALUES (?, ?, ?, ?, ?)";

	$GLOBALS['adb']->pquery($sql, [
		'Accounts',
		(int) $entityData->getId(),
		'notify_external',
		'pending',
		date('Y-m-d H:i:s'),
	]);
}
```

## Handler Location

```
modules/{Module}/HandlersRegister.php
modules/{Module}/handlers/{$handlerName}.php        // class {$handlerName}
modules/{Module}/handlers/{$batchHandlerName}.php   // optional batch handler
```

Global (all modules) static hooks live in `modules/Vtiger/handlers/GlobalEventHandler.php` (`getGlobalEventHandlerClass()`).

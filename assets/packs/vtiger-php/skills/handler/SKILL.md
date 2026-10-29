---
name: handler
description: "VTiger event handlers — VTEventHandler, beforesave/aftersave, before/afterdelete, batch events, HandlersRegister. Use when: hook sự kiện record, xử lý trước/sau khi lưu/xoá, side-effect khi save; keywords: handler, aftersave, event."
user-invocable: false
---

# VTiger Event Handler Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Creating record lifecycle hooks (before/after save, delete, restore, merge)
- Auto-updating related records on changes
- Enforcing business rules at record level
- Triggering notifications or external API calls on record events

## Available Events

| Event | Trigger Point | Common Use Cases |
|-------|---------------|------------------|
| `vtiger.entity.beforesave` | Before record save (also `.modifiable` before it, `.final` after it — `data/CRMEntity.php`) | Validate data, auto-calculate fields (`$entityData->set()`) |
| `vtiger.entity.aftersave` | After record save (`.final` variant exists) | Update related records, send notifications, sync external systems |
| `vtiger.entity.beforedelete` | Before record delete | Prevent deletion, cascade delete related records |
| `vtiger.entity.afterdelete` | After record delete | Cleanup related data, log deletions |
| `vtiger.entity.beforerestore` / `afterrestore` | Restore from recycle bin | Re-establish links, notify users |
| `vtiger.entity.beforemerge` / `aftermerge` | Record merge | Move custom related data |
| `vtiger.batchevent.save` / `.delete` | Import (`modules/Import/actions/Data.php`) / undo import — receives an ARRAY of `VTEntityData` | Bulk post-processing |

There are no link/unlink entity events in this codebase. For relation link/unlink, add static `relationLinked($relationModel, $sourceRecordId, $relatedRecordId, $relatedModuleName)` / `relationUnlinked(...)` to the module handler class; `Vtiger_Relation_Model::addRelation()/deleteRelation()` call them via `getEventHandlerClass()` (exemplar `modules/Accounts/handlers/AccountHandler.php:58`).

## Handler Skeleton

File `modules/MyModule/handlers/MyModuleHandler.php` — class name MUST equal `$handlerName` in `HandlersRegister.php` and the file basename.

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

class MyModuleHandler extends VTEventHandler {

	// Signature must match VTEventHandler::handleEvent($name, $data) — no param/return types
	public function handleEvent($eventName, $entityData) {
		if ($entityData->getModuleName() != 'MyModule') return;

		// before* events: let exceptions propagate — throwing is what blocks the save/delete
		if ($eventName === 'vtiger.entity.beforesave') {
			$this->beforeSave($entityData);
		}

		if ($eventName === 'vtiger.entity.beforedelete') {
			$this->beforeDelete($entityData);
		}

		// after* events: the record is already written — log failures, don't break the request
		if ($eventName === 'vtiger.entity.aftersave') {
			try {
				$this->afterSave($entityData);
			}
			catch (Throwable $e) {
				error_log("MyModuleHandler {$eventName}: " . $e->getMessage());
			}
		}
	}

	private function beforeSave(VTEntityData $entityData): void {
		if (empty($entityData->get('implementation_year'))) {
			$entityData->set('implementation_year', date('Y'));
		}
	}

	private function beforeDelete(VTEntityData $entityData): void {
		if ($entityData->get('status') === 'Locked') {
			throw new Exception('Cannot delete a locked record');
		}
	}

	private function afterSave(VTEntityData $entityData): void {
		$recordId = (int) $entityData->getId();
		$isNew = $entityData->isNew();

		// Your logic here
	}
}
```

## EntityData Methods (`include/events/VTEntityData.inc`)

| Method | Return Type | Description |
|--------|-------------|-------------|
| `getId()` | string | Record ID (CRMID) — empty in beforesave of a new record |
| `getModuleName()` | string | Module name (Accounts, Contacts, etc.) |
| `isNew()` | bool | True if new record — set in `fromCRMEntity()`, valid in both beforesave and aftersave |
| `getData()` | array | All field values (`column_fields`) |
| `get($fieldName)` / `set($fieldName, $value)` | mixed | Read / write one field; `set()` in beforesave changes what gets saved |
| `focus` (property) | CRMEntity | Underlying legacy entity (avoid if possible) |

## Registration Pattern

`modules/MyModule/HandlersRegister.php` (exemplar `modules/CPMasterPlan/HandlersRegister.php`):

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

$registeredEvents = array(
	'vtiger.entity.beforesave',
	'vtiger.entity.aftersave',
	'vtiger.entity.beforedelete',
	'vtiger.entity.afterdelete',
);

$handlerName = 'MyModuleHandler';
```

QuickRepair (`modules/Vtiger/actions/QuickRepair.php`) globs `modules/*/HandlersRegister.php` and registers each event with class `$handlerName` from `modules/<M>/handlers/<$handlerName>.php`. `vtiger.batchevent.*` events are registered with `$batchHandlerName` instead — see [Registration](references/registration.md).

**Activation:** run Quick Repair: `index.php?module=Vtiger&parent=Settings&action=QuickRepair`.

## Critical Pitfalls

1. **Module check REQUIRED** - Handler fires for ALL modules, check `getModuleName()` first
2. **Keep lightweight** - Handler runs inside save/delete, don't run slow operations
3. **Exceptions:** throw in before* to block; wrap after* work in `try/catch (Throwable)` and log — never wrap the before* branches (that swallows the block)
4. **Use scheduler** for heavy tasks - Queue work, don't process inline
5. **Signature** - `handleEvent($eventName, $entityData)` untyped; a typed override fatals ("must be compatible")
6. **Quick Repair required** - Changes to HandlersRegister.php need Quick Repair
7. **Type cast IDs** - `(int) $entityData->getId()` for safety

## References

- [Event Handler](references/event-handler.md) - Event semantics, field access, examples
- [Registration](references/registration.md) - HandlersRegister.php, batch handler, Quick Repair, troubleshooting

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Handler chuẩn (tung.nguyen 2/2): `modules/CPMasterPlan/handlers/CPMasterPlanHandler.php`
- Batch handler (Tuyen Tran + tin/tung touch 4/11 — đối chiếu thêm): `modules/HelpDesk/handlers/HelpDesksBatchHandler.php` (registered via `$batchHandlerName` in `modules/HelpDesk/HandlersRegister.php`)

## Verify

```bash
php -l <handler file>
# Handler đã đăng ký chưa:
mysql <db> -e "SELECT event_name, handler_path, is_active FROM vtiger_eventhandlers WHERE handler_class='<Class>'"
# Trigger event thật (save record qua UI/API) rồi check side-effect + PHP error log
```

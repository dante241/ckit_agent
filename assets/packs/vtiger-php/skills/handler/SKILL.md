---
name: handler
description: "VTiger event handlers — VTEventHandler, aftersave, beforedelete, link/unlink, HandlersRegister. Use when: hook sự kiện record, xử lý sau khi lưu/xoá, side-effect khi save; keywords: handler, aftersave, event."
user-invocable: false
---

# VTiger Event Handler Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Creating record lifecycle hooks (before/after save, delete)
- Implementing relationship event handlers (link/unlink)
- Auto-updating related records on changes
- Enforcing business rules at record level
- Triggering notifications or external API calls on record events

## Available Events

| Event | Trigger Point | Common Use Cases |
|-------|---------------|------------------|
| `vtiger.entity.aftersave` | After record save | Update related records, send notifications, sync external systems |
| `vtiger.entity.beforesave` | Before record save | Validate data, auto-calculate fields, enforce business rules |
| `vtiger.entity.beforedelete` | Before record delete | Prevent deletion, cascade delete related records |
| `vtiger.entity.afterdelete` | After record delete | Cleanup related data, log deletions |
| `vtiger.entity.link` | After relationship created | Update aggregate fields, sync permissions |
| `vtiger.entity.unlink` | After relationship removed | Recalculate totals, cleanup dependencies |
| `vtiger.entity.afterrestore` | After record restored from recycle bin | Re-establish links, notify users |

## Handler Skeleton

```php
<?php

/**
 * @author Your Name
 * @create date YYYY-MM-DD
 */

class MyModule_MyHandler_Handler extends VTEventHandler {

    public function handleEvent(string $eventName, VTEntityData $entityData): void {
        // Guard clause - check module name
        $moduleName = $entityData->getModuleName();
        if ($moduleName !== 'Accounts') {
            return;
        }

        try {
            switch ($eventName) {
                case 'vtiger.entity.aftersave':
                    $this->afterSave($entityData);
                    break;

                case 'vtiger.entity.beforedelete':
                    $this->beforeDelete($entityData);
                    break;

                case 'vtiger.entity.link':
                    $this->afterLink($entityData);
                    break;
            }
        }
        catch (Throwable $e) {
            error_log("Handler error in $eventName: " . $e->getMessage());
            // Don't throw - prevents record save/delete
        }
    }

    private function afterSave(VTEntityData $entityData): void {
        $recordId = (int) $entityData->getId();
        $isNew = $entityData->isNew();

        // Your logic here
    }

    private function beforeDelete(VTEntityData $entityData): void {
        $recordId = (int) $entityData->getId();

        // Your logic here
    }

    private function afterLink(VTEntityData $entityData): void {
        // Your logic here
    }
}
```

## EntityData Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `getId()` | string | Record ID (CRMID) |
| `getModuleName()` | string | Module name (Accounts, Contacts, etc.) |
| `isNew()` | bool | True if new record (only in aftersave) |
| `getData()` | array | All field values (DB format) |
| `get($fieldName)` | mixed | Single field value |
| `getFocus()` | object | Legacy record object (avoid if possible) |

## Registration Pattern

Create `HandlersRegister.php` in module root or `custom/include/events/`:

```php
<?php

/**
 * Event handlers registration for MyModule
 * Location: modules/MyModule/HandlersRegister.php
 * OR: custom/include/events/HandlersRegister.php
 */

$handlersRegister = [
    // After save handler
    [
        'eventName' => 'vtiger.entity.aftersave',
        'handlerFile' => 'modules/MyModule/handlers/MyHandler.php',
        'className' => 'MyModule_MyHandler_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // Before delete handler
    [
        'eventName' => 'vtiger.entity.beforedelete',
        'handlerFile' => 'modules/MyModule/handlers/AnotherHandler.php',
        'className' => 'MyModule_AnotherHandler_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // Link handler
    [
        'eventName' => 'vtiger.entity.link',
        'handlerFile' => 'modules/MyModule/handlers/LinkHandler.php',
        'className' => 'MyModule_LinkHandler_Handler',
        'condition' => '',
        'dependent' => '',
    ],
];
```

**Important:** Multiple events for same handler = multiple entries in array.

**Activation:** Run Quick Repair (Settings → Module Manager → Quick Repair)

## Critical Pitfalls

1. **Module check REQUIRED** - Handler fires for ALL modules, check `$moduleName` first
2. **Keep lightweight** - Handler blocks save/delete, don't run slow operations
3. **ALWAYS try-catch** - Uncaught exceptions prevent record save/delete
4. **Use scheduler** for heavy tasks - Queue work, don't process inline
5. **isNew() only in aftersave** - Other events don't have new/edit distinction
6. **Quick Repair required** - Changes to HandlersRegister.php need Quick Repair
7. **Type cast IDs** - `(int) $entityData->getId()` for safety

## References

- [Event Handler](references/event-handler.md) - Handler skeleton, EntityData methods, locations
- [Registration](references/registration.md) - HandlersRegister.php format, Quick Repair

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Handler chuẩn (tung.nguyen 2/2): `modules/CPMasterPlan/handlers/CPMasterPlanHandler.php`
- Batch handler (Tuyen Tran + tin/tung touch 4/11 — đối chiếu thêm): `modules/HelpDesk/handlers/HelpDesksBatchHandler.php`

## Verify

```bash
php -l <handler file>
# Handler đã đăng ký chưa:
mysql <db> -e "SELECT * FROM vtiger_eventhandlers WHERE handler_class='<Class>'"
# Trigger event thật (save record qua UI/API) rồi check side-effect + logs/
```

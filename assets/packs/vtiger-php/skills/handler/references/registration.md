# Handler Registration - HandlersRegister.php

## Format (read by QuickRepair)

`modules/Vtiger/actions/QuickRepair.php` globs `modules/*/HandlersRegister.php`, includes each file and reads three globals:

| Variable | Meaning |
|----------|---------|
| `$registeredEvents` | array of event names to register |
| `$handlerName` | class + file basename for every non-batch event: `modules/<M>/handlers/<$handlerName>.php` |
| `$batchHandlerName` | class + file basename for `vtiger.batchevent.*` events (optional) |

Each event is registered via `Vtiger_Event::register($moduleModel, $eventName, $className, $path)` → rows in `vtiger_eventhandlers` (`cond = ''`, `dependent_on = '[]'`) and `vtiger_eventhandler_module`.

Full example with a batch handler (`modules/HelpDesk/HandlersRegister.php`):

```php
$registeredEvents = array(
	'vtiger.entity.beforesave',
	'vtiger.entity.aftersave',
	'vtiger.entity.beforedelete',
	'vtiger.entity.afterdelete',
	'vtiger.batchevent.save',
	'vtiger.batchevent.beforedelete',
	'vtiger.batchevent.afterdelete',
	'vtiger.batchevent.beforerestore',
	'vtiger.batchevent.afterrestore',
);

$handlerName = 'HelpDesksHandler';
$batchHandlerName = 'HelpDesksBatchHandler';
```

Rules:
- Class name == `$handlerName` == file basename (no `<Module>_<X>_Handler` autoload naming — the file is loaded by path; `getEventHandlerClass()` in `include/utils/CommonUtils.php` relies on the same convention).
- One handler class per module, one register file per module; list every event the class handles in `$registeredEvents`.
- If you list a `vtiger.batchevent.*` event you MUST set `$batchHandlerName` and create that file.

## Batch handler

- `handleEvent($eventName, $entityDataList)` receives an ARRAY of `VTEntityData` (`vtiger.batchevent.save` from Import, `vtiger.batchevent.delete` from undo-import).
- Optional static `processRecords(&$recordModel)` is called per row by list views through `handleProcessRecords()` (`include/utils/CommonUtils.php`) to post-format display values (exemplar `modules/HelpDesk/handlers/HelpDesksBatchHandler.php`).

## Activation

1. Create `modules/<M>/handlers/<$handlerName>.php` (skeleton in `SKILL.md`).
2. Create/extend `modules/<M>/HandlersRegister.php`.
3. Run Quick Repair: `index.php?module=Vtiger&parent=Settings&action=QuickRepair` (admin).
4. Verify:

```sql
SELECT event_name, handler_path, handler_class, is_active
FROM vtiger_eventhandlers
WHERE handler_class = 'MyModuleHandler';
```

## Troubleshooting

### Handler Not Firing

1. Row missing in `vtiger_eventhandlers` → re-run Quick Repair; check `$handlerName` spelling.
2. File path wrong → must be exactly `modules/<M>/handlers/<$handlerName>.php`.
3. `php -l modules/<M>/handlers/<$handlerName>.php`.
4. Guard returns early → log `$eventName` + `getModuleName()` at the top of `handleEvent` temporarily.
5. Import/bulk save → entity events fire only when `$batchSaveConfig['trigger_entity_events']` is on; otherwise only the batch event.

### Handler Causing Errors

1. Check the PHP error log (php-fpm/nginx error log) — `error_log()` output lands there.
2. after* logic: wrap in `try/catch (Throwable)` and log; before* logic: exceptions are intentional (they block the save/delete).
3. Disable temporarily:
   ```sql
   UPDATE vtiger_eventhandlers SET is_active = 0 WHERE handler_class = 'MyModuleHandler';
   ```

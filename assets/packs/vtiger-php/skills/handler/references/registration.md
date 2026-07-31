# Handler Registration - HandlersRegister.php

## HandlersRegister.php Format

```php
<?php

/**
 * Event handlers registration
 * Location: modules/{Module}/HandlersRegister.php
 * OR: custom/include/events/HandlersRegister.php
 */

$handlersRegister = [
    // Entry format
    [
        'eventName' => 'vtiger.entity.aftersave',
        'handlerFile' => 'modules/Accounts/handlers/UpdateRelated.php',
        'className' => 'Accounts_UpdateRelated_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // Multiple events for same handler = multiple entries
    [
        'eventName' => 'vtiger.entity.beforedelete',
        'handlerFile' => 'modules/Accounts/handlers/UpdateRelated.php',
        'className' => 'Accounts_UpdateRelated_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // Another handler
    [
        'eventName' => 'vtiger.entity.link',
        'handlerFile' => 'modules/Accounts/handlers/LinkHandler.php',
        'className' => 'Accounts_LinkHandler_Handler',
        'condition' => '',
        'dependent' => '',
    ],
];
```

## Field Definitions

| Field | Description | Example |
|-------|-------------|---------|
| `eventName` | Event to listen to | `vtiger.entity.aftersave` |
| `handlerFile` | Path to handler class file | `modules/Accounts/handlers/Handler.php` |
| `className` | Handler class name | `Accounts_UpdateRelated_Handler` |
| `condition` | SQL condition (rarely used) | `''` (usually empty) |
| `dependent` | Depends on other handler (rarely used) | `''` (usually empty) |

## Complete Example

```php
<?php

/**
 * CPSocialFeedback event handlers
 * Location: modules/CPSocialFeedback/HandlersRegister.php
 */

$handlersRegister = [
    // Update campaign stats after feedback save
    [
        'eventName' => 'vtiger.entity.aftersave',
        'handlerFile' => 'modules/CPSocialFeedback/handlers/UpdateCampaignStats.php',
        'className' => 'CPSocialFeedback_UpdateCampaignStats_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // Cleanup relations before delete
    [
        'eventName' => 'vtiger.entity.beforedelete',
        'handlerFile' => 'modules/CPSocialFeedback/handlers/CleanupRelations.php',
        'className' => 'CPSocialFeedback_CleanupRelations_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // Sync to external system after save
    [
        'eventName' => 'vtiger.entity.aftersave',
        'handlerFile' => 'custom/include/EventHandlers/SyncExternal.php',
        'className' => 'SyncExternal_Handler',
        'condition' => '',
        'dependent' => '',
    ],
];
```

## Activation Steps

### 1. Create Handler File

```php
// modules/Accounts/handlers/UpdateRelated.php
<?php

class Accounts_UpdateRelated_Handler extends VTEventHandler {

    public function handleEvent(string $eventName, VTEntityData $entityData): void {
        $moduleName = $entityData->getModuleName();
        if ($moduleName !== 'Accounts') {
            return;
        }

        try {
            if ($eventName === 'vtiger.entity.aftersave') {
                $this->updateRelatedContacts($entityData);
            }
        }
        catch (Throwable $e) {
            error_log("Handler error: " . $e->getMessage());
        }
    }

    private function updateRelatedContacts(VTEntityData $entityData): void {
        // Logic here
    }
}
```

### 2. Create HandlersRegister.php

```php
// modules/Accounts/HandlersRegister.php
<?php

$handlersRegister = [
    [
        'eventName' => 'vtiger.entity.aftersave',
        'handlerFile' => 'modules/Accounts/handlers/UpdateRelated.php',
        'className' => 'Accounts_UpdateRelated_Handler',
        'condition' => '',
        'dependent' => '',
    ],
];
```

### 3. Run Quick Repair

1. Navigate to: **Settings → Module Manager → Quick Repair**
2. Click "Quick Repair" button
3. System scans all HandlersRegister.php files
4. Handlers registered to `vtiger_eventhandlers` table

### 4. Verify Registration

```sql
SELECT * FROM vtiger_eventhandlers
WHERE handler_class = 'Accounts_UpdateRelated_Handler';
```

Expected result:
```
event_name: vtiger.entity.aftersave
handler_path: modules/Accounts/handlers/UpdateRelated.php
handler_class: Accounts_UpdateRelated_Handler
is_active: 1
```

## Multiple Events Example

If handler listens to multiple events, create multiple entries:

```php
$handlersRegister = [
    // After save
    [
        'eventName' => 'vtiger.entity.aftersave',
        'handlerFile' => 'modules/Invoice/handlers/InvoiceSync.php',
        'className' => 'Invoice_InvoiceSync_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // Before delete
    [
        'eventName' => 'vtiger.entity.beforedelete',
        'handlerFile' => 'modules/Invoice/handlers/InvoiceSync.php',
        'className' => 'Invoice_InvoiceSync_Handler',
        'condition' => '',
        'dependent' => '',
    ],

    // After delete
    [
        'eventName' => 'vtiger.entity.afterdelete',
        'handlerFile' => 'modules/Invoice/handlers/InvoiceSync.php',
        'className' => 'Invoice_InvoiceSync_Handler',
        'condition' => '',
        'dependent' => '',
    ],
];
```

## Sample HandlersRegister.php

Copy from existing sample:

```bash
cp modules/CPSocialFeedback/HandlersRegister.sample.php \
   modules/MyModule/HandlersRegister.php
```

Edit to match your handler:

```php
<?php

/**
 * MyModule event handlers registration
 * Copy from HandlersRegister.sample.php
 */

$handlersRegister = [
    [
        'eventName' => 'vtiger.entity.aftersave',
        'handlerFile' => 'modules/MyModule/handlers/MyHandler.php',
        'className' => 'MyModule_MyHandler_Handler',
        'condition' => '',
        'dependent' => '',
    ],
];
```

## Troubleshooting

### Handler Not Firing

1. **Check registration:**
   ```sql
   SELECT * FROM vtiger_eventhandlers
   WHERE handler_class = 'YourClass_Handler';
   ```

2. **Run Quick Repair** again

3. **Check handler file exists:**
   ```bash
   ls -la modules/YourModule/handlers/YourHandler.php
   ```

4. **Check handler syntax:**
   ```php
   php -l modules/YourModule/handlers/YourHandler.php
   ```

5. **Add debug logging:**
   ```php
   public function handleEvent(string $eventName, VTEntityData $entityData): void {
       error_log("Handler called: $eventName for " . $entityData->getModuleName());
       // ...
   }
   ```

### Handler Causing Errors

1. **Check error logs:**
   ```bash
   tail -f logs/vtigercrm.log
   ```

2. **Verify try-catch exists:**
   ```php
   try {
       // Handler logic
   }
   catch (Throwable $e) {
       error_log("Error: " . $e->getMessage());
   }
   ```

3. **Disable handler temporarily:**
   ```sql
   UPDATE vtiger_eventhandlers
   SET is_active = 0
   WHERE handler_class = 'YourClass_Handler';
   ```

## Locations

### Module-Specific

```
modules/{Module}/HandlersRegister.php
modules/{Module}/handlers/{Handler}.php
```

### Custom/Global

```
custom/include/events/HandlersRegister.php
custom/include/EventHandlers/{Handler}.php
```

## Critical Rules

1. **MUST run Quick Repair** after changes to HandlersRegister.php
2. **Multiple events = multiple array entries** with same handler file
3. **condition and dependent** usually empty string
4. **Handler file path** relative to VTiger root
5. **Class name** must match class in handler file
6. **Copy from sample** to get correct format
7. **Verify in database** after Quick Repair
8. **Disable via SQL** for quick troubleshooting

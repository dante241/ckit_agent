---
description: "Migration conventions — naming YYYY.MM.DD.HH.mm.ss, anon class CPMigration_Base_Model, idempotent. Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(modules/CPMigration/migrations/**), tool:write(modules/CPMigration/migrations/**)"
condition: ".*"
---

# Migration File Conventions

## File Naming

`YYYY.MM.DD.HH.mm.ss_DescriptiveName.php`

Example: `2025.04.16.10.30.00_AddSocialAds.php`

## Template

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @name MigrationName
 * @create date YYYY-MM-DD HH:mm:ss
 */

return new class extends CPMigration_Base_Model {

    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        // Migration logic
        return self::UP_SUCCESS;
    }

    public function down(): int {
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

Migration header keeps `@create date YYYY-MM-DD HH:mm:ss` (skill `migration` + existing migrations), not the `YYYY.MM.DD` of php-conventions.

## Helper Methods

- `$this->pquery($sql, $params)` — prepared SQL
- `$this->createCronjob($name, $frequencySeconds, $moduleName, $handlerFile, $runBy = 'Cronjob')` — register cron job (`$handlerFile` = `.service` under `cron/modules/<Module>/`)
- `$this->createPicklistValues($fieldName, $moduleName, $options, $idFieldName = '', $isOverride = false)` — add picklist values; `$isOverride = true` runs `DELETE FROM <picklist table>` first (wipes every existing value)
- Idempotent helpers: `isTableExists`, `isColumnExists`, `isIndexExists`, `addColumnIfMissing`, `addIndexIfMissing`, `dropIndexIfExists`

Examples: skill `migration`.

## Return Codes

`self::UP_SUCCESS` / `self::UP_FAILED`; `self::DOWN_NOT_SUPPORTED` (default) / `self::DOWN_SUCCESS` / `self::DOWN_FAILED`.

## Reminder

After creating a migration, **alert the user** to run pending migrations.

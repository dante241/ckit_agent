---
paths:
  - "modules/CPMigration/migrations/**"
---

# Migration File Conventions

> Loads only when editing migration files.

## File Naming

`YYYY.MM.DD.HH.mm.ss_DescriptiveName.php`

Example: `2025.04.16.10.30.00_AddSocialAds.php`

## Template

```php
<?php

/**
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

## Helper Methods

- `$this->pquery($sql, $params)` — prepared SQL
- `$this->createCronjob(...)` — register cron job
- `$this->createPicklistValues(...)` — add picklist values

### Examples

```php
// 1. Schema change
$this->pquery("ALTER TABLE vtiger_campaign ADD social_campaign_id VARCHAR(255)");

// 2. Register cron job — (Name, frequency_seconds, Module, handler_path)
$this->createCronjob('SyncSocialCampaigns', 900, 'Campaigns', 'modules/Campaigns/crons/SyncSocialCampaigns.php');

// 3. Add picklist values — (fieldname, Module, [['key'=>..., 'color'=>'#hex'], ...], '', addToRoles)
$this->createPicklistValues('campaigntype', 'Campaigns', [
    ['key' => 'Facebook Ads', 'color' => '#1877F2'],
    ['key' => 'Zalo Ads',     'color' => '#0068FF'],
], '', true);
```

## Return Codes

- `self::UP_SUCCESS` — migration applied
- `self::DOWN_NOT_SUPPORTED` — rollback not implemented (default)
- `self::DOWN_SUCCESS` — rollback applied

## Reminder

After creating migration, **alert user** to run pending migrations (per `primary-workflow.md` Phase 10).

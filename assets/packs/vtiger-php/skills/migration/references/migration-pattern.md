# Migration Pattern Reference

## File Location

**Custom modules:** `custom/migrations/`
**Core modules:** `modules/{Module}/migrations/`

Both locations are auto-discovered by CPMigration system.

## Naming Convention

**Format:** `YYYY.MM.DD.HH.mm.ss_DescriptiveName.php`

**Critical:** Dot-separated timestamp, NOT underscores or hyphens.

**Examples:**
```
2025.04.16.10.30.00_AddSocialAdsModule.php
2025.11.05.14.22.13_AlterCampaignsAddLastSync.php
2026.02.10.09.15.00_CreateNotificationIndexes.php
```

## File Header

**Required docblock:**
```php
<?php

/**
 * @name AddLastSyncDatetimeToCampaigns
 * @create date 2026-02-10 14:30:00
 */
```

## Anonymous Class Structure

**MUST extend CPMigration_Base_Model:**
```php
return new class extends CPMigration_Base_Model {

    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        // Forward migration
        return self::UP_SUCCESS;
    }

    public function down(): int {
        // Reverse migration
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

## Required Methods

### up(): int

**Returns:**
- `self::UP_SUCCESS` — migration executed successfully
- `self::UP_FAILED` — migration failed
- `self::UP_ALREADY_EXECUTED` — already applied, skip

**Pattern:**
```php
public function up(): int {
    try {
        $this->pquery($sql, $params);
        return self::UP_SUCCESS;
    } catch (Exception $e) {
        error_log('Migration failed: ' . $e->getMessage());
        return self::UP_FAILED;
    }
}
```

### down(): int

**Returns:**
- `self::DOWN_SUCCESS` — rollback successful
- `self::DOWN_FAILED` — rollback failed
- `self::DOWN_NOT_SUPPORTED` — no rollback available

**Pattern:**
```php
public function down(): int {
    // Reverse all changes from up()
    $this->pquery("ALTER TABLE vtiger_table DROP COLUMN column_name");
    return self::DOWN_SUCCESS;
}
```

## Database Operations

### Use $this->pquery()

```php
$sql = "ALTER TABLE vtiger_campaigns ADD COLUMN last_sync_datetime DATETIME";
$this->pquery($sql);
```

### Global $adb Access

```php
global $adb;
$result = $adb->pquery($sql, $params);
```

## Common Operations

### CREATE TABLE

```php
$sql = "CREATE TABLE IF NOT EXISTS vtiger_cpsocialmedia (
    cpsocialmediaid INT AUTO_INCREMENT PRIMARY KEY,
    platform VARCHAR(50) NOT NULL,
    account_id VARCHAR(255) NOT NULL,
    access_token TEXT,
    INDEX idx_platform (platform)
) ENGINE=InnoDB DEFAULT CHARSET=utf8";
$this->pquery($sql);
```

### ALTER TABLE

```php
// Add column
$sql = "ALTER TABLE vtiger_campaigns
        ADD COLUMN last_sync_datetime DATETIME DEFAULT NULL";
$this->pquery($sql);

// Modify column
$sql = "ALTER TABLE vtiger_campaigns
        MODIFY COLUMN campaignstatus VARCHAR(100)";
$this->pquery($sql);

// Drop column
$sql = "ALTER TABLE vtiger_campaigns DROP COLUMN old_field";
$this->pquery($sql);
```

### CREATE INDEX

```php
$sql = "CREATE INDEX idx_last_sync ON vtiger_campaigns(last_sync_datetime)";
$this->pquery($sql);
```

### INSERT Seed Data

```php
$sql = "INSERT INTO vtiger_config (category, name, value)
        VALUES (?, ?, ?)";
$this->pquery($sql, ['SocialAds', 'facebook_api_version', 'v18.0']);
```

## Idempotent Checks

**Check column exists:**
```php
global $adb;
$result = $adb->pquery("SHOW COLUMNS FROM vtiger_campaigns LIKE 'last_sync_datetime'");
if ($adb->num_rows($result) == 0) {
    $sql = "ALTER TABLE vtiger_campaigns ADD COLUMN last_sync_datetime DATETIME";
    $this->pquery($sql);
}
```

**Check table exists:**
```php
$sql = "CREATE TABLE IF NOT EXISTS vtiger_table (...)";
$this->pquery($sql);
```

**Check index exists:**
```php
global $adb;
$result = $adb->pquery("SHOW INDEX FROM vtiger_campaigns WHERE Key_name = 'idx_last_sync'");
if ($adb->num_rows($result) == 0) {
    $this->pquery("CREATE INDEX idx_last_sync ON vtiger_campaigns(last_sync_datetime)");
}
```

## Helper Methods

### createCronjob

```php
$this->createCronjob(
    'Sync Social Campaigns',
    900,  // 15 minutes
    'Campaigns',
    'cron/SyncSocialCampaigns.php'
);
```

### createPicklistValues

```php
$values = [
    ['key' => 'Active', 'color' => '#28a745'],
    ['key' => 'Paused', 'color' => '#ffc107'],
    ['key' => 'Archived', 'color' => '#6c757d'],
];
$this->createPicklistValues('campaignstatus', 'Campaigns', $values, '', true);
```

## Batch Large Data

**For 1000+ rows, chunk into batches:**
```php
public function up(): int {
    global $adb;
    $batchSize = 500;
    $offset = 0;

    while (true) {
        $sql = "SELECT * FROM old_table LIMIT ?, ?";
        $result = $adb->pquery($sql, [$offset, $batchSize]);

        if ($adb->num_rows($result) == 0) break;

        while ($row = $adb->fetchByAssoc($result)) {
            $insert = "INSERT INTO new_table (field) VALUES (?)";
            $this->pquery($insert, [$row['value']]);
        }

        $offset += $batchSize;
    }

    return self::UP_SUCCESS;
}
```

## Critical Pitfalls Summary

1. **Anonymous class ONLY** — no `class MigrationName`
2. **Dot-separated timestamp** — `2025.04.16` not `2025_04_16`
3. **@name + @create date** in docblock
4. **$isRunBeforeQuickRepair = false** explicitly
5. **Return constants** — `UP_SUCCESS` not `true`
6. **down() reverses up()** completely
7. **Idempotent checks** — safe to re-run
8. **Batch large data** — chunks of 500-1000

# Migration Pattern Reference

> Naming, header template, anonymous-class skeleton and return constants: see `../SKILL.md` and `rule://migration-rules`. This file only holds operation recipes.

## File Location

Single location: `modules/CPMigration/migrations/`. `CPMigration_Migrator_Model` globs only `modules/CPMigration/migrations/*.php`; there is no per-module or `custom/` migrations directory.

## up() Error Handling

`$this->pquery()` throws on SQL failure. Catch at the `up()` boundary only when you need to return `UP_FAILED` with a log line:

```php
public function up(): int {
    try {
        $this->pquery($sql, $params);
        return self::UP_SUCCESS;
    }
    catch (Throwable $e) {
        error_log('Migration failed: ' . $e->getMessage());
        return self::UP_FAILED;
    }
}
```

## Common Operations

### CREATE TABLE

```php
if (!$this->isTableExists('vtiger_cpsocialmedia')) {
    $this->pquery("CREATE TABLE vtiger_cpsocialmedia (
        cpsocialmediaid INT AUTO_INCREMENT PRIMARY KEY,
        platform VARCHAR(50) NOT NULL,
        account_id VARCHAR(255) NOT NULL,
        access_token TEXT,
        INDEX idx_platform (platform)
    ) ENGINE=InnoDB DEFAULT CHARSET=utf8");
}
```

### ALTER TABLE

```php
// Add column (idempotent)
$this->addColumnIfMissing('vtiger_campaign', 'last_sync_datetime', 'DATETIME DEFAULT NULL');

// Modify column
$this->pquery("ALTER TABLE vtiger_campaign MODIFY COLUMN campaignstatus VARCHAR(200)");

// Drop column
if ($this->isColumnExists('vtiger_campaign', 'old_field')) {
    $this->pquery("ALTER TABLE vtiger_campaign DROP COLUMN old_field");
}
```

### CREATE / DROP INDEX

```php
$this->addIndexIfMissing('vtiger_campaign', 'idx_last_sync', 'last_sync_datetime');
$this->dropIndexIfExists('vtiger_campaign', 'idx_old');
```

### INSERT Seed Data

`vtiger_config` has only `(category, value)` — one JSON blob per category:

```php
$exists = (int) $this->db->getOne("SELECT COUNT(*) FROM vtiger_config WHERE category = ?", ['social_ads_config']);

if (!$exists) {
    $this->pquery("INSERT INTO vtiger_config (category, value) VALUES (?, ?)", [
        'social_ads_config',
        json_encode(['facebook_api_version' => 'v18.0']),
    ]);
}
```

## Helper Methods

### createCronjob

Signature, `.service` handler path, frequency/`run_by` values and `down()` cleanup: see `skill://cron` → `references/registration.md` (canonical). Migration-specific note: `createCronjob()` first deletes any `vtiger_cron_task` row with the same name, so a re-run re-registers instead of duplicating — no extra exists-guard needed.

### createPicklistValues

Signature: `createPicklistValues(string $fieldName, string $moduleName, array $options, string $idFieldName = '', bool $isOverride = false)`. Keys are lowercase snake_case. Leave `$isOverride` false: `true` deletes every existing value first.

```php
$values = [
    ['key' => 'active', 'color' => '#28a745'],
    ['key' => 'paused', 'color' => '#ffc107'],
    ['key' => 'archived', 'color' => '#6c757d'],
];
$this->createPicklistValues('cpsocialcampaign_statuses', 'CPSocialCampaign', $values);
```

## Batch Large Data

**For 1000+ rows, chunk into batches:**
```php
public function up(): int {
    $batchSize = 500;
    $offset = 0;

    while (true) {
        $sql = "SELECT * FROM old_table LIMIT ?, ?";
        $result = $this->pquery($sql, [$offset, $batchSize]);

        if ($this->db->num_rows($result) == 0) break;

        while ($row = $this->db->fetchByAssoc($result)) {
            $insert = "INSERT INTO new_table (field) VALUES (?)";
            $this->pquery($insert, [$row['value']]);
        }

        $offset += $batchSize;
    }

    return self::UP_SUCCESS;
}
```

---
name: migration
description: "VTiger DB migrations — CPMigration_Base_Model, YYYY.MM.DD naming, idempotent schema change. Use when: tạo bảng mới, alter table, đổi schema, migration; field trên module có sẵn → skill field (BFR)."
user-invocable: false
---

# VTiger Migration Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`. Template, helper list and return codes are owned by `rule://migration-rules` — this skill only adds context.

## When to Use

Use this skill when:
- Altering database schema (CREATE/ALTER/DROP tables)
- Adding/modifying columns, indexes, constraints
- Inserting seed data or initial configuration
- Creating cron jobs programmatically
- Adding picklist values during migration
- Modifying module structure requiring DB changes

## Migration File Naming

**Format:** `YYYY.MM.DD.HH.mm.ss_DescriptiveName.php`

**Examples:**
- `2025.04.16.10.30.00_AddSocialAdsModule.php`
- `2025.11.05.14.22.13_AlterCampaignsAddLastSync.php`
- `2026.02.10.09.15.00_CreateNotificationIndexes.php`

**Rules:**
- Timestamp uses DOTS not underscores
- Descriptive name in PascalCase
- Must be unique (timestamp collision = fail)

## Location

Single location: `modules/CPMigration/migrations/*.php` — `CPMigration_Migrator_Model` only globs that directory (applied files tracked in `vtiger_migration_history`).

## Anonymous Class Pattern

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @name MigrationDescriptiveName
 * @create date YYYY-MM-DD HH:mm:ss
 */

return new class extends CPMigration_Base_Model {

    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        // Forward migration logic
        return self::UP_SUCCESS;
    }

    public function down(): int {
        // Reverse migration logic (optional)
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

## Return Constants (`CPMigration_Base_Model`)

| Constant | Meaning |
|----------|---------|
| `UP_SUCCESS` | Migration executed successfully (also return it when idempotent checks find nothing to do) |
| `UP_FAILED` | Migration failed |
| `DOWN_SUCCESS` | Rollback successful |
| `DOWN_FAILED` | Rollback failed |
| `DOWN_NOT_SUPPORTED` | No rollback available |

## Helper Methods (`modules/CPMigration/models/Base.php`)

```php
$this->pquery($sql, $params);                 // throws Exception on failure
$this->multipquery([$sql1, $sql2]);           // param-less statements, wrapped in a transaction
$this->db;                                    // PearDatabase (getOne, num_rows, fetchByAssoc, getUniqueID)

// Idempotent schema guards
$this->isTableExists($table);
$this->isColumnExists($table, $column);
$this->isIndexExists($table, $indexName);
$this->addColumnIfMissing($table, $column, $definition);
$this->addIndexIfMissing($table, $indexName, $columns);
$this->dropIndexIfExists($table, $indexName);

// Feature helpers
$this->createCronjob($name, $frequencySeconds, $moduleName, $handlerFile, $runBy = 'Cronjob'); // details: skill://cron references/registration.md
$this->createPicklistValues($fieldName, $moduleName, $options, $idFieldName = '', $isOverride = false);
```

`createPicklistValues(..., $isOverride = true)` runs `DELETE FROM <picklist table>` first — it wipes every existing value. It also does not skip existing keys, so guard re-runs yourself.

## Critical Pitfalls

1. **MUST use anonymous class** — no named classes
2. **Timestamp dots not underscores** — `2025.04.16` not `2025_04_16`
3. **Header docblock** — `@author`, `@email`, `@name`, `@create date`
4. **$isRunBeforeQuickRepair = false** — set explicitly
5. **Return constants** — never return true/false
6. **down() reverse up()** — undo all changes
7. **Idempotent** — use the `is*Exists` / `*IfMissing` helpers instead of hand-rolled `SHOW COLUMNS`
8. **Batch large data** — chunked inserts for 1000+ rows

## Reference Files

- [Migration Pattern](references/migration-pattern.md) — Complete implementation guide

## Quick Example

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @name AddLastSyncDatetimeToCampaign
 * @create date 2026-02-10 14:30:00
 */

return new class extends CPMigration_Base_Model {

    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        $this->addColumnIfMissing('vtiger_campaign', 'last_sync_datetime', 'DATETIME DEFAULT NULL');
        return self::UP_SUCCESS;
    }

    public function down(): int {
        if ($this->isColumnExists('vtiger_campaign', 'last_sync_datetime')) {
            $this->pquery("ALTER TABLE vtiger_campaign DROP COLUMN last_sync_datetime");
        }

        return self::DOWN_SUCCESS;
    }
};
```

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Migration tạo bảng (tung.nguyen 2/2): `modules/CPMigration/migrations/2026.04.11.10.26.43_AddReportViewSharingTables.php`
- Migration init DB feature (Tin Bui 2/2): `modules/CPMigration/migrations/2024.12.27.11.47.26_InitDBForSocialAdsFeature.php`
- Migration đăng ký cron (Tùng Nguyễn): `modules/CPMigration/migrations/2026.06.22.12.35.00_SyncZaloGroupInfoCron.php`

## Verify

```bash
php -l <migration file>
# Chạy migration runner trên DB dev, confirm idempotent (chạy 2 lần không lỗi)
# Check schema sau chạy: mysql <db> -e "SHOW COLUMNS FROM <table>"
```

---
name: migration
description: "VTiger DB migrations — CPMigration_Base_Model, YYYY.MM.DD naming, idempotent schema change. Use when: tạo bảng mới, alter table, đổi schema, migration; field trên module có sẵn → skill field (BFR)."
user-invocable: false
---

# VTiger Migration Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

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

## Anonymous Class Pattern

```php
<?php

/**
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

## Required Methods

### up(): int
- Executes forward migration
- MUST return constant: `UP_SUCCESS`, `UP_FAILED`, `UP_ALREADY_EXECUTED`
- Use `$this->pquery($sql, $params)` for queries
- Check existence before creating (idempotent)

### down(): int
- Reverses migration (optional but recommended)
- MUST return constant: `DOWN_SUCCESS`, `DOWN_FAILED`, `DOWN_NOT_SUPPORTED`
- Reverse all changes from up()

## Return Constants

| Constant | Meaning |
|----------|---------|
| `UP_SUCCESS` | Migration executed successfully |
| `UP_FAILED` | Migration failed |
| `UP_ALREADY_EXECUTED` | Already applied, skip |
| `DOWN_SUCCESS` | Rollback successful |
| `DOWN_FAILED` | Rollback failed |
| `DOWN_NOT_SUPPORTED` | No rollback available |

## Helper Methods

**Database Operations:**
```php
$this->pquery($sql, $params);  // Prepared statement
global $adb;  // Direct DB access if needed
```

**Built-in Helpers:**
```php
$this->createCronjob($name, $frequency, $module, $handlerPath);
$this->createPicklistValues($fieldName, $module, $values, $color, $isRole);
```

## Critical Pitfalls

1. **MUST use anonymous class** — no named classes
2. **Timestamp dots not underscores** — `2025.04.16` not `2025_04_16`
3. **@name + @create date required** in docblock
4. **$isRunBeforeQuickRepair = false** — set explicitly
5. **Return constants** — never return true/false
6. **down() reverse up()** — undo all changes
7. **Check exists before create** — idempotent migrations
8. **Batch large data** — chunked inserts for 1000+ rows

## Reference Files

- [Migration Pattern](references/migration-pattern.md) — Complete implementation guide

## Quick Example

```php
<?php
/**
 * @name AddLastSyncDatetimeToCampaigns
 * @create date 2026-02-10 14:30:00
 */
return new class extends CPMigration_Base_Model {
    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        $sql = "ALTER TABLE vtiger_campaigns
                ADD COLUMN last_sync_datetime DATETIME DEFAULT NULL";
        $this->pquery($sql);
        return self::UP_SUCCESS;
    }

    public function down(): int {
        $sql = "ALTER TABLE vtiger_campaigns DROP COLUMN last_sync_datetime";
        $this->pquery($sql);
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

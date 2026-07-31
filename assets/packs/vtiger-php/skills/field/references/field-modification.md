# Field Modification & Deletion

> Workflows for deleting fields and changing UITypes on existing fields.

## Delete Field (4 Steps)

### Step 1: Migration — Remove field metadata from vtiger_field

```php
return new class extends CPMigration_Base_Model {

    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        // Step 1: Remove field metadata
        $this->pquery("DELETE FROM vtiger_field WHERE columnname = 'fieldname' AND tablename = 'vtiger_moduletable'");

        // Step 2: Drop the column
        $this->pquery("ALTER TABLE vtiger_moduletable DROP COLUMN IF EXISTS fieldname");

        return self::UP_SUCCESS;
    }

    public function down(): int {
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

**Batch delete example** (from `2025.09.29_CreateCloudeventCustomerDataTable.php`):

```php
$this->multipquery([
    "DELETE FROM vtiger_field WHERE columnname IN ('customer_name', 'mobile') AND tablename = 'vtiger_cpreceipt'",
    "ALTER TABLE vtiger_cpreceipt DROP COLUMN IF EXISTS customer_name",
    "ALTER TABLE vtiger_cpreceipt DROP COLUMN IF EXISTS mobile",
]);
```

### Step 3: Remove from BlocksAndFieldsRegister.php

Delete the field entry from the `$fields` array in `modules/{Module}/BlocksAndFieldsRegister.php`.

### Step 4: Remove Language Labels

Remove `LBL_FIELDNAME` and any related picklist value keys from:
- `languages/en_us/ModuleName.php`
- `languages/vn_vn/ModuleName.php`
- `languages/en_us/dev/ModuleName.php` (if exists)

### Post-Deletion Checklist

1. Run migration
2. Run Quick Repair
3. Verify field no longer appears in EditView/DetailView
4. Check no PHP errors from references to deleted field

---

## Change UIType (2 Steps)

> **Important:** Some UIType changes require column type changes too.

### Step 1: Migration — Update vtiger_field

```php
return new class extends CPMigration_Base_Model {

    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        $this->pquery(
            "UPDATE vtiger_field SET uitype = ?, typeofdata = ? WHERE fieldname = ? AND tablename = ?",
            [70, 'DT~O', 'token_expired_date', 'vtiger_cpadvertisingaccount']
        );

        return self::UP_SUCCESS;
    }

    public function down(): int {
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

**Real example** (from `2025.08.13_UpdateAdsAccountTokenDateUIType.php`):

```php
$this->pquery("UPDATE vtiger_field SET uitype = 70, typeofdata = 'DT~O' WHERE fieldname = 'token_expired_date'");
```

### Step 2: Update BlocksAndFieldsRegister.php

Change `uitype` and `typeofdata` values for the field entry in the `$fields` array.

### Common UIType Change Scenarios

| From | To | Also Update |
|------|----|-------------|
| 1 (text) → 16 (picklist) | `typeofdata` stays `V~O` | Create picklist values via migration |
| 1 (text) → 10 (reference) | `columntype` → `varchar(15)` | Add RelationshipsRegister entry |
| 5 (date) → 70 (datetime) | `typeofdata` → `DT~O` | May need ALTER COLUMN to `datetime` |
| 16 (picklist) → 15 (picklist+role) | No `typeofdata` change | Role-based permissions now apply |

### Post-Change Checklist

1. Run migration
2. Run Quick Repair
3. Verify field renders correctly in EditView
4. Test save with new UIType behavior

---

## Cross-References

- **`migration`** skill — migration patterns
- **`field`** SKILL.md — full UITypes table
- [field-creation.md](field-creation.md) — creating new fields

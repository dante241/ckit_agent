# Field Creation Workflow

> **NO migration needed.** BlocksAndFieldsRegister + Quick Repair auto-creates DB columns.

## Step 1: Register Field in BlocksAndFieldsRegister.php

File: `modules/{Module}/BlocksAndFieldsRegister.php`

### If new block needed — add to both `$editViewBlocks` AND `$detailViewBlocks`:

```php
'LBL_BLOCK_NAME' => array(
    'blocklabel' => 'LBL_BLOCK_NAME',
    'sequence' => '16',          // next available sequence
    'show_title' => '0',
    'visible' => '0',
    'create_view' => '0',
    'edit_view' => '0',
    'detail_view' => '0',
    'display_status' => '1',
    'iscustom' => '1'
),
```

### Add field entry to `$fields` array:

```php
'fieldname' => array(
    'columnname' => 'fieldname',
    'tablename' => 'vtiger_moduletable',
    'generatedtype' => '2',         // 1=system, 2=custom
    'uitype' => '16',               // see UIType table
    'fieldname' => 'fieldname',
    'fieldlabel' => 'LBL_FIELDNAME',
    'readonly' => '1',
    'presence' => '2',              // 2=active, 1=inactive
    'defaultvalue' => '',
    'maximumlength' => '100',
    'sequence' => '3',              // position in block
    'displaytype' => '1',           // 1=edit+detail, 2=detail only, 3=hidden
    'typeofdata' => 'V~O',         // see typeofdata table
    'quickcreate' => '1',          // 1=show, 0=required, 3=hide
    'quickcreatesequence' => '0',
    'info_type' => 'BAS',          // BAS=basic, ADV=advanced
    'masseditable' => '1',         // 1=yes, 0=no, 2=restricted
    'helpinfo' => '[]',
    'summaryfield' => '0',         // 1=show in list header
    'headerfield' => '0',          // 1=show in record header
    'isunique' => '0',
    'editview_sequence' => '3',
    'editview_presence' => '2',
    'columntype' => 'varchar(100)',
    'editview_block_name' => 'LBL_BLOCK_NAME',
    'detailview_block_name' => 'LBL_BLOCK_NAME'
),
```

### Field Properties Reference

| Property | Values | Description |
|----------|--------|-------------|
| `uitype` | See SKILL.md UITypes table | Field type |
| `displaytype` | `1`=edit+detail, `2`=detail only, `3`=hidden, `6`=system | Visibility |
| `typeofdata` | `V~M`=mandatory, `V~O`=optional, `I~O`=int, `DT~O`=datetime, `C~O`=checkbox | Validation |
| `quickcreate` | `0`=show+required, `1`=show, `3`=hide | Quick create form |
| `presence` | `2`=active (visible), `1`=inactive (hidden from layout editor) | Field status |
| `generatedtype` | `1`=system field, `2`=custom field | Origin |
| `info_type` | `BAS`=basic info, `ADV`=advanced info | Section |
| `masseditable` | `0`=no, `1`=yes, `2`=restricted | Mass edit |

### Column type mappings

| UIType | columntype |
|--------|------------|
| 1, 2 (text) | `varchar(100)` or `varchar(255)` |
| 7 (int) | `int` |
| 9 (decimal) | `decimal(25,8)` |
| 10 (reference) | `varchar(15)` |
| 15, 16 (picklist) | `varchar(100)` or `varchar(255)` |
| 19, 21 (textarea) | `text` or `mediumtext` |
| 56 (checkbox) | `varchar(3)` |
| 5 (date) | `date` |
| 50 (editable datetime) | `datetime` |
| 70 (readonly datetime) | `datetime` |
| 71 (currency) | `decimal(25,8)` |

## Step 1a: Picklist — Create Values via Migration

> **IMPORTANT:** ALWAYS ask user for dropdown values if not provided.
> Values = lowercase English, words separated by `_` (e.g., `not_synced`, `pending`, `failed`)

For **UIType 16** (no role-based permission — most common):

```php
<?php
return new class extends CPMigration_Base_Model {
    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        $this->createPicklistValues('fieldname', 'Module', [
            ['key' => 'pending', 'color' => '#ffc107'],
            ['key' => 'synced', 'color' => '#28a745'],
            ['key' => 'failed', 'color' => '#dc3545'],
        ], '', true);
        return self::UP_SUCCESS;
    }

    public function down(): int {
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

For **UIType 15** (role-based permission): same syntax, but set `uitype => '15'` in BlocksAndFieldsRegister.

**Note:** Picklist values are auto-created as language keys. Add translations in language files.

## Step 1b: Reference — Register Relationship

For **UIType 10** (related module reference):

1. Add field in BlocksAndFieldsRegister with `'uitype' => '10'`, `'columntype' => 'varchar(15)'`
2. Register relationship in `modules/{RelatedModule}/RelationshipsRegister.php`:

```php
$relationships = array(
    array(
        'leftSideModule' => 'RelatedModule',
        'rightSideModule' => 'ThisModule',
        'relationshipType' => '1:N',
        'relationshipName' => 'LBL_THISMODULE_LIST',
        'enabledActions' => array(),
        'listingFunctionName' => 'get_dependents_list',
        'leftSideReferenceFieldName' => null,
        'rightSideReferenceFieldName' => 'reference_fieldname'
    )
);
```

## Step 2: Add Language Labels

Add to `languages/en_us/ModuleName.php` AND `languages/vn_vn/ModuleName.php`:

```php
$languageStrings = array(
    // Block label (if new block)
    'LBL_BLOCK_NAME' => 'Block Display Name',

    // Field label
    'LBL_FIELDNAME' => 'Field Display Name',

    // Picklist values (if applicable)
    'pending' => 'Pending',
    'synced' => 'Synced',
    'failed' => 'Failed',
);
```

## Post-Creation Checklist

1. Run Quick Repair to sync field registry (auto-creates DB columns)
2. Verify field appears in EditView and DetailView
3. Verify picklist values (if applicable)
4. Test save and retrieve

## Cross-References

- **`language`** skill — language file structure and 3-tier loading
- **`field`** SKILL.md — UITypes table and custom layouts

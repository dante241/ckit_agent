# Field Creation Workflow

> **KHÔNG migration để tạo field.** BlocksAndFieldsRegister (BFR) + QuickRepair tạo `vtiger_field` + cột DB theo `columntype`.

## Step 1: Register Field in BlocksAndFieldsRegister.php

File: `modules/{Module}/BlocksAndFieldsRegister.php` (DEV team: `.dev.php`). Mọi giá trị là **chuỗi**.

### Block mới — thêm vào CẢ `$editViewBlocks` VÀ `$detailViewBlocks`:

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

Block thiếu key (< 9) hoặc field thiếu `fieldname` bị QuickRepair **bỏ qua im lặng**.

### Field entry — đủ 26 key:

```php
'fieldname' => array(
    'columnname' => 'fieldname',
    'tablename' => 'vtiger_moduletable',
    'generatedtype' => '2',         // 1=system, 2=custom
    'uitype' => '16',               // SKILL.md UITypes table
    'fieldname' => 'fieldname',
    'fieldlabel' => 'LBL_FIELDNAME',
    'readonly' => '1',              // legacy, giữ '1'
    'presence' => '2',              // 0=luôn hiện (không ẩn được), 2=active, 1=inactive
    'defaultvalue' => '',
    'maximumlength' => '100',
    'sequence' => '3',              // vị trí trong block DetailView
    'displaytype' => '1',           // 1=edit+detail, 2=detail only, 3=hidden
    'typeofdata' => 'V~O',          // PHẢI khớp columntype (SKILL.md)
    'quickcreate' => '2',           // 0=bắt buộc, 1=không hiện, 2=hiện, 3=không cho phép
    'quickcreatesequence' => '0',
    'info_type' => 'BAS',
    'masseditable' => '1',          // 1=yes, 0=no
    'helpinfo' => '[]',
    'summaryfield' => '0',          // 1=hiện ở list/summary
    'headerfield' => '0',           // 1=hiện ở record header
    'isunique' => '0',
    'editview_sequence' => '3',
    'editview_presence' => '2',
    'columntype' => 'varchar(255)', // LUÔN khai báo; thiếu → VARCHAR(100) (relation: INT(11))
    'editview_block_name' => 'LBL_BLOCK_NAME',
    'detailview_block_name' => 'LBL_BLOCK_NAME'
),
```

`quickcreate` theo `Vtiger_Field_Model::QUICKCREATE_*`: `1` = **KHÔNG** hiện trong Quick Create (hay nhầm là "hiện").

### uitype → typeofdata → columntype (field mới)

| UIType | typeofdata | columntype |
|--------|-----------|------------|
| 1, 2 | `V~O` / `V~M` | `varchar(100)` / `varchar(255)` |
| 4 | `V~O` | `varchar(100)` |
| 5 | `D~O` | `date` |
| 7 (số nguyên) | `I~O` | `int(11)` (`bigint` khi > 10 chữ số) |
| 7 (thập phân) | `NN~O` | `decimal(p,s)` |
| 9 | `N~O~2~2` | `decimal(5,2)` |
| **10** (reference) | **`I~O`** | **`int(11)`** |
| 11 | `V~O` | `varchar(30)` |
| 13 | `E~O` | `varchar(100)` |
| 14 | `T~O` | `time` |
| 15, 16 | `V~O` | `varchar(255)` |
| 17 | `V~O` | `varchar(255)` |
| 19 | `V~O` | `mediumtext` |
| 21, 33 | `V~O` | `text` |
| 56 | `C~O` | `varchar(3)` |
| 70 | `DT~O` | `datetime` |
| 71, 72 | `N~O` | `decimal(25,8)` |

`~M` thay `~O` khi bắt buộc. Field đã tồn tại có typeofdata chứa `M` → QuickRepair mở quyền xem/sửa cho mọi profile.

## Step 1a: Picklist — tạo values bằng migration

> ALWAYS ask user for dropdown values if not provided. Key = lowercase English, `_` (`not_synced`, `pending`).

```php
<?php

/**
 * @author Your Name
 * @email your.email@cloudgo.vn
 * @name AddFieldnamePicklistValues
 * @create date YYYY-MM-DD HH:mm:ss
 */

return new class extends CPMigration_Base_Model {

	protected $isRunBeforeQuickRepair = false;	// picklist table vtiger_<fieldname> is created by QuickRepair

	public function up(): int {
		$this->createPicklistValues('fieldname', 'Module', [
			['key' => 'pending', 'color' => '#ffc107'],
			['key' => 'synced', 'color' => '#28a745'],
			['key' => 'failed', 'color' => '#dc3545'],
		]);

		return self::UP_SUCCESS;
	}

	public function down(): int {
		return self::DOWN_NOT_SUPPORTED;
	}
};
```

- Chữ ký: `createPicklistValues(string $fieldName, string $moduleName, array $options, string $idFieldName = '', bool $isOverride = false)`. `$isOverride = true` **xoá hết values cũ** trước khi insert — chỉ dùng khi chủ đích thay toàn bộ.
- Key/color được nối thẳng vào SQL → KHÔNG chứa dấu `'`.
- `pquery()`/`multipquery()` của migration ném `Exception` khi SQL lỗi (không nuốt lỗi).
- uitype 15 (role-based): cùng cú pháp, `'uitype' => '15'` trong BFR. ⚠️ Nhánh dựng `vtiger_role2picklist` trong `createPicklistValues()` không bao giờ chạy (`multipquery()` trả `void`), và `repairRoles()` của QuickRepair chạy TRƯỚC migration "after" → role không phải admin chỉ thấy values sau **lần QuickRepair kế tiếp**. Ưu tiên uitype 16.

Thêm nhãn cho từng value key vào `$languageStrings` của module (en_us + vn_vn).

## Step 1b: Reference (UIType 10)

1. BFR: `'uitype' => '10'`, `'typeofdata' => 'I~O'`, `'columntype' => 'int(11)'`.
2. Module được tham chiếu: khai báo trong `modules/{RelatedModule}/RelationshipsRegister.php` — QuickRepair ("Repairing relationships") tạo `vtiger_fieldmodulerel` + related list từ `rightSideReferenceFieldName`:

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

## Step 2: Language Labels

`languages/en_us/{Module}.php` VÀ `languages/vn_vn/{Module}.php` (DEV team: `languages/<locale>/dev/{Module}.php`), trong `$languageStrings` (không phải `$jsLanguageStrings` — EP-002):

```php
$languageStrings = array(
    'LBL_BLOCK_NAME' => 'Block Display Name',   // block mới
    'LBL_FIELDNAME' => 'Field Display Name',
    'pending' => 'Pending',                     // picklist values
    'synced' => 'Synced',
    'failed' => 'Failed',
);
```

## Step 3: QuickRepair + Post-Creation Checklist

1. QuickRepair (web để thấy output: `index.php?module=Vtiger&parent=Settings&action=QuickRepair`) → dòng `+ New fields for <Module>: fieldname (picklist)`.
2. Migration picklist (`$isRunBeforeQuickRepair = false`) tự chạy ở cuối CÙNG lần QuickRepair đó (`runAfterQuickRepairMigrations`).
3. EditView / DetailView / QuickCreate hiện đúng; save rỗng + có giá trị (số/relation rỗng phải lưu 0, không lỗi).
4. Picklist values hiển thị đúng nhãn ở cả 2 locale.

## Cross-References

- **`language`** skill — language file structure
- **`migration`** skill — migration patterns
- `field` SKILL.md — UITypes table, QuickRepair sync rules

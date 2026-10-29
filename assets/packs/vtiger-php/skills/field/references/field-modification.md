# Field Modification & Deletion

> QuickRepair chỉ sync một phần thuộc tính của field đã tồn tại (bảng trong SKILL.md). Phần còn lại cần migration — và BFR PHẢI sửa cho khớp, nếu không DB và BFR lệch nhau.

## Thay đổi gì cần gì

| Thay đổi | BFR | Migration | Ghi chú |
|---|---|---|---|
| typeofdata, displaytype, presence, quickcreate, masseditable, summaryfield, headerfield, defaultvalue, sequence, block | ✅ | — | QuickRepair UPDATE |
| columntype nới rộng cùng loại (varchar(100)→(255), int→bigint, decimal tăng) | ✅ | — | QuickRepair `ALTER … MODIFY`, giữ NULL/DEFAULT/COLLATE/COMMENT; 1 ALTER/bảng |
| columntype varchar/text → int | ✅ | Chỉ khi có giá trị không phải số nguyên chuẩn | QuickRepair kiểm toàn bộ giá trị trước; có giá trị lạ → "Skipped …" |
| columntype thu hẹp, int→varchar, date↔datetime, đổi unsigned | ✅ | ✅ `ALTER TABLE` | QuickRepair bỏ qua; migration `$isRunBeforeQuickRepair = true` để sync thấy kiểu mới |
| uitype, fieldlabel, maximumlength, helpinfo | ✅ | ✅ `UPDATE vtiger_field` | QuickRepair không UPDATE các cột này |
| Xoá field | ✅ (xoá entry) | ✅ | Xem dưới |

## Đổi uitype / label / kiểu cột

Dựa trên migration thật `2025.05.26_UpdateSponsorDetailField.php` (text → tiền + đổi label):

```php
<?php

/**
 * @author Your Name
 * @email your.email@cloudgo.vn
 * @name UpdateSponsorDetailField
 * @create date YYYY-MM-DD HH:mm:ss
 */

return new class extends CPMigration_Base_Model {

	protected $isRunBeforeQuickRepair = true;	// column type must be final before the register sync compares it

	public function up(): int {
		$this->multipquery([
			"UPDATE vtiger_field SET uitype = 71, typeofdata = 'N~O' WHERE columnname = 'sponsorship_value' AND tablename = 'vtiger_cpsponsordetail'",
			"ALTER TABLE `vtiger_cpsponsordetail` MODIFY COLUMN `sponsorship_value` decimal(25,8) NULL DEFAULT NULL",
			"UPDATE vtiger_field SET fieldlabel = 'LBL_SPONSOR' WHERE columnname = 'related_sponsor' AND tablename = 'vtiger_cpsponsordetail'",
		]);

		return self::UP_SUCCESS;
	}

	public function down(): int {
		return self::DOWN_NOT_SUPPORTED;
	}
};
```

Rồi sửa entry BFR: `uitype`, `typeofdata`, `columntype`, `fieldlabel` giống hệt migration; thêm label mới vào en_us + vn_vn.

- `ALTER … MODIFY` thay **toàn bộ** định nghĩa cột → ghi lại NULL/DEFAULT/COLLATE cần giữ.
- Đổi sang kiểu số: dữ liệu text cũ phải được làm sạch trước (`UPDATE … SET col = NULL WHERE col = ''`), nếu không strict mode fail ALTER.
- `multipquery()` mở transaction nhưng DDL (`ALTER`) tự commit — không rollback được; mỗi câu ném `Exception` khi lỗi.

### Kịch bản hay gặp

| Từ → Sang | typeofdata | columntype | Thêm |
|---|---|---|---|
| 1 → 16 | `V~O` | giữ varchar | `createPicklistValues()` cho các giá trị đang có |
| 1 → 10 | `I~O` | `int(11)` | dữ liệu text phải là crmid hoặc NULL; RelationshipsRegister |
| 5 → 70 | `DT~O` | `datetime` | migration `ALTER` (QuickRepair không đổi date→datetime) |
| 16 → 15 | `V~O` | giữ | role-based: values hiện cho role sau QuickRepair |
| 1/V~ → 7 | `I~O` | `int(11)` | làm sạch giá trị không phải số trước |

## Xoá field

1. **Migration** — dùng vtlib để dọn `vtiger_profile2field`, `vtiger_def_org_field`, related list và cache field (raw `DELETE FROM vtiger_field` để lại rác):

```php
<?php

/**
 * @author Your Name
 * @email your.email@cloudgo.vn
 * @name DeleteCustomerNameField
 * @create date YYYY-MM-DD HH:mm:ss
 */

return new class extends CPMigration_Base_Model {

	protected $isRunBeforeQuickRepair = false;

	public function up(): int {
		$moduleInstance = Vtiger_Module::getInstance('CPReceipt');
		$field = $moduleInstance ? Vtiger_Field::getInstance('customer_name', $moduleInstance) : false;

		if ($field) {	// getInstance returns false when the field is already gone
			$field->delete();
		}

		$this->pquery('ALTER TABLE vtiger_cpreceipt DROP COLUMN IF EXISTS customer_name');

		return self::UP_SUCCESS;
	}

	public function down(): int {
		return self::DOWN_NOT_SUPPORTED;
	}
};
```

   Picklist field: thêm `DROP TABLE IF EXISTS vtiger_<fieldname>`, `vtiger_<fieldname>_seq` và xoá dòng `vtiger_picklist` / `vtiger_role2picklist` (mẫu: `2026.09.22.10.00.00_MergeUserWorkingStatusIntoIsOnline.php`).

2. **BFR** — xoá entry khỏi `$fields` ở MỌI file đang khai báo (`BlocksAndFieldsRegister.php`, `.dev.php`, `.cus.php`, `GlobalBlocksAndFieldsConfig.php`). Còn entry → QuickRepair **tạo lại** field + cột.
3. **Language** — xoá `LBL_FIELDNAME` + key picklist value ở `languages/{en_us,vn_vn}/{Module}.php` (và `dev/` nếu có).
4. Gỡ field khỏi `custom/PopupAndRelationListLayout.php`, `RelationshipsRegister.php`, code/tpl đang `get('<field>')`.

Migration và BFR đi CÙNG một commit/deploy.

## Post-change checklist

1. QuickRepair web — không có dòng "Skipped/Failed" cho cột vừa đổi.
2. `SELECT uitype, typeofdata, fieldlabel FROM vtiger_field WHERE …` + `SHOW COLUMNS` khớp BFR.
3. EditView/DetailView/ListView render đúng; save rỗng + có giá trị.
4. Xoá field: `grep -rn "<fieldname>" modules layouts languages` không còn tham chiếu.

## Cross-References

- **`migration`** skill — migration patterns
- `field` SKILL.md — QuickRepair sync rules, UITypes table
- [field-creation.md](field-creation.md) — creating new fields

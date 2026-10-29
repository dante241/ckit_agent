---
name: field
description: "VTiger fields — UIType, typeofdata/columntype, picklist, BlocksAndFieldsRegister, QuickRepair sync, EditView/DetailView/QuickCreate layout. Use when: thêm/sửa/xoá field, cột dữ liệu, picklist, dropdown, uitype, layout field; KHÔNG dùng cho schema table mới → skill migration."
user-invocable: false
---

# VTiger Fields

> Conventions: `.omp/rules/cloudgo-development-rules.md`
> Số liệu bên dưới đo từ `modules/*/BlocksAndFieldsRegister.php` ngày 2026-10-29.

## Field Creation Flow (MANDATORY — đúng 3 bước)

```
1. BFR:       modules/{Module}/BlocksAndFieldsRegister.php — thêm entry vào $fields (đủ 26 key, xem references/field-creation.md)
              (block mới thì thêm $editViewBlocks + $detailViewBlocks trước)
2. Language:  fieldlabel 'LBL_X' phải có trong CẢ languages/en_us/{Module}.php VÀ languages/vn_vn/{Module}.php ($languageStrings — EP-002)
3. Sync:      QuickRepair (Settings → Quick Repair, hoặc `php cli_tool.php -c quick_repair`) → migration "before" → tạo field + cột DB theo 'columntype' → relationships → migration "after"
```

- **CẤM migration/ALTER TABLE để TẠO field** — QuickRepair đọc BFR, tạo `vtiger_field` + cột. Migration chỉ cho: bảng không phải field, picklist values, đổi những thuộc tính QuickRepair KHÔNG sync (bảng dưới), xoá field.
- BFR được merge theo thứ tự: `BlocksAndFieldsRegister.php` (R&D) → `.dev.php` (DEV team) → `.cus.php` (Layout Editor của khách) → `GlobalBlocksAndFieldsConfig.php`. Layout Editor ghi đè file và **xoá mọi comment** trong BFR.
- Chọn uitype: tra bảng dưới, KHÔNG đoán theo VTiger docs trên mạng. Nghi ngờ → copy entry của một field cùng loại trong BFR thật.

## QuickRepair sync gì cho field ĐÃ tồn tại (`Vtiger_BlockAndField_Helper::syncFieldToDb`)

| Thuộc tính | Sửa BFR là đủ? |
|---|---|
| block, sequence, presence, editview_*, displaytype, **typeofdata**, defaultvalue, isunique, readonly, quickcreate, masseditable, summaryfield, headerfield | ✅ QuickRepair UPDATE `vtiger_field` |
| **columntype** | ⚠️ Chỉ **nới rộng** cùng loại (int→bigint, varchar(100)→varchar(255), decimal tăng precision/scale) và text→int khi MỌI giá trị là số nguyên chuẩn trong range (rỗng = NULL). Thu hẹp, int→text, date→datetime, đổi signed, PK, bảng `vtiger_crmentity` → **bỏ qua**, ghi "Skipped/Failed ..." trong output QuickRepair web |
| **uitype**, **fieldlabel**, columnname, tablename, maximumlength, helpinfo, generatedtype, info_type, quickcreatesequence | ❌ KHÔNG sync → migration `UPDATE vtiger_field` + sửa BFR cho khớp (references/field-modification.md) |

## UITypes Quick Reference (BFR thật — Count = số field đang dùng)

| UIType | Loại | typeofdata | columntype | Field mẫu | Count |
|--------|------|-----------|------------|-----------|-------|
| 1 | Text 1 dòng | `V~O`/`V~M` | varchar(100) / varchar(255) | `source` | 750 |
| 2 | Name field (bắt buộc của entity) | `V~M` | varchar(255) | `name` | 140 |
| 4 | Auto-number (xxx_no) | `V~O` | varchar(100) | `cpmasterplan_no` | 90 |
| 5 | Date | `D~O`/`D~M` | date | `collected_date` | 111 |
| 7 | Số nguyên | `I~O` | int(11) (`bigint` khi > 10 chữ số) | `attempt_count` | 114 |
| 7 | Số thập phân (Layout Editor "Decimal") | `NN~O` | decimal(p,s) | — | (trong 114) |
| 9 | Phần trăm | `N~O~2~2` | decimal(5,2) | — | 6 |
| **10** | **Reference module khác** | **`I~O`/`I~M`** | **int(11)** | `related_campaign` | **243** (241 int) |
| 11 | Phone | `V~O` | varchar(30) | `customer_phone` | 39 |
| 13 | Email | `E~O` | varchar(100) | `customer_email` | 30 |
| 14 | Time | `T~O` | time | `scheduled_send_time` | 29 |
| 15 | Picklist role-based | `V~O` | varchar(200) | `ticketcategories` | 105 |
| **16** | **Picklist thường (MẶC ĐỊNH cho dropdown)** | `V~O` | varchar(255) / varchar(100) | `users_department` | **416** |
| 17 | URL | `V~O` | varchar(255) | `website` | 24 |
| 19 | Text dài (description) | `V~O` | **mediumtext** | `description` | 147 |
| 21 | Textarea | `V~O` | text | `content` | 119 |
| 33 | Multi-select picklist (lưu `a |##| b`) | `V~O` | text | `cpagenda_fare_class` | 25 |
| 52 | User hệ thống (createdby) | `V~O` | int(11) | `createdby` | 213 |
| 53 | Owner (assigned_user_id) | `V~M` | int(11) | `assigned_user_id` | 224 |
| 56 | Checkbox (lưu `'1'`/`'0'`) | `C~O` | varchar(3) (Layout Editor) | `is_evaluated` | 208 |
| 69 | Image | `V~O` | varchar(150) | `imagename` | 16 |
| **70** | **DateTime (KHÔNG dùng 50)** | `DT~O` | datetime | `createdtime` | **253** |
| 71 | Tiền | `N~O` | decimal(25,8) | `amount` | 109 |
| 72 | Đơn giá | `N~O` | decimal(25,8) | `unit_price` | 35 |

⚠️ Sai kinh điển: dùng 50 cho datetime (50 map `reference` trong `vtiger_ws_fieldtype`; datetime là **70**) · dùng 15 cho dropdown thường (chuẩn **16**) · tưởng 19 là varchar (nó là **mediumtext**) · uitype 10 khai báo `varchar` + `V~O` (cũ, trước #27245).

## typeofdata PHẢI khớp columntype (giá trị rỗng khi save — `CRMEntity::get_column_value`)

| typeofdata | Rỗng được lưu thành | Dùng cho cột |
|---|---|---|
| `I~` / `N~` / `NN~` | `0` | int / bigint / decimal |
| uitype 5, 6, 23, 70 (`D~`/`DT~`) và 14 | `NULL` | date / datetime / time |
| còn lại (`V~`, `E~`, `C~`…) | `''` | varchar / text |

- Cột int/decimal mà typeofdata `V~` → lưu `''`: MySQL strict **fail INSERT**, non-strict lưu 0 kèm warning. Field MỚI: luôn ghép đúng (field cũ đang lệch giữ nguyên — quyết định 2026-10-29).
- Relation (10, 51, 57-59, 73, 75, 76, 78, 80, 81): khai báo **`int(11)` + `I~O`** trong BFR. vtlib `__create` tự đổi `V~` → `I~` cho cột int, nhưng QuickRepair lần sau ghi lại typeofdata từ BFR → BFR để `V~O` là ping-pong. Rỗng lưu `0`.
- Field mới KHÔNG khai báo `'columntype'` → vtlib mặc định `VARCHAR(100)` (relation: `INT(11)`). Luôn khai báo rõ.

## Picklist

- Values: `Vtiger_Util_Helper::getPickListValues('fieldname')` → `array<id, value>` (đã cache). Nhãn: `vtranslate($value, $moduleName)`.
- Tạo values: migration `createPicklistValues()` với `$isRunBeforeQuickRepair = false` (bảng `vtiger_<fieldname>` do QuickRepair tạo) — xem references/field-creation.md. Value key = lowercase English `_` (`not_synced`); hỏi user nếu chưa có danh sách.
- Dynamic picklist (giá trị từ query) — override `getPicklistValues()` trong `modules/{Module}/models/Field.php` (ref thật: `modules/CPEmployee/models/Field.php`):

```php
<?php

/**
 * @author Your Name
 * @email your.email@cloudgo.vn
 * @create date YYYY.MM.DD
 */

class {Module}_Field_Model extends Vtiger_Field_Model {

	/** @var array<string, array<string, string>> option map per field, kept for the current request */
	private static array $dynamicPicklistValues = [];

	/** @return array<string, string> option value => label */
	public function getPicklistValues(): array {
		if ($this->getName() !== 'my_dynamic_field') {
			return parent::getPicklistValues() ?: [];
		}

		return self::$dynamicPicklistValues['my_dynamic_field'] ??= self::loadMyDynamicFieldValues();
	}

	/** @return array<string, string> */
	private static function loadMyDynamicFieldValues(): array {
		$db = PearDatabase::getInstance();
		$result = $db->pquery('SELECT id, name FROM vtiger_my_table WHERE deleted = 0', []);
		if (!$result) return [];

		$values = [];

		while ($row = $db->fetchByAssoc($result)) {
			$values[(string) $row['id']] = decodeUTF8($row['name']);	// decodeUTF8: Vietnamese text from fetchByAssoc
		}

		return $values;
	}
}
```

Rules: chỉ xử lý field của mình, còn lại `parent::` · cache `static` có khoá chỉ sống trong 1 request — KHÔNG TTL giả · `pquery` có params · kiểm `$result` · chữ ký override giữ giống class cha.

## Custom Field Layouts

`modules/{Module}/custom/{EditView,DetailView,QuickCreate}.php` = mảng tĩnh `$displayParams` (`scripts`, `form.hiddenFields`, `fields.<name>.customTemplate`); `PopupAndRelationListLayout.php` = `$popupLayout` + `$relationListLayout`. Chi tiết: references/field-layout.md.

## Critical Pitfalls

1. **Không migration để tạo field**; không `new Vtiger_Field()` + `$block->addField()` (bỏ qua BFR, QuickRepair sau đó không biết field).
2. **Đổi uitype / fieldlabel** = migration `UPDATE vtiger_field` + sửa BFR (QuickRepair không sync 2 cột này).
3. **Đổi columntype ngoài phạm vi nới rộng** = migration `ALTER TABLE` (`$isRunBeforeQuickRepair = true` nếu BFR mới phụ thuộc kiểu mới) — QuickRepair chỉ báo "Skipped".
4. **typeofdata khớp columntype** (bảng trên); relation = `int(11)` + `I~O`.
5. **DateTime = 70** cho mọi trường hợp; Time-only = 14.
6. Field name snake_case; picklist field name có prefix module, số nhiều (`tour_types`).
7. Custom UIType class = `{Module}_{DataType}_UIType` chọn theo `getFieldDataType()`, KHÔNG có đăng ký uitype ≥ 1024 / Extensions.php — references/custom-uitype.md.

## References

- [field-creation.md](references/field-creation.md) — BFR entry đầy đủ, picklist, reference (**MUST READ**)
- [field-modification.md](references/field-modification.md) — đổi uitype/label/columntype, xoá field
- [field-types.md](references/field-types.md) — data type, multipicklist, reference
- [field-layout.md](references/field-layout.md) — custom EditView/DetailView layouts
- [custom-uitype.md](references/custom-uitype.md) — custom UIType class theo data type

Exemplar BFR: `modules/CPMasterPlan/BlocksAndFieldsRegister.php`.

## Verify (đủ 4 check)

```bash
# 1. Sync + field vào DB đúng uitype/typeofdata/cột:
#    Web: index.php?module=Vtiger&parent=Settings&action=QuickRepair — đọc dòng "+ New fields" và "* <Module>: Skipped/Failed ..."
#    (CLI `php cli_tool.php -c quick_repair` nuốt toàn bộ output, chỉ in "Finished!")
mysql <db> -e "SELECT fieldname,uitype,typeofdata,columnname,tablename FROM vtiger_field WHERE fieldname='<field>'"
mysql <db> -e "SHOW COLUMNS FROM <table> LIKE '<column>'"
# So với bảng UITypes + bảng typeofdata↔columntype — lệch = sai

# 2. Label có ở CẢ 2 locale, ĐÚNG array (EP-002):
for L in en_us vn_vn; do
  awk '/languageStrings = array/{a="LBL"} /jsLanguageStrings = array/{a="JS"} /<LBL_KEY>/{print FILENAME" ["a"] "$0}' languages/$L/<Module>.php
done

# 3. Không migration nào TẠO cột này:
grep -rl '<column_name>' modules/CPMigration/migrations/ && echo "❌ kiểm tra lại migration" || echo "OK"

# 4. Render + save thật (để trống field số/relation cũng phải save được):
# QuickRepair đã xoá cache Smarty (cache/templates_c/v7); sửa .tpl mà không QuickRepair thì: rm -rf cache/templates_c/v7/*
# browser: EditView đúng label tiếng Việt, đúng control; Save rỗng + có giá trị; DetailView hiển thị đúng
```

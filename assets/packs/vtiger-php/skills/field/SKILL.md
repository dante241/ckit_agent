---
name: field
description: "VTiger fields — UIType, picklist, BlocksAndFieldsRegister, EditView/DetailView/QuickCreate layout. Use when: thêm/sửa/xoá field, cột dữ liệu, picklist, dropdown, uitype, layout field; KHÔNG dùng cho schema table mới → skill migration."
user-invocable: false
---

# VTiger Field Types & UITypes

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Adding new fields to modules
- Understanding UIType field types
- Working with picklists and multipicklists
- Creating custom UITypes
- Customizing field layouts (EditView, DetailView, QuickCreate)

## Field Creation Flow (MANDATORY — đúng 3 bước, sai 1 = bug)

```
1. BFR:       modules/{Module}/BlocksAndFieldsRegister.php — thêm entry vào $fields
              (block mới thì thêm $editViewBlocks + $detailViewBlocks trước)
2. Language:  fieldlabel 'LBL_X' phải có trong CẢ languages/en_us/{Module}.php
              VÀ languages/vn_vn/{Module}.php ($languageStrings — EP-002)
3. Sync:      php cli_tool.php quick_repair  → tự tạo cột DB
```

- **CẤM viết migration/ALTER TABLE cho field** — quick_repair đọc BFR và tự tạo cột. Migration chỉ dành cho bảng KHÔNG phải field entity (bảng quan hệ, bảng config, index).
- **Chọn uitype: tra bảng dưới (build từ BFR thật của codebase), KHÔNG đoán theo VTiger docs trên mạng.** Nghi ngờ → grep 1 field cùng loại trong `modules/*/BlocksAndFieldsRegister.php` và copy entry đó.

## UITypes Quick Reference (số liệu từ BFR thật của codebase — Count = số field đang dùng)

| UIType | Loại | typeofdata | columntype | Field mẫu trong repo | Count |
|--------|------|-----------|------------|---------------------|-------|
| 1 | Text 1 dòng | `V~O`/`V~M` | varchar(100) | `source`, `tags` | 701 |
| 2 | Name field (bắt buộc của entity) | `V~M` | varchar(255) | `name` | 131 |
| 4 | Auto-number (xxx_no) | `V~O` | varchar(100) | `cpmasterplan_no` | 85 |
| 5 | Date picker | `D~O` | date | `collected_date` | 105 |
| 7 | Số nguyên | `I~O` | int | `attempt_count` | 112 |
| 10 | Reference module khác | `V~O` | varchar(100) | `related_campaign` | 235 |
| 11 | Phone | `V~O` | varchar(30) | `customer_phone` | 39 |
| 13 | Email | `E~O` | varchar(100) | `customer_email` | 28 |
| 14 | Time picker | `T~O` | time | `scheduled_send_time` | 28 |
| 15 | Picklist role-based (ít dùng) | `V~O` | varchar | `ticketcategories` | 105 |
| **16** | **Picklist thường (MẶC ĐỊNH cho dropdown)** | `V~O` | varchar(100) | `users_department` | **389** |
| 17 | URL | `V~O` | varchar(255) | `website` | 23 |
| 19 | Text dài (description) | `V~O` | **mediumtext** | `description` | 138 |
| 21 | Textarea vừa | `V~O` | text | `content`, `feedback` | 111 |
| 33 | Multi-select picklist | `V~O` | text | `cpagenda_fare_class` | 24 |
| 52 | User ref hệ thống (createdby) | `V~O` | int(19) | `createdby` | 192 |
| 53 | Owner (assigned_user_id) | `V~M` | int(19) | `assigned_user_id` | 213 |
| 56 | Checkbox boolean | `C~O` | varchar(3) | `starred` | 196 |
| 69 | Image | `V~O` | varchar | `imagename` | 16 |
| **70** | **DateTime (dùng cái này, KHÔNG dùng 50)** | `DT~O` | datetime | `createdtime` | **236** |
| 71 | Số thập phân / tiền | `N~O` | decimal(25,8) | `amount` | 109 |
| 72 | Đơn giá (currency no-symbol) | `N~O` | decimal | `unit_price` | 10 |

⚠️ Sai kinh điển: dùng 50 cho datetime (codebase dùng **70**) · dùng 15 cho dropdown thường (chuẩn là **16**) · tưởng 19 là varchar(100) (nó là **mediumtext**).

## Custom Field Layouts

### Location
```
modules/{Module}/custom/
├── EditView.php                      # Edit form — scripts, hiddenFields, field overrides
├── DetailView.php                    # Detail view — same structure as EditView
├── QuickCreate.php                   # Quick create — same structure as EditView
└── PopupAndRelationListLayout.php    # Popup/relation list — different structure
```

### EditView / DetailView / QuickCreate Pattern
Static `$displayParams` array with `scripts`, `form.hiddenFields`, `fields` keys:

```php
<?php
    $displayParams = array(
        'scripts' => '
            <script type="text/javascript" src="{vresource_url("modules/{Module}/resources/EditView.js")}"></script>
            {include file="modules/PBXManager/tpls/PhoneSelectorTemplate.tpl"}
        ',
        'form' => array(
            'hiddenFields' => '',
        ),
        'fields' => array(
            'field_name' => [
                'customTemplate' => '{include file="modules/{Module}/tpls/CustomField.tpl"}',
            ],
        ),
    );
```

### PopupAndRelationListLayout Pattern
Uses `$popupLayout` + `$relationListLayout` arrays (NOT `$displayParams`):

```php
<?php
$popupLayout = array(
    'display_fields' => array('full_name', 'account_id', 'mobile', 'email'),
    'sort_field' => '',
    'sort_order' => 'ASC'
);
$relationListLayout = array(
    'display_fields' => array('full_name', 'account_id', 'title', 'mobile', 'email'),
    'sort_field' => 'modifiedtime',
    'sort_order' => 'DESC'
);
```

## Picklist Quick Reference

```php
// Get picklist values
$values = Vtiger_Util_Helper::getPickListValues('fieldname');

// Get translated picklist value
$label = getTranslatedString($value, $moduleName);
```

## Dynamic Picklist (Custom Field_Model)

Override `getPicklistValues()` to populate dropdowns from DB queries instead of static picklist tables.

### File Location
```
modules/{Module}/models/Field.php
```

### Pattern

```php
<?php

/**
 * @author Your Name
 * @create date YYYY.MM.DD
 * Purpose: Dynamic picklist values for {Module}
 */

class {Module}_Field_Model extends Vtiger_Field_Model {

    private static $picklistCache = [];
    private const CACHE_TTL = 3600; // 1 hour

    public function getPicklistValues() {
        $fieldName = $this->getName();

        if ($fieldName == 'my_dynamic_field') {
            return $this->getCachedValues('my_dynamic_field', function () {
                return $this->getMyDynamicFieldValues();
            });
        }

        return parent::getPicklistValues();
    }

    private function getCachedValues(string $key, callable $loader): array {
        $now = time();

        if (isset(self::$picklistCache[$key]) && ($now - self::$picklistCache[$key]['time']) < self::CACHE_TTL) {
            return self::$picklistCache[$key]['data'];
        }

        $data = $loader();
        self::$picklistCache[$key] = ['data' => $data, 'time' => $now];

        return $data;
    }

    private function getMyDynamicFieldValues(): array {
        global $adb;
        $result = $adb->pquery("SELECT id, name FROM vtiger_table WHERE deleted = 0", []);
        $data = [];

        while ($row = $adb->fetchByAssoc($result)) {
            $row = decodeUTF8($row); // REQUIRED for Vietnamese text
            $data[$row['id']] = $row['name'];
        }

        return $data;
    }
}
```

### Rules

1. **Always `parent::getPicklistValues()` as fallback** — for fields not handled by your override
2. **Always `decodeUTF8()`** on `fetchByAssoc()` results — prevents garbled Vietnamese text
3. **Always `pquery()` with params** — never concatenate SQL
4. **Use static cache with TTL** — avoids repeated DB queries within same request and across short-lived requests (1h default)
5. **Delegate to helper methods** — keep `getPicklistValues()` clean, put SQL in private methods
6. **Reference:** `modules/CPEmployee/models/Field.php`

## Field Creation (2-Step Process — NO Migration Needed)

> **IMPORTANT:** Quick Repair reads BlocksAndFieldsRegister and auto-creates DB columns. NO migration needed.

1. **Register**: Add entry in `modules/{Module}/BlocksAndFieldsRegister.php` (`$editViewBlocks` + `$detailViewBlocks` if new block, then `$fields`)
2. **Language**: Add labels in both `en_us` and `vn_vn`
3. **Run Quick Repair** to sync DB

> **Picklist rule:** ALWAYS ask user for dropdown values if not provided. Values = lowercase English, words separated by `_` (e.g., `not_synced`, `pending`, `failed`)

## Critical Pitfalls

1. **NO migration for field creation** — BlocksAndFieldsRegister + Quick Repair handles it
2. **Do NOT use `new Vtiger_Field()` + `$block->addField()`** — bypasses BlocksAndFieldsRegister
3. **UIType cannot change** after field creation — delete and recreate instead
4. **Field name snake_case** — database column naming
5. **DateTime = uitype 70 cho MỌI trường hợp** (cả system timestamp lẫn editable — codebase có 0 field uitype 50; `check_in_time`/`starttime`/`accept_task_date` đều 70). Time-only picker = 14
6. **Custom layout uses static `$displayParams` array** — NOT closures/functions. PopupAndRelationListLayout uses `$popupLayout` + `$relationListLayout` instead

## References

- [field-creation.md](references/field-creation.md) — **Field creation workflow (MUST READ)**
- [field-modification.md](references/field-modification.md) — Modifying existing fields
- [field-types.md](references/field-types.md) — 35 built-in UITypes table
- [field-layout.md](references/field-layout.md) — Custom EditView/DetailView layouts
- [custom-uitype.md](references/custom-uitype.md) — Creating custom UITypes

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- BlocksAndFieldsRegister chuẩn (tung.nguyen 5/5): `modules/CPMasterPlan/BlocksAndFieldsRegister.php`

Field mới KHÔNG BAO GIỜ qua ALTER TABLE — luôn BFR + quick_repair.

## Verify (chạy đủ 4 check — thiếu 1 là chưa xong)

```bash
# 1. Sync + field vào DB đúng uitype:
php cli_tool.php quick_repair
mysql <db> -e "SELECT fieldname,uitype,typeofdata,columnname FROM vtiger_field WHERE fieldname='<field>'"
# So uitype/typeofdata với bảng Quick Reference ở trên — lệch = sai loại field

# 2. Label có ở CẢ 2 locale, ĐÚNG array (EP-002):
for L in en_us vn_vn; do
  awk '/languageStrings = array/{a="LBL"} /jsLanguageStrings = array/{a="JS"} /<LBL_KEY>/{print FILENAME" ["a"] "$0}' languages/$L/<Module>.php
done
# Kỳ vọng: mỗi locale 1 dòng [LBL]. Thiếu locale nào → label hiện raw key ở locale đó

# 3. KHÔNG có migration nào đụng cột này:
grep -rl '<column_name>' modules/CPMigration/migrations/ && echo "❌ CÓ MIGRATION THỪA — xoá đi" || echo "OK"

# 4. Render thật:
rm -f test/templates_c/*.php
# Mở EditView qua browser/chrome-devtools: field hiện đúng label tiếng Việt, đúng loại control (dropdown/date/checkbox...)
```

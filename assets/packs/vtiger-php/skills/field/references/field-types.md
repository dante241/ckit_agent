# VTiger Field Data Types

Bảng uitype → typeofdata → columntype cho field mới: SKILL.md + [field-creation.md](field-creation.md). File này nói về **data type** (quyết định UIType class, template, validator) và cách đọc/ghi từng loại.

## uitype → data type (`Vtiger_Field_Model::getFieldDataType()`)

Thứ tự quyết định:
1. Ca đặc biệt hardcode: 69 `image`, 9 `percentage`, 26 `documentsFolder`, 27 `fileLocationType`, 28 `documentsFileUpload`, 83 `productTax`, 117 `currencyList`, 53 `owner`, 255 `personName`, tên chứa `tags` → `tags`.
2. Bảng `vtiger_ws_fieldtype` (uitype → fieldtype):

| uitype | data type |
|---|---|
| 10, 50, 51, 52, 57-59, 66, 73, 75-78, 80, 81, 117, 357 | `reference` |
| 11 | `phone` |
| 13 | `email` |
| 15, 16 | `picklist` |
| 17 | `url` |
| 19, 21 | `text` |
| 33 | `multipicklist` |
| 56, 156 | `boolean` |
| 71, 72 | `currency` |

3. Không có trong bảng (1, 2, 4, 5, 7, 14, 70…) → theo **prefix typeofdata**: `V` string · `I` integer · `N`/`NN` double · `D`/`DT` date · `T` time · `E` email · `P` password. `date`/`time` mà cột thật là `datetime` → `datetime`.

Hệ quả: uitype 7 + `I~` = integer, uitype 7 + `NN~` = double; uitype 1 + `N~` = double. Sai typeofdata = sai validator + sai UIType class, không chỉ sai giá trị rỗng.

UIType class nạp theo data type: `{Module}_{Ucfirst(datatype)}_UIType` (`modules/{Module}/uitypes/{Datatype}.php`) → `Vtiger_{Datatype}_UIType` → `Vtiger_Base_UIType`. Xem [custom-uitype.md](custom-uitype.md).

## Picklist

```php
// array<id, value> from vtiger_<fieldname>, cached
$values = Vtiger_Util_Helper::getPickListValues('fieldname');

// Field model: array<value, translated label>, respects role-based (15) access
$fieldModel = Vtiger_Field_Model::getInstance('fieldname', $moduleModel);
$options = $fieldModel instanceof Vtiger_Field_Model ? ($fieldModel->getPicklistValues() ?: []) : [];

// Label
$label = vtranslate($value, $moduleName);   // tpl: {vtranslate($VALUE, $MODULE)}
```

Tạo values: migration `createPicklistValues()` ([field-creation.md](field-creation.md#step-1a-picklist--tạo-values-bằng-migration)).

## Multipicklist (uitype 33)

DB lưu các value nối bằng `' |##| '` (có khoảng trắng hai bên):

```php
// Write
$recordModel->set('cpagenda_fare_class', implode(' |##| ', $selectedValues));

// Read — '' / null must become [] before array functions
$selectedValues = array_values(array_filter(explode(' |##| ', (string) $recordModel->get('cpagenda_fare_class')), 'strlen'));

// Display ("Value 1, Value 2")
$display = $recordModel->getDisplayValue('cpagenda_fare_class');
```

## Reference (uitype 10)

Cột `int(11)`, typeofdata `I~O`; `0` = chưa chọn.

```php
$referenceId = (int) $recordModel->get('related_field');
$fieldModel = $recordModel->getModule()->getField('related_field');	// false when the field is missing
$allowedModules = $fieldModel instanceof Vtiger_Field_Model ? ($fieldModel->getReferenceList() ?: []) : [];
$referenceModule = $referenceId > 0 && isRecordExists($referenceId) ? getSalesEntityType($referenceId) : '';

// Exists + module is one the field allows + permission
if ($referenceModule !== '' && in_array($referenceModule, $allowedModules, true)
	&& Users_Privileges_Model::isPermitted($referenceModule, 'DetailView', $referenceId)) {
	$referenceModel = Vtiger_Record_Model::getInstanceById($referenceId, $referenceModule);
}

// Set
$recordModel->set('related_field', $relatedRecordId);   // int; 0/'' clears
```

Field nhiều module (Contacts/Accounts…): module thật lấy bằng `getSalesEntityType($id)`, KHÔNG `getReferenceList()[0]`.

## Field model

```php
$fieldModel = Vtiger_Field_Model::getInstance('fieldname', $moduleModel);
if (!$fieldModel instanceof Vtiger_Field_Model) return;	// field missing or inactive

$uitype = (int) $fieldModel->get('uitype');
$dataType = $fieldModel->getFieldDataType();
$isMandatory = $fieldModel->isMandatory();
$isEditable = $fieldModel->isEditable();
$isViewable = $fieldModel->isViewable();
```

## displaytype

| Giá trị | Ý nghĩa |
|---|---|
| 1 | Edit + Detail |
| 2 | Chỉ Detail (read-only) |
| 3 | Ẩn khỏi form |
| 4, 5 | Hệ thống (export vẫn lấy 1, 2, 4, 5) |
| 6 | Field hệ thống không hiển thị (vd `starred`) |

## Critical Rules

1. typeofdata quyết định data type cho uitype không có trong `vtiger_ws_fieldtype` → khai báo đúng prefix.
2. Multipicklist separator = `' |##| '`.
3. Picklist lưu **key**, hiển thị qua `vtranslate`.
4. Giá trị đọc từ record là chuỗi → ép kiểu khi gán, so sánh `===`.
5. Đổi uitype của field đã có = migration ([field-modification.md](field-modification.md)), không phải xoá/tạo lại.

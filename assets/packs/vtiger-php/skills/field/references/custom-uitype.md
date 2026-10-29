# Custom UIType (hiển thị / nhập liệu riêng cho một field)

> Codebase KHÔNG có đăng ký uitype mới (không có key `vtlib.uitype.handler.extended`, không có dải ≥ 1024). Field vẫn dùng uitype chuẩn trong BFR; hành vi riêng gắn qua **data type**.

## Cơ chế (`Vtiger_Base_UIType::getInstanceFromField`)

```
Vtiger_Field_Model::getFieldDataType()          → 'channel'
→ modules/{Module}/uitypes/Channel.php           class {Module}_Channel_UIType   (nếu file tồn tại)
→ modules/Vtiger/uitypes/Channel.php             class Vtiger_Channel_UIType
→ Vtiger_Base_UIType
```

Data type đến từ uitype / typeofdata (xem [field-types.md](field-types.md)). Muốn hành vi riêng cho MỘT field:

1. Khai báo field trong BFR bằng uitype chuẩn (vd `1`, `V~O`, `varchar(100)`).
2. Override `getFieldDataType()` trong `modules/{Module}/models/Field.php` trả data type riêng cho field đó.
3. Tạo `modules/{Module}/uitypes/{Datatype}.php` với class `{Module}_{Datatype}_UIType` (autoload theo tên).
4. Template (nếu cần) ở `layouts/v7/modules/{Module}/uitypes/` — `vtemplate_path()` tìm ở module trước rồi fallback `modules/Vtiger/`.

Ví dụ thật: `modules/CPEcommerceProduct/models/Field.php` (`channel`, `shopId`) + `modules/CPEcommerceIntegration/uitypes/Channel.php`. Chỉ đổi cách hiển thị một picklist → override class sẵn có, vd `modules/CPPayment/uitypes/Picklist.php` (`CPPayment_Picklist_UIType extends Vtiger_Picklist_UIType`).

## Field model

```php
<?php

/**
 * @author Your Name
 * @email your.email@cloudgo.vn
 * @create date YYYY.MM.DD
 */

class CPGoal_Field_Model extends Vtiger_Field_Model {

	public function getFieldDataType() {
		if ($this->getName() === 'color') return 'colorPicker';

		return parent::getFieldDataType();
	}
}
```

Giữ chữ ký override giống hệt cha: `Vtiger_Field_Model::getFieldDataType()` không tham số, không kiểu trả về.

## UIType class

`modules/CPGoal/uitypes/ColorPicker.php`:

```php
<?php

/**
 * @author Your Name
 * @email your.email@cloudgo.vn
 * @create date YYYY.MM.DD
 */

class CPGoal_ColorPicker_UIType extends Vtiger_Base_UIType {

	// Edit control: layouts/v7/modules/CPGoal/uitypes/ColorPicker.tpl
	public function getTemplateName() {
		return 'uitypes/ColorPicker.tpl';
	}

	// Detail/List text: the escaped hex value; the colour swatch belongs in the DetailView tpl + module CSS
	public function getDisplayValue($value, $record = false, $recordInstance = false) {
		$color = (string) $value;
		if (!preg_match('/^#[0-9A-Fa-f]{6}$/', $color)) return '';

		return htmlspecialchars($color, ENT_QUOTES, 'UTF-8');
	}

	// Value stored in the column
	public function getDBInsertValue($value) {
		return strtoupper(trim((string) $value));
	}
}
```

Chữ ký các method có thể override (`modules/Vtiger/uitypes/Base.php`) — **giữ đúng số tham số + default, KHÔNG thêm kiểu tham số**:

| Method | Dùng cho |
|---|---|
| `getTemplateName()` | tpl control trong EditView (mặc định `uitypes/String.tpl`) |
| `getDetailViewTemplateName()` | tpl ở Detail/Summary/Header (mặc định `uitypes/StringDetailView.tpl`) |
| `getListSearchTemplateName()` | ô tìm kiếm ở ListView |
| `getDisplayValue($value, $record = false, $recordInstance = false)` | text Detail/List |
| `getRelatedListDisplayValue($value)` | related list |
| `getEditViewDisplayValue($value)` | giá trị đổ vào input (cha dùng `htmlentities`) |
| `getDBInsertValue($value)` | giá trị trước khi lưu |

Không có hook `validate()` ở UIType: validate client-side bằng `data-validation-engine` trong tpl, server-side trong Save action/handler.

## Template

`layouts/v7/modules/CPGoal/uitypes/ColorPicker.tpl` — copy cấu trúc `layouts/v7/modules/Vtiger/uitypes/String.tpl`, KHÔNG CSS/JS inline (CSS → `modules/CPGoal/resources/*.css`, JS → `layouts/v7/modules/CPGoal/resources/Edit.js`):

```smarty
{strip}
	{assign var="FIELD_INFO" value=$FIELD_MODEL->getFieldInfo()}
	{assign var="SPECIAL_VALIDATOR" value=$FIELD_MODEL->getValidator()}
	{if (!$FIELD_NAME)}
		{assign var="FIELD_NAME" value=$FIELD_MODEL->getFieldName()}
	{/if}
	<input type="color"
		id="{$MODULE}_editView_fieldName_{$FIELD_NAME}"
		data-fieldname="{$FIELD_NAME}"
		class="inputElement color-picker"
		name="{$FIELD_NAME}"
		value="{$FIELD_MODEL->getEditViewDisplayValue($FIELD_MODEL->get('fieldvalue'))}"
		{if !empty($SPECIAL_VALIDATOR)}data-validator="{Zend_Json::encode($SPECIAL_VALIDATOR)}"{/if}
		{if $FIELD_INFO["mandatory"] eq true} data-rule-required="true" {/if}
	/>
{/strip}
```

## Verify

1. `php -l` cho Field.php + uitypes/*.php, rồi mở EditView/DetailView — sai chữ ký override chỉ lộ khi nạp class.
2. EditView: control mới hiện, save giá trị → cột DB đúng giá trị `getDBInsertValue`.
3. DetailView + ListView + related list hiển thị qua `getDisplayValue`, ký tự `<"'` được escape.

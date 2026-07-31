# VTiger Custom UITypes

## Overview

Create custom field types by extending `Vtiger_Base_UIType` and registering in `Extensions.php`.

## Handler Class Location

`modules/{Module}/uitypes/{CustomName}.php`

## Handler Class Template

```php
<?php

/**
 * Custom UIType Handler
 * @author Your Name
 * @create date YYYY-MM-DD
 */

class {Module}_{CustomName}_UIType extends Vtiger_Base_UIType {

    /**
     * Display value in DetailView and ListView
     * @return string HTML output
     */
    public function getDisplayValue(): string {
        $value = $this->get('field')->get('fieldvalue');

        if (empty($value)) {
            return '';
        }

        // Custom display logic
        return '<span class="custom-display">' . htmlspecialchars($value) . '</span>';
    }

    /**
     * Edit view input field
     * @param Vtiger_Request $request
     * @param bool $editmode
     * @return string HTML input
     */
    public function getEditViewDisplayValue(Vtiger_Request $request = null, bool $editmode = false): string {
        $fieldName = $this->get('field')->getName();
        $fieldValue = $this->get('field')->get('fieldvalue');

        // Return custom input HTML
        return '<input type="text"
                       name="' . $fieldName . '"
                       value="' . htmlspecialchars($fieldValue) . '"
                       class="inputElement custom-input"
                       data-uitype="custom" />';
    }

    /**
     * Template name for EditView (optional)
     * @return string Template path
     */
    public function getTemplateName(): string {
        return 'uitypes/CustomField.tpl';
    }

    /**
     * Get list view display value
     * @return string
     */
    public function getListViewDisplayValue(): string {
        return $this->getDisplayValue();
    }

    /**
     * Get API/export display value
     * @return mixed
     */
    public function getApiDisplayValue() {
        return $this->get('field')->get('fieldvalue');
    }

    /**
     * Validate field value before save
     * @param mixed $value
     * @return bool
     */
    public function validate($value): bool {
        // Custom validation logic
        if (empty($value)) {
            return true;  // Allow empty
        }

        // Example: Validate format
        return preg_match('/^[A-Z0-9-]+$/', $value);
    }

    /**
     * Get value for database save
     * @param mixed $value
     * @return mixed Processed value
     */
    public function getDBValue($value) {
        // Transform value before saving
        return strtoupper(trim($value));
    }
}
```

## Registration in Extensions.php

### File: `modules/{Module}/Extensions.php`

```php
<?php

return [
    // Register custom UIType handler
    'vtlib.uitype.handler.extended' => [
        '{Module}_{CustomName}_UIType_Model' => 'modules/{Module}/uitypes/{CustomName}.php',
    ],
];
```

## Custom UIType Number

### Choose UIType Number
- **Built-in**: 1-120 (reserved)
- **Custom**: Start at **1024** to avoid conflicts
- **Range**: 1024-9999 for custom UITypes

### Assign in Field Definition

```php
// In migration or field creation
$field = new Vtiger_Field();
$field->name = 'custom_field';
$field->label = 'Custom Field';
$field->uitype = 1024;  // Custom UIType number
$field->typeofdata = 'V~O';  // V=varchar, O=optional
```

## Custom Template (Optional)

### Template Location
`layouts/v7/modules/{Module}/uitypes/CustomField.tpl`

### Template Example

```smarty
{*+**************************************************************************
 * Custom UIType Template
 ****************************************************************************}
{assign var="FIELD_INFO" value=$FIELD_MODEL->getFieldInfo()}
{assign var="SPECIAL_VALIDATOR" value=$FIELD_MODEL->getValidator()}
{assign var="FIELD_NAME" value=$FIELD_MODEL->get('name')}

<div class="custom-field-container">
    <input type="text"
           name="{$FIELD_NAME}"
           value="{$FIELD_MODEL->get('fieldvalue')}"
           class="inputElement {$FIELD_INFO['class']}"
           data-validation-engine="validate[{$SPECIAL_VALIDATOR}]"
           data-fieldname="{$FIELD_NAME}"
           data-fieldtype="custom"
           {if $MODE eq 'edit'}readonly{/if} />

    <button type="button"
            class="btn btn-sm custom-picker"
            data-target="{$FIELD_NAME}">
        <i class="fa fa-search"></i>
    </button>
</div>
```

## JavaScript Handler (Optional)

### Location
`layouts/v7/modules/{Module}/resources/Edit.js`

### JavaScript Pattern

```javascript
CustomView_BaseController_Js('Module_Edit_Js', {}, {

    registerEvents: function() {
        this._super();
        this.registerCustomUITypeHandlers();
    },

    registerCustomUITypeHandlers: function() {
        var container = this.getEditViewContainer();

        // Custom UIType button handler
        container.find('.custom-picker').on('click', function() {
            var target = jQuery(this).data('target');
            var input = container.find('[name="' + target + '"]');

            // Custom picker logic
            app.helper.showModal({
                url: 'index.php?module=Module&view=CustomPicker',
                cb: function(modal) {
                    modal.find('.select-item').on('click', function() {
                        var value = jQuery(this).data('value');
                        input.val(value).trigger('change');
                        app.helper.hideModal();
                    });
                }
            });
        });

        // Custom validation
        container.find('[data-fieldtype="custom"]').on('blur', function() {
            var value = jQuery(this).val();

            if (!self.validateCustomFormat(value)) {
                app.helper.showErrorNotification({
                    message: 'Invalid format'
                });
            }
        });
    },

    validateCustomFormat: function(value) {
        // Custom validation logic
        return /^[A-Z0-9-]+$/.test(value);
    }
});
```

## Run Quick Repair

After registering custom UIType in Extensions.php:

1. Navigate to **Settings → Module Manager → Module Tools**
2. Click **Quick Repair**
3. System will register the custom UIType handler

## Complete Example: Color Picker UIType

### Handler: `modules/CPGoal/uitypes/ColorPicker.php`

```php
<?php

class CPGoal_ColorPicker_UIType extends Vtiger_Base_UIType {

    public function getDisplayValue(): string {
        $value = $this->get('field')->get('fieldvalue');

        if (empty($value)) {
            return '';
        }

        return '<span class="color-badge" style="background-color: ' . htmlspecialchars($value) . ';">'
             . htmlspecialchars($value) . '</span>';
    }

    public function getEditViewDisplayValue(Vtiger_Request $request = null, bool $editmode = false): string {
        $fieldName = $this->get('field')->getName();
        $fieldValue = $this->get('field')->get('fieldvalue');

        return '<input type="color"
                       name="' . $fieldName . '"
                       value="' . htmlspecialchars($fieldValue) . '"
                       class="inputElement color-picker" />';
    }

    public function validate($value): bool {
        // Validate hex color
        return preg_match('/^#[0-9A-Fa-f]{6}$/', $value);
    }

    public function getDBValue($value) {
        return strtoupper($value);  // Store as uppercase
    }
}
```

### Register in Extensions.php

```php
return [
    'vtlib.uitype.handler.extended' => [
        'CPGoal_ColorPicker_UIType_Model' => 'modules/CPGoal/uitypes/ColorPicker.php',
    ],
];
```

### Create Field in Migration

```php
$field = new Vtiger_Field();
$field->name = 'color';
$field->label = 'Color';
$field->table = 'vtiger_cpgoal';
$field->column = 'color';
$field->columntype = 'VARCHAR(7)';
$field->uitype = 1025;  // Custom UIType number
$field->typeofdata = 'V~O';
$block->addField($field);
```

## Key Methods Reference

| Method | Return Type | Purpose |
|--------|-------------|---------|
| `getDisplayValue()` | string | DetailView/ListView display |
| `getEditViewDisplayValue()` | string | EditView input field |
| `getTemplateName()` | string | Custom template path (optional) |
| `getListViewDisplayValue()` | string | ListView display |
| `getApiDisplayValue()` | mixed | API/export value |
| `validate()` | bool | Validate before save |
| `getDBValue()` | mixed | Transform for DB storage |

## Critical Pitfalls

1. **UIType number unique** — Use 1024+ for custom types
2. **Run Quick Repair** after Extensions.php changes
3. **Class naming**: `{Module}_{Name}_UIType` (NOT UIType_Model suffix)
4. **Return type declarations** on all methods (PHP 7+)
5. **Always htmlspecialchars** in display methods for XSS protection
6. **Template path relative** to `layouts/v7/modules/`

# VTiger Form Validation

## JavaScript Validation Pattern

### Using form.vtValidate() + form.valid()

`form.vtValidate()` initialises jQuery Validate on the form and returns the validator object — always truthy, so never use it as the pass/fail check. Ask `form.valid()` (core does this, e.g. `layouts/v7/modules/Vtiger/resources/Detail.js`).

```javascript
registerFormSubmit: function() {
    var self = this;
    var form = this.getForm();

    form.vtValidate();

    form.on('submit', function(e) {
        e.preventDefault();  // CRITICAL: Prevent page reload

        if (!form.valid()) {
            return false;
        }

        self.saveForm(form);
        return false;
    });
},

saveForm: function(form) {
    app.helper.showProgress();

    var params = form.serializeFormData();

    app.request.post({ data: params }).then(function(error, data) {
        app.helper.hideProgress();

        if (error) {
            app.helper.showErrorNotification({
                message: error.message
            });
            return;
        }

        app.helper.showSuccessNotification({
            message: data.message
        });

        window.location.href = data.redirect_url;
    });
}
```

## Validation Attributes (v7: jQuery Validate `data-rule-*`)

v7 `vtValidate` is jQuery Validate: rules come from `data-rule-<method>` attributes (e.g. `layouts/v7/modules/Vtiger/AddCommentForm.tpl`). The v6 `data-validation-engine="validate[...]"` syntax is legacy — don't use it in new templates.

### Basic Attributes
```smarty
{* Required field *}
<input type="text" name="field_name" class="inputElement" data-rule-required="true" />

{* Email *}
<input type="text" name="email" class="inputElement" data-rule-required="true" data-rule-email="true" />

{* Integer *}
<input type="text" name="quantity" class="inputElement" data-rule-WholeNumber="true" />

{* Positive number / currency *}
<input type="text" name="amount" class="inputElement" data-rule-positive="true" data-rule-currency="true" />

{* URL *}
<input type="text" name="website" class="inputElement" data-rule-url="true" />
```

## Built-in Rules

Methods registered in `layouts/v7/modules/Vtiger/resources/validation.js` (`jQuery.validator.addMethod`) plus jQuery Validate's own (`required`, `minlength`, `maxlength`, `min`, `max`, …). Commonly used in v7 templates:

```smarty
data-rule-required="true"
data-rule-email="true"
data-rule-url="true"
data-rule-WholeNumber="true"
data-rule-positive="true"
data-rule-currency="true"
data-rule-date="true"
data-rule-time="true"
data-rule-illegal="true"
data-rule-asciiOnly="true"
data-rule-maxlength="50"
```

Note: the v7 `phone` method always returns true — it validates nothing.

## Custom Rule

### Add Custom Rule
```javascript
jQuery.validator.addMethod('upperCode', function(value, element) {
    return this.optional(element) || /^[A-Z0-9-]+$/.test(value);
}, app.vtranslate('JS_ONLY_UPPERCASE_NUMBERS_HYPHENS'));
```

### Use in Template
```smarty
<input type="text" name="code" class="inputElement" data-rule-required="true" data-rule-upperCode="true" />
```

## Real-Time Validation

### Validate on Blur
```javascript
registerFieldValidation: function() {
    var container = this.getEditViewContainer();

    container.find('.inputElement').on('blur', function() {
        var field = jQuery(this);
        var value = field.val();

        // Custom validation logic
        if (field.attr('name') === 'goal_value') {
            if (parseFloat(value) <= 0) {
                vtUtils.showValidationMessage(field, app.vtranslate('JS_VALUE_MUST_BE_POSITIVE'));
                return false;
            }

            vtUtils.hideValidationMessage(field);
        }
    });
}
```

### Clear Validation Message
```javascript
vtUtils.hideValidationMessage(field);
```

## Backend Validation

### Save Action Validation
```php
<?php

class CPGoal_Save_Action extends Vtiger_Save_Action {

    public function process(Vtiger_Request $request) {
        // Validate input
        $validation = $this->validateInput($request);

        if (!$validation['success']) {
            $response = new Vtiger_Response();
            $response->setError($validation['message']);
            $response->emit();
            return;
        }

        // Continue with save
        parent::process($request);
    }

    protected function validateInput(Vtiger_Request $request): array {
        $goalName = trim($request->get('goal_name'));
        $targetValue = (float) $request->get('target_value');

        // Required field check
        if (empty($goalName)) {
            return [
                'success' => false,
                'message' => vtranslate('LBL_GOAL_NAME_REQUIRED', 'CPGoal')
            ];
        }

        // Value range check
        if ($targetValue <= 0) {
            return [
                'success' => false,
                'message' => vtranslate('LBL_TARGET_VALUE_INVALID', 'CPGoal')
            ];
        }

        // Duplicate check
        if ($this->isDuplicateGoalName($goalName, (int) $request->get('record'))) {
            return [
                'success' => false,
                'message' => vtranslate('LBL_DUPLICATE_GOAL_NAME', 'CPGoal')
            ];
        }

        return ['success' => true];
    }

    protected function isDuplicateGoalName(string $name, int $excludeId = 0): bool {
        global $adb;

        $sql = "SELECT COUNT(*) AS count
                FROM vtiger_cpgoal g
                INNER JOIN vtiger_crmentity e ON e.crmid = g.cpgoalid
                WHERE e.deleted = 0
                AND g.goal_name = ?
                AND g.cpgoalid != ?";

        $result = $adb->pquery($sql, [$name, $excludeId]);
        $row = $adb->fetchByAssoc($result);

        return (int) $row['count'] > 0;
    }
}
```

## Dynamic Table Validation

### Add/Remove Rows Pattern
```javascript
registerDynamicTableEvents: function() {
    var self = this;
    var container = this.getEditViewContainer();

    // Add row
    container.on('click', '.add-row', function() {
        var table = jQuery(this).closest('table');
        var newRow = table.find('tbody tr:first').clone();

        // Clear values
        newRow.find('input').val('');
        newRow.find('select').val('');

        table.find('tbody').append(newRow);
        self.calculateTotal();
    });

    // Remove row
    container.on('click', '.remove-row', function() {
        var row = jQuery(this).closest('tr');
        var table = row.closest('table');

        if (table.find('tbody tr').length > 1) {
            row.remove();
            self.calculateTotal();
        } else {
            app.helper.showErrorNotification({
                message: app.vtranslate('JS_AT_LEAST_ONE_ROW_REQUIRED')
            });
        }
    });

    // Calculate on change
    container.on('change', '.item-quantity, .item-price', function() {
        self.calculateRowTotal(jQuery(this).closest('tr'));
        self.calculateTotal();
    });
},

calculateRowTotal: function(row) {
    var quantity = parseFloat(row.find('.item-quantity').val()) || 0;
    var price = parseFloat(row.find('.item-price').val()) || 0;
    var total = quantity * price;

    row.find('.item-total').text(total.toFixed(2));
},

calculateTotal: function() {
    var grandTotal = 0;

    jQuery('.item-total').each(function() {
        grandTotal += parseFloat(jQuery(this).text()) || 0;
    });

    jQuery('#grand-total').text(grandTotal.toFixed(2));
}
```

## Conditional Validation

### Show/Hide Fields with Validation
```javascript
registerGoalTypeChange: function() {
    var form = this.getForm();

    form.find('[name="goal_type"]').on('change', function() {
        var goalType = jQuery(this).val();
        var revenueField = form.find('[name="revenue_target"]').closest('.fieldRow');

        if (goalType === 'Revenue') {
            revenueField.show();
            // Make required (form.vtValidate() must have run)
            form.find('[name="revenue_target"]').rules('add', { required: true, positive: true });
        }
        else {
            revenueField.hide();
            // Remove required
            form.find('[name="revenue_target"]').rules('remove', 'required positive');
        }
    }).trigger('change');
}
```

## Validation Messages

### Custom Error Messages
```javascript
// Show an error message on a field
vtUtils.showValidationMessage(field, app.vtranslate('JS_TARGET_VALUE_INVALID'));

// Hide it
vtUtils.hideValidationMessage(field);
```

### Translation Keys
```php
// languages/en_us/CPGoal.php
$jsLanguageStrings = array(
    'JS_GOAL_NAME_REQUIRED' => 'Goal name is required',
    'JS_TARGET_VALUE_INVALID' => 'Target value must be greater than 0',
    'JS_DUPLICATE_GOAL_NAME' => 'Goal name already exists',
    'JS_AT_LEAST_ONE_ROW_REQUIRED' => 'At least one row is required',
);
```

## Serialize Form Data

### Standard Serialization
```javascript
var params = form.serializeFormData();
// Returns object: { field1: value1, field2: value2, ... }
```

### Custom Serialization
```javascript
var params = {
    module: 'CPGoal',
    action: 'Save',
    goal_name: form.find('[name="goal_name"]').val(),
    target_value: form.find('[name="target_value"]').val(),
    items: []
};

// Serialize table rows
form.find('tbody tr').each(function() {
    var row = jQuery(this);
    params.items.push({
        name: row.find('.item-name').val(),
        quantity: row.find('.item-quantity').val(),
        price: row.find('.item-price').val()
    });
});
```

## Critical Pitfalls

1. **e.preventDefault()** on form submit — prevents page reload
2. **form.vtValidate()** to set up validation, then **form.valid()** before save — vtValidate's return value is always truthy
3. **Backend validation always** — never trust client-side only
4. **Clear validation prompts** when fixing errors
5. **Dynamic fields** — update validation rules when showing/hiding
6. **At least one row** in dynamic tables — prevent empty submission
7. **Type cast values** — parseFloat/parseInt before calculations

# VTiger Form Validation

## JavaScript Validation Pattern

### Using form.vtValidate()

```javascript
registerFormSubmit: function() {
    var self = this;
    var form = this.getForm();

    form.on('submit', function(e) {
        e.preventDefault();  // CRITICAL: Prevent page reload

        // Validate form
        if (!form.vtValidate()) {
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

## HTML5 Validation Attributes

### Basic Attributes
```smarty
{* Required field *}
<input type="text"
       name="field_name"
       class="inputElement"
       required
       data-validation-engine="validate[required]" />

{* Email validation *}
<input type="email"
       name="email"
       class="inputElement"
       data-validation-engine="validate[required,custom[email]]" />

{* Number validation *}
<input type="number"
       name="quantity"
       class="inputElement"
       min="1"
       max="100"
       step="1"
       data-validation-engine="validate[required,custom[integer]]" />

{* URL validation *}
<input type="url"
       name="website"
       class="inputElement"
       data-validation-engine="validate[custom[url]]" />

{* Pattern validation *}
<input type="text"
       name="phone"
       class="inputElement"
       pattern="[0-9]{10}"
       data-validation-engine="validate[custom[phone]]" />
```

## Data Validation Engine Rules

### Built-in Rules
```smarty
{* Required *}
data-validation-engine="validate[required]"

{* Email *}
data-validation-engine="validate[required,custom[email]]"

{* Number (integer) *}
data-validation-engine="validate[custom[integer]]"

{* Number (decimal) *}
data-validation-engine="validate[custom[number]]"

{* Phone *}
data-validation-engine="validate[custom[phone]]"

{* URL *}
data-validation-engine="validate[custom[url]]"

{* Min/Max length *}
data-validation-engine="validate[minSize[5],maxSize[50]]"

{* Min/Max value *}
data-validation-engine="validate[min[0],max[100]]"

{* Multiple rules *}
data-validation-engine="validate[required,custom[email],maxSize[100]]"
```

## Custom Regex Validation

### Add Custom Rule
```javascript
jQuery.validationEngineLanguage.allRules.customRule = {
    regex: /^[A-Z0-9-]+$/,
    alertText: "* Only uppercase letters, numbers, and hyphens allowed"
};
```

### Use in Template
```smarty
<input type="text"
       name="code"
       class="inputElement"
       data-validation-engine="validate[required,custom[customRule]]" />
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
                field.validationEngine('showPrompt', 'Value must be greater than 0', 'error');
                return false;
            }
        }
    });
}
```

### Clear Validation Prompt
```javascript
field.validationEngine('hide');
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
        if ($this->isDuplicateGoalName($goalName, $request->get('record'))) {
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
            // Make required
            form.find('[name="revenue_target"]').attr('data-validation-engine', 'validate[required,custom[number]]');
        } else {
            revenueField.hide();
            // Remove required
            form.find('[name="revenue_target"]').attr('data-validation-engine', '');
        }
    }).trigger('change');
}
```

## Validation Messages

### Custom Error Messages
```javascript
// Show custom error
field.validationEngine('showPrompt', 'Custom error message', 'error');

// Show warning
field.validationEngine('showPrompt', 'Warning message', 'pass');

// Hide prompt
field.validationEngine('hide');
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
2. **form.vtValidate()** before save — triggers validation engine
3. **Backend validation always** — never trust client-side only
4. **Clear validation prompts** when fixing errors
5. **Dynamic fields** — update validation rules when showing/hiding
6. **At least one row** in dynamic tables — prevent empty submission
7. **Type cast values** — parseFloat/parseInt before calculations

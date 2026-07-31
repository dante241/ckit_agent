# VTiger Custom Field Layouts

## Overview

Custom field layouts customize how fields appear in EditView, DetailView, QuickCreate, and Popup/RelationList views. Uses static `$displayParams` array pattern (NOT closures/functions).

## Location

```
modules/{Module}/custom/
├── EditView.php                      # Edit form customization
├── DetailView.php                    # Detail view customization
├── QuickCreate.php                   # Quick create form (same structure as EditView)
└── PopupAndRelationListLayout.php    # Popup selector & related list columns
```

## EditView / DetailView / QuickCreate

All three share the same `$displayParams` structure.

### File: `modules/{Module}/custom/EditView.php`

```php
<?php
    $displayParams = array(
        'scripts' => '
            <script type="text/javascript" src="{vresource_url("modules/{Module}/resources/EditView.js")}"></script>
        ',
        'form' => array(
            'hiddenFields' => '

            ',
        ),
        'fields' => array(

        ),
    );
```

### Structure Keys

| Key | Type | Purpose |
|-----|------|---------|
| `scripts` | string | Smarty template string — JS `<script>` tags and `{include}` for TPL partials |
| `form.hiddenFields` | string | Hidden `<input>` fields injected into the form |
| `fields` | array | Per-field customization, keyed by field name |

### Per-Field Customization (`fields` key)

Override rendering of specific fields using `customTemplate`:

```php
'fields' => array(
    'tax_code' => [
        'customTemplate' => '{include file="modules/Accounts/tpls/TaxCodeEditView.tpl"}',
    ],
    'dw_accounts_info' => [
        'customTemplate' => '{include file="modules/CPDWIntegration/tpls/CustomerAccountList.tpl"}',
    ],
),
```

### Including Scripts and TPL Partials

```php
'scripts' => '
    <script type="text/javascript" src="{vresource_url("modules/Contacts/resources/EditView.js")}"></script>
    <script type="text/javascript" src="{vresource_url("resources/ChatBotHandler.js")}"></script>
    <script type="text/javascript" src="{vresource_url("modules/PBXManager/resources/RecordingPopup.js")}"></script>

    {include file="modules/PBXManager/tpls/PhoneSelectorTemplate.tpl"}
',
```

> **Note:** `scripts` is a Smarty template string — use `{vresource_url()}` for JS paths and `{include}` for TPL partials.

### Real Examples

**Contacts EditView** — JS include only:
```php
<?php
    $displayParams = array(
        'scripts' => '
            <script type="text/javascript" src="{vresource_url("modules/Contacts/resources/EditView.js")}"></script>
        ',
        'form' => array(
            'hiddenFields' => '

            ',
        ),
        'fields' => array(

        ),
    );
```

**Contacts DetailView** — Multiple scripts + TPL partial + field customTemplate:
```php
<?php
    // Added by Hieu Nguyen on 2019-12-30
    $displayParams = array(
        'scripts' => '
            <script type="text/javascript" src="{vresource_url("resources/ChatBotHandler.js")}"></script>
            <script type="text/javascript" src="{vresource_url("modules/PBXManager/resources/RecordingPopup.js")}"></script>
            <script type="text/javascript" src="{vresource_url("modules/CPMauticIntegration/resources/MauticHistory.js")}"></script>
            <script type="text/javascript" src="{vresource_url("modules/CPEventRegistration/resources/EventRegistrationHelper.js")}"></script>

            {include file="modules/PBXManager/tpls/PhoneSelectorTemplate.tpl"}
        ',
        'form' => array(
            'hiddenFields' => '

            ',
        ),
        'fields' => array(
            // Added by Vu Mai on 2024-07-31
            'dw_accounts_info' => array(
                'customTemplate' => '{include file="modules/CPDWIntegration/tpls/CustomerAccountList.tpl"}',
            ),
        ),
    );
```

## PopupAndRelationListLayout

**Completely different structure** — uses `$popupLayout` and `$relationListLayout` arrays.

### File: `modules/{Module}/custom/PopupAndRelationListLayout.php`

```php
<?php

/* System auto-generated on YYYY-MM-DD HH:mm:ss.  */

$popupLayout = array(
    'display_fields' => array(
        'full_name',
        'account_id',
        'mobile',
        'email',
        'title',
        'assigned_user_id'
    ),
    'sort_field' => '',
    'sort_order' => 'ASC'
);

$relationListLayout = array(
    'display_fields' => array(
        'salutationtype',
        'full_name',
        'account_id',
        'title',
        'mobile',
        'email',
        'phone',
        'purchase_status',
        'remark',
        'assigned_user_id'
    ),
    'sort_field' => '',
    'sort_order' => 'ASC'
);
```

### Structure

| Variable | Key | Type | Purpose |
|----------|-----|------|---------|
| `$popupLayout` | `display_fields` | array | Field names shown in popup selector |
| `$popupLayout` | `sort_field` | string | Default sort column (empty = default) |
| `$popupLayout` | `sort_order` | string | `ASC` or `DESC` |
| `$relationListLayout` | `display_fields` | array | Field names shown in related lists |
| `$relationListLayout` | `sort_field` | string | Default sort column |
| `$relationListLayout` | `sort_order` | string | `ASC` or `DESC` |

> **Tip:** Often auto-generated by system. Manually edit to control which columns appear.

## Layout Editor (Admin UI)

**Admin -> Module Manager -> Module -> Layout Editor**

- Drag-and-drop field reordering
- Block creation and management
- Field visibility control
- No code required for basic layout changes

## Conditional Field Display (JavaScript)

For dynamic show/hide based on user interaction, use JS controller:

**Location:** `layouts/v7/modules/{Module}/resources/Edit.js`

```javascript
registerFieldDependency: function() {
    var self = this;
    var form = this.getForm();

    form.find('[name="goal_type"]').on('change', function() {
        var goalType = jQuery(this).val();
        var revenueField = form.find('[name="revenue_target"]').closest('.fieldRow');

        if (goalType === 'Revenue') {
            revenueField.show();
        } else {
            revenueField.hide();
        }
    }).trigger('change');
}
```

## Critical Rules

1. **Static `$displayParams` array** — NOT closure/function pattern
2. **`scripts` is a Smarty template string** — use `{vresource_url()}` and `{include}`
3. **`customTemplate` uses Smarty `{include}`** — path relative to `layouts/v7/` or `modules/`
4. **PopupAndRelationListLayout is separate** — uses `$popupLayout` + `$relationListLayout`, NOT `$displayParams`
5. **QuickCreate uses same structure as EditView** — identical `$displayParams` format
6. **JavaScript** for dynamic show/hide — in `layouts/v7/modules/{Module}/resources/Edit.js`
7. **Layout Editor** for simple reordering — custom files for scripts, templates, field overrides

## Common Use Cases

| Use Case | File | Method |
|----------|------|--------|
| Add JS to EditView | EditView.php | `scripts` key with `<script>` tag |
| Add JS to DetailView | DetailView.php | `scripts` key with `<script>` tag |
| Include TPL partial | EditView/DetailView.php | `scripts` key with `{include}` |
| Custom field template | EditView/DetailView.php | `fields.{name}.customTemplate` |
| Hidden form fields | EditView.php | `form.hiddenFields` |
| Popup columns | PopupAndRelationListLayout.php | `$popupLayout.display_fields` |
| Related list columns | PopupAndRelationListLayout.php | `$relationListLayout.display_fields` |
| Dynamic show/hide | Edit.js | jQuery event handler |

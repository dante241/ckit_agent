# Smarty Templates

> VTiger template syntax, variable assignment, and rendering patterns

## Template Location

```
layouts/v7/modules/<Module>/<TemplateName>.tpl
```

### Example Paths

```
layouts/v7/modules/Contacts/CustomView.tpl
layouts/v7/modules/Contacts/CustomModal.tpl
layouts/v7/modules/CPGoal/Config.tpl
layouts/v7/modules/Products/CheckWarranty.tpl
```

### Custom View Templates (CustomView_Base_View)

Templates for custom views live in module directory, NOT layouts:
```
modules/<Module>/tpls/<ViewName>.tpl
```

Rendered with `display()` (absolute path) instead of `view()` (dot notation):
```php
// Custom view — absolute path
$viewer->display('modules/CPSocialIntegration/tpls/SocialConfig.tpl');

// Core view — module name resolves to layouts/v7/
$viewer->view('Detail.tpl', $moduleName);
```

### Partials (Reusable Components)

```
layouts/v7/modules/<Module>/Partials/<PartialName>.tpl
```

## Variable Assignment in PHP

```php
<?php

class Contacts_CustomView_View extends Vtiger_Index_View {

    public function process(Vtiger_Request $request): void {
        $viewer = $this->getViewer($request);

        // Assign scalar values
        $viewer->assign('MODULE', 'Contacts');
        $viewer->assign('RECORD_ID', 123);
        $viewer->assign('PAGE_TITLE', 'Custom View');

        // Assign arrays
        $viewer->assign('ITEMS', ['Item 1', 'Item 2', 'Item 3']);
        $viewer->assign('USER_DATA', [
            'name' => 'John Doe',
            'email' => 'john@example.com'
        ]);

        // Assign objects
        $recordModel = Vtiger_Record_Model::getInstanceById(123, 'Contacts');
        $viewer->assign('RECORD', $recordModel);

        // Render template (MUST pass module name)
        $viewer->view('CustomView.tpl', 'Contacts');
    }
}
```

## Smarty Syntax

### Variable Output

```smarty
{* Simple variable *}
<h1>{$PAGE_TITLE}</h1>

{* Array access *}
<p>{$USER_DATA.name}</p>
<p>{$USER_DATA.email}</p>

{* Array by index *}
<p>{$ITEMS[0]}</p>

{* Object method call *}
<p>{$RECORD->getName()}</p>
<p>{$RECORD->get('firstname')}</p>
<p>{$RECORD->getId()}</p>

{* Escaped HTML (default) *}
<div>{$USER_INPUT}</div>

{* Unescaped HTML (use with caution) *}
<div>{$HTML_CONTENT nofilter}</div>
```

### Control Structures

#### Conditional (if/else)

```smarty
{if $RECORD_ID > 0}
    <p>Editing record #{$RECORD_ID}</p>
{else}
    <p>Creating new record</p>
{/if}

{* With elseif *}
{if $STATUS eq 'active'}
    <span class="badge badge-success">Active</span>
{elseif $STATUS eq 'inactive'}
    <span class="badge badge-warning">Inactive</span>
{else}
    <span class="badge badge-secondary">Unknown</span>
{/if}

{* Logical operators *}
{if $USER->isAdmin() && $RECORD_ID > 0}
    <button class="btn btn-danger">Delete</button>
{/if}

{if empty($ERRORS)}
    <p>No errors found</p>
{/if}
```

#### Loops (foreach)

```smarty
{* Array loop *}
<ul>
{foreach $ITEMS as $ITEM}
    <li>{$ITEM}</li>
{/foreach}
</ul>

{* Array with index *}
<ul>
{foreach $ITEMS as $INDEX => $ITEM}
    <li>{$INDEX + 1}. {$ITEM}</li>
{/foreach}
</ul>

{* Object array loop *}
<table class="table">
{foreach $RECORDS as $RECORD}
    <tr>
        <td>{$RECORD->get('firstname')}</td>
        <td>{$RECORD->get('lastname')}</td>
        <td>{$RECORD->get('email')}</td>
    </tr>
{/foreach}
</table>

{* Empty check *}
{if !empty($RECORDS)}
    {foreach $RECORDS as $RECORD}
        <div>{$RECORD->getName()}</div>
    {/foreach}
{else}
    <p>No records found</p>
{/if}
```

### Translation

```smarty
{* Basic translation *}
{vtranslate('LBL_SAVE', $MODULE)}

{* With variables (sprintf-style) *}
{vtranslate('LBL_RECORDS_SELECTED', $MODULE, $COUNT)}

{* Translation in attributes *}
<button title="{vtranslate('LBL_SAVE', $MODULE)}">
    {vtranslate('LBL_SAVE', $MODULE)}
</button>

{* JavaScript translation (in template) *}
<script>
var message = app.vtranslate('JS_SAVE_SUCCESS');
</script>
```

### Comments

```smarty
{* Single line comment *}

{*
   Multi-line
   comment
   block
*}

{* Comments are not rendered in HTML output *}
```

## Complete Template Example

```smarty
{*
 * CustomView.tpl
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 *}

<div class="custom-view-container" id="customViewContainer" data-module="{$MODULE}">

    {* Header *}
    <div class="custom-view-header">
        <h2>{vtranslate('LBL_CUSTOM_VIEW', $MODULE)}</h2>

        {if $RECORD_ID > 0}
            <span class="record-id">#{$RECORD_ID}</span>
        {/if}
    </div>

    {* Record Details *}
    {if !empty($RECORD)}
        <div class="record-details">
            <div class="row">
                <div class="col-md-6">
                    <label>{vtranslate('LBL_FIRSTNAME', $MODULE)}:</label>
                    <span>{$RECORD->get('firstname')}</span>
                </div>
                <div class="col-md-6">
                    <label>{vtranslate('LBL_LASTNAME', $MODULE)}:</label>
                    <span>{$RECORD->get('lastname')}</span>
                </div>
            </div>
        </div>
    {/if}

    {* Custom Data List *}
    {if !empty($CUSTOM_DATA)}
        <div class="custom-data-list">
            <h3>{vtranslate('LBL_CUSTOM_DATA', $MODULE)}</h3>
            <table class="table table-bordered">
                <thead>
                    <tr>
                        <th>{vtranslate('LBL_NAME', $MODULE)}</th>
                        <th>{vtranslate('LBL_VALUE', $MODULE)}</th>
                        <th>{vtranslate('LBL_STATUS', $MODULE)}</th>
                    </tr>
                </thead>
                <tbody>
                    {foreach $CUSTOM_DATA as $INDEX => $DATA}
                        <tr data-index="{$INDEX}">
                            <td>{$DATA.name}</td>
                            <td>{$DATA.value}</td>
                            <td>
                                {if $DATA.status eq 'active'}
                                    <span class="badge badge-success">
                                        {vtranslate('LBL_ACTIVE', $MODULE)}
                                    </span>
                                {else}
                                    <span class="badge badge-secondary">
                                        {vtranslate('LBL_INACTIVE', $MODULE)}
                                    </span>
                                {/if}
                            </td>
                        </tr>
                    {/foreach}
                </tbody>
            </table>
        </div>
    {else}
        <div class="alert alert-info">
            {vtranslate('LBL_NO_DATA_FOUND', $MODULE)}
        </div>
    {/if}

    {* Action Buttons *}
    <div class="custom-view-actions">
        <button type="button" class="btn btn-success" id="btnSave">
            {vtranslate('LBL_SAVE', $MODULE)}
        </button>
        <button type="button" class="btn btn-default" id="btnCancel">
            {vtranslate('LBL_CANCEL', $MODULE)}
        </button>
    </div>

</div>
```

## Modal Template Example

```smarty
{*
 * CustomModal.tpl
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 *}

<div class="modal-dialog modal-lg">
    <div class="modal-content">

        {* Modal Header *}
        <div class="modal-header">
            <button type="button" class="close" data-dismiss="modal">
                <span>&times;</span>
            </button>
            <h4 class="modal-title">
                {vtranslate('LBL_CUSTOM_MODAL', $MODULE)}
            </h4>
        </div>

        {* Modal Body *}
        <div class="modal-body">
            <form id="customModalForm" data-module="{$MODULE}">

                {if $RECORD_ID > 0}
                    <input type="hidden" name="record" value="{$RECORD_ID}" />
                {/if}

                <div class="form-group">
                    <label>{vtranslate('LBL_NAME', $MODULE)}</label>
                    <input type="text" class="form-control" name="name"
                           value="{$RECORD->get('name')}" required />
                </div>

                <div class="form-group">
                    <label>{vtranslate('LBL_STATUS', $MODULE)}</label>
                    <select class="form-control" name="status">
                        {foreach $STATUS_OPTIONS as $VALUE => $LABEL}
                            <option value="{$VALUE}"
                                {if $RECORD->get('status') eq $VALUE}selected{/if}>
                                {vtranslate($LABEL, $MODULE)}
                            </option>
                        {/foreach}
                    </select>
                </div>

            </form>
        </div>

        {* Modal Footer *}
        <div class="modal-footer">
            <button type="button" class="btn btn-success" id="btnModalSave">
                {vtranslate('LBL_SAVE', $MODULE)}
            </button>
            <button type="button" class="btn btn-default" data-dismiss="modal">
                {vtranslate('LBL_CANCEL', $MODULE)}
            </button>
        </div>

    </div>
</div>
```

## Common Patterns

### Data Attribute Pattern

```smarty
<div data-module="{$MODULE}"
     data-record="{$RECORD_ID}"
     data-status="{$RECORD->get('status')}">
    {* Content *}
</div>
```

### Conditional CSS Classes

```smarty
<div class="record-status {if $STATUS eq 'active'}status-active{else}status-inactive{/if}">
    {$STATUS}
</div>
```

### Include Partials

```smarty
{* Include reusable template *}
{include file="modules/Vtiger/partials/Header.tpl"}

{* Include with variables *}
{include file="modules/$MODULE/Partials/RecordInfo.tpl" RECORD=$RECORD}
```

## Critical Rules

1. **Always Escape**: Variables are auto-escaped. Use `{$VAR nofilter}` only for trusted HTML.
2. **Module Name Required**: Always pass `$moduleName` as 2nd arg to `$viewer->view()`.
3. **Use vtranslate()**: Never hardcode text labels. Use translation keys.
4. **Check Empty**: Use `{if !empty($VAR)}` before loops and object access.
5. **NO PHP in Templates**: Smarty templates should not contain PHP code.
6. **Data Attributes**: Use for JS controller access to IDs and state.

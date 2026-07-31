# VTiger Modal Dialogs

## JavaScript Modal Pattern

### Basic Modal Flow
1. **app.request.post** to get modal content HTML
2. **app.helper.showModal** to display modal
3. **registerModalEvents** to bind modal-specific handlers
4. **app.helper.hideModal** to close modal

### Complete Modal Example

```javascript
showCustomModal: function() {
    var self = this;

    app.helper.showProgress();

    var params = {
        module: 'CPGoal',
        view: 'CustomModal',  // BasicAjax_View
        record: recordId
    };

    app.request.post({ data: params }).then(function(error, data) {
        app.helper.hideProgress();

        if (error) {
            app.helper.showErrorNotification({
                message: app.vtranslate('JS_ERROR_LOADING_MODAL')
            });
            return;
        }

        // Show modal with callback
        app.helper.showModal(data, {
            cb: function(modal) {
                self.registerModalEvents(modal);
            }
        });
    });
},

registerModalEvents: function(modal) {
    var self = this;

    // Save button handler
    modal.find('.save-modal').on('click', function() {
        var form = modal.find('form');

        if (!form.vtValidate()) {
            return false;
        }

        self.saveModalData(form, modal);
    });

    // Cancel button handler
    modal.find('.cancel-modal').on('click', function() {
        app.helper.hideModal();
    });
},

saveModalData: function(form, modal) {
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

        app.helper.hideModal();

        // Refresh parent view
        window.location.reload();
    });
}
```

## Modal View (Backend)

### View Controller
**File**: `modules/{Module}/views/CustomModal.php`

```php
<?php

/**
 * Custom Modal View
 */
class CPGoal_CustomModal_View extends Vtiger_BasicAjax_View {

    /**
     * Render modal content
     */
    public function process(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $recordId = (int) $request->get('record');

        $viewer = $this->getViewer($request);

        // Prepare data
        if (!empty($recordId)) {
            $record = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);
            $viewer->assign('RECORD', $record);
        }

        $viewer->assign('MODULE', $moduleName);
        $viewer->assign('MODE', $request->getMode());

        // Render template
        $viewer->view('CustomModal.tpl', $moduleName);
    }
}
```

## Modal Template (Smarty)

### Template Structure
**File**: `layouts/v7/modules/{Module}/CustomModal.tpl`

```smarty
{*+**************************************************************************
 * Custom Modal Template
 ****************************************************************************}

<div class="modal-dialog modal-lg">
    <div class="modal-content">

        {* Modal Header *}
        <div class="modal-header">
            <button type="button" class="close" data-dismiss="modal" aria-label="Close">
                <span aria-hidden="true">&times;</span>
            </button>
            <h4 class="modal-title">
                {vtranslate('LBL_MODAL_TITLE', $MODULE)}
            </h4>
        </div>

        {* Modal Body *}
        <div class="modal-body">
            <form class="modal-form" data-module="{$MODULE}">
                <input type="hidden" name="module" value="{$MODULE}" />
                <input type="hidden" name="action" value="SaveModal" />

                {if $RECORD}
                    <input type="hidden" name="record" value="{$RECORD->getId()}" />
                {/if}

                <div class="form-group">
                    <label class="control-label">
                        {vtranslate('LBL_FIELD_NAME', $MODULE)}
                        <span class="redColor">*</span>
                    </label>
                    <input type="text"
                           name="field_name"
                           class="inputElement"
                           value="{$RECORD->get('field_name')}"
                           data-validation-engine="validate[required]" />
                </div>

                <div class="form-group">
                    <label class="control-label">
                        {vtranslate('LBL_DESCRIPTION', $MODULE)}
                    </label>
                    <textarea name="description"
                              class="inputElement"
                              rows="4">{$RECORD->get('description')}</textarea>
                </div>
            </form>
        </div>

        {* Modal Footer *}
        <div class="modal-footer">
            <button type="button" class="btn btn-default cancel-modal">
                <i class="fa fa-times"></i>
                {vtranslate('LBL_CANCEL', $MODULE)}
            </button>
            <button type="button" class="btn btn-success save-modal">
                <i class="fa fa-check"></i>
                {vtranslate('LBL_SAVE', $MODULE)}
            </button>
        </div>

    </div>
</div>
```

## Confirmation Dialog

### Simple Confirmation
```javascript
app.helper.showConfirmationBox({
    message: app.vtranslate('JS_CONFIRM_DELETE')
}).then(function(confirmed) {
    if (confirmed) {
        // User clicked "Yes"
        self.deleteRecord(recordId);
    } else {
        // User clicked "No"
        return;
    }
});
```

### Advanced Confirmation
```javascript
Vtiger_Helper_Js.showConfirmationBox({
    title: app.vtranslate('JS_CONFIRMATION_TITLE'),
    message: app.vtranslate('JS_CONFIRM_MESSAGE'),
    buttons: {
        confirm: {
            label: app.vtranslate('JS_YES'),
            className: 'btn-success'
        },
        cancel: {
            label: app.vtranslate('JS_NO'),
            className: 'btn-default'
        }
    },
    callback: function(result) {
        if (result) {
            self.performAction();
        }
    }
});
```

## Modal Sizes

### CSS Classes
```smarty
{* Small modal *}
<div class="modal-dialog modal-sm">

{* Medium modal (default) *}
<div class="modal-dialog">

{* Large modal *}
<div class="modal-dialog modal-lg">

{* Extra large (custom) *}
<div class="modal-dialog" style="width: 90%;">
```

## Clone Modal DOM Pattern

### Why Clone?
Modals are removed from DOM on close. If reopening, clone original.

```javascript
var modalTemplate = null;

showModal: function() {
    var self = this;

    if (modalTemplate) {
        // Reuse cached template
        app.helper.showModal(modalTemplate.clone(), {
            cb: function(modal) {
                self.registerModalEvents(modal);
            }
        });
        return;
    }

    // First time: load from server
    app.request.post({ data: params }).then(function(error, data) {
        modalTemplate = jQuery(data);  // Cache original

        app.helper.showModal(modalTemplate.clone(), {
            cb: function(modal) {
                self.registerModalEvents(modal);
            }
        });
    });
}
```

## Load HTML via AJAX

### Using view= Instead of action=
```javascript
var params = {
    module: 'CPGoal',
    view: 'ModalContent',  // Loads HTML, not JSON
    record: recordId
};

app.request.post({ data: params }).then(function(error, html) {
    app.helper.showModal(html, {
        cb: function(modal) {
            // Register events
        }
    });
});
```

## CSS Naming Convention

### Modal-Specific Classes
```css
/* Use kebab-case with descriptive names */
.custom-modal-container { }
.custom-modal-header { }
.custom-modal-body { }
.custom-modal-footer { }
.custom-modal-button { }
```

## Critical Pitfalls

1. **Clone modal DOM** if reusing — modals removed on close
2. **view= for HTML**, action= for JSON
3. **form.vtValidate()** before submit
4. **app.helper.showProgress** during AJAX
5. **Register events in callback** — modal not in DOM until shown
6. **Prevent form submit** — use button handlers, not form.submit
7. **Use data-dismiss="modal"** on close button for Bootstrap

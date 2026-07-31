# VTiger AJAX Patterns

## Frontend app.request.post Pattern

### Basic POST Request
```javascript
var params = {
    module: 'CPGoal',
    action: 'CalculateProgress',
    record: recordId
};

app.request.post({ data: params }).then(function(error, data) {
    if (error) {
        // Error occurred (network error, server error, or setError)
        app.helper.showErrorNotification({
            message: error.message || app.vtranslate('JS_ERROR_OCCURRED')
        });
        return;
    }

    // Success - data contains response.result from backend
    app.helper.showSuccessNotification({
        message: data.message
    });

    console.log('Result:', data);
});
```

## Error Handling Flow

### Error-First Callback Pattern
```javascript
app.request.post({ data: params }).then(function(error, data) {
    // ALWAYS check error first
    if (error) {
        // Handle error case
        return;
    }

    // Now safe to use data
    console.log(data);
});
```

### Error Sources

1. **Network Error**: Connection failed, timeout
2. **Server Error**: PHP exception, 500 error
3. **Application Error**: Backend called `$response->setError()`

### Backend Error Response
```php
// In Action
$response->setError('Custom error message');
$response->emit();
```

### Frontend Receives
```javascript
// error parameter will contain: { message: 'Custom error message' }
if (error) {
    console.log(error.message);  // "Custom error message"
}
```

## Load HTML via AJAX

### Using view= Parameter
```javascript
var params = {
    module: 'CPGoal',
    view: 'ModalContent',  // Load View (HTML), not Action (JSON)
    record: recordId
};

app.request.post({ data: params }).then(function(error, html) {
    if (error) {
        app.helper.showErrorNotification({
            message: error.message
        });
        return;
    }

    // html contains rendered template
    app.helper.showModal(html, {
        cb: function(modal) {
            // Register modal events
        }
    });
});
```

## Form Submit via Serialize

### Serialize Form Data
```javascript
var form = jQuery('#edit-form');
var params = form.serializeFormData();

// Returns object:
// {
//     module: 'CPGoal',
//     action: 'Save',
//     field1: 'value1',
//     field2: 'value2',
//     ...
// }

app.request.post({ data: params }).then(function(error, data) {
    if (error) {
        app.helper.showErrorNotification({
            message: error.message
        });
        return;
    }

    window.location.href = data.redirect_url;
});
```

### Manual Serialization
```javascript
var params = {
    module: 'CPGoal',
    action: 'Save',
    record: form.find('[name="record"]').val(),
    goal_name: form.find('[name="goal_name"]').val(),
    target_value: form.find('[name="target_value"]').val()
};
```

## Progress Indicator

### Show/Hide Progress
```javascript
// Show before AJAX
app.helper.showProgress();

app.request.post({ data: params }).then(function(error, data) {
    // Hide after response (both success and error)
    app.helper.hideProgress();

    if (error) {
        // Handle error
        return;
    }

    // Handle success
});
```

### With Custom Message
```javascript
app.helper.showProgress('Processing...');
```

## Confirmation Dialog

### Confirm Before Action
```javascript
handleDelete: function(recordId) {
    var self = this;

    app.helper.showConfirmationBox({
        message: app.vtranslate('JS_CONFIRM_DELETE')
    }).then(function(confirmed) {
        if (!confirmed) {
            return;
        }

        // User confirmed, proceed with delete
        app.helper.showProgress();

        var params = {
            module: 'CPGoal',
            action: 'Delete',
            record: recordId
        };

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

            self.getListViewRecords();  // Refresh list
        });
    });
}
```

## GET Request

### Using app.request.get
```javascript
var url = 'index.php?module=CPGoal&action=GetData&record=' + recordId;

app.request.get({ url: url }).then(function(error, response) {
    if (error) {
        app.helper.showErrorNotification({
            message: error.message
        });
        return;
    }

    console.log(response);
});
```

## Key JavaScript Objects

### app.helper Methods

| Method | Purpose | Parameters |
|--------|---------|------------|
| `showProgress()` | Show loading spinner | message (optional) |
| `hideProgress()` | Hide loading spinner | none |
| `showSuccessNotification()` | Success message | { message: string } |
| `showErrorNotification()` | Error message | { message: string } |
| `showModal()` | Display modal | html, { cb: function } |
| `hideModal()` | Close modal | none |
| `showConfirmationBox()` | Confirm dialog | { message: string } |

### app.request Methods

| Method | Purpose | Parameters |
|--------|---------|------------|
| `post()` | POST request | { data: object } |
| `get()` | GET request | { url: string } |

### app.vtranslate

```javascript
// Get JavaScript translation
var message = app.vtranslate('JS_SUCCESS_MESSAGE');

// With module context
var message = app.vtranslate('JS_SUCCESS_MESSAGE', 'CPGoal');
```

## Complete AJAX Flow Example

```javascript
// JavaScript (in List.js or Detail.js)
CustomView_BaseController_Js('CPGoal_List_Js', {}, {

    registerEvents: function() {
        this._super();
        this.registerSyncButton();
    },

    registerSyncButton: function() {
        var self = this;
        var container = this.getListViewContainer();

        container.on('click', '.sync-goals-btn', function(e) {
            e.preventDefault();
            self.syncGoals();
            return false;
        });
    },

    syncGoals: function() {
        var self = this;

        // Confirm first
        app.helper.showConfirmationBox({
            message: app.vtranslate('JS_CONFIRM_SYNC_GOALS')
        }).then(function(confirmed) {
            if (!confirmed) {
                return;
            }

            // Show progress
            app.helper.showProgress(app.vtranslate('JS_SYNCING'));

            // AJAX request
            var params = {
                module: 'CPGoal',
                action: 'SyncGoals'
            };

            app.request.post({ data: params }).then(function(error, data) {
                app.helper.hideProgress();

                if (error) {
                    app.helper.showErrorNotification({
                        message: error.message
                    });
                    return;
                }

                // Success
                app.helper.showSuccessNotification({
                    message: data.message
                });

                // Refresh list
                self.getListViewRecords();
            });
        });
    }
});
```

```php
// PHP Action (backend)
<?php

class CPGoal_SyncGoals_Action extends Vtiger_Action_Controller {

    public function checkPermission(Vtiger_Request $request) {
        if (!Users_Privileges_Model::isPermitted('CPGoal', 'EditView')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED', 'CPGoal'));
        }
    }

    public function process(Vtiger_Request $request) {
        $response = new Vtiger_Response();

        try {
            $logic = CPGoal_Logic_Helper::getInstance();
            $result = $logic->syncGoalsFromExternal();

            $response->setResult([
                'success' => true,
                'message' => vtranslate('LBL_SYNC_SUCCESS', 'CPGoal'),
                'synced_count' => $result['count']
            ]);

        } catch (Exception $e) {
            $response->setError($e->getMessage());
        }

        $response->emit();
    }
}
```

## Critical Pitfalls

1. **Error-first callback** — always check `if (error)` first
2. **showProgress/hideProgress** — hide in both success and error paths
3. **Don't use jQuery.ajax** — use `app.request.post()` for VTiger handling
4. **view= for HTML, action= for JSON** — different response types
5. **app.vtranslate** for translations — don't hardcode strings
6. **Confirmation before destructive actions** — delete, bulk update, etc.
7. **Refresh UI after success** — reload list, update detail view, etc.

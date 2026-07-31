# VTiger Custom Buttons

## JavaScript Handler Pattern

### Controller Structure
```javascript
/*
 * FileName.js
 * Module: ModuleName
 * Purpose: Button handlers and UI interactions
 */

CustomView_BaseController_Js('ModuleName_ViewName_Js', {}, {

    registerEvents: function() {
        this._super();
        this.registerButtonEvents();
    },

    registerButtonEvents: function() {
        var self = this;
        var container = this.getContainer();  // Cache container

        // Button click handler
        container.on('click', '.custom-button', function(e) {
            e.preventDefault();
            self.handleCustomAction(e);
            return false;
        });
    },

    handleCustomAction: function(e) {
        var self = this;
        var element = jQuery(e.currentTarget);
        var recordId = element.data('record-id');

        app.helper.showProgress();

        var params = {
            module: 'ModuleName',
            action: 'CustomAction',
            record: recordId
        };

        app.request.post({ data: params }).then(function(error, data) {
            app.helper.hideProgress();

            if (error) {
                app.helper.showErrorNotification({
                    message: app.vtranslate('JS_ERROR_OCCURRED')
                });
                return;
            }

            app.helper.showSuccessNotification({
                message: app.vtranslate('JS_SUCCESS')
            });

            // Refresh list or detail view
            self.getListViewRecords();  // For ListView
            // OR
            window.location.reload();  // For DetailView
        });
    },

    getContainer: function() {
        return jQuery('#listViewContainer');  // Or appropriate container
    }
});
```

## Button Locations

### 1. ListView Buttons

#### Add Button via ListView Model
**File**: `modules/{Module}/models/ListView.php`

```php
<?php

class CPGoal_ListView_Model extends Vtiger_ListView_Model {

    /**
     * Add custom buttons to ListView
     */
    public function getBasicLinks() {
        $links = parent::getBasicLinks();

        $links[] = [
            'linktype' => 'LISTVIEWBASIC',
            'linklabel' => 'LBL_SYNC_GOALS',
            'linkurl' => 'javascript:CPGoal_List_Js.syncGoals()',
            'linkicon' => 'fa-refresh',
        ];

        $links[] = [
            'linktype' => 'LISTVIEWBASIC',
            'linklabel' => 'LBL_EXPORT_REPORT',
            'linkurl' => 'javascript:CPGoal_List_Js.exportReport()',
            'linkicon' => 'fa-download',
        ];

        return $links;
    }

    /**
     * Add mass action buttons
     */
    public function getListViewMassActions($linkParams) {
        $links = parent::getListViewMassActions($linkParams);

        $links[] = [
            'linktype' => 'LISTVIEWMASSACTION',
            'linklabel' => 'LBL_BULK_UPDATE',
            'linkurl' => 'javascript:CPGoal_List_Js.bulkUpdate()',
            'linkicon' => 'fa-edit',
        ];

        return $links;
    }
}
```

### 2. DetailView Buttons

#### Add Button via Record Model
**File**: `modules/{Module}/models/Record.php`

```php
<?php

class CPGoal_Record_Model extends Vtiger_Record_Model {

    /**
     * Add custom buttons to DetailView
     */
    public function getDetailViewLinks($linkParams) {
        $links = parent::getDetailViewLinks($linkParams);

        $links[] = [
            'linktype' => 'DETAILVIEWBASIC',
            'linklabel' => 'LBL_CALCULATE_PROGRESS',
            'linkurl' => 'javascript:CPGoal_Detail_Js.calculateProgress()',
            'linkicon' => 'fa-calculator',
        ];

        $links[] = [
            'linktype' => 'DETAILVIEW',
            'linklabel' => 'LBL_SYNC_DATA',
            'linkurl' => 'javascript:CPGoal_Detail_Js.syncData(' . $this->getId() . ')',
            'linkicon' => 'fa-sync',
        ];

        return $links;
    }
}
```

### 3. EditView Buttons

#### Add Button via Module Model
**File**: `modules/{Module}/models/Module.php`

```php
<?php

class CPGoal_Module_Model extends Vtiger_Module_Model {

    /**
     * Add custom buttons to EditView
     */
    public function getEditViewLinks($linkParams) {
        $links = parent::getEditViewLinks($linkParams);

        $links[] = [
            'linktype' => 'EDITVIEWBASIC',
            'linklabel' => 'LBL_SAVE_AND_CONTINUE',
            'linkurl' => 'javascript:CPGoal_Edit_Js.saveAndContinue()',
            'linkicon' => 'fa-save',
        ];

        return $links;
    }
}
```

### 4. RelatedList Buttons

#### Add Button to Related List
**File**: `modules/{Module}/models/RelationListView.php`

```php
<?php

class CPGoal_RelationListView_Model extends Vtiger_RelationListView_Model {

    /**
     * Add buttons to related list
     */
    public function getRelatedLinks() {
        $links = parent::getRelatedLinks();

        $links[] = [
            'linktype' => 'RELATEDLIST',
            'linklabel' => 'LBL_QUICK_ADD',
            'linkurl' => 'javascript:CPGoal_RelatedList_Js.quickAdd()',
            'linkicon' => 'fa-plus',
        ];

        return $links;
    }
}
```

## DOM Caching Pattern

```javascript
registerEvents: function() {
    var self = this;

    // Cache frequently accessed elements
    var container = this.getContainer();
    var listView = container.find('#listViewContents');
    var toolbar = container.find('.listViewActionsContainer');

    // Use cached elements
    toolbar.on('click', '.sync-button', function() {
        self.handleSync(listView);
    });
}
```

## AJAX Request Pattern

```javascript
handleButtonClick: function(recordId) {
    app.helper.showProgress();

    var params = {
        module: 'CPGoal',
        action: 'CalculateProgress',
        record: recordId
    };

    app.request.post({ data: params }).then(function(error, data) {
        app.helper.hideProgress();

        if (error) {
            app.helper.showErrorNotification({
                message: error.message || app.vtranslate('JS_ERROR')
            });
            return;
        }

        if (data.success) {
            app.helper.showSuccessNotification({
                message: data.message
            });

            // Update UI with result
            jQuery('#progress-value').text(data.result.progress + '%');
        }
    });
}
```

## Conditional Buttons

### Based on Record Status
```php
public function getDetailViewLinks($linkParams) {
    $links = parent::getDetailViewLinks($linkParams);

    $status = $this->get('status');

    // Show "Complete" button only for Active goals
    if ($status === 'Active') {
        $links[] = [
            'linktype' => 'DETAILVIEWBASIC',
            'linklabel' => 'LBL_MARK_COMPLETE',
            'linkurl' => 'javascript:CPGoal_Detail_Js.markComplete()',
            'linkicon' => 'fa-check',
        ];
    }

    // Show "Reactivate" button only for Completed goals
    if ($status === 'Completed') {
        $links[] = [
            'linktype' => 'DETAILVIEWBASIC',
            'linklabel' => 'LBL_REACTIVATE',
            'linkurl' => 'javascript:CPGoal_Detail_Js.reactivate()',
            'linkicon' => 'fa-undo',
        ];
    }

    return $links;
}
```

### Based on User Permission
```php
public function getDetailViewLinks($linkParams) {
    $links = parent::getDetailViewLinks($linkParams);

    $currentUser = Users_Record_Model::getCurrentUserModel();

    if ($currentUser->isAdminUser()) {
        $links[] = [
            'linktype' => 'DETAILVIEWBASIC',
            'linklabel' => 'LBL_ADMIN_ACTION',
            'linkurl' => 'javascript:CPGoal_Detail_Js.adminAction()',
            'linkicon' => 'fa-cog',
        ];
    }

    return $links;
}
```

## Button Link Types

| Link Type | Location | Purpose |
|-----------|----------|---------|
| `LISTVIEWBASIC` | ListView top bar | Main list actions |
| `LISTVIEWMASSACTION` | ListView (selected) | Bulk operations |
| `DETAILVIEWBASIC` | DetailView top bar | Record actions |
| `DETAILVIEW` | DetailView top bar | Additional actions |
| `EDITVIEWBASIC` | EditView top bar | Form actions |
| `RELATEDLIST` | Related list header | Related record actions |

## Critical Pitfalls

1. **return false** after preventDefault in event handler
2. **Cache container** — don't query DOM repeatedly
3. **app.helper.showProgress** before AJAX, hideProgress after
4. **Error-first callback** — check error param first
5. **Use app.vtranslate** for JS translations, not hardcoded strings

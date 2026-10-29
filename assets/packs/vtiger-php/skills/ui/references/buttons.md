# VTiger Custom Buttons

## JavaScript Handler Pattern

### Controller Structure
```javascript
/*
    FileName.js
    Author: Your Name
    Date: YYYY-MM-DD
    Purpose: Button handlers and UI interactions
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

Hooks return `Vtiger_Link_Model` objects grouped by link type, except `getBasicLinks()` (raw arrays that `getListViewLinks()` converts). Always start from `parent::` and append to the right key.

### 1. ListView Buttons

**File**: `modules/{Module}/models/ListView.php`

```php
<?php

class CPGoal_ListView_Model extends Vtiger_ListView_Model {

	// Top-bar buttons: raw arrays, converted by getListViewLinks()
	public function getBasicLinks() {
		$basicLinks = parent::getBasicLinks();

		$basicLinks[] = [
			'linktype' => 'LISTVIEWBASIC',
			'linklabel' => 'LBL_SYNC_GOALS',
			'linkurl' => 'javascript:CPGoal_List_Js.syncGoals()',
			'linkicon' => '',
		];

		return $basicLinks;
	}

	// Mass actions: Vtiger_Link_Model objects under 'LISTVIEWMASSACTION'
	public function getListViewMassActions($linkParams) {
		$links = parent::getListViewMassActions($linkParams);

		$links['LISTVIEWMASSACTION'][] = Vtiger_Link_Model::getInstanceFromValues([
			'linktype' => 'LISTVIEWMASSACTION',
			'linklabel' => 'LBL_BULK_UPDATE',
			'linkurl' => 'javascript:CPGoal_List_Js.bulkUpdate()',
			'linkicon' => '',
		]);

		return $links;
	}
}
```

`ListViewHeader.tpl` renders every `LISTVIEWBASIC` button with a fixed `fa-plus` icon; a `javascript:` URL becomes the `onclick`.

### 2. DetailView Buttons

**File**: `modules/{Module}/models/DetailView.php` — the hook lives on `Vtiger_DetailView_Model`, not the Record model (real example: `modules/CPBillCategory/models/DetailView.php`).

```php
<?php

class CPGoal_DetailView_Model extends Vtiger_DetailView_Model {

	public function getDetailViewLinks($linkParams) {
		$linkModelList = parent::getDetailViewLinks($linkParams);
		$recordId = $this->getRecord()->getId();

		// Button in the top bar; linkcss is added to the button class
		$linkModelList['DETAILVIEWBASIC'][] = Vtiger_Link_Model::getInstanceFromValues([
			'linktype' => 'DETAILVIEWBASIC',
			'linklabel' => 'LBL_CALCULATE_PROGRESS',
			'linkurl' => 'javascript:CPGoal_Detail_Js.calculateProgress()',
			'linkicon' => 'fa fa-calculator',
			'linkcss' => 'btn-primary',
		]);

		// Item in the "more" dropdown; no custom class is rendered, only the javascript: onclick
		$linkModelList['DETAILVIEW'][] = Vtiger_Link_Model::getInstanceFromValues([
			'linktype' => 'DETAILVIEW',
			'linklabel' => 'LBL_SYNC_DATA',
			'linkurl' => 'javascript:CPGoal_Detail_Js.syncData(' . $recordId . ')',
			'linkicon' => '',
		]);

		return $linkModelList;
	}
}
```

### 3. EditView Buttons

There is no rendering link hook for EditView: `Vtiger_EditView_Model::getEditViewLinks()` returns LISTVIEW* links and no v7 template reads it. Add EditView buttons in the module's EditView JS (loaded via `modules/{Module}/custom/EditView.php`, see SKILL.md § Loading Standalone JS) or in a `customTemplate` from that file.

### 4. RelatedList Buttons

**File**: `modules/{Module}/models/RelationListView.php` — the hook is `getLinks()` (no `getRelatedLinks()`), returning `['LISTVIEWBASIC' => [Vtiger_Link_Model, ...]]`.

`partials/RelatedListHeader.tpl` renders those links only as Select / Add buttons (`selectRelation` / `addButton`), so this hook is for removing or adjusting them (e.g. `modules/CPEventRegistration/models/RelationListView.php`), not for custom JS actions:

```php
<?php

class CPGoal_RelationListView_Model extends Vtiger_RelationListView_Model {

	public function getLinks() {
		$links = parent::getLinks();

		// Hide Select/Add buttons for a read-only relation
		if ($this->getRelatedModuleModel()->getName() == 'CPGoalLog') {
			$links['LISTVIEWBASIC'] = [];
		}

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
	$linkModelList = parent::getDetailViewLinks($linkParams);
	$status = $this->getRecord()->get('status');

	// Show "Complete" button only for Active goals
	if ($status === 'Active') {
		$linkModelList['DETAILVIEWBASIC'][] = Vtiger_Link_Model::getInstanceFromValues([
			'linktype' => 'DETAILVIEWBASIC',
			'linklabel' => 'LBL_MARK_COMPLETE',
			'linkurl' => 'javascript:CPGoal_Detail_Js.markComplete()',
			'linkicon' => 'fa fa-check',
		]);
	}

	return $linkModelList;
}
```

### Based on User Permission
```php
public function getDetailViewLinks($linkParams) {
	$linkModelList = parent::getDetailViewLinks($linkParams);
	$currentUser = Users_Record_Model::getCurrentUserModel();

	if ($currentUser->isAdminUser()) {
		$linkModelList['DETAILVIEWBASIC'][] = Vtiger_Link_Model::getInstanceFromValues([
			'linktype' => 'DETAILVIEWBASIC',
			'linklabel' => 'LBL_ADMIN_ACTION',
			'linkurl' => 'javascript:CPGoal_Detail_Js.adminAction()',
			'linkicon' => 'fa fa-cog',
		]);
	}

	return $linkModelList;
}
```

## Button Link Types

| Link Type | Hook | Rendered in |
|-----------|------|-------------|
| `LISTVIEWBASIC` | `{Module}_ListView_Model::getBasicLinks()` | ListView top bar |
| `LISTVIEWMASSACTION` | `{Module}_ListView_Model::getListViewMassActions()` | ListView actions (selected records) |
| `DETAILVIEWBASIC` | `{Module}_DetailView_Model::getDetailViewLinks()` | DetailView top-bar buttons (`linkcss`, `linkicon`, `linktooltip`) |
| `DETAILVIEW` | `{Module}_DetailView_Model::getDetailViewLinks()` | DetailView "more" dropdown |
| `LISTVIEWBASIC` (related) | `{Module}_RelationListView_Model::getLinks()` | Related list Select/Add buttons |

## Critical Pitfalls

1. **return false** after preventDefault in event handler
2. **Cache container** — don't query DOM repeatedly
3. **app.helper.showProgress** before AJAX, hideProgress after
4. **Error-first callback** — check error param first
5. **Use app.vtranslate** for JS translations, not hardcoded strings

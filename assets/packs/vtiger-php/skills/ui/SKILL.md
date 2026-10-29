---
name: ui
description: "VTiger UI frontend — button, modal JS, form validation, app.request, notification UI. Use when: thêm nút, modal, validate form, tương tác JS/AJAX frontend, giao diện động; page/tpl mới → skill view."
user-invocable: false
---

# VTiger UI Components

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Adding custom buttons to ListView/DetailView
- Creating modal dialogs and popups
- Implementing form validation
- Showing notifications and progress indicators
- Building interactive UI with AJAX

## UI Component Overview

| Component | JS Object | Purpose |
|-----------|-----------|---------|
| AJAX Request | `app.request.post()` | Make AJAX calls |
| Progress Indicator | `app.helper.showProgress()` | Show loading spinner |
| Notification | `app.helper.showSuccessNotification()` | Show success/error messages |
| Modal | `app.helper.showModal()` | Display modal dialogs |
| Confirmation | `app.helper.showConfirmationBox()` | Confirm user actions |
| Form Validation | `form.vtValidate()` + `form.valid()` | Validate form inputs |
| Translation | `app.vtranslate()` | Get JS translations |

## Key JavaScript Objects

### app.request
```javascript
// POST request
app.request.post({ data: params }).then(function(error, data) {
    if (error) {
        // Handle error
    }
    else {
        // Handle success
    }
});

// GET request
app.request.get({ url: url }).then(function(error, response) { });
```

### app.helper
```javascript
// Progress indicator
app.helper.showProgress();
app.helper.hideProgress();

// Notifications
app.helper.showSuccessNotification({ message: 'Success!' });
app.helper.showErrorNotification({ message: 'Error occurred' });

// Modal
app.helper.showModal(html, { cb: function(modal) { } });
app.helper.hideModal();

// Confirmation
app.helper.showConfirmationBox({
    message: 'Are you sure?'
}).then(function(confirmed) { });
```

## JS File Types & Location Rules

### 3 Types of JS Files

| Type | Location | Base Class | Auto-loaded? |
|------|----------|-----------|-------------|
| **Core View** (inherit parent) | `layouts/v7/modules/{Module}/resources/Detail.js` | `Vtiger_Detail_Js` / `Vtiger_List_Js` / `Vtiger_Edit_Js` | Yes (by convention) |
| **Custom View** (standalone controller) | `modules/{Module}/resources/SocialConfig.js` | `CustomView_BaseController_Js` | Yes (by `CustomView_Base_View`) |
| **Standalone** (no inheritance) | `modules/{Module}/resources/EditView.js` | None | No (manual include via `modules/{Module}/custom/EditView.php` or `getHeaderScripts`) |

### Other File Locations

| File Type | Custom Features | Core Views |
|-----------|----------------|------------|
| **CSS** | `modules/{Module}/resources/{Name}.css` | `modules/{Module}/resources/{Name}.css` |
| **TPL** | `modules/{Module}/tpls/{Name}.tpl` | `layouts/v7/modules/{Module}/{Name}.tpl` |

---

## Core View JS (inherit parent controller)

Location: `layouts/v7/modules/{Module}/resources/{View}.js`
Auto-loaded by framework. Naming must match: `{Module}_{View}_Js`.

```javascript
Vtiger_Detail_Js("ModuleName_Detail_Js", {}, {

    registerEvents: function() {
        this._super();  // MUST call parent
        this.registerCustomEvents();
    },

    registerCustomEvents: function() {
        var self = this;
        // Custom events here
    }
});
```

**Inheritance chain:**
- Detail: `Vtiger_Detail_Js` → `Inventory_Detail_Js` → `SalesOrder_Detail_Js`
- List: `Vtiger_List_Js` → `ModuleName_List_Js`
- Edit: `Vtiger_Edit_Js` → `ModuleName_Edit_Js`

---

## Custom View JS (standalone controller with framework base)

Location: `modules/{Module}/resources/{ViewName}.js`
Auto-loaded by `CustomView_Base_View`. Naming: `{Module}_{ViewName}_Js`.

```javascript
/*
    ViewName.js
    Author: Dev Name
    Date: YYYY-MM-DD
    Purpose: Brief description
*/

CustomView_BaseController_Js('ModuleName_ViewName_Js', {}, {

    registerEvents: function() {
        this._super();
        this.initForm();
    },

    getForm: function() {
        return $('form#config');   // the base controller has no getForm(); define it per view
    },

    initForm: function() {
        let self = this;
        let form = this.getForm();

        form.find('#btnSave').on('click', function() {
            self.handleSave(form);
        });
    },

    handleSave: function(form) {
        app.helper.showProgress();
        let params = {
            module: 'ModuleName',
            action: 'HandleAjax',
            mode: 'saveData',
            data: form.serializeObject()
        };

        app.request.post({ data: params }).then(function(err, data) {
            app.helper.hideProgress();
            if (err) {
                app.helper.showErrorNotification({ message: err.message });
                return;
            }
            app.helper.showSuccessNotification({ message: app.vtranslate('JS_SAVE_SUCCESS') });
        });
    }
});
```

---

## Standalone JS (no inheritance)

Location: `modules/{Module}/resources/{Name}.js`
Must be included manually (see below). No base class, no `_super()`.

### Pattern A: Object Literal

```javascript
/*
    EditView.js
    Author: Dev Name
    Date: YYYY-MM-DD
    Purpose: Brief description
*/

var ModuleName_ViewName = {

    initEvents: function() {
        this.registerSomething();
    },

    registerSomething: function() {
        let self = this;
        // logic...
    }
};

$(function() {
    ModuleName_ViewName.initEvents();
});
```

### Pattern B: Anonymous Class (ES6+)

```javascript
/**
 * @author Dev Name
 * @email dev@cloudgo.vn
 * @create date YYYY.MM.DD
 * @desc Brief description
 */

let ModuleName_ViewName = new class {

    initEvents() {
        this.registerSomething();
    }

    registerSomething() {
        let self = this;
        // logic...
    }
};

$(function() {
    ModuleName_ViewName.initEvents();
});
```

### Loading Standalone JS

**EditView / DetailView of an entity module** — `Vtiger_Edit_View` / `Vtiger_Detail_View` `require` `modules/{Module}/custom/EditView.php` / `custom/DetailView.php` when present; its `$displayParams['scripts']` is printed into the page (e.g. `modules/CPMasterPlan/custom/EditView.php`):

```php
<?php

$displayParams = [
	'scripts' => '
		<link type="text/css" rel="stylesheet" href="{vresource_url("modules/CPMasterPlan/resources/EditView.css")}">
		<script type="text/javascript" src="{vresource_url("modules/CPMasterPlan/resources/EditView.js")}"></script>
	',
];
```

**Any view controller** — override `getHeaderScripts()` (same signature as the parent) and convert names with `checkAndConvertJsScripts()`; the template calls `->getSrc()` on every item, so never push raw arrays (e.g. `modules/CPAgenda/views/Edit.php`):

```php
function getHeaderScripts(Vtiger_Request $request) {
	$headerScriptInstances = parent::getHeaderScripts($request);
	$jsScriptInstances = $this->checkAndConvertJsScripts(['modules.SalesOrder.resources.EditView']);

	return array_merge($headerScriptInstances, $jsScriptInstances);
}
```

---

## Button with CSS Class Pattern (for custom JS)

When JS lives in `modules/{Module}/resources/` (not on the controller), use a CSS class + event delegation. Only `DETAILVIEWBASIC` buttons render `linkcss` as a class; the `DETAILVIEW` "more" dropdown `<li>/<a>` renders no custom class, so use a `javascript:` `linkurl` there instead.

```php
// In models/DetailView.php (class {Module}_DetailView_Model extends Vtiger_DetailView_Model)
public function getDetailViewLinks($linkParams) {
	$linkModelList = parent::getDetailViewLinks($linkParams);

	$linkModelList['DETAILVIEWBASIC'][] = Vtiger_Link_Model::getInstanceFromValues([
		'linktype' => 'DETAILVIEWBASIC',
		'linklabel' => 'LBL_UPDATE_STATUS',
		'linkurl' => 'javascript:void(0)',
		'linkcss' => 'update-so-status-btn',
		'linkicon' => 'fa fa-refresh',
	]);

	return $linkModelList;
}
```

```javascript
// In modules/{Module}/resources/DetailView.js
jQuery(document).on('click', '.update-so-status-btn', function(e) {
    e.preventDefault();
    // handler logic
    return false;
});
```

## Critical Pitfalls

1. **e.preventDefault** on form submit to prevent page reload
2. **app.request.post** NOT jQuery.ajax (VTiger wrapper handles errors)
3. **Clone modal DOM** if reusing — modals are removed on close
4. **showProgress/hideProgress** for async operations
5. **Validation** — `form.vtValidate()` only sets up jQuery Validate and returns a truthy object; check `form.valid()` before submitting
6. **Custom JS location** — `modules/{Module}/resources/`, NOT `layouts/v7/` for custom features
7. **Author name** — use actual dev name, never "Claude AI"
8. **Core view JS** — ALWAYS call `this._super()` in `registerEvents`
9. **Standalone JS** — ALWAYS wrap init in `$(function() { })` for DOM ready
10. **Naming** — `{Module}_{ViewName}_Js` for controller-based, `{Module}_{ViewName}` for standalone

## References

- [buttons.md](references/buttons.md) — Custom buttons in ListView/DetailView
- [modals.md](references/modals.md) — Modal dialogs and popups
- [form-validation.md](references/form-validation.md) — Form validation patterns

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- JS controller custom view (tung.nguyen 5/5): `modules/CPMasterPlan/resources/MasterPlanView.js`
- JS EditView (tung.nguyen 4/4): `modules/CPMasterPlan/resources/EditView.js`

## Verify

```bash
# JS/CSS loaded via vresource_url() get an mtime cache-buster, so a normal reload picks up edits (no hard refresh needed)
# Verify with the omp `browser` tool: browser.open → tab.goto(url) → interact → tab.screenshot(); check console errors via tab.evaluate / tab.run
```

# CSS/JS Inclusion

> Asset loading for VTiger Views

File separation (no inline CSS/JS, no HTML in PHP) and CSS/JS/TPL locations: see **File Separation Rules** in `.omp/rules/cloudgo-development-rules.md` (always loaded) — not repeated here. `app.request` / `app.helper` / `app.vtranslate` reference: `.omp/rules/javascript-rules.md` ("Key JavaScript Objects").

### Real Example Paths

```
# Core views → layouts/v7/
layouts/v7/modules/Contacts/resources/Detail.js
layouts/v7/modules/Accounts/resources/Edit.js
layouts/v7/modules/Products/resources/Detail.js

# Custom views → modules/  (CustomView_Base_View auto-loads <ViewName>.js/.css)
modules/CPMasterPlan/resources/MasterPlanView.js
modules/CPMasterPlan/resources/MasterPlanView.css
modules/CPMasterPlan/tpls/MasterPlanView.tpl

# CSS → always modules/
modules/Contacts/resources/DetailView.css
```

`CustomView_Base_View` needs no `getHeaderScripts()`/`getHeaderCss()` override. The sections below are for `Vtiger_Index_View` subclasses.

## Including JavaScript in View

### Method: `getHeaderScripts()`

The header list holds `Vtiger_JsScript_Model` objects (`JSResources.tpl` calls `$jsModel->getType()` / `getSrc()`); a raw array fatals. Always convert names with `checkAndConvertJsScripts()`:

```php
class Contacts_CustomView_View extends Vtiger_Index_View {

    function getHeaderScripts(Vtiger_Request $request) {
        // MUST call parent to preserve existing scripts
        $headerScriptInstances = parent::getHeaderScripts($request);

        $jsFileNames = [
            'modules.Contacts.resources.CustomView',
            'modules.Contacts.resources.CustomModal',
        ];
        $jsScriptInstances = $this->checkAndConvertJsScripts($jsFileNames);

        return array_merge($headerScriptInstances, $jsScriptInstances);
    }
}
```

### Name → Path Resolution (`includes/runtime/Controller.php` `checkAndConvertJsScripts`)

```
'modules.Contacts.resources.CustomView'
→ JS: modules/Contacts/resources/CustomView.js (checked first, repo root)
→ JS: layouts/v7/modules/Contacts/resources/CustomView.js (fallback)
'~modules/Contacts/resources/CustomView.js'
→ literal path from repo root (no fallback)
```

Missing files are silently skipped — check the rendered `<script>` tags when a controller doesn't run.

## Including CSS in View

### Method: `getHeaderCss()`

```php
class Contacts_CustomView_View extends Vtiger_Index_View {

    function getHeaderCss(Vtiger_Request $request) {
        // MUST call parent to preserve existing styles
        $headerCssInstances = parent::getHeaderCss($request);

        $cssFileNames = [
            '~modules/Contacts/resources/CustomView.css',
            '~modules/Contacts/resources/CustomModal.css',
        ];
        $cssInstances = $this->checkAndConvertCssStyles($cssFileNames);

        return array_merge($headerCssInstances, $cssInstances);
    }
}
```

`checkAndConvertCssStyles()` has no layout fallback — the file must exist at the given path.

## JavaScript Controller Pattern

### File: `modules/Contacts/resources/CustomView.js` (custom view)

`CustomView_BaseController_Js` (`modules/CustomView/resources/BaseController.js`) only provides `init`, `addComponents`, `getFieldValue`, `registerEvents` — define `getContainer()` yourself; the record id comes from `app.getRecordId()`.

```javascript
/*
	CustomView.js
	Author: Your Name
	Date: YYYY-MM-DD
	Purpose: Custom view controller for Contacts module
*/

CustomView_BaseController_Js('Contacts_CustomView_Js', {}, {

	getContainer: function () {
		return jQuery('.custom-view-container');
	},

	registerEvents: function () {
		this._super();  // MUST call parent
		this.registerButtonEvents();
	},

	registerButtonEvents: function () {
		var self = this;
		var container = this.getContainer();  // Cache DOM

		container.find('#btnSave').on('click', function () {
			self.handleSave();
			return false;
		});
	},

	handleSave: function () {
		app.helper.showProgress();

		var params = {
			module: 'Contacts',
			action: 'SaveAjax',  // Action controller for JSON
			record: app.getRecordId()
		};

		app.request.post({ data: params }).then(function (error, data) {
			app.helper.hideProgress();

			if (error) {
				app.helper.showErrorNotification({ message: app.vtranslate('JS_ERROR_OCCURRED') });
				return;
			}

			app.helper.showSuccessNotification({ message: app.vtranslate('JS_SAVED_SUCCESSFULLY') });
		});
	}
});
```

### Controller Naming Pattern

| View | Controller Extends | Name Pattern |
|------|-------------------|--------------|
| List | `Vtiger_List_Js` | `Contacts_List_Js` |
| Detail | `Vtiger_Detail_Js` | `Contacts_Detail_Js` |
| Edit | `Vtiger_Edit_Js` | `Contacts_Edit_Js` |
| Custom | `CustomView_BaseController_Js` | `Contacts_CustomView_Js` |

## CSS Conventions

### File: `modules/Contacts/resources/CustomView.css`

```css
/*
 * CustomView.css
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

/* Use kebab-case for class names */
.custom-view-container {
    padding: 20px;
    background-color: var(--primary-1);
}

.custom-view-header {
    font-size: 18px;
    font-weight: bold;
    color: var(--white-1);
}

/* Text overflow pattern */
.custom-view-title {
    white-space: nowrap !important;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* Module-specific prefix to avoid conflicts */
.contacts-custom-field {
    margin-bottom: 10px;
}
```

### CSS Variables (VTiger v7)

```css
background-color: var(--primary-1);
color: var(--white-1);
border-color: var(--border-color);
```

## Common Pitfalls

1. **Inline CSS/JS**: Always use separate files, never inline in PHP/TPL
2. **Raw arrays in header lists**: `$scripts[] = ['src' => ...]` fatals in the template — use `checkAndConvertJsScripts()` / `checkAndConvertCssStyles()`
3. **Missing Parent Call**: Always call `parent::getHeaderScripts()` and `parent::getHeaderCss()`
4. **Missing `_super()`**: Always call `this._super()` in `registerEvents()`
5. **Silent skip**: a misspelled asset name is dropped without error — verify the rendered tag

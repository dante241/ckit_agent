# CSS/JS Inclusion

> File separation rules and asset loading for VTiger Views

## CRITICAL: File Separation Rules (MANDATORY)

**These rules are NON-NEGOTIABLE. Violations will break the codebase.**

1. **NO INLINE CSS/JS**: CSS and JavaScript MUST be in separate files, NEVER inline in PHP or TPL files
2. **CSS/JS Location**:
   - CSS: **ALWAYS** `modules/<Module>/resources/<ViewName>.css`
   - JS (core views — List, Edit, Detail that inherit parent controller): `layouts/v7/modules/<Module>/resources/<ViewName>.js`
   - JS (custom views — Config, reports, custom pages): `modules/<Module>/resources/<ViewName>.js`
3. **NO HTML IN PHP**: HTML markup MUST be in `.tpl` template files, NEVER directly in PHP classes

### Correct Structure (core view — inherits parent controller)

```
layouts/v7/modules/Contacts/
├── Detail.tpl                        # HTML template
├── resources/
│   └── Detail.js                     # JS controller (core view)
modules/Contacts/
├── resources/
│   └── Detail.css                    # CSS (always here)
```

### Correct Structure (custom view — standalone controller)

```
modules/Contacts/
├── resources/
│   ├── CustomView.js                 # JS controller (custom view)
│   ├── CustomView.css                # Styles
│   ├── CustomModal.js                # Modal controller
│   └── CustomModal.css               # Modal styles
layouts/v7/modules/Contacts/
├── CustomView.tpl                    # HTML template
├── CustomModal.tpl                   # Modal template
```

### ❌ WRONG - Inline CSS/JS in PHP

```php
// DON'T DO THIS
class Contacts_CustomView_View extends Vtiger_Index_View {
    public function process(Vtiger_Request $request): void {
        echo '<style>.my-class { color: red; }</style>';
        echo '<script>$(function() { alert("hi"); });</script>';
        echo '<div class="container">...</div>';
    }
}
```

### ✅ CORRECT - Separate Files

```php
// PHP View - data only
class Contacts_CustomView_View extends Vtiger_Index_View {
    public function process(Vtiger_Request $request): void {
        $viewer = $this->getViewer($request);
        $viewer->assign('recordData', $data);
        $viewer->view('CustomView.tpl', $moduleName);
    }

    public function getHeaderScripts(Vtiger_Request $request): array {
        $scripts = parent::getHeaderScripts($request);
        $scripts[] = ['src' => 'modules.Contacts.resources.CustomView'];
        return $scripts;
    }

    public function getHeaderCss(Vtiger_Request $request): array {
        $css = parent::getHeaderCss($request);
        $css[] = ['href' => 'modules.Contacts.resources.CustomView'];
        return $css;
    }
}
```

## JavaScript File Location

**Core views** (List, Edit, Detail — inherit parent controller):
```
layouts/v7/modules/<Module>/resources/<ViewName>.js
```

**Custom views** (Config, reports, custom pages — standalone controller):
```
modules/<Module>/resources/<ViewName>.js
```

### Example Paths

```
# Core views → layouts/v7/
layouts/v7/modules/Contacts/resources/Detail.js
layouts/v7/modules/Accounts/resources/List.js
layouts/v7/modules/Products/resources/Edit.js

# Custom views → modules/
modules/Products/resources/CheckWarranty.js
modules/CPGoal/resources/Config.js
modules/Reports/resources/ChartReport.js
```

## CSS File Location

CSS files are **ALWAYS** in `modules/`, never in `layouts/v7/`:
```
modules/<Module>/resources/<ViewName>.css
```

### Example Paths

```
modules/Contacts/resources/CustomView.css
modules/Products/resources/CheckWarranty.css
modules/CPGoal/resources/Config.css
```

## Including JavaScript in View

### Method: `getHeaderScripts()`

```php
<?php

class Contacts_CustomView_View extends Vtiger_Index_View {

    public function getHeaderScripts(Vtiger_Request $request): array {
        // MUST call parent to preserve existing scripts
        $scripts = parent::getHeaderScripts($request);

        // Add module-specific JS using DOT NOTATION
        $scripts[] = ['src' => 'modules.Contacts.resources.CustomView'];

        // Add multiple scripts
        $scripts[] = ['src' => 'modules.Contacts.resources.CustomModal'];
        $scripts[] = ['src' => 'modules.Contacts.resources.Helpers'];

        return $scripts;
    }
}
```

### Dot Notation Path Mapping

```
'modules.Contacts.resources.CustomView'
→ JS: layouts/v7/modules/Contacts/resources/CustomView.js (checked first)
→ JS: modules/Contacts/resources/CustomView.js (fallback)
→ CSS: modules/Contacts/resources/CustomView.css (always here)
```

**NEVER use file paths directly:**
```php
// ❌ WRONG
$scripts[] = 'layouts/v7/modules/Contacts/resources/CustomView.js';

// ✅ CORRECT
$scripts[] = ['src' => 'modules.Contacts.resources.CustomView'];
```

## Including CSS in View

### Method: `getHeaderCss()`

```php
<?php

class Contacts_CustomView_View extends Vtiger_Index_View {

    public function getHeaderCss(Vtiger_Request $request): array {
        // MUST call parent to preserve existing styles
        $css = parent::getHeaderCss($request);

        // Add module-specific CSS using DOT NOTATION
        $css[] = ['href' => 'modules.Contacts.resources.CustomView'];

        // Add multiple stylesheets
        $css[] = ['href' => 'modules.Contacts.resources.CustomModal'];

        return $css;
    }
}
```

## JavaScript Controller Pattern

### File: `modules/Contacts/resources/CustomView.js` (custom view)

```javascript
/*
 * CustomView.js
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 * Purpose: Custom view controller for Contacts module
 */

// Extend base controller (use appropriate parent class)
CustomView_BaseController_Js('Contacts_CustomView_Js', {}, {

    /**
     * Register all event handlers
     */
    registerEvents: function() {
        this._super();  // MUST call parent
        this.registerButtonEvents();
        this.registerFormEvents();
    },

    /**
     * Button click handlers
     */
    registerButtonEvents: function() {
        var self = this;
        var container = this.getContainer();  // Cache DOM

        container.find('#btnSave').on('click', function() {
            self.handleSave();
            return false;
        });
    },

    /**
     * Save handler with AJAX
     */
    handleSave: function() {
        app.helper.showProgress();

        var params = {
            module: 'Contacts',
            action: 'SaveAjax',  // Action controller for JSON
            record: this.getRecordId()
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
                message: app.vtranslate('JS_SAVED_SUCCESSFULLY')
            });
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
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
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

## Key JavaScript Objects

| Object | Methods | Purpose |
|--------|---------|---------|
| `app.request` | `post()`, `get()` | AJAX requests |
| `app.helper` | `showProgress()`, `hideProgress()` | Loading indicator |
| `app.helper` | `showSuccessNotification()`, `showErrorNotification()` | Notifications |
| `app.helper` | `showModal()`, `hideModal()` | Modal dialogs |
| `app` | `vtranslate(key)` | Get JS translation |

## Common Pitfalls

1. **Inline CSS/JS**: Always use separate files, never inline in PHP/TPL
2. **File Paths**: Use dot notation `modules.X.resources.Y`, not file paths
3. **Missing Parent Call**: Always call `parent::getHeaderScripts()` and `parent::getHeaderCss()`
4. **Missing `_super()`**: Always call `this._super()` in `registerEvents()`
5. **Wrong Key**: Use `src` for JS, `href` for CSS in array

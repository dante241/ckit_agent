# Config View Pattern

> Admin settings pages: View + Action + TPL + JS + CSS all in `modules/Settings/Vtiger/`

## File Structure

All files live under `modules/Settings/Vtiger/` (NOT in `layouts/v7/`):

```
modules/Settings/Vtiger/
├── views/<ConfigName>.php              # View controller
├── actions/Save<ConfigName>.php        # Save action
├── tpls/<ConfigName>.tpl               # Main template
├── tpls/<ConfigName>*RowTemplate.tpl   # Sub-templates (optional)
├── resources/<ConfigName>.js           # JS controller
└── resources/<ConfigName>.css          # Styles
```

## BaseConfig_View (Auto-loads JS/CSS)

**Location**: `modules/Settings/Vtiger/views/BaseConfig.php`

Key behavior: automatically loads JS and CSS files matching the view name.

```php
class Settings_Vtiger_BaseConfig_View extends Settings_Vtiger_Index_View {

    public function getHeaderScripts(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $viewName = $request->get('view');

        $jsFileNames = array(
            "~modules/CustomView/resources/BaseController.js",
            "~modules/Settings/{$moduleName}/resources/{$viewName}.js",
        );

        $jsScriptInstances = $this->checkAndConvertJsScripts($jsFileNames);
        $headerScriptInstances = parent::getHeaderScripts($request);
        $headerScriptInstances = array_merge($headerScriptInstances, $jsScriptInstances);
        return $headerScriptInstances;
    }

    function getHeaderCss(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $viewName = $request->get('view');
        $cssFileNames = array("~modules/Settings/{$moduleName}/resources/{$viewName}.css");

        $cssInstances = $this->checkAndConvertCssStyles($cssFileNames);
        $headerCssInstances = parent::getHeaderCss($request);
        $headerCssInstances = array_merge($headerCssInstances, $cssInstances);
        return $headerCssInstances;
    }
}
```

**IMPORTANT**: The `~` prefix means absolute path from webroot. No dot notation here — config views use direct file paths.

## View Controller

Extend `Settings_Vtiger_BaseConfig_View` — JS/CSS auto-loaded by parent.

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

class Settings_Vtiger_GlobalSearchConfig_View extends Settings_Vtiger_BaseConfig_View {

    public function getPageTitle(Vtiger_Request $request) {
        $moduleName = $request->getModule(false);
        return vtranslate('LBL_GLOBAL_SEARCH_PAGE_TITLE', $moduleName);
    }

    public function process(Vtiger_Request $request) {
        $configs = Settings_Vtiger_Config_Model::loadConfig('global_search', true);
        $moduleName = $request->getModule(false);

        $viewer = $this->getViewer($request);
        $viewer->assign('CONFIGS', $configs);
        $viewer->assign('MODULE_NAME', $moduleName);

        $viewer->display('modules/Settings/Vtiger/tpls/GlobalSearchConfig.tpl');
    }
}
```

**Key points**:
- Class name: `Settings_Vtiger_{ConfigName}_View`
- `getPageTitle()` — return translated page title
- `process()` — load config, assign to viewer, display template
- Template path: `modules/Settings/Vtiger/tpls/{ConfigName}.tpl` (absolute from webroot)
- No need to override `getHeaderScripts()`/`getHeaderCss()` — parent auto-loads

## Action Controller

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

class Settings_Vtiger_SaveGlobalSearchConfig_Action extends Settings_Vtiger_Basic_Action {

    function checkPermission(Vtiger_Request $request) {
        return true;
    }

    function validateRequest(Vtiger_Request $request) {
        $request->validateWriteAccess();
    }

    function process(Vtiger_Request $request) {
        $configs = $request->get('configs');
        Settings_Vtiger_Config_Model::saveConfig('global_search', $configs);

        $response = new Vtiger_Response();
        $response->setResult(true);
        $response->emit();
    }
}
```

**Key points**:
- Class name: `Settings_Vtiger_Save{ConfigName}_Action`
- Extends `Settings_Vtiger_Basic_Action`
- Use `exposeMethod()` in constructor for multiple actions in one controller
- Always type-cast request data: `(bool)`, `(string)`, `(int)`
- Return JSON via `Vtiger_Response`

### Multiple Actions Pattern (exposeMethod)

```php
class Settings_Vtiger_SaveCallCenterConfig_Action extends Settings_Vtiger_Basic_Action {

    public function __construct() {
        parent::__construct();
        $this->exposeMethod('saveConfig');
        $this->exposeMethod('toggleConfig');
        $this->exposeMethod('saveConnection');
        $this->exposeMethod('disconnect');
    }

    public function saveConfig(Vtiger_Request $request) { /* ... */ }
    public function toggleConfig(Vtiger_Request $request) { /* ... */ }
    public function saveConnection(Vtiger_Request $request) { /* ... */ }
    public function disconnect(Vtiger_Request $request) { /* ... */ }
}
```

Called from JS with `mode` param:
```javascript
let params = {
    module: 'Vtiger',
    parent: 'Settings',
    action: 'SaveCallCenterConfig',
    mode: 'saveConfig',       // maps to exposeMethod name
    config: formData,
};
```

## JS Controller

```javascript
/*
 * ConfigName.js
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

CustomView_BaseController_Js('Settings_Vtiger_ConfigName_Js', {}, {

    registerEvents: function () {
        this._super();
        this.registerFormSubmit();
    },

    getForm: function () {
        return $('form[name="configs"]');
    },

    registerFormSubmit: function () {
        var self = this;

        this.getForm().vtValidate({
            submitHandler: function () {
                var formData = self.getForm().serializeFormData();

                var params = {
                    module: 'Vtiger',
                    parent: 'Settings',
                    action: 'SaveConfigName',
                    configs: formData,
                };

                app.helper.showProgress();

                app.request.post({ data: params }).then(function (err, res) {
                    app.helper.hideProgress();

                    if (err) {
                        app.helper.showErrorNotification({ message: app.vtranslate('JS_SAVE_ERROR') });
                        return;
                    }

                    app.helper.showSuccessNotification({ message: app.vtranslate('JS_SAVE_SUCCESS') });
                });

                return;
            }
        });
    }
});
```

**Key points**:
- Class name: `Settings_Vtiger_{ConfigName}_Js`
- Extends `CustomView_BaseController_Js` (not Vtiger_List_Js or similar)
- Always call `this._super()` in `registerEvents()`
- Use `vtValidate()` for form submission with validation
- AJAX params: `module: 'Vtiger'`, `parent: 'Settings'`, `action: 'Save{ConfigName}'`

## URL Pattern

```
index.php?module=Vtiger&parent=Settings&view={ConfigName}
```

Examples:
```
index.php?module=Vtiger&parent=Settings&view=CallCenterConfig
index.php?module=Vtiger&parent=Settings&view=GlobalSearchConfig
index.php?module=Vtiger&parent=Settings&view=CallCenterConfig&tab=Connection&mode=ShowList
```

## Toggle (Enable/Disable) Pattern

Common pattern for feature toggle on config pages:

```javascript
toggleConfig: function (enable) {
    app.helper.showProgress();

    var params = {
        module: 'Vtiger',
        parent: 'Settings',
        action: 'SaveConfigName',
        mode: 'toggleConfig',
        enable: enable,
    };

    app.request.post({ data: params }).then(function (err, res) {
        app.helper.hideProgress();

        if (err) {
            app.helper.showErrorNotification({ message: err.message });
            return;
        }

        app.helper.showSuccessNotification({
            message: app.vtranslate(enable ? 'JS_ENABLE_SUCCESS' : 'JS_DISABLE_SUCCESS')
        });
    });
}
```

## Checklist: Creating a New Config View

1. Create view: `modules/Settings/Vtiger/views/{ConfigName}.php` extending `BaseConfig_View`
2. Create action: `modules/Settings/Vtiger/actions/Save{ConfigName}.php` extending `Basic_Action`
3. Create template: `modules/Settings/Vtiger/tpls/{ConfigName}.tpl`
4. Create JS: `modules/Settings/Vtiger/resources/{ConfigName}.js`
5. Create CSS: `modules/Settings/Vtiger/resources/{ConfigName}.css`
6. Add language keys to `languages/en_us/Settings/Vtiger.php` (or dev/ tier)
7. Register menu entry (if needed) via migration

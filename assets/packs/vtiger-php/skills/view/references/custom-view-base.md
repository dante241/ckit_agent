# CustomView_Base_View

> Module-level custom views with auto-loaded JS/CSS resources

## Source

`modules/CustomView/views/Base.php` — extends `Vtiger_Index_View`

## Key Behavior

- Auto-loads JS: `~/modules/{Module}/resources/{ViewName}.js` + fallback `~/modules/Vtiger/resources/{ViewName}.js`
- Auto-loads CSS: `~/modules/{Module}/resources/{ViewName}.css` + fallback `~/modules/Vtiger/resources/{ViewName}.css`
- Also loads `~/modules/CustomView/resources/BaseController.js` (parent JS controller)
- `~` prefix = absolute path from webroot (NOT dot notation)
- `$isFullView` constructor param: `true` = full page with header/footer, `false` = frameless
- **No need to override** `getHeaderScripts()`/`getHeaderCss()` — auto-loaded by base class

## File Structure

All files in `modules/<Module>/` (NOT `layouts/v7/`):

```
modules/<Module>/
├── views/<ViewName>.php      # extends CustomView_Base_View
├── actions/<ActionName>.php  # AJAX save endpoints
├── tpls/<ViewName>.tpl       # template (display() with absolute path)
├── resources/<ViewName>.js   # JS controller (auto-loaded)
└── resources/<ViewName>.css  # styles (auto-loaded)
```

## View Controller Skeleton

```php
class <Module>_<ViewName>_View extends CustomView_Base_View {

    function __construct() {
        parent::__construct($isFullView = true);
    }

    public function checkPermission(Vtiger_Request $request) {
        // Custom permission logic (not isAdmin — module-specific)
    }

    public function getPageTitle(Vtiger_Request $request) {
        return vtranslate('LBL_PAGE_TITLE', $request->getModule(false));
    }

    public function process(Vtiger_Request $request) {
        $tab = $request->get('tab', 'GeneralConfig');
        $config = <Module>_Config_Helper::getConfig();

        $viewer = $this->getViewer($request);
        $viewer->assign('TAB', $tab);
        $viewer->assign('CONFIG', $config);

        // Tab-specific data loading
        if ($tab == 'GeneralConfig') {
            $viewer->assign('ROLE_LIST', Settings_Roles_Record_Model::getAll());
        }

        // IMPORTANT: use display() with absolute path, NOT view()
        $viewer->display('modules/<Module>/tpls/<ViewName>.tpl');
    }
}
```

**Key difference from Vtiger_Index_View**: use `$viewer->display('modules/...')` (absolute path), not `$viewer->view('Name.tpl', $module)` (resolves to layouts/v7/).

## Multi-Tab Config TPL Pattern

```smarty
{strip}
<div id="config-page">
    <form autocomplete="off" id="config" name="config" data-tab="{$TAB}">
        <div class="box">
            <!-- Nav Tabs -->
            <div id="main-tabs-container">
                <ul class="nav nav-tabs tabs">
                    <li class="nav-item {if $TAB == 'GeneralConfig'}active{/if}">
                        <a class="nav-link" data-tab='GeneralConfig'
                           href="index.php?module=<Module>&view=<ViewName>&tab=GeneralConfig">
                            {vtranslate('LBL_GENERAL_CONFIG', $MODULE_NAME)}
                        </a>
                    </li>
                    <li class="nav-item {if $TAB == 'Connection'}active{/if}">
                        <a class="nav-link" data-tab='Connection'
                           href="index.php?module=<Module>&view=<ViewName>&tab=Connection">
                            {vtranslate('LBL_CONNECTION', $MODULE_NAME)}
                        </a>
                    </li>
                </ul>
            </div>

            <!-- Tab Content -->
            <div class="tab-content">
                {if $TAB == 'GeneralConfig'}
                    <div class="box-body tab-pane active">
                        {* General config form fields *}
                    </div>
                {elseif $TAB == 'Connection'}
                    <div class="box-body tab-pane active">
                        {* Connection form fields *}
                    </div>
                {/if}
            </div>
        </div>
    </form>
</div>
{/strip}
```

**Pattern notes**:
- Tab navigation via URL params (server-side rendering, NOT JS tab switching)
- Each tab = separate page load with `?tab=X` param
- `data-tab` attribute for JS controller to read active tab
- Form wraps entire config — JS uses `form.deepSerializeFormData()` to collect nested data
- `{strip}` removes whitespace for cleaner HTML output

## JS Controller Pattern

```javascript
CustomView_BaseController_Js('<Module>_<ViewName>_Js', {}, {

    registerEvents: function () {
        this._super();
        var tab = this.getActiveTab();

        if (tab == 'GeneralConfig') this.initGeneralConfig();
        if (tab == 'Connection') this.initConnection();
    },

    getForm: function () {
        return $('form#config');
    },

    getActiveTab: function () {
        return this.getForm().data('tab');
    },

    initGeneralConfig: function () {
        var form = this.getForm();
        form.find('.bootstrap-switch').bootstrapSwitch();
        form.find('.select2').select2();

        form.vtValidate({
            submitHandler: function () {
                // Collect and save config via AJAX
            }
        });
    }
});
```

**Key**: extends `CustomView_BaseController_Js` (loaded automatically by base view), NOT `Vtiger_List_Js` or similar.

## Real Examples in Codebase

| Module | View | Tabs |
|--------|------|------|
| `CPSocialIntegration` | `SocialConfig` | General, Zalo, Facebook, IndividualZalo, Website, Telegram |
| `CPSocialIntegration` | `Report` | (single view) |
| `CPSocialIntegration` | `Chat` | (single view) |

## When to Use

- Module-specific config pages (NOT global Settings — use `BaseConfig_View` instead)
- Custom reports with module-specific UI
- Chat/messaging interfaces
- Any custom full-page view needing auto-loaded resources from `modules/{Module}/resources/`

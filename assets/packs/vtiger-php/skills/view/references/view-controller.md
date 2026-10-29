# View Controller Patterns

> Reference for VTiger View class structure and lifecycle

## Full Page View (Vtiger_Index_View)

Complete page with header, navigation, and footer.

### Complete Example

```php
<?php

/**
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 */

class Contacts_CustomView_View extends Vtiger_Index_View {

    // Overrides keep the parent signature exactly (no added param/return types)
    function checkPermission(Vtiger_Request $request) {
        parent::checkPermission($request);   // forbidden-feature gate
        $moduleName = (string) $request->getModule();
        $recordId = (int) $request->get('record');

        // Module access, plus record access when a record is given
        if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView', $recordId ?: false)) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }

    // Render header and navigation - MUST call parent
    function preProcess(Vtiger_Request $request, $display = true) {
        $viewer = $this->getViewer($request);
        $viewer->assign('PAGE_TITLE', vtranslate('LBL_CUSTOM_VIEW', $request->getModule()));

        parent::preProcess($request, $display);
    }

    function process(Vtiger_Request $request) {
        $moduleName = (string) $request->getModule();
        $recordId = (int) $request->get('record');

        $recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);
        $customData = $this->getCustomData($recordModel);

        $viewer = $this->getViewer($request);
        $viewer->assign('MODULE', $moduleName);
        $viewer->assign('RECORD_ID', $recordId);
        $viewer->assign('RECORD', $recordModel);
        $viewer->assign('CUSTOM_DATA', $customData);

        $viewer->view('CustomView.tpl', $moduleName);
    }

    // Render footer - MUST call parent
    function postProcess(Vtiger_Request $request) {
        parent::postProcess($request);
    }

    // Header lists hold Vtiger_JsScript_Model objects — convert names, never append raw arrays
    function getHeaderScripts(Vtiger_Request $request) {
        $headerScriptInstances = parent::getHeaderScripts($request);
        $jsScriptInstances = $this->checkAndConvertJsScripts(['modules.Contacts.resources.CustomView']);

        return array_merge($headerScriptInstances, $jsScriptInstances);
    }

    function getHeaderCss(Vtiger_Request $request) {
        $headerCssInstances = parent::getHeaderCss($request);
        $cssInstances = $this->checkAndConvertCssStyles(['~modules/Contacts/resources/CustomView.css']);

        return array_merge($headerCssInstances, $cssInstances);
    }

    // New helper methods may be typed
    protected function getCustomData(Vtiger_Record_Model $recordModel): array {
        return [];
    }
}
```

## Ajax Fragment View (Vtiger_BasicAjax_View)

HTML fragment without header/footer - for modals and AJAX content.

### Complete Example

```php
<?php

/**
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 */

class Contacts_CustomModal_View extends Vtiger_BasicAjax_View {

    // Parent is `function checkPermission() { }` (no parameter) — keep that exact signature
    function checkPermission() {
        $request = new Vtiger_Request($_REQUEST, $_REQUEST);
        $moduleName = (string) $request->getModule();

        if (!Users_Privileges_Model::isPermitted($moduleName, 'EditView')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }

    // Render content - NO preProcess or postProcess
    function process(Vtiger_Request $request) {
        $moduleName = (string) $request->getModule();
        $recordId = (int) $request->get('record');

        $recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);

        $viewer = $this->getViewer($request);
        $viewer->assign('MODULE', $moduleName);
        $viewer->assign('RECORD_ID', $recordId);
        $viewer->assign('RECORD', $recordModel);

        $viewer->view('CustomModal.tpl', $moduleName);
    }
}
```

## Common VTiger View Base Classes

| View Type | Base Class | Use Case |
|-----------|-----------|----------|
| List | `Vtiger_List_View` | Record listing with filters |
| Detail | `Vtiger_Detail_View` | Record detail page |
| Edit | `Vtiger_Edit_View` | Create/edit form |
| Popup | `Vtiger_Popup_View` | Record selection popup |
| Index | `Vtiger_Index_View` | Custom full page |
| BasicAjax | `Vtiger_BasicAjax_View` | Ajax HTML fragment |
| QuickCreateAjax | `Vtiger_QuickCreateAjax_View` | Quick create modal |
| MassActionAjax | `Vtiger_MassActionAjax_View` | Bulk action modal |
| IndexAjax | `Vtiger_IndexAjax_View` | Ajax page content |
| Dashboard | `Vtiger_Dashboard_View` (file `modules/Vtiger/views/DashBoard.php`) | Dashboard widgets |
| Calendar | `Calendar_Calendar_View` | Calendar interface |
| CustomView | `CustomView_Base_View` | Module config, reports, custom pages |

## Template Rendering Pattern

```php
// Get viewer instance
$viewer = $this->getViewer($request);

// Assign variables (available as {$VAR} in template)
$viewer->assign('KEY', $value);
$viewer->assign('ARRAY_DATA', ['item1', 'item2']);
$viewer->assign('OBJECT', $recordModel);

// Render template (MUST pass module name)
$viewer->view('TemplateName.tpl', $moduleName);
```

## Critical Rules

1. **View = HTML Output**: Never return JSON. Use Action controllers for JSON.
2. **Always Call Parent**: In `preProcess()`, `postProcess()`, `getHeaderScripts()`, `getHeaderCss()`.
3. **Type Cast Request Data**: `(string)`, `(int)` for all `$request->get()` values.
4. **Module Name Required**: Always pass as 2nd arg to `$viewer->view()`.
5. **BasicAjax NO pre/postProcess**: Don't extend these methods in Ajax views.
6. **Assets via `checkAndConvertJsScripts()` / `checkAndConvertCssStyles()`**: dot names (`modules.Contacts.resources.CustomView`) or `~modules/...` paths; never raw arrays.

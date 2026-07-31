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

    /**
     * Security check - runs first
     */
    public function checkPermission(Vtiger_Request $request): void {
        $moduleName = (string) $request->getModule();

        // Check module access
        if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }

        // Check record access
        $recordId = (int) $request->get('record');
        if ($recordId > 0) {
            $recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);
            if (!$recordModel->isViewable()) {
                throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
            }
        }
    }

    /**
     * Render header and navigation - MUST call parent
     */
    public function preProcess(Vtiger_Request $request, $display = true): void {
        parent::preProcess($request, $display);

        // Optional: Add breadcrumbs or custom header elements
        $viewer = $this->getViewer($request);
        $viewer->assign('PAGE_TITLE', vtranslate('LBL_CUSTOM_VIEW', $request->getModule()));
    }

    /**
     * Main content rendering
     */
    public function process(Vtiger_Request $request): void {
        $moduleName = (string) $request->getModule();
        $recordId = (int) $request->get('record');

        // Prepare data
        $recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);
        $customData = $this->getCustomData($recordModel);

        // Assign to template
        $viewer = $this->getViewer($request);
        $viewer->assign('MODULE', $moduleName);
        $viewer->assign('RECORD_ID', $recordId);
        $viewer->assign('RECORD', $recordModel);
        $viewer->assign('CUSTOM_DATA', $customData);

        // Render template
        $viewer->view('CustomView.tpl', $moduleName);
    }

    /**
     * Render footer - MUST call parent
     */
    public function postProcess(Vtiger_Request $request): void {
        $viewer = $this->getViewer($request);
        $viewer->assign('FOOTER_DATA', $this->getFooterData());

        parent::postProcess($request);
    }

    /**
     * Include JavaScript files - MUST call parent
     */
    public function getHeaderScripts(Vtiger_Request $request): array {
        $scripts = parent::getHeaderScripts($request);

        // Add module-specific JS (dot notation)
        $scripts[] = ['src' => 'modules.Contacts.resources.CustomView'];

        return $scripts;
    }

    /**
     * Include CSS files - MUST call parent
     */
    public function getHeaderCss(Vtiger_Request $request): array {
        $css = parent::getHeaderCss($request);

        // Add module-specific CSS (dot notation)
        $css[] = ['href' => 'modules.Contacts.resources.CustomView'];

        return $css;
    }

    /**
     * Helper methods
     */
    protected function getCustomData(Vtiger_Record_Model $recordModel): array {
        // Business logic here
        return [];
    }

    protected function getFooterData(): array {
        return ['timestamp' => date('Y-m-d H:i:s')];
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

    /**
     * Security check - runs first
     */
    public function checkPermission(Vtiger_Request $request): void {
        $moduleName = (string) $request->getModule();

        if (!Users_Privileges_Model::isPermitted($moduleName, 'EditView')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }

    /**
     * Render content - NO preProcess or postProcess
     */
    public function process(Vtiger_Request $request): void {
        $moduleName = (string) $request->getModule();
        $recordId = (int) $request->get('record');

        // Prepare data
        $recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);

        // Assign to template
        $viewer = $this->getViewer($request);
        $viewer->assign('MODULE', $moduleName);
        $viewer->assign('RECORD_ID', $recordId);
        $viewer->assign('RECORD', $recordModel);

        // Render modal template
        $viewer->view('CustomModal.tpl', $moduleName);
    }
}
```

## 13 VTiger Screen Types

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
| DashBoard | `Vtiger_DashBoard_View` | Dashboard widgets |
| Unrelated | `Vtiger_Unrelated_View` | Related list picker |
| Calendar | `Vtiger_Calendar_View` | Calendar interface |
| PDF | `Vtiger_PDF_View` | PDF export |
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
6. **Dot Notation for Assets**: `modules.Contacts.resources.CustomView` not file paths.

---
name: view
description: "VTiger View controllers + Smarty TPL — trang HTML, modal, CustomView_Base_View, layouts. Use when: tạo/sửa trang, view, màn hình, template, tpl, modal, popup; keywords: view, page, Smarty, tpl, CustomView, render."
user-invocable: false
---

# VTiger View Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

Use this skill when:
- Creating new HTML pages or modals
- Rendering Smarty templates
- Building list views, detail views, edit forms
- Adding custom UI screens to modules
- Including CSS/JavaScript resources

## View Types

### 1. Custom Full Page View (`CustomView_Base_View`) — RECOMMENDED DEFAULT

Full page with **auto-loaded JS/CSS** from `modules/{Module}/resources/`. No need to override `getHeaderScripts`/`getHeaderCss`.

**Use for:** Config pages, reports, dashboards, any custom standalone page.

**File structure** (everything in `modules/<Module>/`, NOT `layouts/v7/`):
```
modules/<Module>/
├── views/<ViewName>.php      # extends CustomView_Base_View
├── tpls/<ViewName>.tpl       # template (use display() with absolute path)
├── resources/<ViewName>.js   # JS controller (auto-loaded, same name as view)
└── resources/<ViewName>.css  # styles (auto-loaded, same name as view)
```

**Skeleton:**
```php
<?php

/**
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.03.04
 */

class <Module>_<ViewName>_View extends CustomView_Base_View {

    function __construct() {
        parent::__construct($isFullView = true);
    }

    public function checkPermission(Vtiger_Request $request): void {
        $moduleName = (string) $request->getModule();
        if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }

    public function process(Vtiger_Request $request): void {
        $moduleName = (string) $request->getModule();

        $viewer = $this->getViewer($request);
        $viewer->assign('MODULE', $moduleName);
        // IMPORTANT: use display() with absolute path, NOT view()
        $viewer->display('modules/<Module>/tpls/<ViewName>.tpl');
    }
}
```

**JS controller** (auto-loaded, extends `CustomView_BaseController_Js`):
```javascript
CustomView_BaseController_Js('<Module>_<ViewName>_Js', {}, {

    registerEvents: function () {
        this._super();
        // register events here
    }
});
```

URL: `index.php?module=<Module>&view=<ViewName>`

### 2. Core View Override (`Vtiger_Index_View`)

For views that need header/footer/navigation and **extend core view types** (List, Detail, Edit).

**Use for:** Overriding standard List/Detail/Edit views, or pages that need `layouts/v7/` template resolution.

**File structure:**
```
modules/<Module>/
├── views/<ViewName>.php                          # extends Vtiger_Index_View
├── resources/<ViewName>.css                      # CSS (always here)
layouts/v7/modules/<Module>/
├── <ViewName>.tpl                                # template (use view())
├── resources/<ViewName>.js                       # JS controller (core views)
```

**Key difference:** Must manually include JS/CSS via `getHeaderScripts()`/`getHeaderCss()` with dot notation.

```php
public function process(Vtiger_Request $request): void {
    $viewer = $this->getViewer($request);
    $viewer->assign('MODULE', $request->getModule());
    $viewer->view('CustomView.tpl', $request->getModule());  // resolves to layouts/v7/
}

public function getHeaderScripts(Vtiger_Request $request): array {
    $scripts = parent::getHeaderScripts($request);
    $scripts[] = ['src' => 'modules.<Module>.resources.<ViewName>'];
    return $scripts;
}

public function getHeaderCss(Vtiger_Request $request): array {
    $css = parent::getHeaderCss($request);
    $css[] = ['href' => 'modules.<Module>.resources.<ViewName>'];
    return $css;
}
```

### 3. Consolidated Modal View — ViewModal.php (PREFERRED for modals)

> **ALWAYS prefer `ViewModal.php`** over creating separate modal view files.
> Check if `modules/{Module}/views/ViewModal.php` exists — reuse it. Create only if absent.

```php
class ModuleName_ViewModal_View extends CustomView_Base_View {
    function __construct() {
        $this->exposeMethod('getUpdateStatusModal');
        $this->exposeMethod('getAnotherModal');
    }

    function checkPermission(Vtiger_Request $request) { return; }

    public function process(Vtiger_Request $request) {
        $mode = $request->getMode();
        if (!empty($mode) && $this->isMethodExposed($mode)) {
            $this->invokeExposedMethod($mode, $request);
            return;
        }
    }

    public function getUpdateStatusModal(Vtiger_Request $request) {
        $viewer = new Vtiger_Viewer();
        $viewer->assign('MODULE', $request->getModule());
        $viewer->display('modules/ModuleName/tpls/UpdateStatusModal.tpl');
    }
}
```

URL: `index.php?module=SalesOrder&view=ViewModal&mode=getUpdateStatusModal&record=123`
TPL: `modules/{Module}/tpls/{ModalName}.tpl` (NOT layouts/v7/)

### 4. Ajax Fragment View (`Vtiger_BasicAjax_View`)

HTML fragment without header/footer. Use for standalone modal views (only if ViewModal.php is inappropriate):
- One-off modals and popups
- AJAX-loaded content

URL: `index.php?module=Contacts&view=CustomModal`

## Decision Guide

| Scenario | Use | Template render |
|----------|-----|-----------------|
| New custom page (config, report, dashboard) | `CustomView_Base_View` | `$viewer->display('modules/...')` |
| Override core view (List, Detail, Edit) | `Vtiger_*_View` | `$viewer->view('Name.tpl', $module)` |
| Modal dialog | `ViewModal.php` (consolidated) | `$viewer->display('modules/...')` |
| AJAX fragment | `Vtiger_BasicAjax_View` | `$viewer->view('Name.tpl', $module)` |

## View Lifecycle

```
Full Page: checkPermission → preProcess → process → postProcess
Ajax:      checkPermission → process
```

| Method | Purpose | Must Call Parent? |
|--------|---------|-------------------|
| `checkPermission()` | Security check | No |
| `preProcess()` | Render header/nav | **YES** |
| `process()` | Render main content | No |
| `postProcess()` | Render footer | **YES** |
| `getHeaderScripts()` | Include JS files (Vtiger_Index_View only) | **YES** |
| `getHeaderCss()` | Include CSS files (Vtiger_Index_View only) | **YES** |

## Critical Rules

1. **Default to `CustomView_Base_View`** for new custom views — auto-loads JS/CSS, simpler code
2. **Prefer ViewModal.php** — consolidate modal views into one file per module, use `mode` param to route
3. **View = HTML, Action = JSON**: Never return JSON from View. Use Action controllers instead.
4. **`display()` vs `view()`**: `CustomView_Base_View` uses `$viewer->display('modules/...')`, `Vtiger_Index_View` uses `$viewer->view('Name.tpl', $module)`
5. **TPL location**: `CustomView_Base_View` → `modules/{Module}/tpls/`, `Vtiger_Index_View` → `layouts/v7/modules/{Module}/`
6. **JS/CSS location**: Custom views → `modules/{Module}/resources/`, Core views → `layouts/v7/modules/{Module}/resources/` (JS) + `modules/{Module}/resources/` (CSS)
7. **Dot Notation for Assets**: Use `modules.Contacts.resources.CustomView` (NOT file paths) — only needed for `Vtiger_Index_View`
8. **Author name** — use actual dev name in file headers, never "Claude AI"

## Reference Files

- [CustomView Base](references/custom-view-base.md) - CustomView_Base_View, multi-tab config, auto-loaded resources
- [View Controller Patterns](references/view-controller.md) - Full class examples, lifecycle
- [CSS/JS Inclusion](references/css-js.md) - File separation rules, asset loading
- [Smarty Templates](references/smarty-tpl.md) - TPL syntax, translation, partials

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Custom view chuẩn CustomView_Base_View (tung.nguyen): `modules/CPMasterPlan/views/MasterPlanView.php`
- Ajax view (tung.nguyen): `modules/CPMasterPlan/views/ViewAjax.php`
- Core List view (tung.nguyen): `modules/CPProjectBoard/views/List.php`

## Verify

```bash
php -l <file>
rm -f test/templates_c/*.php   # clear Smarty cache — TPL đổi mà không clear = thấy bản cũ
curl -s 'http://localhost/vtiger/index.php?module=<Module>&view=<ViewName>' -H 'Cookie: PHPSESSID=<sid>' | grep -c '<div'
# Kỳ vọng: >0, không blank page. UI thay đổi → chụp screenshot qua chrome-devtools MCP đính vào report.
```

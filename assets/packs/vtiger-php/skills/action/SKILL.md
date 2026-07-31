---
name: action
description: "VTiger Action controllers — AJAX/JSON endpoint, HandleAjax, Vtiger_Response, checkPermission. Use when: tạo/sửa AJAX endpoint, xử lý form submit, api cho JS frontend; keywords: ajax, action, HandleAjax, JSON response, endpoint."
user-invocable: false
---

# VTiger Action Controllers

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Creating AJAX endpoints that return JSON
- Handling form submissions
- Processing asynchronous requests
- Building API-like endpoints for JavaScript

## Action vs View

| Aspect | Action | View |
|--------|--------|------|
| **Response** | JSON | HTML |
| **Extends** | `Vtiger_Action_Controller` | `Vtiger_Index_View` |
| **URL Param** | `action=ActionName` | `view=ViewName` |
| **Purpose** | AJAX backend | Page rendering |
| **Output** | `Vtiger_Response` | Smarty template |

## Action là CONTROLLER MỎNG (MANDATORY — layering)

Action bản chất là controller: chỉ nhận request, xử lý nhẹ, rồi chuyển cho helper. KHÔNG viết business logic hay SQL trong action.

```
Action (controller mỏng)          Logic Helper (xử lý)           Data Helper (lấy dữ liệu)
─────────────────────────         ──────────────────────         ─────────────────────────
1. checkPermission                {Module}_Logic_Helper          {Module}_Data_Helper
2. Type-cast request params  →    business rules, validate,  →   pquery/fetchByAssoc/
3. Gọi Logic helper               orchestration, transform        decodeUTF8 — DB ONLY
4. Wrap kết quả Vtiger_Response   (gọi Data để lấy dữ liệu)      (không business logic)
```

Luật:
- Action trong `process()`: chỉ cast params → gọi `{Module}_Logic_Helper::method()` → `setResult`/`setError` → `emit`. Thấy SQL hoặc vòng lặp xử lý dữ liệu trong action = SAI layer, dời xuống helper.
- Action KHÔNG gọi thẳng Data helper — luôn đi qua Logic (Logic mới là người gọi Data).
- Data helper: query + trả dữ liệu thô, KHÔNG chứa business rule. Logic helper: xử lý/quyết định, KHÔNG chứa SQL.
- Chi tiết 2 dạng helper: skill `module` → Helpers Pattern + `references/helpers-pattern.md`.

## Consolidated HandleAjax Pattern (PREFERRED)

> **ALWAYS prefer `HandleAjax.php`** over creating separate action files.
> Check if `modules/{Module}/actions/HandleAjax.php` exists — reuse it. Create only if absent.

### URL Pattern
```
index.php?module=SalesOrder&action=HandleAjax&mode=updateStatus&record=123
```

### HandleAjax Class Skeleton
```php
<?php

/**
 * @author Dev Name
 * @email dev@cloudgo.vn
 * @create date YYYY.MM.DD
 * @desc ModuleName Ajax Action Handler
 */

class ModuleName_HandleAjax_Action extends Vtiger_Action_Controller {

    function checkPermission(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $moduleModel = Vtiger_Module_Model::getInstance($moduleName);
        $currentUserPrivilegesModel = Users_Privileges_Model::getCurrentUserPrivilegesModel();

        if (!$currentUserPrivilegesModel->hasModulePermission($moduleModel->getId())) {
            throw new AppException(vtranslate($moduleName, $moduleName) . ' ' . vtranslate('LBL_NOT_ACCESSIBLE'));
        }
    }

    public function process(Vtiger_Request $request) {
        $mode = $request->get('mode');

        if (method_exists($this, $mode)) {
            $this->$mode($request);
        }
        else {
            $response = new Vtiger_Response();
            $response->setResult(['success' => 0]);
            $response->emit();
        }
    }

    function updateStatus(Vtiger_Request $request) {
        $response = new Vtiger_Response();

        try {
            // Business logic here
            $response->setResult(['success' => 1, 'message' => 'OK']);
        }
        catch (Exception $e) {
            $response->setError($e->getMessage());
        }

        $response->emit();
        return;
    }
}
```

### Frontend Call (HandleAjax)
```javascript
var params = {
    module: 'SalesOrder',
    action: 'HandleAjax',
    mode: 'updateStatus',
    record: recordId
};
app.request.post({ data: params }).then(function(error, data) { });
```

---

## Standalone Action (ONLY if HandleAjax is inappropriate)

### URL Pattern
```
index.php?module=CPGoal&action=CalculateProgress&record=123
```

### Action Class Skeleton

```php
<?php

class CPGoal_CalculateProgress_Action extends Vtiger_Action_Controller {

    public function checkPermission(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $recordId = (int) $request->get('record');

        // Check module permission
        if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView', $recordId)) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED', $moduleName));
        }
    }

    public function process(Vtiger_Request $request) {
        $response = new Vtiger_Response();

        try {
            $recordId = (int) $request->get('record');
            $record = Vtiger_Record_Model::getInstanceById($recordId, 'CPGoal');

            // Business logic
            $progress = $this->calculateProgress($record);

            $response->setResult([
                'success' => true,
                'progress' => $progress,
                'message' => vtranslate('LBL_PROGRESS_CALCULATED', 'CPGoal')
            ]);

        }
        catch (Exception $e) {
            $response->setError($e->getMessage());
        }

        $response->emit();
    }

    private function calculateProgress($record): float {
        // Implementation
        return 75.5;
    }
}
```

## Critical Pitfalls

1. **Prefer HandleAjax.php** — consolidate AJAX actions into one file per module, use `mode` param to route
2. **Action returns JSON only** — never echo HTML
3. **$response->emit() calls exit** — still add `return;` after for safety
4. **Always try-catch** in process method
5. **checkPermission must throw AppException** for access denied
6. **Frontend callback (err,data)** — error-first pattern
7. **Author name** — use actual dev name in file headers, never "Claude AI"
8. **Thin controller** — SQL/business logic trong action = sai layer; action chỉ cast params + gọi Logic helper + wrap response (xem mục "Action là CONTROLLER MỎNG")

## References

- [action-controller.md](references/action-controller.md) — Complete Action class template
- [ajax.md](references/ajax.md) — Frontend AJAX patterns with app.request

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- HandleAjax chuẩn (tung.nguyen 5/5 commits): `modules/CPMasterPlan/actions/HandleAjax.php`
- HandleAjax thứ hai để đối chiếu (tung.nguyen): `modules/CPProjectBoard/actions/HandleAjax.php`

CẤM viết action từ trí nhớ VTiger open-source — bắt chước file trên.

## Verify (chạy sau khi code, TRƯỚC khi báo xong)

```bash
php -l modules/<Module>/actions/HandleAjax.php
# Smoke endpoint (cookie session từ browser đang login):
curl -s 'http://localhost/vtiger/index.php?module=<Module>&action=HandleAjax&mode=<mode>' \
  -H 'Cookie: PHPSESSID=<sid>' --data '<params>' | head -c 500
# Kỳ vọng: JSON {"success":true,...} — KHÔNG phải HTML/blank/PHP error
```

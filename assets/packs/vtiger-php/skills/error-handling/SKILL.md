---
name: error-handling
description: "VTiger error handling — Vtiger_Response setError, AppException, LoggerManager, permission deny. Use when: xử lý lỗi, try-catch, trả lỗi JSON, log lỗi, exception; keywords: error, exception, log."
user-invocable: false
---

# VTiger Error Handling

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

- Implementing Action controllers with error responses
- Adding try-catch blocks around risky operations
- Logging errors, webhooks, API calls, admin actions
- Checking permissions before CRUD operations
- Gating features by license/package
- Handling external service failures
- Validating request data and CSRF tokens

## Error Handling Flow

```
Request → validateWriteAccess → checkPermission → isForbiddenFeature
  → Business Logic (try-catch)
    → On success: Vtiger_Response::setResult() + emit()
    → On failure: Vtiger_Response::setError() + saveLog()
```

## Quick Reference

### Core Components

| Component | Key Methods | Purpose |
|-----------|-------------|---------|
| `Vtiger_Response` | `setError($code, $msg)`, `setResult($data)`, `emit()` | AJAX response formatting |
| `AppException` | extends Exception, adds `$title` field | User-facing errors |
| `LoggerManager` | `getLogger($category)->info($log)` | Log4PHP logging |
| `checkPermission()` | `Users_Privileges_Model::hasModulePermission()` | Module/record access |
| `isForbiddenFeature()` | Returns boolean | License feature gate |

### When to Try-Catch

| Operation | Need Try-Catch? | Reason |
|-----------|-----------------|--------|
| `Vtiger_Record_Model::getInstanceById()` | **YES** | Throws if record not found/deleted |
| `$record->save()` | **YES** | Validation, duplicate, DB errors |
| `$record->delete()` | **YES** | DB constraints, handlers may throw |
| External API calls | **YES** | Timeout, connection, malformed response |
| Webhook handlers | **YES** | Unknown input format |
| `$request->get()` | NO | Returns null if missing |
| `vtlib_purify()` | NO | Sanitizes, doesn't throw |
| Simple `pquery()` | NO | Returns result object, check with `getAffectedRowCount()` |

## Critical Pitfalls

1. **Global Catch**: API entry points should catch both `Exception` and `\Throwable`
2. **Exit After setResponse**: `setResponse()` calls `exit` — still add `return;` after for safety
3. **Log Categories**: MUST exist in `log4php.properties` — new category requires appender config
4. **Permission Denied**: Use `AppException` (not Exception) for user-facing errors
5. **CSRF Validation**: Always call `validateWriteAccess()` in POST Actions
6. **Feature Gates**: Use `checkAccessForbiddenFeature()` to auto-throw, or `isForbiddenFeature()` + manual check

## File References

- [Response and Exception Patterns](./references/response-and-exception.md)
- [Logging Patterns](./references/logging.md)
- [Validation and Permission Patterns](./references/validation-and-permission.md)

## Examples from Codebase

### Basic Action Error Handling

```php
class MyModule_SaveData_Action extends Vtiger_Action_Controller {
    public function checkPermission(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $moduleModel = Vtiger_Module_Model::getInstance($moduleName);
        $userPrivilegesModel = Users_Privileges_Model::getCurrentUserPrivilegesModel();
        if (!$userPrivilegesModel->hasModulePermission($moduleModel->getId())) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }
    }

    public function process(Vtiger_Request $request) {
        $response = new Vtiger_Response();
        try {
            $recordId = (int) $request->get('record');
            $recordModel = Vtiger_Record_Model::getInstanceById($recordId);
            $recordModel->set('status', 'active');
            $recordModel->save();

            $response->setResult(['success' => true, 'id' => $recordId]);
        }
        catch (Exception $e) {
            $logger = LoggerManager::getLogger('PLATFORM');
            $logger->error('Save failed: ' . $e->getMessage());
            $response->setError(500, $e->getMessage());
        }
        $response->emit();
    }
}
```

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- try-catch + Vtiger_Response error shape (tung.nguyen): `modules/CPMasterPlan/actions/HandleAjax.php`

## Verify

```bash
# Test CẢ error path: gọi endpoint với input sai (id không tồn tại, thiếu param)
curl -s '...&mode=<mode>' --data 'record=999999999' -H 'Cookie: PHPSESSID=<sid>'
# Kỳ vọng: JSON {"success":false,"error":...} — KHÔNG phải stack trace / blank / HTML
```

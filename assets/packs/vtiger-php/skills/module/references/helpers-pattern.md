# VTiger Helpers Pattern

## Overview

Helpers separate concerns into two layers:
- **Data Helper**: database access (queries, raw rows)
- **Logic Helper**: business rules, validation, orchestration

Repo practice: helpers are classes of **static methods** — none of the ~128 `modules/*/helpers/Data.php` / `Logic.php` files use a `getInstance()` singleton. Call them as `CPDemo_Data_Helper::fetchActiveItems()`.

> The samples use a **fictional** module `CPDemo` (`vtiger_cpdemo`: `cpdemoid`, `name`, `demo_status`, `progress`, `target_value`, `due_date`). Check real column names with `SHOW COLUMNS FROM vtiger_<module>` / `vtiger_field` before writing SQL — a wrong column makes `pquery()` return `false` silently (skill `vtiger-pquery-silent-false`).

## Data Helper (Database Layer)

### Purpose
Execute database queries, return raw data. No business rules.

### Location
`modules/{Module}/helpers/Data.php`

### Pattern
```php
class CPDemo_Data_Helper {

	// Fetch active items with their owner
	public static function fetchActiveItems(): array {
		global $adb;

		$sql = "SELECT d.cpdemoid, d.name, d.progress, d.due_date, e.smownerid
			FROM vtiger_cpdemo AS d
			INNER JOIN vtiger_crmentity AS e ON (e.crmid = d.cpdemoid AND e.deleted = 0)
			WHERE d.demo_status = ?
			ORDER BY d.name ASC";
		$result = $adb->pquery($sql, ['Active']);
		$items = [];

		if ($result === false) return $items;

		while ($row = $adb->fetchByAssoc($result)) {
			$items[] = decodeUTF8($row);
		}

		return $items;
	}

	// Get one item row by id, null when missing
	public static function fetchItemById(int $itemId): ?array {
		global $adb;

		$sql = "SELECT d.cpdemoid, d.name, d.progress, d.target_value, d.due_date, e.smownerid
			FROM vtiger_cpdemo AS d
			INNER JOIN vtiger_crmentity AS e ON (e.crmid = d.cpdemoid AND e.deleted = 0)
			WHERE d.cpdemoid = ?";
		$result = $adb->pquery($sql, [$itemId]);

		if ($result === false || $adb->num_rows($result) === 0) return null;

		return decodeUTF8($adb->fetchByAssoc($result));
	}

	// Update progress directly (skips save handlers)
	public static function updateProgress(int $itemId, float $progress): bool {
		global $adb;

		$sql = "UPDATE vtiger_cpdemo SET progress = ? WHERE cpdemoid = ?";
		return $adb->pquery($sql, [$progress, $itemId]) !== false;
	}
}
```

### Key Rules for Data Helper

1. **Database operations** — no business rules
2. **Always use pquery** with params (never concatenate SQL)
3. **Check `=== false`** on the pquery result — it does not throw
4. **Always decodeUTF8** on fetchByAssoc results
5. **Join `vtiger_crmentity` with `deleted = 0`**; owner is `e.smownerid` (there is no `assigned_user_id` column)
6. **Type declarations** on parameters and return values of new methods

## Logic Helper (Business Logic Layer)

### Purpose
Business rules, validation, calculations, orchestration. Usually calls the Data helper for DB access.

### Location
`modules/{Module}/helpers/Logic.php`

### Pattern
```php
class CPDemo_Logic_Helper {

	// Active items with computed status fields
	public static function getActiveItems(): array {
		$items = CPDemo_Data_Helper::fetchActiveItems();

		foreach ($items as &$item) {
			$item['is_completed'] = (float) $item['progress'] >= 100;
			$item['days_remaining'] = self::calculateDaysRemaining((string) $item['due_date']);
		}
		unset($item);

		return $items;
	}

	// Validate and save an update through the Record model
	public static function processUpdate(int $itemId, array $updateData): array {
		if (empty($updateData['name'])) {
			return ['success' => false, 'message' => 'Name is required'];
		}

		try {
			$record = Vtiger_Record_Model::getInstanceById($itemId, 'CPDemo');
		}
		catch (Throwable $e) {
			// getInstanceById throws for a missing or deleted record
			return ['success' => false, 'message' => 'Record not found'];
		}

		if (isset($updateData['current_value'])) {
			$updateData['progress'] = self::calculateProgress((float) $updateData['current_value'], (float) $record->get('target_value'));
			unset($updateData['current_value']);
		}

		$record->set('mode', 'edit');

		foreach ($updateData as $field => $value) {
			$record->set($field, $value);
		}

		$record->save();

		return ['success' => true, 'record_id' => $record->getId()];
	}

	private static function calculateProgress(float $current, float $target): float {
		if ($target == 0) return 0;

		return min(($current / $target) * 100, 100);
	}

	private static function calculateDaysRemaining(string $dueDate): int {
		if (in_array($dueDate, ['', '0000-00-00'])) return 0;

		return (int) floor((strtotime($dueDate) - strtotime(date('Y-m-d'))) / 86400);
	}
}
```

### Key Rules for Logic Helper

1. **Business logic** — validation, calculations, transformations
2. **Prefer the Data helper for SQL**; existing modules (e.g. `CPMasterPlan_Logic_Helper`) still run `pquery` inline — keep new SQL in Data when practical
3. **Type declarations** on parameters and return values of new methods
4. **Private methods** for internal logic

## Splitting Helpers (>200 Lines)

When a helper exceeds 200 lines, split by purpose:

```
modules/CPDemo/helpers/
├── Data.php                    # CPDemo_Data_Helper
├── Logic.php                   # CPDemo_Logic_Helper
├── Calculation.php             # CPDemo_Calculation_Helper
└── Notification.php            # CPDemo_Notification_Helper
```

Naming pattern: `{Module}_{Purpose}_Helper`, file name = `{Purpose}.php` (no `Helper` suffix).

## Who Calls What

- Views/Actions call the **Logic** helper for business flows (validate → save → side effects).
- Plain read-only lookups may call the **Data** helper directly — the repo does this widely (e.g. `CPMasterPlan/views/ViewAjax.php`, `CPMasterPlan/actions/HandleAjax.php`).

```php
class CPDemo_HandleAjax_Action extends Vtiger_Action_Controller {

	public function checkPermission(Vtiger_Request $request) {
		$request->validateWriteAccess();

		if (!Users_Privileges_Model::isPermitted($request->getModule(), 'EditView', (int) $request->get('record'))) {
			throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
		}
	}

	public function process(Vtiger_Request $request) {
		$updateData = $request->get('data') ?: [];
		$result = CPDemo_Logic_Helper::processUpdate((int) $request->get('record'), is_array($updateData) ? $updateData : []);

		$response = new Vtiger_Response();
		$response->setResult($result);
		$response->emit();
	}
}
```

Pitfalls: see [SKILL.md § Critical Pitfalls](../SKILL.md#critical-pitfalls).

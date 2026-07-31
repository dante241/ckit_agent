# VTiger Helpers Pattern

## Overview

Helpers separate concerns into two layers:
- **Data Helper**: Database access only (queries, raw data)
- **Logic Helper**: Business logic, validation, orchestration

## Data Helper (Database Layer)

### Purpose
Execute database queries, return raw data. NO business logic.

### Location
`modules/{Module}/helpers/Data.php`

### Pattern
```php
class CPGoal_Data_Helper {

    private static $instance = null;

    public static function getInstance(): self {
        if (self::$instance === null) {
            self::$instance = new self();
        }
        return self::$instance;
    }

    /**
     * Fetch active goals from database
     * @return array Raw database rows
     */
    public function fetchActiveGoals(): array {
        global $adb;

        $sql = "SELECT g.*, e.assigned_user_id
                FROM vtiger_cpgoal g
                INNER JOIN vtiger_crmentity e ON e.crmid = g.cpgoalid
                WHERE e.deleted = 0 AND g.status = ?
                ORDER BY g.goal_name ASC";

        $result = $adb->pquery($sql, ['Active']);
        $goals = [];

        while ($row = $adb->fetchByAssoc($result)) {
            $goals[] = decodeUTF8($row);  // CRITICAL: Always decode
        }

        return $goals;
    }

    /**
     * Get goal by ID
     * @param int $goalId
     * @return array|null Goal data or null
     */
    public function fetchGoalById(int $goalId): ?array {
        global $adb;

        $sql = "SELECT g.*, e.assigned_user_id
                FROM vtiger_cpgoal g
                INNER JOIN vtiger_crmentity e ON e.crmid = g.cpgoalid
                WHERE e.deleted = 0 AND g.cpgoalid = ?";

        $result = $adb->pquery($sql, [$goalId]);

        if ($adb->num_rows($result) > 0) {
            return decodeUTF8($adb->fetchByAssoc($result));
        }

        return null;
    }

    /**
     * Update goal progress (direct SQL for performance)
     * @param int $goalId
     * @param float $progress
     * @return bool Success
     */
    public function updateGoalProgress(int $goalId, float $progress): bool {
        global $adb;

        $sql = "UPDATE vtiger_cpgoal SET progress_percentage = ? WHERE cpgoalid = ?";
        $adb->pquery($sql, [$progress, $goalId]);

        return true;
    }

    /**
     * Search goals by keyword
     * @param string $keyword
     * @return array Matching goals
     */
    public function searchGoals(string $keyword): array {
        global $adb;

        $keyword = '%' . $keyword . '%';

        $sql = "SELECT g.*, e.assigned_user_id
                FROM vtiger_cpgoal g
                INNER JOIN vtiger_crmentity e ON e.crmid = g.cpgoalid
                WHERE e.deleted = 0
                AND (g.goal_name LIKE ? OR g.description LIKE ?)
                ORDER BY g.goal_name ASC";

        $result = $adb->pquery($sql, [$keyword, $keyword]);
        $goals = [];

        while ($row = $adb->fetchByAssoc($result)) {
            $goals[] = decodeUTF8($row);
        }

        return $goals;
    }
}
```

### Key Rules for Data Helper

1. **Only database operations** — no business logic
2. **Always use pquery** with params (never concatenate SQL)
3. **Always decodeUTF8** on fetchByAssoc results
4. **Return raw data** — no transformations
5. **Type declarations** on parameters and return values
6. **Singleton pattern** for instance management

## Logic Helper (Business Logic Layer)

### Purpose
Business rules, validation, calculations, orchestration. Calls Data helper for DB access.

### Location
`modules/{Module}/helpers/Logic.php`

### Pattern
```php
class CPGoal_Logic_Helper {

    private static $instance = null;

    public static function getInstance(): self {
        if (self::$instance === null) {
            self::$instance = new self();
        }
        return self::$instance;
    }

    /**
     * Get active goals with calculated progress
     * @return array Goals with business logic applied
     */
    public function getActiveGoals(): array {
        $data = CPGoal_Data_Helper::getInstance();
        $goals = $data->fetchActiveGoals();

        // Apply business logic
        foreach ($goals as &$goal) {
            $goal['is_completed'] = $this->isGoalCompleted($goal);
            $goal['status_label'] = $this->getStatusLabel($goal);
            $goal['days_remaining'] = $this->calculateDaysRemaining($goal);
        }

        return $goals;
    }

    /**
     * Process goal update with validation
     * @param int $goalId
     * @param array $updateData
     * @return array Result with status and message
     */
    public function processGoalUpdate(int $goalId, array $updateData): array {
        // Validation
        if (empty($updateData['goal_name'])) {
            return [
                'success' => false,
                'message' => 'Goal name is required',
            ];
        }

        // Get existing record
        $record = Vtiger_Record_Model::getInstanceById($goalId, 'CPGoal');

        if (empty($record)) {
            return [
                'success' => false,
                'message' => 'Goal not found',
            ];
        }

        // Business logic: Calculate progress
        if (isset($updateData['current_value']) && isset($updateData['target_value'])) {
            $progress = $this->calculateProgress(
                (float) $updateData['current_value'],
                (float) $updateData['target_value']
            );
            $updateData['progress_percentage'] = $progress;
        }

        // Update record
        $record->set('mode', 'edit');
        foreach ($updateData as $field => $value) {
            $record->set($field, $value);
        }
        $record->save();

        return [
            'success' => true,
            'message' => 'Goal updated successfully',
            'record_id' => $record->getId(),
        ];
    }

    /**
     * Calculate goal progress percentage
     * @param float $current
     * @param float $target
     * @return float Progress percentage
     */
    private function calculateProgress(float $current, float $target): float {
        if ($target == 0) return 0;

        $progress = ($current / $target) * 100;
        return min($progress, 100);  // Cap at 100%
    }

    /**
     * Check if goal is completed
     * @param array $goal
     * @return bool
     */
    private function isGoalCompleted(array $goal): bool {
        $progress = (float) ($goal['progress_percentage'] ?? 0);
        return $progress >= 100;
    }

    /**
     * Get human-readable status label
     * @param array $goal
     * @return string
     */
    private function getStatusLabel(array $goal): string {
        $progress = (float) ($goal['progress_percentage'] ?? 0);

        if ($progress >= 100) return 'Completed';
        if ($progress >= 75) return 'On Track';
        if ($progress >= 50) return 'In Progress';
        if ($progress > 0) return 'Started';

        return 'Not Started';
    }

    /**
     * Calculate days remaining until goal deadline
     * @param array $goal
     * @return int Days remaining (negative if overdue)
     */
    private function calculateDaysRemaining(array $goal): int {
        if (empty($goal['deadline_date'])) return 0;

        $deadline = strtotime($goal['deadline_date']);
        $today = strtotime(date('Y-m-d'));

        $diff = $deadline - $today;
        return (int) floor($diff / 86400);
    }
}
```

### Key Rules for Logic Helper

1. **Calls Data helper** for database access
2. **Business logic only** — validation, calculations, transformations
3. **Never use global $adb** directly
4. **Type declarations** on parameters and return values
5. **Singleton pattern** for instance management
6. **Private methods** for internal logic

## Splitting Helpers (>200 Lines)

When helper exceeds 200 lines, split by purpose:

### Example: Split by Domain
```
modules/CPGoal/helpers/
├── Data.php                    # Base Data helper
├── GoalCalculation.php         # CPGoal_GoalCalculation_Helper
├── GoalValidation.php          # CPGoal_GoalValidation_Helper
└── GoalNotification.php        # CPGoal_GoalNotification_Helper
```

### Naming Pattern
`{Module}_{Purpose}_Helper`

```php
class CPGoal_GoalCalculation_Helper {
    public function calculateProgress(): float { }
    public function calculateMetrics(): array { }
}

class CPGoal_GoalValidation_Helper {
    public function validateGoalData(array $data): array { }
    public function checkPermissions(int $userId): bool { }
}
```

## Usage Rules

### ❌ WRONG: Action calls Data directly
```php
class CPGoal_Save_Action extends Vtiger_Action_Controller {
    public function process(Vtiger_Request $request) {
        $data = CPGoal_Data_Helper::getInstance();
        $goals = $data->fetchActiveGoals();  // NEVER!
    }
}
```

### ✅ CORRECT: Action calls Logic, Logic calls Data
```php
class CPGoal_Save_Action extends Vtiger_Action_Controller {
    public function process(Vtiger_Request $request) {
        $logic = CPGoal_Logic_Helper::getInstance();
        $goals = $logic->getActiveGoals();  // Logic handles business rules
    }
}
```

## Critical Pitfalls

1. **Always decodeUTF8** in Data helper after fetchByAssoc
2. **Never business logic in Data helper** — only queries
3. **Never global $adb in Logic helper** — use Data helper
4. **Always pquery with params** — no SQL concatenation
5. **Split helpers at 200 lines** — maintain focused modules

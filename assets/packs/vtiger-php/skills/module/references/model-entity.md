# VTiger Models & Entity

## Entity Definition (CRMEntity)

### Purpose
Define database structure, fields, relationships for a module.

### Location
`modules/{Module}/{Module}.php`

### Class Structure
```php
class CPGoal extends CRMEntity {
    // Table definitions
    public $table_name = 'vtiger_cpgoal';
    public $table_index = 'cpgoalid';

    // MUST include vtiger_crmentity for proper CRM behavior
    public $tab_name = [
        'vtiger_crmentity',
        'vtiger_cpgoal',
        'vtiger_cpgoalcf',  // Custom fields
    ];

    public $tab_name_index = [
        'vtiger_crmentity' => 'crmid',
        'vtiger_cpgoal' => 'cpgoalid',
        'vtiger_cpgoalcf' => 'cpgoalid',
    ];

    // Field definitions
    public $column_fields = [
        'goal_name' => '',
        'goal_type' => '',
        'target_value' => 0,
        'assigned_user_id' => '',
    ];

    // Custom fields table
    public $customFieldTable = ['vtiger_cpgoalcf', 'cpgoalid'];

    // ListView columns
    public $list_fields_name = [
        'Goal Name' => 'goal_name',
        'Type' => 'goal_type',
        'Assigned To' => 'assigned_user_id',
    ];

    // Default order
    public $default_order_by = 'goal_name';
    public $default_sort_order = 'ASC';
}
```

### Critical Rule
**Entity tab_name MUST include `vtiger_crmentity`** for:
- Ownership tracking (assigned_user_id)
- Soft delete support (deleted field)
- Creation/modification timestamps
- CRM-wide features (sharing, workflows, etc.)

## Record Model (Instance Operations)

### Purpose
CRUD operations on individual records.

### Location
`modules/{Module}/models/Record.php`

### Get Existing Record
```php
// By ID
$record = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);

// By conditions
$conditions = ['field_name' => $value];
$record = Vtiger_Record_Model::getInstanceByConditions($moduleName, $conditions);
```

### Create New Record
```php
$record = Vtiger_Record_Model::getCleanInstance($moduleName);
$record->set('field_name', $value);
$record->set('assigned_user_id', $userId);
$record->save();

$recordId = $record->getId();
```

### Update Existing Record
```php
// Method 1: set + save
$record = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);
$record->set('mode', 'edit');  // CRITICAL: Required for updates
$record->set('field_name', $newValue);
$record->save();

// Method 2: updateData (batch update)
$data = [
    'field_name' => $newValue,
    'another_field' => $anotherValue,
];
$record->updateData($data)->save();
```

### Delete Record
```php
$record->delete();  // Soft delete (sets deleted=1)
```

### Get Field Values
```php
$value = $record->get('field_name');
$id = $record->getId();
$moduleName = $record->getModuleName();
```

### Custom Record Model
```php
class CPGoal_Record_Model extends Vtiger_Record_Model {

    // Custom getters
    public function getGoalProgress(): float {
        $target = (float) $this->get('target_value');
        $current = (float) $this->get('current_value');

        if ($target == 0) return 0;
        return ($current / $target) * 100;
    }

    // Custom business logic
    public function isCompleted(): bool {
        return $this->getGoalProgress() >= 100;
    }

    // Custom save logic (override)
    public function save() {
        // Pre-save calculations
        $this->set('progress_percentage', $this->getGoalProgress());

        return parent::save();
    }
}
```

## Module Model (Module-Level Operations)

### Purpose
Module-wide operations, metadata, search.

### Location
`modules/{Module}/models/Module.php`

### Get Module Instance
```php
$moduleModel = Vtiger_Module_Model::getInstance($moduleName);
```

### Module Information
```php
$moduleId = $moduleModel->getId();
$moduleName = $moduleModel->getName();
$isEntityModule = $moduleModel->isEntityModule();
```

### Field Information
```php
// Get all fields
$fields = $moduleModel->getFields();

// Get specific field
$field = $moduleModel->getField('field_name');
$fieldType = $field->getFieldDataType();
$uitype = $field->get('uitype');
```

### Custom Module Model
```php
class CPGoal_Module_Model extends Vtiger_Module_Model {

    // Module-wide queries
    public function getAllActiveGoals(): array {
        $logic = CPGoal_Logic_Helper::getInstance();
        return $logic->fetchActiveGoals();
    }

    // Module statistics
    public function getModuleStatistics(): array {
        return [
            'total_goals' => $this->getRecordCount(),
            'completed_goals' => $this->getCompletedCount(),
        ];
    }

    // Custom search
    public function searchByKeyword(string $keyword): array {
        $data = CPGoal_Data_Helper::getInstance();
        return $data->searchGoals($keyword);
    }
}
```

## ListView Model

### Purpose
Handle list view queries, filtering, pagination.

### Location
`modules/{Module}/models/ListView.php`

### Custom ListView
```php
class CPGoal_ListView_Model extends Vtiger_ListView_Model {

    // Custom query conditions
    public function getBasicLinks() {
        $links = parent::getBasicLinks();

        // Add custom list view button
        $links[] = [
            'linktype' => 'LISTVIEWBASIC',
            'linklabel' => 'LBL_SYNC_GOALS',
            'linkurl' => 'javascript:CPGoal_List_Js.syncGoals()',
            'linkicon' => 'fa-refresh',
        ];

        return $links;
    }
}
```

## Critical Pitfalls

1. **Always set('mode','edit')** before updating records
2. **Always decodeUTF8** on database results:
```php
while ($row = $adb->fetchByAssoc($result)) {
    $row = decodeUTF8($row);  // CRITICAL
}
```

3. **Always use pquery with params**:
```php
// ❌ WRONG
$sql = "SELECT * FROM table WHERE id = " . $id;

// ✅ CORRECT
$sql = "SELECT * FROM table WHERE id = ?";
$result = $adb->pquery($sql, [$id]);
```

4. **Entity tab_name MUST include vtiger_crmentity**

5. **Never call Data helper from Action/View** — use Logic helper instead

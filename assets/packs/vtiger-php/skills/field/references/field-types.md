# VTiger Field Types (UITypes)

## Built-in UITypes (35 Types)

| UIType | Category | Type | Description | DB Type |
|--------|----------|------|-------------|---------|
| **1** | Text | Text | Single-line text | varchar(100) |
| **2** | Text | Text | Long text field | varchar(255) |
| **19** | Text | Text | Standard text field | varchar(100) |
| **21** | Text | Textarea | Multi-line text area | text |
| **24** | Text | Text | Text with special handling | text |
| **7** | Number | Integer | Integer number | int(11) |
| **9** | Number | Decimal | Decimal/float with precision | decimal(25,8) |
| **71** | Number | Currency | Currency with symbol | decimal(25,8) |
| **72** | Number | Currency | Secondary currency | decimal(25,8) |
| **11** | Contact | Phone | Phone number | varchar(50) |
| **13** | Contact | Email | Email address | varchar(100) |
| **17** | Contact | URL | Website URL | varchar(255) |
| **85** | Contact | Skype | Skype ID | varchar(100) |
| **5** | DateTime | Date | Date picker (Y-m-d) | date |
| **6** | DateTime | Date | Date without user format | date |
| **14** | DateTime | Time | Time picker (HH:mm) | time |
| **50** | DateTime | DateTime | Date + time picker | datetime |
| **70** | DateTime | Date | Creation date (auto) | datetime |
| **15** | Picklist | Picklist | Single-select dropdown | varchar(100) |
| **16** | Picklist | Picklist | Dropdown with dependency | varchar(100) |
| **33** | Picklist | Multipicklist | Multi-select dropdown | text |
| **115** | Picklist | Picklist | Advanced picklist | varchar(100) |
| **56** | Boolean | Checkbox | Boolean checkbox | int(1) |
| **10** | Reference | Related | Related module (1:1) | int(19) |
| **51** | Reference | Related | Related with custom | int(19) |
| **66** | Reference | Related | Related list | int(19) |
| **73** | Reference | Related | Account reference | int(19) |
| **53** | Owner | Owner | Assigned user/group | int(19) |
| **69** | Media | Image | Image upload | text |
| **28** | Media | Document | Document/file | int(19) |
| **4** | System | AutoNumber | Auto-increment number | varchar(100) |
| **52** | System | Owner | Creator user | int(19) |
| **77** | System | Status | Record status | varchar(50) |
| **120** | Custom | Shared | Shared owner | text |
| **1024** | Custom | Custom | Base for custom UITypes | varies |

## Picklist Operations

### Get Picklist Values
```php
// Method 1: Vtiger_Util_Helper
$values = Vtiger_Util_Helper::getPickListValues('fieldname');

// Returns array: ['value1', 'value2', 'value3']
```

```php
// Method 2: Field Model
$field = Vtiger_Field_Model::getInstance('fieldname', $moduleModel);
$picklistValues = $field->getPicklistValues();

// Returns array: [
//     ['label' => 'Display', 'value' => 'actual_value'],
//     ...
// ]
```

### Get Translated Picklist Value
```php
// Single value translation
$label = getTranslatedString($value, $moduleName);

// In template
{vtranslate($VALUE, $MODULE)}
```

### Create Picklist in Migration
```php
$this->createPicklistValues('fieldname', 'ModuleName', [
    ['key' => 'active', 'color' => '#28a745'],
    ['key' => 'inactive', 'color' => '#dc3545'],
    ['key' => 'pending', 'color' => '#ffc107'],
], '', true);  // Last param: replace existing
```

### Update Picklist Values Programmatically
```php
$field = Vtiger_Field::getInstance('fieldname', $moduleInstance);
$field->setPicklistValues([
    'New Value 1',
    'New Value 2',
    'New Value 3',
]);
```

## Field Data Type Mapping

| UIType | getFieldDataType() | JavaScript Type |
|--------|-------------------|-----------------|
| 1, 2, 19 | 'string' | string |
| 7 | 'integer' | number |
| 9, 71, 72 | 'double' | number |
| 5, 6, 50 | 'date' | Date |
| 14 | 'time' | string |
| 11 | 'phone' | string |
| 13 | 'email' | string |
| 17 | 'url' | string |
| 15, 16 | 'picklist' | string |
| 33 | 'multipicklist' | array |
| 56 | 'boolean' | boolean |
| 10, 51, 66, 73 | 'reference' | number |
| 53 | 'owner' | number |
| 69 | 'image' | string (path) |

## Common Field Properties

### Via Field Model
```php
$field = Vtiger_Field_Model::getInstance('fieldname', $moduleModel);

// Basic properties
$uitype = $field->get('uitype');
$fieldName = $field->getName();
$fieldLabel = $field->get('label');
$tableName = $field->getTableName();
$columnName = $field->getColumnName();

// Field characteristics
$isMandatory = $field->isMandatory();
$isEditable = $field->isEditable();
$isWritable = $field->isWritable();
$isViewable = $field->isViewable();

// Data type
$dataType = $field->getFieldDataType();
$displayType = $field->getDisplayType();
```

## Field Display Types

| Display Type | Meaning | Behavior |
|-------------|---------|----------|
| 1 | Visible | Show in Edit and Detail |
| 2 | Read-only | Show in Detail only |
| 3 | Hidden | Never show in forms |
| 4 | Hidden | Internal field |

## Multipicklist Operations

### Save Multipicklist
```php
// Format: pipe-separated with leading/trailing pipes
$value = ' |~|~| value1 |~|~| value2 |~|~| value3 |~|~|';
$record->set('multipicklist_field', $value);
```

### Read Multipicklist
```php
$raw = $record->get('multipicklist_field');
// " |~|~| value1 |~|~| value2 |~|~|"

// Convert to array
$values = array_filter(explode(' |##| ', $raw));
// ['value1', 'value2']

// Get display value (uses Record Model)
$displayValue = $record->getDisplayValue('multipicklist_field');
// "Value 1, Value 2"
```

## Reference Field (UIType 10)

### Get Referenced Record
```php
$referenceId = $record->get('related_field');

if (!empty($referenceId)) {
    $referenceModule = $field->getReferenceList()[0];
    $referenceRecord = Vtiger_Record_Model::getInstanceById($referenceId, $referenceModule);
}
```

### Set Reference Field
```php
$record->set('related_field', $relatedRecordId);
```

## Critical Rules

1. **UIType cannot change** after field creation — must delete/recreate
2. **Custom UITypes start at 1024** to avoid conflicts
3. **Multipicklist format**: ` |~|~| value |~|~|` (spaces + pipes)
4. **Picklist keys** are stored, labels are translated
5. **Field names** use snake_case for database columns
6. **Always run Quick Repair** after field schema changes

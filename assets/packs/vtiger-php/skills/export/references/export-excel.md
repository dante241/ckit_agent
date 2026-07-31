# Export Excel - FastExcelHelper

## Overview

FastExcelHelper wraps `avadim/fast-excel-writer` for high-performance Excel exports with formatting.

## Basic Pattern

```php
use include\utils\FastExcelHelper;

// In your Action/View class
$header = [
    ['name' => 'Account Name'],
    ['name' => 'Phone'],
    ['name' => 'Amount', 'type' => 'currency', 'format' => '#,##0.00'],
];

$data = [];
while ($row = $adb->fetchByAssoc($result)) {
    $row = decodeUTF8($row); // CRITICAL for Vietnamese text
    $data[] = [
        $row['accountname'],
        $row['phone'],
        (float) $row['amount'], // Type cast for numeric
    ];
}

$fileName = 'accounts_export_' . date('YmdHis') . '.xlsx';
FastExcelHelper::makeFile($header, $data, $fileName);

// NO CODE HERE - makeFile calls ob_clean() and save() which outputs file
```

## Header Structure with Types

```php
$header = [
    // Simple text column
    ['name' => 'ID'],

    // Integer with thousand separator
    ['name' => 'Quantity', 'type' => 'integer', 'format' => '#,##0'],

    // Currency/decimal
    ['name' => 'Amount', 'type' => 'currency', 'format' => '#,##0.00'],

    // Percentage
    ['name' => 'Growth Rate', 'type' => 'percentage', 'format' => '0.00%'],

    // Date only
    ['name' => 'Created Date', 'type' => 'date', 'format' => 'DD-MM-YYYY'],

    // Date and time
    ['name' => 'Modified', 'type' => 'datetime', 'format' => 'DD-MM-YYYY HH:MM:SS'],
];
```

## Column Format Mapping

| Type | Excel Format | PHP Input | Display |
|------|-------------|-----------|---------|
| `integer` | `#,##0` | `(int) $value` | 1,234 |
| `double` | `#,##0.00` | `(float) $value` | 1,234.56 |
| `currency` | `#,##0.00` | `(float) $value` | 1,234.56 |
| `percentage` | `0.00%` | `0.1234` | 12.34% |
| `date` | `DD-MM-YYYY` | `2025-02-10` | 10-02-2025 |
| `datetime` | `DD-MM-YYYY HH:MM:SS` | `2025-02-10 14:30:00` | 10-02-2025 14:30:00 |

## Internal Flow

1. **Create Workbook**: FastExcelWriter instance
2. **Write Header**: Bold white text, blue background (#4472C4)
3. **Write Body**: Borders, auto-width columns, text wrapping
4. **Clean Output Buffer**: `ob_clean()` removes any prior output
5. **Save**: Streams file directly to browser with download headers

## Header Styling (Auto-Applied)

- Font: Bold, white color
- Background: Blue (#4472C4)
- Alignment: Center horizontal/vertical
- Height: 25

## Body Styling (Auto-Applied)

- Borders: Thin black on all sides
- Auto-width: Columns sized to content
- Text wrap: Enabled
- Alignment: Top-left

## Data Preparation

```php
// ALWAYS decode UTF-8 for Vietnamese text
while ($row = $adb->fetchByAssoc($result)) {
    $row = decodeUTF8($row);

    // Type cast numeric values
    $data[] = [
        $row['accountname'],                    // String
        (int) $row['id'],                       // Integer
        (float) $row['amount'],                 // Float
        DateTimeField::convertToUserFormat(     // Date conversion
            $row['createdtime']
        ),
    ];
}
```

## Permission Check

```php
if (!Users_Privileges_Model::isPermitted($moduleName, 'Export')) {
    throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
}
```

## Complete Example

```php
class Accounts_ExportExcel_Action extends Vtiger_Action_Controller {

    public function process(Vtiger_Request $request) {
        $moduleName = $request->getModule();

        // Check permission
        if (!Users_Privileges_Model::isPermitted($moduleName, 'Export')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }

        // Get records
        $sql = "SELECT accountname, phone, amount, createdtime
                FROM vtiger_account
                INNER JOIN vtiger_crmentity ON crmid = accountid
                WHERE deleted = 0
                LIMIT 10000";
        $result = $GLOBALS['adb']->query($sql);

        // Build header
        $header = [
            ['name' => 'Account Name'],
            ['name' => 'Phone'],
            ['name' => 'Amount', 'type' => 'currency', 'format' => '#,##0.00'],
            ['name' => 'Created Date', 'type' => 'date', 'format' => 'DD-MM-YYYY'],
        ];

        // Build data
        $data = [];
        while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
            $row = decodeUTF8($row);
            $data[] = [
                $row['accountname'],
                $row['phone'],
                (float) $row['amount'],
                $row['createdtime'],
            ];
        }

        // Generate file
        $fileName = 'accounts_' . date('YmdHis') . '.xlsx';
        FastExcelHelper::makeFile($header, $data, $fileName);
        // NO CODE AFTER THIS LINE
    }
}
```

## Critical Rules

1. **ALWAYS** `decodeUTF8()` on database rows
2. **NO CODE** after `makeFile()` - it exits
3. **Type cast** numeric values for proper formatting
4. **Check permissions** before export
5. **Limit records** to avoid memory issues (use pagination for large exports)

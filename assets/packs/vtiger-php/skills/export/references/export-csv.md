# Export CSV - UTF-8 BOM Pattern

## Overview

CSV exports for Vietnamese text require UTF-8 BOM for proper Excel compatibility.

## Basic Pattern

```php
class Accounts_ExportCSV_Action extends Vtiger_Action_Controller {

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

        // Set headers
        $fileName = 'accounts_' . date('YmdHis') . '.csv';
        header('Content-Type: text/csv; charset=utf-8');
        header('Content-Disposition: attachment; filename="' . $fileName . '"');

        // Open output stream
        $output = fopen('php://output', 'w');

        // CRITICAL: Write UTF-8 BOM for Vietnamese text
        fwrite($output, chr(0xEF) . chr(0xBB) . chr(0xBF));

        // Write header row
        fputcsv($output, ['Account Name', 'Phone', 'Amount', 'Created Date']);

        // Write data rows
        while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
            $row = decodeUTF8($row);
            fputcsv($output, [
                $row['accountname'],
                $row['phone'],
                $row['amount'],
                $row['createdtime'],
            ]);
        }

        fclose($output);
        exit(); // CRITICAL: Must exit after output
    }
}
```

## UTF-8 BOM Explanation

```php
// Write BOM (Byte Order Mark) for UTF-8
fwrite($output, chr(0xEF) . chr(0xBB) . chr(0xBF));
```

**Why BOM is needed:**
- Excel uses BOM to detect UTF-8 encoding
- Without BOM, Vietnamese characters (ă, â, ê, ô, ơ, ư, etc.) display as garbage
- BOM is 3 bytes: EF BB BF

## fputcsv() vs Manual String Concatenation

**CORRECT:**
```php
fputcsv($output, [$col1, $col2, $col3]);
```

**WRONG:**
```php
fwrite($output, "$col1,$col2,$col3\n"); // Breaks if data has commas/quotes
```

**Why fputcsv():**
- Auto-escapes commas in data
- Auto-escapes quotes
- Handles newlines in data
- Standard CSV format

## Headers Configuration

```php
// Content type with charset
header('Content-Type: text/csv; charset=utf-8');

// Force download with filename
header('Content-Disposition: attachment; filename="' . $fileName . '"');

// Optional: Prevent caching
header('Cache-Control: no-cache, must-revalidate');
header('Expires: Sat, 26 Jul 1997 05:00:00 GMT');
```

## Data Preparation

```php
while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
    // ALWAYS decode UTF-8
    $row = decodeUTF8($row);

    // Format dates if needed
    $createdDate = DateTimeField::convertToUserFormat($row['createdtime']);

    // Format numbers if needed
    $amount = number_format((float) $row['amount'], 2, '.', '');

    fputcsv($output, [
        $row['accountname'],
        $row['phone'],
        $amount,
        $createdDate,
    ]);
}
```

## Large Dataset Pattern

```php
// For very large exports, process in batches
$offset = 0;
$limit = 1000;

do {
    $sql = "SELECT * FROM vtiger_account
            INNER JOIN vtiger_crmentity ON crmid = accountid
            WHERE deleted = 0
            LIMIT $offset, $limit";
    $result = $GLOBALS['adb']->query($sql);
    $rowCount = $GLOBALS['adb']->num_rows($result);

    while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
        $row = decodeUTF8($row);
        fputcsv($output, [
            $row['accountname'],
            $row['phone'],
        ]);
    }

    $offset += $limit;
} while ($rowCount == $limit);
```

## Complete Example with Filters

```php
class Products_ExportCSV_Action extends Vtiger_Action_Controller {

    public function process(Vtiger_Request $request) {
        $moduleName = $request->getModule();

        if (!Users_Privileges_Model::isPermitted($moduleName, 'Export')) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }

        // Get filter params
        $category = $request->get('category');
        $status = $request->get('status');

        // Build query
        $sql = "SELECT productname, productcode, unit_price, qtyinstock
                FROM vtiger_products
                INNER JOIN vtiger_crmentity ON crmid = productid
                WHERE deleted = 0";
        $params = [];

        if (!empty($category)) {
            $sql .= " AND productcategory = ?";
            $params[] = $category;
        }

        if (!empty($status)) {
            $sql .= " AND discontinued = ?";
            $params[] = ($status == 'Active' ? 0 : 1);
        }

        $result = $GLOBALS['adb']->pquery($sql, $params);

        // Set headers
        $fileName = 'products_' . date('YmdHis') . '.csv';
        header('Content-Type: text/csv; charset=utf-8');
        header('Content-Disposition: attachment; filename="' . $fileName . '"');

        // Output stream
        $output = fopen('php://output', 'w');

        // UTF-8 BOM
        fwrite($output, chr(0xEF) . chr(0xBB) . chr(0xBF));

        // Header
        fputcsv($output, [
            vtranslate('Product Name', $moduleName),
            vtranslate('Product Code', $moduleName),
            vtranslate('Unit Price', $moduleName),
            vtranslate('Qty In Stock', $moduleName),
        ]);

        // Data
        while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
            $row = decodeUTF8($row);
            fputcsv($output, [
                $row['productname'],
                $row['productcode'],
                number_format((float) $row['unit_price'], 2, '.', ''),
                (int) $row['qtyinstock'],
            ]);
        }

        fclose($output);
        exit();
    }
}
```

## Critical Rules

1. **ALWAYS** write UTF-8 BOM: `chr(0xEF).chr(0xBB).chr(0xBF)`
2. **ALWAYS** use `fputcsv()`, never manual string concat
3. **ALWAYS** `decodeUTF8()` on database rows
4. **ALWAYS** `exit()` after output
5. **Check permissions** before export
6. **Use batch processing** for large datasets

# Export Excel - FastExcelHelper

## Overview

`FastExcelHelper` (`include/utils/FastExcelHelper.php`) is a global class (no namespace) wrapping `avadim/fast-excel-writer`. Load it with `require_once`, not `use`.

`makeFile($header, $data, $fileName)`:
- calls `ob_clean()`, writes header + body, then `$workBook->save(decodeUTF8($fileName))` — **writes a file to disk** at `$fileName`;
- returns `$fileName`. It sends NO HTTP headers and does NOT exit. Streaming the file to the browser is your job.

Real callers: `modules/Import/helpers/ImportLog.php:73-95` (tempfile → Document attachment), `modules/CPContentIdea/helpers/Import.php:750`.

## Basic Pattern

```php
require_once('include/utils/FastExcelHelper.php');

// Header items: a plain string = text column; ['label' => ..., 'type' => ...] = typed column
$header = [
	'Account Name',
	'Phone',
	['label' => 'Annual Revenue', 'type' => 'currency'],
];

$data = [];

while ($row = $adb->fetchByAssoc($result)) {
	$data[] = [
		$row['accountname'],
		$row['phone'],
		(float) $row['annualrevenue'],
	];
}

$tempFile = tempnam(sys_get_temp_dir(), 'export_');   // no extension needed (ImportLog does the same)
FastExcelHelper::makeFile($header, $data, $tempFile);

// makeFile only wrote the file: stream it yourself
header('Content-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet');
header('Content-Disposition: attachment; filename="accounts_' . date('YmdHis') . '.xlsx"');
header('Content-Length: ' . filesize($tempFile));
readfile($tempFile);
unlink($tempFile);
```

## Header Structure

`writeHeader()` reads `$value['label'] ?? $key` for array items (a plain string item is used as-is) and formats the column only from `$value['type']` via `formatExcelColumn()`. Keys such as `'name'` or `'format'` are ignored — a `['name' => ...]` list prints the numeric keys 0, 1, 2. An array item without `'type'` triggers an "Undefined array key type" warning on PHP 8, so use a plain string for untyped columns.

```php
$header = [
	'ID',                                                   // text
	['label' => 'Quantity', 'type' => 'integer'],
	['label' => 'Amount', 'type' => 'currency'],
	['label' => 'Growth Rate', 'type' => 'percentage'],
	['label' => 'Created Date', 'type' => 'date'],
	['label' => 'Modified', 'type' => 'datetime'],
];
```

## Column Format Mapping (`formatExcelColumn`)

| Type | Excel Format |
|------|-------------|
| `integer`, `number`, `int` | `#,##0` |
| `double`, `currency` | `#,##0.00` |
| `percentage`, `percent` | `#,##0.00` (plain number, NOT `%`, no ×100) |
| `date` | `DD-MM-YYYY` |
| `datetime`, `date_time` | `DD-MM-YYYY HH:MM:SS` |
| anything else | `string` |

## Internal Flow

1. **Create Workbook**: `Excel::create()`, title = file name, company `CloudGO`
2. **Write Header**: row 1, bold white text on `568fcb` fill, centered, thin border, row height 20
3. **Write Body**: each row passed through `decodeUTF8()` then `writeRow()` with thin border, vertical-center, height 20, auto width
4. **Clean Output Buffer**: `ob_clean()`
5. **Save**: `save()` writes the file to `$fileName`; returns `$fileName`

## Data Preparation

`writeBody()` already applies `decodeUTF8()` (html_entity_decode) to every row, so do NOT decode rows again before `makeFile()` — that double-decodes values that legitimately contain entities.

```php
while ($row = $adb->fetchByAssoc($result)) {
	$data[] = [
		$row['accountname'],                                   // String
		(int) $row['accountid'],                               // Integer
		(float) $row['annualrevenue'],                         // Float
		DateTimeField::convertToUserFormat($row['createdtime']),   // Date conversion
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
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('include/utils/FastExcelHelper.php');

class Accounts_ExportExcel_Action extends Vtiger_Action_Controller {

	function checkPermission(Vtiger_Request $request) {
		$moduleName = $request->getModule();

		if (!Users_Privileges_Model::isPermitted($moduleName, 'Export')) {
			throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
		}
	}

	function process(Vtiger_Request $request) {
		global $adb;

		$sql = "SELECT a.accountname, a.phone, a.annualrevenue, e.createdtime
			FROM vtiger_account AS a
			INNER JOIN vtiger_crmentity AS e ON (e.crmid = a.accountid AND e.deleted = 0)
			LIMIT ?";
		$result = $adb->pquery($sql, [10000]);

		$header = [
			'Account Name',
			'Phone',
			['label' => 'Annual Revenue', 'type' => 'currency'],
			['label' => 'Created Time', 'type' => 'datetime'],
		];

		$data = [];

		while ($row = $adb->fetchByAssoc($result)) {
			$data[] = [
				$row['accountname'],
				$row['phone'],
				(float) $row['annualrevenue'],
				$row['createdtime'],
			];
		}

		$tempFile = tempnam(sys_get_temp_dir(), 'accounts_');
		FastExcelHelper::makeFile($header, $data, $tempFile);

		header('Content-Type: application/vnd.openxmlformats-officedocument.spreadsheetml.sheet');
		header('Content-Disposition: attachment; filename="accounts_' . date('YmdHis') . '.xlsx"');
		header('Content-Length: ' . filesize($tempFile));
		readfile($tempFile);
		unlink($tempFile);
	}
}
```

## Critical Rules

1. `require_once('include/utils/FastExcelHelper.php')` — the class has no namespace
2. Header items use `label` + `type`; there is no `name`/`format` key
3. `makeFile()` only writes a file — send headers + `readfile()` yourself (or attach the file, as ImportLog does), then remove the temp file
4. Do NOT `decodeUTF8()` rows yourself — `writeBody()` already does
5. **Type cast** numeric values for proper formatting
6. **Check permissions** before export
7. **Limit records** to avoid memory issues (use pagination for large exports)

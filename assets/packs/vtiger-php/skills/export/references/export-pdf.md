# Export PDF - wkhtmltopdf API

## Overview

Prefer the global function `generatePDF()` (`include/utils/CommonUtils.php`), which posts HTML to a wkhtmltopdf HTTP API and saves the returned PDF. A legacy TCPDF helper exists (`include/utils/PDFGenerator.php`), but new code should use `generatePDF()`.

Real caller: `include/utils/InventoryPDFUtils.php:31-66` (renders `modules/Inventory/tpls/PDF.tpl`, calls `generatePDF()`, then streams the file with download headers).

## generatePDF() Signature

```php
/**
 * @return string|false  $outputFile on success, false when $wkhtmltopdfAPIConfig['enable'] is off
 */
function generatePDF($outputFile, $htmlFilePath = null, $htmlContent = null, $options = null)
```

- `$outputFile` — path the PDF is written to (`file_put_contents`); defaults to `output.pdf`.
- `$htmlFilePath` / `$htmlContent` — pass one; `$htmlContent` wins, otherwise the file is read.
- `$options` — a **wkhtmltopdf CLI option string**, not an array. Default: `'--margin-left 0.6in --margin-top 0.4in --margin-right 0.6in --margin-bottom 0.5in'`.
- There is NO download/inline mode (`'D'`/`'I'`/`'F'`): the function always writes `$outputFile` and returns its path. Downloading is your job (headers + `readfile()`).
- It does not check the HTTP status of the API call: an API error body gets written into `$outputFile`. Verify the output (e.g. starts with `%PDF`) when it matters.

## wkhtmltopdf API Config

`config_override.php` defines:

```php
$wkhtmltopdfAPIConfig = [
	'enable' => true,
	'api_url' => '<wkhtmltopdf API endpoint>',
	'access_token' => '<secret>',   // sent as "Authorization: Bearer"; never print or commit it
];
```

The renderer is remote, so the HTML must be self-contained: it cannot load local CSS files or `file://` assets.

## Basic Pattern

HTML goes in a `.tpl`, CSS in a `.css` file — never HTML/CSS strings in PHP (cloudgo-development-rules File Separation). Because the remote renderer can't fetch the stylesheet, read the `.css` file in PHP and let the template inject its content.

```
modules/Accounts/
├── actions/ExportPDF.php
├── resources/PDF.css        # print styles (separate file)
└── tpls/PDF.tpl             # HTML document; injects {$PDF_CSS} inside <style>
```

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

class Accounts_ExportPDF_Action extends Vtiger_Action_Controller {

	function checkPermission(Vtiger_Request $request) {
		$moduleName = $request->getModule();
		$recordId = (int) $request->get('record');

		if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView', $recordId)) {
			throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
		}
	}

	function process(Vtiger_Request $request) {
		$moduleName = $request->getModule();
		$recordId = (int) $request->get('record');
		$recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);

		$viewer = new Vtiger_Viewer();
		$viewer->assign('RECORD_MODEL', $recordModel);
		$viewer->assign('MODULE_NAME', $moduleName);
		$viewer->assign('PDF_CSS', file_get_contents("modules/{$moduleName}/resources/PDF.css"));
		$html = $viewer->fetch("modules/{$moduleName}/tpls/PDF.tpl");

		$outputFile = tempnam(sys_get_temp_dir(), 'pdf_');
		$outputFile = generatePDF($outputFile, null, $html);

		if (empty($outputFile) || !file_exists($outputFile)) {
			// Define LBL_PDF_GENERATION_FAILED in languages/<locale>/<Module>.php
			throw new AppException(vtranslate('LBL_PDF_GENERATION_FAILED', $moduleName));
		}

		$downloadName = getModuleSequenceNumber($moduleName, $recordId) . '.pdf';
		header('Content-Type: application/pdf');
		header('Content-Disposition: attachment; filename="' . $downloadName . '"');
		header('Content-Length: ' . filesize($outputFile));
		readfile($outputFile);
		unlink($outputFile);
	}
}
```

Use `Content-Disposition: inline` instead of `attachment` to display in the browser. To keep the file (e.g. attach it to a record), write it under `storage/` and skip the streaming.

`modules/Accounts/tpls/PDF.tpl`:

```smarty
<!DOCTYPE html>
<html>
<head>
	<meta charset="utf-8">
	<style>{$PDF_CSS}</style>
</head>
<body>
	<div class="pdf-header">{$RECORD_MODEL->getName()}</div>
	<div class="pdf-field">
		<span class="pdf-label">{vtranslate('Phone', $MODULE_NAME)}:</span>
		<span>{$RECORD_MODEL->get('phone')}</span>
	</div>
</body>
</html>
```

`modules/Accounts/resources/PDF.css`:

```css
body { font-family: "DejaVu Sans", Arial, sans-serif; font-size: 12pt; }
.pdf-header { font-size: 20pt; font-weight: bold; margin-bottom: 20px; }
.pdf-label { font-weight: bold; display: inline-block; width: 200px; }
```

## Reference / Line-Item Values

- Reference fields: print the label with `getParentName($recordModel->get('account_id'))` (as `modules/Inventory/tpls/PDF.tpl:104` does); `getDisplayValue()` on a reference field returns an HTML link.
- Inventory line items: `getAssociatedProducts($moduleName, $recordModel->getEntity())` (see `InventoryPDFUtils::getHTMLContent`), or a prepared query:

```php
$sql = "SELECT p.productname, r.quantity, r.listprice
	FROM vtiger_inventoryproductrel AS r
	INNER JOIN vtiger_products AS p ON (p.productid = r.productid)
	WHERE r.id = ?
	ORDER BY r.sequence_no";
$result = $adb->pquery($sql, [$recordId]);
```

## Critical Rules

1. Call the global `generatePDF()` — there is no `GeneratePDF` class/namespace
2. `$options` is a wkhtmltopdf CLI string; there are no `'D'`/`'I'`/`'F'` modes
3. `generatePDF()` returns `false` when the API is disabled — check before streaming
4. Stream the written file yourself (headers + `readfile()`), then `unlink()` temp files
5. HTML in `.tpl`, CSS in `resources/*.css` injected as text (remote renderer can't load files)
6. **Use DejaVu Sans** or web-safe fonts for Vietnamese text
7. Check `DetailView` permission for the record before rendering

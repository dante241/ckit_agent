# Export PDF - wkhtmltopdf API

## Overview

VTiger uses **wkhtmltopdf API** (NOT TCPDF) for PDF generation via HTML rendering.

## Critical Difference

**Use wkhtmltopdf API:**
```php
use include\utils\GeneratePDF;

$pdf = GeneratePDF::generatePDF($outputFile, $htmlFilePath, $htmlContent, $options);
```

**DON'T use TCPDF:**
```php
// WRONG - VTiger doesn't use TCPDF
$pdf = new TCPDF();
```

## Basic Pattern

```php
use include\utils\GeneratePDF;

class Accounts_ExportPDF_Action extends Vtiger_Action_Controller {

    public function process(Vtiger_Request $request) {
        $moduleName = $request->getModule();
        $recordId = (int) $request->get('record');

        if (!Users_Privileges_Model::isPermitted($moduleName, 'DetailView', $recordId)) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }

        // Get record data
        $recordModel = Vtiger_Record_Model::getInstanceById($recordId);

        // Build HTML content
        $html = $this->buildHTMLContent($recordModel);

        // Generate PDF filename
        $fileName = $this->getPDFFileName($moduleName, $recordId);

        // Mode: 'F' = save to file, 'D' = download
        $mode = $request->get('mode', 'D');

        if ($mode == 'F') {
            // Save to storage directory
            $outputFile = 'storage/' . $fileName;
            GeneratePDF::generatePDF($outputFile, '', $html, []);
            return $outputFile;
        }
        else {
            // Download directly
            GeneratePDF::generatePDF($fileName, '', $html, ['mode' => 'D']);
            exit();
        }
    }

    private function buildHTMLContent($recordModel) {
        $html = '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { font-size: 24px; font-weight: bold; margin-bottom: 20px; }
        .field { margin: 10px 0; }
        .label { font-weight: bold; width: 200px; display: inline-block; }
    </style>
</head>
<body>';

        $html .= '<div class="header">' . $recordModel->getName() . '</div>';

        foreach ($recordModel->getData() as $fieldName => $fieldValue) {
            $fieldModel = $recordModel->getModule()->getField($fieldName);
            if ($fieldModel && $fieldModel->isViewable()) {
                $displayValue = $recordModel->getDisplayValue($fieldName);
                $html .= '<div class="field">';
                $html .= '<span class="label">' . vtranslate($fieldModel->get('label'), $recordModel->getModuleName()) . ':</span> ';
                $html .= '<span>' . htmlspecialchars($displayValue) . '</span>';
                $html .= '</div>';
            }
        }

        $html .= '</body></html>';

        return $html;
    }
}
```

## generatePDF() Signature

```php
GeneratePDF::generatePDF(
    string $outputFile,    // Output filename or path
    string $htmlFilePath,  // Path to HTML file (use '' if passing $htmlContent)
    string $htmlContent,   // HTML string (use '' if passing $htmlFilePath)
    array $options         // Optional settings
);
```

## Mode Options

```php
// Mode 'F': Save to file (storage/)
$outputFile = 'storage/invoices/invoice_123.pdf';
GeneratePDF::generatePDF($outputFile, '', $html, []);

// Mode 'D': Download (browser)
GeneratePDF::generatePDF('invoice.pdf', '', $html, ['mode' => 'D']);
exit(); // Required after download mode

// Mode 'I': Inline display (browser)
GeneratePDF::generatePDF('invoice.pdf', '', $html, ['mode' => 'I']);
exit();
```

## wkhtmltopdf API Config

```php
// Global config in config.inc.php or config.env.php
$wkhtmltopdfAPIConfig = [
    'url' => 'http://wkhtmltopdf-service:8080/convert',
    'timeout' => 30,
];
```

**Check service is running:**
```bash
curl http://wkhtmltopdf-service:8080/health
```

## getPDFFileName Pattern

```php
private function getPDFFileName(string $moduleName, int $recordId): string {
    $recordModel = Vtiger_Record_Model::getInstanceById($recordId, $moduleName);

    // Get module sequence number (Invoice No, Quote No, etc.)
    $sequenceField = $this->getModuleSequenceField($moduleName);
    if ($sequenceField) {
        $sequenceNo = $recordModel->get($sequenceField);
        $fileName = $sequenceNo . '.pdf';
    }
    else {
        $fileName = $moduleName . '_' . $recordId . '.pdf';
    }

    return $fileName;
}

private function getModuleSequenceField(string $moduleName): ?string {
    $sequenceFields = [
        'Invoice' => 'invoice_no',
        'Quotes' => 'quote_no',
        'SalesOrder' => 'salesorder_no',
        'PurchaseOrder' => 'purchaseorder_no',
    ];

    return $sequenceFields[$moduleName] ?? null;
}
```

## HTML Template with Styles

```php
private function buildHTMLContent($data) {
    $html = '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        @page {
            margin: 20mm;
            size: A4;
        }
        body {
            font-family: "DejaVu Sans", Arial, sans-serif;
            font-size: 12pt;
            line-height: 1.5;
        }
        .header {
            text-align: center;
            font-size: 20pt;
            font-weight: bold;
            margin-bottom: 30px;
            border-bottom: 2px solid #333;
            padding-bottom: 10px;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            margin: 20px 0;
        }
        th, td {
            border: 1px solid #ddd;
            padding: 8px;
            text-align: left;
        }
        th {
            background-color: #f2f2f2;
            font-weight: bold;
        }
        .total {
            font-size: 14pt;
            font-weight: bold;
            text-align: right;
            margin-top: 20px;
        }
    </style>
</head>
<body>';

    // Content here

    $html .= '</body></html>';
    return $html;
}
```

## Complete Invoice Example

```php
class Invoice_ExportPDF_Action extends Vtiger_Action_Controller {

    public function process(Vtiger_Request $request) {
        $recordId = (int) $request->get('record');

        if (!Users_Privileges_Model::isPermitted('Invoice', 'DetailView', $recordId)) {
            throw new AppException(vtranslate('LBL_PERMISSION_DENIED'));
        }

        $recordModel = Vtiger_Record_Model::getInstanceById($recordId, 'Invoice');
        $html = $this->buildInvoiceHTML($recordModel);

        $fileName = $recordModel->get('invoice_no') . '.pdf';
        GeneratePDF::generatePDF($fileName, '', $html, ['mode' => 'D']);
        exit();
    }

    private function buildInvoiceHTML($recordModel) {
        $invoiceNo = $recordModel->get('invoice_no');
        $accountName = $recordModel->get('account_id_display');
        $invoiceDate = $recordModel->get('invoicedate');
        $dueDate = $recordModel->get('duedate');

        // Get line items
        $lineItems = $this->getLineItems($recordModel->getId());

        $html = '<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .header { font-size: 24px; font-weight: bold; text-align: center; margin-bottom: 30px; }
        .info { margin-bottom: 20px; }
        table { width: 100%; border-collapse: collapse; margin: 20px 0; }
        th, td { border: 1px solid #ddd; padding: 8px; }
        th { background-color: #4472C4; color: white; }
        .total { text-align: right; font-weight: bold; font-size: 16px; }
    </style>
</head>
<body>
    <div class="header">INVOICE</div>
    <div class="info">
        <div><strong>Invoice No:</strong> ' . htmlspecialchars($invoiceNo) . '</div>
        <div><strong>Customer:</strong> ' . htmlspecialchars($accountName) . '</div>
        <div><strong>Invoice Date:</strong> ' . htmlspecialchars($invoiceDate) . '</div>
        <div><strong>Due Date:</strong> ' . htmlspecialchars($dueDate) . '</div>
    </div>
    <table>
        <thead>
            <tr>
                <th>Product</th>
                <th>Quantity</th>
                <th>Unit Price</th>
                <th>Total</th>
            </tr>
        </thead>
        <tbody>';

        $grandTotal = 0;
        foreach ($lineItems as $item) {
            $total = (float) $item['quantity'] * (float) $item['listprice'];
            $grandTotal += $total;

            $html .= '<tr>
                <td>' . htmlspecialchars($item['productname']) . '</td>
                <td>' . number_format($item['quantity'], 0) . '</td>
                <td>' . number_format($item['listprice'], 2) . '</td>
                <td>' . number_format($total, 2) . '</td>
            </tr>';
        }

        $html .= '</tbody>
    </table>
    <div class="total">Grand Total: ' . number_format($grandTotal, 2) . '</div>
</body>
</html>';

        return $html;
    }

    private function getLineItems(int $invoiceId): array {
        $sql = "SELECT productname, quantity, listprice
                FROM vtiger_inventoryproductrel
                INNER JOIN vtiger_products ON vtiger_products.productid = vtiger_inventoryproductrel.productid
                WHERE id = ?
                ORDER BY sequence_no";
        $result = $GLOBALS['adb']->pquery($sql, [$invoiceId]);

        $items = [];
        while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
            $items[] = decodeUTF8($row);
        }

        return $items;
    }
}
```

## Critical Rules

1. **Use wkhtmltopdf API**, not TCPDF
2. **Check service** is running before generating
3. **Mode 'D' or 'I'** requires `exit()` after
4. **Mode 'F'** saves to `storage/` directory
5. **HTML must be complete** document with `<html><head><body>`
6. **Include styles** in `<style>` tag, not external CSS
7. **Use web-safe fonts** or DejaVu Sans for UTF-8
8. **htmlspecialchars()** for user data to prevent XSS

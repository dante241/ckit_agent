---
name: export
description: "VTiger export — FastExcelHelper, Excel/CSV UTF-8 BOM, PDF wkhtmltopdf. Use when: xuất file, export excel/csv/pdf, download dữ liệu, báo cáo file; keywords: export, xuất excel, tải file."
user-invocable: false
---

# VTiger Export Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Creating Excel exports with formatted columns
- Generating CSV exports with Vietnamese text
- Creating PDF reports from HTML
- Implementing data export features
- Building download/export actions

## Export Methods Comparison

| Method | Use Case | Format Control | File Size | Performance |
|--------|----------|----------------|-----------|-------------|
| **Excel** | Formatted reports, typed data | High (colors, types, styles) | Medium | Fast (streaming) |
| **CSV** | Simple data, import/export | Low (plain text) | Small | Very Fast |
| **PDF** | Print-ready reports, archives | High (HTML+CSS) | Large | Slow (rendering) |

## FastExcelHelper Quick Reference

```php
require_once('include/utils/FastExcelHelper.php');   // global class, no namespace

// Writes an .xlsx file to $filePath and returns it (no headers, no exit)
FastExcelHelper::makeFile($header, $data, $filePath);

// Header items: plain string (text) or ['label' => ..., 'type' => ...] (no 'name'/'format' keys)
$header = [
    'ID',
    ['label' => 'Amount', 'type' => 'currency'],
    ['label' => 'Percentage', 'type' => 'percentage'],
    ['label' => 'Date', 'type' => 'date'],
    ['label' => 'DateTime', 'type' => 'datetime'],
];
```

## Column Format Types

| Type | Format String | Example Output | Notes |
|------|---------------|----------------|-------|
| `integer` | `#,##0` | 1,234 | Thousand separators |
| `double` | `#,##0.00` | 1,234.56 | 2 decimal places |
| `currency` | `#,##0.00` | 1,234.56 | Same as double |
| `percentage` | `#,##0.00` | 12.34 | Plain number, no `%`, no ×100 |
| `date` | `DD-MM-YYYY` | 31-12-2025 | Date only |
| `datetime` | `DD-MM-YYYY HH:MM:SS` | 31-12-2025 14:30:00 | Full datetime |

## Critical Pitfalls

**Excel:**
- `makeFile()` calls `ob_clean()` + `save()`: it only WRITES a file and returns its path — no headers, no exit, no auto-download. Stream it yourself (headers + `readfile()` + `unlink()`) or attach it (see ImportLog)
- `writeBody()` already `decodeUTF8()`s every row — don't decode again before `makeFile()`
- Check 'Export' permission before allowing download

**CSV:**
- MUST add UTF-8 BOM for Vietnamese text: `chr(0xEF).chr(0xBB).chr(0xBF)`
- Use `fputcsv()` for proper escaping, not manual string concat
- MUST call `exit()` after output

**PDF:**
- Prefer the global `generatePDF($outputFile, $htmlFilePath, $htmlContent, $options)` (wkhtmltopdf API, `include/utils/CommonUtils.php`); a TCPDF helper (`include/utils/PDFGenerator.php`) also exists for legacy code
- `generatePDF()` always writes `$outputFile` and returns its path (or `false` when `$wkhtmltopdfAPIConfig['enable']` is off) — no download/inline mode; stream the file yourself
- HTML must be complete document with styles
- Check wkhtmltopdf service is running

## References

- [Export Excel](references/export-excel.md) - FastExcelHelper using avadim/fast-excel-writer
- [Export CSV](references/export-csv.md) - UTF-8 BOM, fputcsv pattern
- [Export PDF](references/export-pdf.md) - wkhtmltopdf API integration

## Exemplars

- Excel to tempfile → Document attachment: `modules/Import/helpers/ImportLog.php:73-95`
- PDF via `generatePDF()` + manual download headers: `include/utils/InventoryPDFUtils.php:31-66`

## Verify

```bash
php -l <file>
curl -s -o /tmp/export_test.xlsx 'http://localhost/vtiger/index.php?module=<M>&action=ExportData&...' -H 'Cookie: PHPSESSID=<sid>'
file /tmp/export_test.xlsx   # Kỳ vọng: Microsoft Excel / Zip, KHÔNG phải HTML error
```

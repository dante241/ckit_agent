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
use include\utils\FastExcelHelper;

// Basic usage
FastExcelHelper::makeFile($header, $data, $fileName);

// Typed header with formatting
$header = [
    ['name' => 'ID', 'type' => 'integer', 'format' => '#,##0'],
    ['name' => 'Amount', 'type' => 'currency', 'format' => '#,##0.00'],
    ['name' => 'Percentage', 'type' => 'percentage'],
    ['name' => 'Date', 'type' => 'date', 'format' => 'DD-MM-YYYY'],
    ['name' => 'DateTime', 'type' => 'datetime', 'format' => 'DD-MM-YYYY HH:MM:SS'],
];
```

## Column Format Types

| Type | Format String | Example Output | Notes |
|------|---------------|----------------|-------|
| `integer` | `#,##0` | 1,234 | Thousand separators |
| `double` | `#,##0.00` | 1,234.56 | 2 decimal places |
| `currency` | `#,##0.00` | 1,234.56 | Same as double |
| `percentage` | `0.00%` | 12.34% | Auto multiply by 100 |
| `date` | `DD-MM-YYYY` | 31-12-2025 | Date only |
| `datetime` | `DD-MM-YYYY HH:MM:SS` | 31-12-2025 14:30:00 | Full datetime |

## Critical Pitfalls

**Excel:**
- `makeFile()` calls `ob_clean()` + `save()` - NO code after it will execute
- MUST `decodeUTF8()` on data rows before passing to makeFile
- Check 'Export' permission before allowing download
- File auto-downloads, no manual headers needed

**CSV:**
- MUST add UTF-8 BOM for Vietnamese text: `chr(0xEF).chr(0xBB).chr(0xBF)`
- Use `fputcsv()` for proper escaping, not manual string concat
- MUST call `exit()` after output

**PDF:**
- Use wkhtmltopdf API, NOT TCPDF
- Mode 'F' = save to storage/, 'D' = download
- HTML must be complete document with styles
- Check wkhtmltopdf service is running

## References

- [Export Excel](references/export-excel.md) - FastExcelHelper using avadim/fast-excel-writer
- [Export CSV](references/export-csv.md) - UTF-8 BOM, fputcsv pattern
- [Export PDF](references/export-pdf.md) - wkhtmltopdf API integration

## Exemplars (PENDING REVIEW by user)

> ⚠️ Chưa tìm được exemplar thuần Tín Bùi/Tùng Nguyễn cho domain này — file dưới là code tác giả khác, dùng tạm đến khi user chỉ định file chuẩn.

- Export action chuẩn: `modules/CPSocialMessageTemplate/actions/ExportData.php`
- Export thứ hai để đối chiếu: `modules/CPSocialFeedback/actions/ExportData.php`

## Verify

```bash
php -l <file>
curl -s -o /tmp/export_test.xlsx 'http://localhost/vtiger/index.php?module=<M>&action=ExportData&...' -H 'Cookie: PHPSESSID=<sid>'
file /tmp/export_test.xlsx   # Kỳ vọng: Microsoft Excel / Zip, KHÔNG phải HTML error
```

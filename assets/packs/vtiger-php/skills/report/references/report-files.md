# Report Files Reference

Map of the real fixed-report files. Read the files themselves for template/JS bodies — do not re-type them.

## File Structure

```
modules/Reports/
├── custom/
│   ├── CustomReportHandler.php                 → class extending ReportRun (parent of every custom handler)
│   ├── BaseFixedReportHandler.php              → abstract: filters, tables, paging
│   ├── BaseFixedChartReportHandler.php         → abstract: adds Highcharts charts
│   └── <Name>ReportHandler.php                 → your handler (class name = file basename)
├── enums/
│   ├── FilterType.php                          → Reports_FilterType_Enum
│   └── DataType.php                            → Reports_DataType_Enum
├── models/
│   ├── Record.php                              → Reports_Record_Model::getCustomHandler()
│   └── FixedReportTableView.php                → nested header → rowspan/colspan
├── views/
│   ├── Detail.php                              → report page; calls getCustomHandler()
│   └── FixedReportAjax.php                     → AJAX re-render, dispatched by `mode`
├── tpls/BaseFixedReport/
│   ├── Main.tpl                                → page shell; loads the CSS/JS below
│   ├── Filter.tpl                              → uses $FILTER_LAYOUT
│   ├── Table.tpl                               → uses $TABLE_HEADERS, $TABLE_DATA_HEADERS, $TABLE_DATA, $PAGING_MODEL
│   ├── Pagination.tpl
│   ├── Chart.tpl                               → uses $CHART_NAME, $CHART_OPTIONS, $CHART_TOTAL
│   └── NoData.tpl
└── resources/
    ├── CustomReportHelper.js                   → shared helper (custom buttons)
    ├── CustomReport.css
    └── BaseFixedReport/
        ├── BaseFixedReport.js                  → `$(() => new class {...})` — not a Vtiger.Class controller
        ├── BaseFixedReportChartWidget.js       → Highcharts rendering
        ├── Layout.css / Filter.css / Table.css / Chart.css
```

## How a Fixed Report Loads

1. `index.php?module=Reports&view=Detail&record=<reportid>` → `Reports_Detail_View`.
2. `$reportModel->getCustomHandler()` (models/Record.php): reads `vtiger_report.custom_handler_file`, `require_once`s it, instantiates `basename($file, '.php')` with the report id and copies the `ReportRun` state into it.
3. `BaseFixedReportHandler::renderReportResult()` assigns `REPORT_FILTER` (`renderReportFilter()`), `REPORT_TABLES` (`renderReportTables()`), and for chart handlers `REPORT_CHARTS` (`renderReportCharts()`), then displays `$reportMainTemplate`.
4. Filter changes / paging: `BaseFixedReport.js` posts `module=Reports&view=FixedReportAjax&mode=getReportTableUI` (or `mode=getChartUI`) with `report_id`, `table_name`/`chart_name` and `filters`; the view calls `setFilterParams($filters)` and returns the re-rendered HTML.

## Registration

Register the handler file in a migration with `CPMigration_Base_Model::saveCustomizeReportFolder()` + `createCustomizeReport()` (see SKILL.md → Registration). They fill `vtiger_report.custom_handler_file`; there is no other registration path.

## Customization Points

- Override a template by setting the property in the handler: `protected $reportTableTemplate = 'modules/Reports/tpls/<Name>/Table.tpl';` (also `$reportMainTemplate`, `$reportFilterTemplate`, `$reportChartTemplate`). Examples: `ViolationReportHandler`, `EmployeeViolationDetailHandler`.
- Report-specific JS/CSS: put them in `modules/Reports/resources/<Name>.js|css` and load them from the overridden template with `vresource_url()` (e.g. `tpls/GoalResultsOverview.tpl:133-134`).
- Extra template variables: override `renderReportResult()` and assign before `display()` (e.g. `GoalResultsOverviewHandler` assigns `CAN_EXPORT_EXCEL`).

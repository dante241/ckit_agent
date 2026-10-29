---
name: report
description: "VTiger custom reports — BaseFixedReportHandler, chart report, filter, pagination bảng. Use when: tạo/sửa báo cáo, report, chart, biểu đồ, thống kê, filter báo cáo; keywords: report, báo cáo, chart, BaseFixedReportHandler."
user-invocable: false
---

# VTiger Report Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

Activate this skill when:
- Creating custom fixed reports with filters and tables
- Building chart reports with visualizations
- Adding filter fields (picklist, date_range, multipicklist, date)
- Defining table structures with nested headers
- Implementing report data queries with pagination
- Creating export functionality for reports
- Registering custom report handlers

## Report Framework Hierarchy

```
CustomReportHandler (class extending ReportRun)
    ↓
BaseFixedReportHandler (fixed table reports)
    ↓
BaseFixedChartReportHandler (charts + tables)
```

### BaseFixedReportHandler

**Abstract base class for table-based reports with filters.**

**Location:** `modules/Reports/custom/BaseFixedReportHandler.php`

**Key Features:**
- Filter management (PICKLIST, MULTIPICKLIST, DATE_RANGE, DATE)
- Table rendering with nested headers
- Pagination support
- Cell value formatting by data type
- Query condition generation from filters
- AJAX integration for dynamic updates

### BaseFixedChartReportHandler

**Extends BaseFixedReportHandler with chart capabilities.**

**Location:** `modules/Reports/custom/BaseFixedChartReportHandler.php`

**Additional Features:**
- Chart configuration (Highcharts)
- Chart data generation
- Grid layout (columns, row height)
- Chart + table combined views

## 3 Abstract Methods Overview

`BaseFixedReportHandler` declares exactly 3 abstract methods; `BaseFixedChartReportHandler` adds 3 more (`getReportChartSetting`, `getReportChartOptions`, `getReportChartData`). Full working handlers: [report-handler.md](references/report-handler.md).

### 1. getConfiguredFilterFields(): array

**Purpose:** Define filter UI controls and behavior.

**Returns:** Array of filter field configurations.

**Example:**
```php
protected function getConfiguredFilterFields(): array {
    return [
        'status' => [
            'label' => 'LBL_STATUS',
            'uitype' => Reports_FilterType_Enum::PICKLIST,
            'data_source' => [
                'type' => 'picklist',
                'module' => 'HelpDesk',
                'field' => 'ticketstatus',
                'add_all_option' => true
            ]
        ],
        'date_range' => [
            'label' => 'LBL_REPORT_DATA_DATE_RANGE',
            'uitype' => Reports_FilterType_Enum::DATE_RANGE
        ]
    ];
}
```

`add_all_option` makes `'all'` the default value, and `getQueryConditions()` then emits `col = 'all'` (no rows): drop that column from the mapping when the value is `'all'`.

### 2. getReportTableStructure(): array

**Purpose:** Define table columns, headers, and display properties.

**Returns:** Array mapping table names to column structures.

**Example:**
```php
protected function getReportTableStructure(): array {
    return [
        'main_table' => [
            'label' => 'LBL_TICKETS',
            'paging' => true,
            'headers' => [
                [
                    'label' => 'LBL_TICKET_ID',
                    'data_column' => 'ticket_id',
                    'type' => Reports_DataType_Enum::TEXT
                ],
                [
                    'label' => 'LBL_AMOUNT',
                    'data_column' => 'amount',
                    'type' => Reports_DataType_Enum::CURRENCY
                ]
            ]
        ]
    ];
}
```

### 3. getReportTableData(string $tableName, Vtiger_Paging_Model $pagingModel = null): array

**Purpose:** Fetch report data with filters applied.

**Returns:** `['data' => [...], 'total_count' => int]`

**Pattern:**
```php
protected function getReportTableData(string $tableName, Vtiger_Paging_Model $pagingModel = null): array {
    $filterParams = $this->getFilterParams();

    // Build query
    $sql = "SELECT ... FROM ... WHERE deleted = 0";
    $params = [];

    // Apply filters via helper
    $columnsMapping = [
        'status' => 't.status',
        'date_range' => 'c.createdtime'
    ];

    // 'all' (add_all_option) means no status condition
    if (($filterParams['status'] ?? '') === 'all') {
        unset($columnsMapping['status']);
    }

    $conditions = $this->getQueryConditions($columnsMapping);

    foreach ($conditions as $condition) {
        $sql .= " AND " . $condition;
    }

    // Count total
    $countResult = $GLOBALS['adb']->pquery("SELECT COUNT(*) as total FROM (...)", $params);
    $total = (int) $GLOBALS['adb']->query_result($countResult, 0, 'total');

    // Add pagination
    if ($pagingModel) {
        $sql .= " " . $this->getPagingQueryFromPagingModel($pagingModel);
    }

    // Fetch data
    $result = $GLOBALS['adb']->pquery($sql, $params);
    $data = [];

    while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
        $data[] = decodeUTF8($row);
    }

    return ['data' => $data, 'total_count' => $total];
}
```

**Grouping:** there is no base `getGroupByList()`. `TicketSummaryByStatusReportHandler` defines its own `getGroupByList()` and feeds it to a picklist filter through `getCustomFilterPicklistOptions()` — copy that pattern if you need a "group by" selector.

## File Checklist

When creating a custom report, you need:

### Required Files

1. **Handler:** `modules/Reports/custom/YourReportHandler.php`
   - Extend `BaseFixedReportHandler` or `BaseFixedChartReportHandler`
   - Implement the 3 abstract methods (6 for a chart handler)

2. **Template (optional override):** set `protected $reportTableTemplate` / `$reportMainTemplate` in the handler
   - Only if customizing default layout
   - Default: `modules/Reports/tpls/BaseFixedReport/Main.tpl` / `Table.tpl`

3. **JavaScript (optional):** `modules/Reports/resources/YourReport.js`
   - Loaded from your overridden template via `vresource_url()`; `BaseFixedReport.js` is a self-starting `$(() => new class {...})`, not a class to extend
   - Add custom interactions

4. **CSS (optional):** `modules/Reports/resources/YourReport.css`
   - Custom styling only if needed

### Registration (via Migration)

CPMigration_Base_Model provides **2 built-in helper methods** for report registration:

#### `saveCustomizeReportFolder(string $folderName, string $folderCode, string $description = '')`
- Creates or updates a report folder by `code`
- Returns `$folderId`

#### `createCustomizeReport(string $reportName, string $handlerFile, string $reportModule, string $reportColumn, int $folderId = 0)`
- Auto-generates `reportid` via `MAX(reportid) + 1`
- Deletes old report if same `custom_handler_file` exists
- Inserts into 6 tables in a transaction: `vtiger_selectquery`, `vtiger_report`, `vtiger_reportmodules`, `vtiger_reportsortcol` (3 rows), `vtiger_selectcolumn`, updates `vtiger_selectquery_seq`
- If `$folderId = 0`, auto-creates default folder "Báo cáo tùy chỉnh" with code "ReportsCustomize"

**Migration example:**
```php
return new class extends CPMigration_Base_Model {
    protected $isRunBeforeQuickRepair = false;

    public function up(): int {
        // Step 1: Create/get folder
        $folderId = $this->saveCustomizeReportFolder('Báo cáo Bán hàng', 'SalesReports');

        // Step 2: Register report
        $this->createCustomizeReport(
            'Doanh thu theo sản phẩm',                                       // reportName
            'modules/Reports/custom/SalesRevenueByProductReportHandler.php', // handlerFile
            'SalesOrder',                                                     // reportModule (primarymodule)
            '',                                                               // reportColumn
            $folderId                                                         // folderId
        );

        return self::UP_SUCCESS;
    }

    public function down(): int {
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

**Multiple reports in same folder:**
```php
public function up(): int {
    $folderId = $this->saveCustomizeReportFolder('Báo cáo CSKH', 'CustomerService');

    $this->createCustomizeReport('Tổng hợp ticket theo trạng thái', 'modules/Reports/custom/TicketSummaryByStatusReportHandler.php', 'HelpDesk', '', $folderId);
    $this->createCustomizeReport('Chi tiết ticket theo nhân viên', 'modules/Reports/custom/TicketDetailByAgentReportHandler.php', 'HelpDesk', '', $folderId);

    return self::UP_SUCCESS;
}
```

## Critical Pitfalls

### 1. Filter Data Source Confusion

**WRONG:**
```php
'data_source' => [
    'type' => 'picklist',
    'module' => 'Users'  // ❌ Users is not a picklist field
]
```

**CORRECT:**
```php
// For CRM records (Users, Accounts, etc.)
'data_source' => [
    'type' => 'reference',
    'module' => 'Users'
]

// For picklist fields
'data_source' => [
    'type' => 'picklist',
    'module' => 'HelpDesk',
    'field' => 'ticketstatus'
]
```

### 2. Missing decodeUTF8()

**WRONG:**
```php
while ($row = $adb->fetchByAssoc($result)) {
    $data[] = $row;  // ❌ UTF-8 encoding issues
}
```

**CORRECT:**
```php
while ($row = $adb->fetchByAssoc($result)) {
    $data[] = decodeUTF8($row);  // ✅ Always decode
}
```

### 3. Pagination Without Total Count

**WRONG:**
```php
return ['data' => $data];  // ❌ Missing total_count
```

**CORRECT:**
```php
return [
    'data' => $data,
    'total_count' => $total  // ✅ Required for pagination UI
];
```

### 4. Nested Headers Without data_column

**WRONG:**
```php
[
    'label' => 'LBL_PARENT',
    'data_column' => 'parent',  // ❌ Parent has childs, no data_column
    'childs' => [...]
]
```

**CORRECT:**
```php
[
    'label' => 'LBL_PARENT',
    // ✅ No data_column for parents
    'childs' => [
        [
            'label' => 'LBL_CHILD1',
            'data_column' => 'child1',  // ✅ Child has data_column
            'type' => Reports_DataType_Enum::TEXT
        ]
    ]
]
```

### 5. Forgetting Paging Flag

**WRONG:**
```php
'main_table' => [
    'label' => 'LBL_TABLE',
    // ❌ Missing paging flag, no pagination shown
    'headers' => [...]
]
```

**CORRECT:**
```php
'main_table' => [
    'label' => 'LBL_TABLE',
    'paging' => true,  // ✅ Enable pagination
    'headers' => [...]
]
```

## Quick Reference

### Filter Types (Reports_FilterType_Enum)
- `PICKLIST` - Single select dropdown
- `MULTIPICKLIST` - Multi-select dropdown
- `DATE_RANGE` - From/to date picker
- `DATE` - Single date picker

### Data Types (Reports_DataType_Enum)
- `TEXT` - Plain text
- `CURRENCY` - Formatted currency ($1,234.56)
- `NUMBER` - Formatted number (1,234)
- `INT` - Integer
- `FLOAT` - Float with 2 decimals
- `PERCENT` - Percentage (12.34%)
- `DATE` - Date (d-m-Y)
- `DATETIME` - Datetime (d-m-Y H:i:s)
- `REFERENCE` - CRM record label
- `PICKLIST` - Translated picklist value
- `ACTION` - HTML actions (links/buttons)

### Helper Methods
- `getQueryConditions()` - Generate SQL WHERE from filters
- `getPagingModel()` - Create paging model from filter params
- `getPagingQueryFromPagingModel()` - Generate LIMIT clause
- `renderTableCellValue()` - Format cell by data type
- `setupCRMRelationTempTable()` - Create temp table for M2M relations

## Reference Files

- **[report-handler.md](references/report-handler.md)** - Two verified handlers (table, chart+table), getQueryConditions cheatsheet
- **[report-files.md](references/report-files.md)** - Real file map, load flow, template/JS override points
- **[report-framework.md](references/report-framework.md)** - Framework classes, enums, models, views

## See Also

- `view` - View controllers and templates
- `action` - AJAX endpoints
- `export` - Excel/CSV/PDF export
- `migration` - Database migrations for report registration

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Base class ĐỌC TRƯỚC (Tin Bui): `modules/Reports/custom/BaseFixedReportHandler.php`
- Report handler mẫu (Tin Bui): `modules/Reports/custom/TicketSummaryByStatusReportHandler.php`
- Report tổng hợp (Tin Bui): `modules/Reports/custom/ProjectWorkSummaryReportHandler.php`

## Verify

```bash
php -l <handler>
rm -f cache/templates_c/v7/*.php
# Mở report qua browser/curl, check bảng render + filter hoạt động; report có pagination → test trang 2
```

# Report Framework Reference

## Inheritance Hierarchy

```
CustomReportHandler (base)
    ↓
BaseFixedReportHandler (table reports)
    ↓
BaseFixedChartReportHandler (chart + table reports)
```

## Class: BaseFixedReportHandler

**Location:** `modules/Reports/custom/BaseFixedReportHandler.php`

**Purpose:** Abstract base for fixed table reports with filters, pagination, data rendering.

### Required Abstract Methods

```php
// Define filter fields (picklist, date_range, multipicklist, date)
abstract protected function getConfiguredFilterFields(): array;

// Define table column headers with nested structure support
protected abstract function getReportTableStructure(): array;

// Fetch report data with filters and pagination
protected abstract function getReportTableData(string $tableName, Vtiger_Paging_Model $pagingModel = null): array;
```

### Key Protected Properties

```php
protected $defaultPageSize = 20;
protected $reportFilterTemplate = 'modules/Reports/tpls/BaseFixedReport/Filter.tpl';
protected $reportMainTemplate = 'modules/Reports/tpls/BaseFixedReport/Main.tpl';
protected $reportTableTemplate = 'modules/Reports/tpls/BaseFixedReport/Table.tpl';
protected $filterParams;  // Current filter values
protected $filterFields;  // Processed filter field definitions
```

### Filter Management

```php
// Set filter parameters (from request)
public function setFilterParams(array $params): self;

// Get filter parameters with defaults merged
public function getFilterParams(): array;

// Get processed filter fields (cached)
protected function getFilterFields(): array;

// Generate SQL conditions from filters
protected function getQueryConditions(
    array $columnsMapping,     // ['filter_field' => 'sql_column']
    bool $fetchEmptyCondition = false,
    bool $forPrevPeriod = false
): array;
```

### Pagination Helpers

```php
// Create paging model from filter params
protected function getPagingModel(string $objectName): Vtiger_Paging_Model;

// Generate LIMIT query from paging model
protected function getPagingQueryFromPagingModel(Vtiger_Paging_Model $pagingModel): string;
```

### Data Processing

```php
// Override to customize filter picklist options
protected function getCustomFilterPicklistOptions(string $fieldName): array;

// Override to set default filter values
protected function getFilterParamDefaultValues(): array;

// Override to modify headers based on filter params
protected function processStructureHeadersByParams(array $structureHeaders, string $tableName): array;
```

### Rendering Methods

```php
// Render filter UI
public function renderReportFilter(array $params = []): string;

// Render report tables
public function renderReportTables(array $tableNames = []): string;

// Render cell value with type formatting
public function renderTableCellValue($value, array $metaHeader);

// Main render (used by view controller)
public function renderReportResult($filterSql, $showReportName = false, $print = false);
```

### Utility Methods

```php
// Get shared viewer with common assigns
protected function getSharedViewer(): Vtiger_Viewer;

// Setup temp table for M2M relations
protected function setupCRMRelationTempTable(string $moduleName, string $relModuleName): string;

// Get table names from structure
public function getReportTableNames(): array;

// Get calculated row size (for nested data)
public function getRowSize(array $row): int;
```

## Class: BaseFixedChartReportHandler

**Location:** `modules/Reports/custom/BaseFixedChartReportHandler.php`

**Extends:** `BaseFixedReportHandler`

**Purpose:** Adds chart rendering capabilities to table reports.

### Additional Abstract Methods

```php
// Define chart configurations
abstract protected function getReportChartSetting(): array;

// Generate Highcharts/Chart.js options
abstract protected function getReportChartOptions(string $chartName, array $chartOptions = []): array;

// Fetch chart data with filters and pagination
abstract protected function getReportChartData(string $chartName, Vtiger_Paging_Model $pagingModel = null): array;
```

### Chart Properties

```php
protected $defaultChartColumns = 1;      // Charts per row
protected $defaultRowSize = '320px';     // Chart height
protected $reportChartTemplate = 'modules/Reports/tpls/BaseFixedReport/Chart.tpl';
```

### Chart Rendering

```php
// Render chart UI
public function renderReportCharts(array $chartNames = []): string;

// Get chart data for API
public function getReportApiChartData(array $chartNames = []): array;
```

## Enums

### Reports_FilterType_Enum

**Location:** `modules/Reports/enums/FilterType.php`

```php
class Reports_FilterType_Enum {
    public const PICKLIST = 'PICKLIST';          // Single select dropdown
    public const MULTIPICKLIST = 'MULTIPICKLIST'; // Multi-select dropdown
    public const DATE_RANGE = 'DATE_RANGE';       // From/to date picker
    public const DATE = 'DATE';                   // Single date picker
}
```

### Reports_DataType_Enum

**Location:** `modules/Reports/enums/DataType.php`

```php
class Reports_DataType_Enum {
    public const ACTION = 'action';         // HTML actions (links/buttons)
    public const TEXT = 'text';             // Plain text
    public const CURRENCY = 'currency';     // Formatted currency
    public const NUMBER = 'number';         // Formatted number (comma separator)
    public const INT = 'int';               // Integer
    public const FLOAT = 'float';           // Float (2 decimals)
    public const DATE = 'date';             // Date (d-m-Y)
    public const DATETIME = 'date_time';    // Datetime (d-m-Y H:i:s)
    public const PERCENT = 'percent';       // Percentage (2 decimals + %)
    public const REFERENCE = 'relation';    // Record label from CRM ID
    public const PICKLIST = 'picklist';     // Translated picklist value
}
```

## Model: Reports_FixedReportTableView_Model

**Location:** `modules/Reports/models/FixedReportTableView.php`

**Purpose:** Process table structure configuration into renderable headers and data headers.

### Constructor

```php
public function __construct(string $tableName, array $configuredHeaders);
```

### Public Methods

```php
// Get rendered table headers (with rowspan/colspan)
public function getTableHeaders(): array;

// Get data column headers (flattened)
public function getTableDataHeaders(): array;

// Get total data column count
public function getTableDataHeadersNumber(): int;
```

### Header Structure

**Input (configured):**
```php
[
    'label' => 'LBL_COLUMN',
    'data_column' => 'field_name',    // Required for data cells
    'type' => DataType::TEXT,
    'bg_color' => '#008ecf',
    'txt_color' => '#ffffff',
    'tooltip' => 'Description',
    'align' => 'center',
    'childs' => [...]                 // Nested columns (optional)
]
```

**Output (table headers):**
```php
[
    [
        ['label' => 'Col1', 'rows' => 2, 'cols' => 1, 'colidx' => 0],
        ['label' => 'Col2', 'rows' => 1, 'cols' => 2, 'colidx' => 1]
    ],
    [
        ['label' => 'Col2.1', 'rows' => 1, 'cols' => 1, 'colidx' => 1],
        ['label' => 'Col2.2', 'rows' => 1, 'cols' => 1, 'colidx' => 2]
    ]
]
```

## View: Reports_FixedReportAjax_View

**Location:** `modules/Reports/views/FixedReportAjax.php`

**Purpose:** AJAX endpoints for dynamic report updates.

### Modes (Methods)

```php
// Load table data (with filters/pagination)
protected function getReportTableUI(Vtiger_Request $request, Vtiger_Response &$response);

// Load chart UI (with filters/pagination)
protected function getChartUI(Vtiger_Request $request, Vtiger_Response $response);

// Load custom statistics (specific to handler)
protected function getReportStatistics(Vtiger_Request $request, Vtiger_Response $response);
```

### Request Parameters

**getReportTableUI:**
```php
[
    'module' => 'Reports',
    'view' => 'FixedReportAjax',
    'mode' => 'getReportTableUI',
    'report_id' => 123,
    'table_name' => 'main_table',
    'filters' => [
        'status' => 'Open',
        'date_range' => ['from_date' => '2026-01-01', 'to_date' => '2026-01-31'],
        'main_table_page' => 2,
        'main_table_limit' => 20
    ]
]
```

**getChartUI:**
```php
[
    'module' => 'Reports',
    'view' => 'FixedReportAjax',
    'mode' => 'getChartUI',
    'report_id' => 123,
    'chart_name' => 'status_chart',
    'filters' => [...]
]
```

## Report Record Model

**Location:** `modules/Reports/models/Record.php`

### Handler Loading

```php
// Get custom handler instance
public function getCustomHandler(): ?BaseFixedReportHandler {
    $handlerClass = $this->get('handler_class');

    if (empty($handlerClass)) return null;

    $handlerFile = "modules/Reports/custom/{$handlerClass}.php";

    if (!file_exists($handlerFile)) return null;

    require_once($handlerFile);

    if (!class_exists($handlerClass)) return null;

    $handler = new $handlerClass();
    $handler->reportid = $this->getId();
    $handler->reportname = $this->get('reportname');

    return $handler;
}
```

## Handler Registration

### Option 1: Database Record

```sql
INSERT INTO vtiger_report (reportid, reportname, handler_class, folderid, reporttype, ...)
VALUES (
    123,
    'Ticket Summary Report',
    'TicketSummaryReportHandler',
    1,
    'tabular',
    ...
);
```

### Option 2: Migration

```php
public function up(): int {
    $reportId = $this->getUniqueId('vtiger_report');

    $sql = "INSERT INTO vtiger_report (reportid, reportname, handler_class, folderid, reporttype)
            VALUES (?, ?, ?, ?, ?)";
    $params = [
        $reportId,
        'Ticket Summary Report',
        'TicketSummaryReportHandler',
        1,
        'tabular'
    ];

    $this->pquery($sql, $params);

    return self::UP_SUCCESS;
}
```

## Critical Patterns

### Filter Data Source Types

**Reference (CRM records):**
```php
'data_source' => [
    'type' => 'reference',
    'module' => 'Users'  // or 'Accounts', 'Contacts', etc.
]
```

**Picklist (from field):**
```php
'data_source' => [
    'type' => 'picklist',
    'module' => 'HelpDesk',
    'field' => 'ticketstatus',
    'add_all_option' => true  // Add "All" option
]
```

**Custom (override method):**
```php
'data_source' => [
    'type' => 'custom'
]

protected function getCustomFilterPicklistOptions(string $fieldName): array {
    if ($fieldName === 'priority') {
        return [
            'low' => 'Low',
            'high' => 'High'
        ];
    }
    return [];
}
```

### Date Range Default Values

```php
protected function getFilterParamDefaultValues(): array {
    return [
        'date_range' => [
            'from_date' => date('Y-m-01 00:00:00'),
            'to_date' => date('Y-m-t 23:59:59'),
            'prev_from_date' => date('Y-m-01 00:00:00', strtotime('first day of previous month')),
            'prev_to_date' => date('Y-m-t 23:59:59', strtotime('last day of previous month')),
        ]
    ];
}
```

### Pagination Pattern

```php
protected function getReportTableData(string $tableName, Vtiger_Paging_Model $pagingModel = null): array {
    // Count total (without LIMIT)
    $countSql = "SELECT COUNT(*) as total FROM (...)";
    $countResult = $GLOBALS['adb']->pquery($countSql, $params);
    $total = (int) $GLOBALS['adb']->query_result($countResult, 0, 'total');

    // Add pagination
    if ($pagingModel) {
        $sql .= " " . $this->getPagingQueryFromPagingModel($pagingModel);
    }

    // Execute and fetch
    $result = $GLOBALS['adb']->pquery($sql, $params);
    $data = [];

    while ($row = $GLOBALS['adb']->fetchByAssoc($result)) {
        $row = decodeUTF8($row);
        $data[] = $row;
    }

    return [
        'data' => $data,
        'total_count' => $total
    ];
}
```

### Nested Table Headers

```php
[
    'label' => 'LBL_SALES',
    'bg_color' => '#008ecf',
    'childs' => [
        [
            'label' => 'LBL_Q1',
            'data_column' => 'q1_sales',
            'type' => DataType::CURRENCY
        ],
        [
            'label' => 'LBL_Q2',
            'data_column' => 'q2_sales',
            'type' => DataType::CURRENCY
        ]
    ]
]
```

Result: "Sales" header spans 2 columns, with "Q1" and "Q2" below.

### Chart Configuration

```php
protected function getReportChartSetting(): array {
    return [
        'status_chart' => [
            'label' => 'LBL_STATUS_CHART',
            'col_size' => 1,         // Chart width (1-12)
            'row_size' => 1,         // Chart height multiplier
            'paging' => false,       // Enable pagination
            'chart_options' => []    // Highcharts/Chart.js base config
        ]
    ];
}
```

### Chart Options Generation

```php
protected function getReportChartOptions(string $chartName, array $chartOptions = [], array $chartData = []): array {
    $labels = [];
    $values = [];

    foreach ($chartData as $row) {
        $labels[] = $row['status'];
        $values[] = (int) $row['count'];
    }

    return [
        'chart_total' => array_sum($values),
        'chart' => [
            'type' => 'bar'
        ],
        'title' => [
            'text' => 'Tickets by Status'
        ],
        'xAxis' => [
            'categories' => $labels
        ],
        'series' => [
            [
                'name' => 'Count',
                'data' => $values
            ]
        ]
    ];
}
```

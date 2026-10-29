# Report Handler Reference

Two complete handlers, run against the real base classes and the local DB (PHP 7.4 + 8.3): `getReportApiTableData()`, `renderReportTables()`, `getReportApiChartData()` and `renderReportCharts()` all return data/HTML.
For a production-size example read `modules/Reports/custom/TicketSummaryByStatusReportHandler.php` (group-by picklist, previous period, M2M temp table).

## Rules the samples follow

- The handler class name MUST equal the file basename: `Reports_Record_Model::getCustomHandler()` does `require_once($file)` then `new basename($file, '.php')($reportId)`.
- Override signatures exactly as the parent declares them (`getReportTableData(string $tableName, Vtiger_Paging_Model $pagingModel = null): array`). A different signature is a fatal "must be compatible" at load time.
- Read filters with `$this->getFilterParams()`; turn them into SQL with `$this->getQueryConditions(['filter_name' => 'sql_column'])` (values are escaped via `convert2Sql`).
- Return `['data' => [...rows], 'total_count' => int]`; the base reads `total_count` for paging.
- `vtiger_troubletickets` columns are `ticketid`, `ticket_no`, `title`, `status`, `priority`, `severity`, `category`; `createdtime` / `main_owner_id` live on `vtiger_crmentity`.

## Table report (BaseFixedReportHandler)

File `modules/Reports/custom/TicketListByStatusReportHandler.php`:

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('modules/Reports/custom/BaseFixedReportHandler.php');

class TicketListByStatusReportHandler extends BaseFixedReportHandler {

	protected function getConfiguredFilterFields(): array {
		return [
			'created_date' => [
				'uitype' => Reports_FilterType_Enum::DATE_RANGE,
				'label' => 'LBL_REPORT_DATA_DATE_RANGE',
			],
			'ticket_status' => [
				'uitype' => Reports_FilterType_Enum::PICKLIST,
				'data_source' => ['type' => 'picklist', 'module' => 'HelpDesk', 'field' => 'ticketstatus', 'add_all_option' => true],
			],
			'ticket_main_owner_id' => [
				'uitype' => Reports_FilterType_Enum::MULTIPICKLIST,
				'data_source' => ['type' => 'reference', 'module' => 'Users'],
			],
		];
	}

	protected function getReportTableStructure(): array {
		return [
			'ticket_table' => [
				'label' => 'LBL_REPORT_TICKET_TABLE',
				'paging' => true,
				'headers' => [
					['label' => 'LBL_TICKET_NO', 'data_column' => 'ticket_no', 'type' => Reports_DataType_Enum::TEXT],
					['label' => 'LBL_TITLE', 'data_column' => 'title', 'type' => Reports_DataType_Enum::TEXT, 'align' => 'left'],
					['label' => 'LBL_STATUS', 'data_column' => 'status', 'type' => Reports_DataType_Enum::PICKLIST, 'picklist_module' => 'HelpDesk'],
					['label' => 'LBL_MAIN_OWNER_ID', 'data_column' => 'main_owner_id', 'type' => Reports_DataType_Enum::REFERENCE, 'reference_module' => 'Users'],
					['label' => 'LBL_CREATED_TIME', 'data_column' => 'createdtime', 'type' => Reports_DataType_Enum::DATETIME],
				],
			],
		];
	}

	protected function getReportTableData(string $tableName, Vtiger_Paging_Model $pagingModel = null): array {
		$filterParams = $this->getFilterParams();
		$columnsMapping = [
			'created_date' => 'e.createdtime',
			'ticket_status' => 't.status',
			'ticket_main_owner_id' => 'e.main_owner_id',
		];

		// The add_all_option value 'all' means no status condition
		if (($filterParams['ticket_status'] ?? '') === 'all') {
			unset($columnsMapping['ticket_status']);
		}

		$conditions = $this->getQueryConditions($columnsMapping);
		$sql = "SELECT t.ticket_no, t.title, t.status, e.main_owner_id, e.createdtime
			FROM vtiger_troubletickets AS t
			INNER JOIN vtiger_crmentity AS e ON (e.crmid = t.ticketid AND e.deleted = 0)";

		if (!empty($conditions)) {
			$sql .= ' WHERE ' . implode(' AND ', $conditions);
		}

		$totalCount = (int) $GLOBALS['adb']->getOne("SELECT COUNT(*) FROM ({$sql}) AS temp");
		$sql .= ' ORDER BY e.createdtime DESC';

		if (!empty($pagingModel)) {
			$sql .= ' ' . $this->getPagingQueryFromPagingModel($pagingModel);
		}

		$data = [];
		$result = $GLOBALS['adb']->pquery($sql, []);

		while ($result && $row = $GLOBALS['adb']->fetchByAssoc($result)) {
			$data[] = $row;
		}

		return [
			'data' => $data,
			'total_count' => $totalCount,
		];
	}
}
```

## Chart + table report (BaseFixedChartReportHandler)

A chart handler implements the 3 table abstracts plus the 3 chart abstracts: `getReportChartSetting()`, `getReportChartData()`, `getReportChartOptions()`.
`renderReportCharts()` calls `getReportChartData()` (must return `data` + `total_count`), then `getReportChartOptions($chartName, $setting['chart_options'], $rows)` — the options array is Highcharts config plus `chart_total`.

File `modules/Reports/custom/TicketCountByStatusChartReportHandler.php`:

```php
<?php

/**
 * @author Your Name
 * @email your.email@company.vn
 * @create date YYYY.MM.DD
 */

require_once('modules/Reports/custom/BaseFixedChartReportHandler.php');

class TicketCountByStatusChartReportHandler extends BaseFixedChartReportHandler {

	protected function getConfiguredFilterFields(): array {
		return [
			'created_date' => [
				'uitype' => Reports_FilterType_Enum::DATE_RANGE,
				'label' => 'LBL_REPORT_DATA_DATE_RANGE',
			],
		];
	}

	protected function getReportTableStructure(): array {
		return [
			'status_table' => [
				'label' => 'LBL_REPORT_TICKET_BY_STATUS',
				'headers' => [
					['label' => 'LBL_STATUS', 'data_column' => 'status', 'type' => Reports_DataType_Enum::PICKLIST, 'picklist_module' => 'HelpDesk'],
					['label' => 'LBL_REPORT_TICKET_TOTAL_NUMBER', 'data_column' => 'ticket_count', 'type' => Reports_DataType_Enum::NUMBER],
				],
			],
		];
	}

	protected function getReportTableData(string $tableName, Vtiger_Paging_Model $pagingModel = null): array {
		$conditions = $this->getQueryConditions(['created_date' => 'e.createdtime']);
		$sql = "SELECT t.status, COUNT(*) AS ticket_count
			FROM vtiger_troubletickets AS t
			INNER JOIN vtiger_crmentity AS e ON (e.crmid = t.ticketid AND e.deleted = 0)";

		if (!empty($conditions)) {
			$sql .= ' WHERE ' . implode(' AND ', $conditions);
		}

		$sql .= ' GROUP BY t.status ORDER BY ticket_count DESC';

		$data = [];
		$result = $GLOBALS['adb']->pquery($sql, []);

		while ($result && $row = $GLOBALS['adb']->fetchByAssoc($result)) {
			$data[] = $row;
		}

		return [
			'data' => $data,
			'total_count' => count($data),
		];
	}

	protected function getReportChartSetting(): array {
		return [
			'status_chart' => [
				'label' => vtranslate('LBL_REPORT_TICKET_BY_STATUS', 'Reports'),
				'col_size' => 1,
				'row_size' => 1,
				'chart_options' => [
					'chart' => ['type' => 'column'],
				],
			],
		];
	}

	protected function getReportChartData(string $chartName, Vtiger_Paging_Model $pagingModel = null): array {
		switch ($chartName) {
			case 'status_chart': {
				return $this->getReportTableData('status_table', $pagingModel);
			}
		}

		return ['data' => [], 'total_count' => 0];
	}

	// The base passes the rows of getReportChartData() as an extra 3rd argument
	protected function getReportChartOptions(string $chartName, array $chartOptions = [], array $chartData = []): array {
		$categories = $values = [];

		foreach ($chartData as $row) {
			$categories[] = vtranslate($row['status'], 'HelpDesk');
			$values[] = (int) $row['ticket_count'];
		}

		return array_merge($chartOptions, [
			'chart_total' => array_sum($values),
			'xAxis' => ['categories' => $categories],
			'series' => [
				['name' => vtranslate('LBL_REPORT_TICKET_TOTAL_NUMBER', 'Reports'), 'data' => $values],
			],
		]);
	}
}
```

## getQueryConditions() by filter uitype

|uitype|Filter value shape|Generated condition|
|---|---|---|
|`DATE_RANGE`|`['from_date' => ..., 'to_date' => ..., 'prev_from_date' => ..., 'prev_to_date' => ...]`|`col BETWEEN from AND to` (`prev_*` when the 3rd arg `$forPrevPeriod` is true); array of columns → OR-ed|
|`DATE`|`'Y-m-d'` string|`DATE(col) = ?`|
|`PICKLIST`|scalar|`col = ?` — a value of `'all'` (from `add_all_option`) is NOT special-cased: drop that mapping yourself|
|`MULTIPICKLIST`|array, or a string joined by `' \|##\| '`|`col IN (...)`|

Pass `$fetchEmptyCondition = true` (2nd arg) to get `COALESCE(col, '') = ''` for empty values.

## Pagination

- Set `'paging' => true` on the table (or chart) setting; the base builds the paging model from filter params `<table_name>_page` / `<table_name>_limit` (default 20 rows).
- Count without the LIMIT, then append `$this->getPagingQueryFromPagingModel($pagingModel)` (returns `LIMIT offset, limit`).

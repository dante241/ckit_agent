# Report Files Reference

## Complete File Structure

```
modules/Reports/
├── custom/
│   ├── BaseFixedReportHandler.php              → Abstract base class
│   ├── BaseFixedChartReportHandler.php         → Chart extension
│   └── TicketSummaryReportHandler.php          → Custom report handler
├── enums/
│   ├── FilterType.php                          → Filter type constants
│   └── DataType.php                            → Data type constants
├── models/
│   ├── Record.php                              → Report record model
│   └── FixedReportTableView.php                → Table rendering model
├── views/
│   ├── FixedReportAjax.php                     → AJAX view controller
│   └── FixedReport.php                         → Main report view
├── tpls/
│   └── BaseFixedReport/
│       ├── Main.tpl                            → Main report template
│       ├── Filter.tpl                          → Filter section
│       ├── Table.tpl                           → Table rendering
│       ├── Pagination.tpl                      → Pagination controls
│       ├── Chart.tpl                           → Chart container
│       └── NoData.tpl                          → Empty state
└── resources/
    └── BaseFixedReport/
        ├── BaseFixedReport.js                  → Main JS controller
        ├── BaseFixedReportChartWidget.js       → Chart widget
        ├── CustomReportHelper.js               → Helper utilities
        ├── Layout.css                          → Layout styles
        ├── Filter.css                          → Filter styles
        ├── Table.css                           → Table styles
        └── Chart.css                           → Chart styles
```

## Handler Registration

**Option 1: Via Report Record (Database)**

Create report record in `vtiger_report`:
```sql
INSERT INTO vtiger_report (reportid, reportname, handler_class, folderid, ...)
VALUES (
    123,
    'Ticket Summary Report',
    'TicketSummaryReportHandler',
    1,
    ...
);
```

**Option 2: Via Extension (Programmatic)**

In module's `Extensions.php`:
```php
<?php

return [
    'reports' => [
        [
            'name' => 'Ticket Summary Report',
            'handler' => 'TicketSummaryReportHandler',
            'folder' => 'HelpDesk Reports',
            'description' => 'Summary of tickets by status and priority'
        ]
    ]
];
```

## Template Files

### Main.tpl

**Location:** `modules/Reports/tpls/BaseFixedReport/Main.tpl`

```smarty
{*
    Main.tpl
    Main report container
*}

<div class="fixed-report-container">
    {* Page Header *}
    <div class="report-header">
        <h3>{$REPORT_NAME}</h3>
        <div class="report-actions">
            <button class="btn btn-primary btn-export-csv">
                <i class="fa fa-download"></i> {vtranslate('LBL_EXPORT_CSV', 'Reports')}
            </button>
        </div>
    </div>

    {* Filters Section *}
    {include file="modules/Reports/tpls/BaseFixedReport/Filter.tpl"}

    {* Chart Section (if chart report) *}
    {if $HAS_CHART}
        {include file="modules/Reports/tpls/BaseFixedReport/Chart.tpl"}
    {/if}

    {* Table Section *}
    {include file="modules/Reports/tpls/BaseFixedReport/Table.tpl"}

    {* Pagination *}
    {include file="modules/Reports/tpls/BaseFixedReport/Pagination.tpl"}
</div>
```

### Filter.tpl

**Location:** `modules/Reports/tpls/BaseFixedReport/Filter.tpl`

```smarty
{*
    Filter.tpl
    Filter controls
*}

<div class="report-filters">
    <form class="filter-form">
        <div class="row">
            {foreach from=$FILTER_FIELDS item=FILTER}
                <div class="col-md-3">
                    <div class="form-group">
                        <label>{vtranslate($FILTER.label, 'Reports')}</label>

                        {if $FILTER.type eq 'picklist'}
                            <select name="filters[{$FILTER.name}]" class="form-control">
                                <option value="">{vtranslate('LBL_ALL', 'Reports')}</option>
                                {foreach from=$FILTER.options item=OPTION}
                                    <option value="{$OPTION}">{vtranslate($OPTION, 'Reports')}</option>
                                {/foreach}
                            </select>
                        {elseif $FILTER.type eq 'date_range'}
                            <div class="input-daterange input-group">
                                <input type="text" name="filters[{$FILTER.name}][start]" class="form-control date-picker" placeholder="Start Date" />
                                <span class="input-group-addon">to</span>
                                <input type="text" name="filters[{$FILTER.name}][end]" class="form-control date-picker" placeholder="End Date" />
                            </div>
                        {/if}
                    </div>
                </div>
            {/foreach}

            <div class="col-md-12">
                <button type="submit" class="btn btn-primary btn-apply-filters">
                    <i class="fa fa-filter"></i> {vtranslate('LBL_APPLY_FILTERS', 'Reports')}
                </button>
                <button type="button" class="btn btn-default btn-clear-filters">
                    <i class="fa fa-times"></i> {vtranslate('LBL_CLEAR_FILTERS', 'Reports')}
                </button>
            </div>
        </div>
    </form>
</div>
```

### Table.tpl

**Location:** `modules/Reports/tpls/BaseFixedReport/Table.tpl`

```smarty
{*
    Table.tpl
    Report table
*}

<div class="report-table-container">
    {if $DATA && count($DATA) gt 0}
        <table class="table table-bordered table-striped report-table">
            <thead>
                <tr>
                    {foreach from=$TABLE_STRUCTURE item=COLUMN}
                        <th width="{$COLUMN.width|default:'auto'}">
                            {vtranslate($COLUMN.label, 'Reports')}

                            {* Nested columns *}
                            {if $COLUMN.childs}
                                <table class="nested-header">
                                    <tr>
                                        {foreach from=$COLUMN.childs item=CHILD}
                                            <th>{vtranslate($CHILD.label, 'Reports')}</th>
                                        {/foreach}
                                    </tr>
                                </table>
                            {/if}
                        </th>
                    {/foreach}
                </tr>
            </thead>
            <tbody>
                {foreach from=$DATA item=ROW}
                    <tr>
                        {foreach from=$TABLE_STRUCTURE item=COLUMN}
                            <td align="{$COLUMN.align|default:'left'}">
                                {if $COLUMN.type eq 'currency'}
                                    ${$ROW[$COLUMN.field]|number_format:2}
                                {elseif $COLUMN.type eq 'percent'}
                                    {$ROW[$COLUMN.field]}%
                                {elseif $COLUMN.type eq 'action'}
                                    {$ROW[$COLUMN.field]}
                                {else}
                                    {$ROW[$COLUMN.field]}
                                {/if}
                            </td>
                        {/foreach}
                    </tr>
                {/foreach}
            </tbody>
        </table>
    {else}
        {include file="modules/Reports/tpls/BaseFixedReport/NoData.tpl"}
    {/if}
</div>
```

## JavaScript Files

### BaseFixedReport.js

**Location:** `modules/Reports/resources/BaseFixedReport/BaseFixedReport.js`

```javascript
/*
    BaseFixedReport.js
    Main report controller
*/

Vtiger_Index_Js('Reports_BaseFixedReport_Js', {}, {

    registerEvents: function() {
        this._super();
        this.registerFilterEvents();
        this.registerExportEvents();
        this.registerPaginationEvents();
    },

    registerFilterEvents: function() {
        var self = this;

        jQuery('.btn-apply-filters').on('click', function(e) {
            e.preventDefault();
            self.applyFilters();
        });

        jQuery('.btn-clear-filters').on('click', function() {
            self.clearFilters();
        });
    },

    applyFilters: function() {
        var form = jQuery('.filter-form');
        var filters = form.serializeFormData();

        this.loadReportData(filters, 1);
    },

    loadReportData: function(filters, page) {
        var self = this;
        app.helper.showProgress();

        var params = {
            module: 'Reports',
            view: 'FixedReportAjax',
            action: 'getReportTableUI',
            report_id: this.getReportId(),
            filters: filters,
            page: page
        };

        app.request.post({ data: params }).then(function(error, data) {
            app.helper.hideProgress();

            if (!error && data.success) {
                jQuery('.report-table-container').html(data.html);
                self.updatePagination(data.pagination);
            }
        });
    },

    registerExportEvents: function() {
        var self = this;

        jQuery('.btn-export-csv').on('click', function() {
            self.exportToCSV();
        });
    },

    exportToCSV: function() {
        var filters = jQuery('.filter-form').serializeFormData();
        var params = jQuery.param({
            module: 'Reports',
            action: 'ExportData',
            report_id: this.getReportId(),
            filters: filters,
            format: 'csv'
        });

        window.location = 'index.php?' + params;
    },

    getReportId: function() {
        return jQuery('[name="report_id"]').val();
    }
});
```

## CSS Files

### Layout.css

**Location:** `modules/Reports/resources/BaseFixedReport/Layout.css`

```css
.fixed-report-container {
    padding: 20px;
}

.report-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 20px;
}

.report-header h3 {
    margin: 0;
}

.report-actions {
    display: flex;
    gap: 10px;
}
```

### Table.css

**Location:** `modules/Reports/resources/BaseFixedReport/Table.css`

```css
.report-table-container {
    margin-top: 20px;
    overflow-x: auto;
}

.report-table {
    width: 100%;
    border-collapse: collapse;
}

.report-table thead th {
    background-color: var(--gray-1);
    font-weight: 600;
    padding: 12px;
    text-align: left;
}

.report-table tbody td {
    padding: 10px 12px;
    border-bottom: 1px solid var(--gray-2);
}

.report-table tbody tr:hover {
    background-color: var(--gray-1);
}

/* Nested columns */
.nested-header {
    width: 100%;
    margin-top: 5px;
}

.nested-header th {
    border-left: 1px solid var(--gray-3);
    padding: 5px;
    font-weight: normal;
}
```

## How Report Loads

1. **User accesses report:**
   ```
   index.php?module=Reports&view=FixedReport&record=123
   ```

2. **View controller loads:**
   ```php
   Reports_FixedReport_View::process()
   ```

3. **Get handler:**
   ```php
   $reportRecord = Vtiger_Record_Model::getInstanceById(123, 'Reports');
   $handler = $reportRecord->getCustomHandler();
   ```

4. **Render filters and structure:**
   ```php
   $filters = $handler->getConfiguredFilterFields();
   $structure = $handler->getReportTableStructure();
   ```

5. **Fetch data (AJAX):**
   ```javascript
   app.request.post({
       module: 'Reports',
       view: 'FixedReportAjax',
       action: 'getReportTableUI'
   })
   ```

6. **Handler processes:**
   ```php
   $data = $handler->getReportTableData($filters, $page, $limit);
   ```

7. **Render table HTML and return**

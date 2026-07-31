# Report Handler Reference

## Full Handler Template

**Location:** `modules/Reports/custom/TicketSummaryReportHandler.php`

```php
<?php

/**
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 */

class TicketSummaryReportHandler extends BaseFixedReportHandler {

    /**
     * Configure filter fields
     */
    public function getConfiguredFilterFields(): array {
        return [
            [
                'name' => 'status',
                'label' => 'LBL_STATUS',
                'type' => FilterType::PICKLIST,
                'options' => ['Open', 'In Progress', 'Closed', 'Waiting for Customer'],
                'default' => ''
            ],
            [
                'name' => 'priority',
                'label' => 'LBL_PRIORITY',
                'type' => FilterType::MULTIPICKLIST,
                'options' => ['Low', 'Normal', 'High', 'Urgent']
            ],
            [
                'name' => 'date_range',
                'label' => 'LBL_REPORT_DATA_DATE_RANGE',
                'type' => FilterType::DATE_RANGE
            ],
            [
                'name' => 'assigned_user',
                'label' => 'LBL_ASSIGNED_TO',
                'type' => FilterType::PICKLIST,
                'options' => $this->getUsers()
            ]
        ];
    }

    /**
     * Define table structure
     */
    public function getReportTableStructure(): array {
        return [
            [
                'label' => 'LBL_TICKET_ID',
                'field' => 'ticket_id',
                'type' => DataType::TEXT,
                'width' => '10%'
            ],
            [
                'label' => 'LBL_TITLE',
                'field' => 'title',
                'type' => DataType::TEXT,
                'width' => '30%'
            ],
            [
                'label' => 'LBL_STATUS',
                'field' => 'status',
                'type' => DataType::PICKLIST,
                'width' => '15%'
            ],
            [
                'label' => 'LBL_PRIORITY',
                'field' => 'priority',
                'type' => DataType::PICKLIST,
                'width' => '10%'
            ],
            [
                'label' => 'LBL_ASSIGNED_TO',
                'field' => 'assigned_user_name',
                'type' => DataType::TEXT,
                'width' => '15%'
            ],
            [
                'label' => 'LBL_CREATED_TIME',
                'field' => 'createdtime',
                'type' => DataType::DATETIME,
                'width' => '15%'
            ],
            [
                'label' => 'LBL_ACTIONS',
                'field' => 'actions',
                'type' => DataType::ACTION,
                'width' => '5%'
            ]
        ];
    }

    /**
     * Fetch report data with filters and pagination
     */
    public function getReportTableData(array $filters, int $page, int $limit): array {
        global $adb;

        // Build query
        $sql = "SELECT t.ticket_id, t.title, t.status, t.priority, t.createdtime,
                       CONCAT(u.first_name, ' ', u.last_name) as assigned_user_name,
                       t.ticketid
                FROM vtiger_troubletickets t
                INNER JOIN vtiger_crmentity c ON c.crmid = t.ticketid
                LEFT JOIN vtiger_users u ON u.id = c.smownerid
                WHERE c.deleted = ?";
        $params = [0];

        // Apply filters
        if (!empty($filters['status'])) {
            $sql .= " AND t.status = ?";
            $params[] = (string) $filters['status'];
        }

        if (!empty($filters['priority']) && is_array($filters['priority'])) {
            $placeholders = implode(',', array_fill(0, count($filters['priority']), '?'));
            $sql .= " AND t.priority IN ($placeholders)";
            foreach ($filters['priority'] as $priority) {
                $params[] = (string) $priority;
            }
        }

        if (!empty($filters['date_range'])) {
            $startDate = (string) $filters['date_range']['start'];
            $endDate = (string) $filters['date_range']['end'];

            if (!empty($startDate) && !empty($endDate)) {
                $sql .= " AND c.createdtime BETWEEN ? AND ?";
                $params[] = $startDate . ' 00:00:00';
                $params[] = $endDate . ' 23:59:59';
            }
        }

        if (!empty($filters['assigned_user'])) {
            $sql .= " AND c.smownerid = ?";
            $params[] = (int) $filters['assigned_user'];
        }

        // Count total records
        $countSql = "SELECT COUNT(*) as count FROM ($sql) as temp";
        $countResult = $adb->pquery($countSql, $params);
        $total = (int) $adb->query_result($countResult, 0, 'count');

        // Add sorting
        $sql .= " ORDER BY c.createdtime DESC";

        // Add pagination
        $offset = ($page - 1) * $limit;
        $sql .= " LIMIT ? OFFSET ?";
        $params[] = $limit;
        $params[] = $offset;

        // Execute query
        $result = $adb->pquery($sql, $params);
        $data = [];

        while ($row = $adb->fetchByAssoc($result)) {
            $row = decodeUTF8($row);

            // Add action links
            $row['actions'] = $this->getActionLinks((int) $row['ticketid']);

            $data[] = $row;
        }

        return [
            'data' => $data,
            'total' => $total
        ];
    }

    /**
     * Define grouping options
     */
    public function getGroupByList(): array {
        return [
            [
                'label' => 'LBL_STATUS',
                'field' => 'status'
            ],
            [
                'label' => 'LBL_PRIORITY',
                'field' => 'priority'
            ],
            [
                'label' => 'LBL_ASSIGNED_TO',
                'field' => 'assigned_user_id'
            ]
        ];
    }

    /**
     * Get list of users for filter
     */
    protected function getUsers(): array {
        global $adb;

        $sql = "SELECT id, CONCAT(first_name, ' ', last_name) as name
                FROM vtiger_users
                WHERE status = 'Active'
                ORDER BY first_name, last_name";
        $result = $adb->pquery($sql, []);

        $users = [];
        while ($row = $adb->fetchByAssoc($result)) {
            $row = decodeUTF8($row);
            $users[$row['id']] = $row['name'];
        }

        return $users;
    }

    /**
     * Generate action links for each row
     */
    protected function getActionLinks(int $recordId): string {
        $viewUrl = "index.php?module=HelpDesk&view=Detail&record=$recordId";
        $editUrl = "index.php?module=HelpDesk&view=Edit&record=$recordId";

        return '<a href="' . $viewUrl . '" class="btn btn-xs btn-info" title="View">
                    <i class="fa fa-eye"></i>
                </a>
                <a href="' . $editUrl . '" class="btn btn-xs btn-primary" title="Edit">
                    <i class="fa fa-edit"></i>
                </a>';
    }
}
```

## Chart Report Handler

**Extends BaseFixedChartReportHandler for chart support:**

```php
<?php

/**
 * @author CloudGo
 * @email dev@cloudgo.vn
 * @create date 2026.02.10
 */

class TicketChartReportHandler extends BaseFixedChartReportHandler {

    public function getConfiguredFilterFields(): array {
        return [
            [
                'name' => 'date_range',
                'label' => 'LBL_REPORT_DATA_DATE_RANGE',
                'type' => FilterType::DATE_RANGE
            ]
        ];
    }

    public function getReportTableStructure(): array {
        return [
            [
                'label' => 'LBL_STATUS',
                'field' => 'status',
                'type' => DataType::TEXT
            ],
            [
                'label' => 'LBL_COUNT',
                'field' => 'count',
                'type' => DataType::NUMBER
            ],
            [
                'label' => 'LBL_PERCENTAGE',
                'field' => 'percentage',
                'type' => DataType::PERCENT
            ]
        ];
    }

    public function getReportTableData(array $filters, int $page, int $limit): array {
        global $adb;

        $sql = "SELECT t.status, COUNT(*) as count
                FROM vtiger_troubletickets t
                INNER JOIN vtiger_crmentity c ON c.crmid = t.ticketid
                WHERE c.deleted = ?";
        $params = [0];

        if (!empty($filters['date_range'])) {
            $startDate = (string) $filters['date_range']['start'];
            $endDate = (string) $filters['date_range']['end'];

            if (!empty($startDate) && !empty($endDate)) {
                $sql .= " AND c.createdtime BETWEEN ? AND ?";
                $params[] = $startDate . ' 00:00:00';
                $params[] = $endDate . ' 23:59:59';
            }
        }

        $sql .= " GROUP BY t.status ORDER BY count DESC";

        $result = $adb->pquery($sql, $params);
        $data = [];
        $total = 0;

        while ($row = $adb->fetchByAssoc($result)) {
            $row = decodeUTF8($row);
            $total += (int) $row['count'];
            $data[] = $row;
        }

        // Calculate percentages
        foreach ($data as &$row) {
            $row['percentage'] = $total > 0 ? round(($row['count'] / $total) * 100, 2) : 0;
        }

        return [
            'data' => $data,
            'total' => count($data)
        ];
    }

    /**
     * Get chart data
     */
    public function getChartData(array $filters): array {
        $result = $this->getReportTableData($filters, 1, 999);

        $labels = [];
        $values = [];

        foreach ($result['data'] as $row) {
            $labels[] = $row['status'];
            $values[] = (int) $row['count'];
        }

        return [
            'type' => 'bar',  // bar, line, pie, donut
            'labels' => $labels,
            'datasets' => [
                [
                    'label' => 'Tickets by Status',
                    'data' => $values,
                    'backgroundColor' => [
                        '#4CAF50',  // Open - Green
                        '#2196F3',  // In Progress - Blue
                        '#FF9800',  // Waiting - Orange
                        '#9E9E9E'   // Closed - Gray
                    ]
                ]
            ]
        ];
    }
}
```

## Filter Processing Examples

### Single Picklist Filter

```php
if (!empty($filters['status'])) {
    $sql .= " AND t.status = ?";
    $params[] = (string) $filters['status'];
}
```

### Multi-Picklist Filter

```php
if (!empty($filters['priority']) && is_array($filters['priority'])) {
    $placeholders = implode(',', array_fill(0, count($filters['priority']), '?'));
    $sql .= " AND t.priority IN ($placeholders)";
    foreach ($filters['priority'] as $priority) {
        $params[] = (string) $priority;
    }
}
```

### Date Range Filter

```php
if (!empty($filters['date_range'])) {
    $startDate = (string) $filters['date_range']['start'];
    $endDate = (string) $filters['date_range']['end'];

    if (!empty($startDate) && !empty($endDate)) {
        $sql .= " AND c.createdtime BETWEEN ? AND ?";
        $params[] = $startDate . ' 00:00:00';
        $params[] = $endDate . ' 23:59:59';
    }
}
```

### Single Date Filter

```php
if (!empty($filters['created_date'])) {
    $date = (string) $filters['created_date'];
    $sql .= " AND DATE(c.createdtime) = ?";
    $params[] = $date;
}
```

## Pagination Pattern

```php
// Count total
$countSql = "SELECT COUNT(*) as count FROM ($sql) as temp";
$countResult = $adb->pquery($countSql, $params);
$total = (int) $adb->query_result($countResult, 0, 'count');

// Add pagination to main query
$offset = ($page - 1) * $limit;
$sql .= " LIMIT ? OFFSET ?";
$params[] = $limit;
$params[] = $offset;

// Return with total
return [
    'data' => $data,
    'total' => $total
];
```

## Summary/Aggregation Report

```php
public function getReportTableData(array $filters, int $page, int $limit): array {
    global $adb;

    $sql = "SELECT
                t.status,
                COUNT(*) as ticket_count,
                AVG(TIMESTAMPDIFF(HOUR, c.createdtime, NOW())) as avg_age_hours,
                SUM(CASE WHEN t.priority = 'High' THEN 1 ELSE 0 END) as high_priority_count
            FROM vtiger_troubletickets t
            INNER JOIN vtiger_crmentity c ON c.crmid = t.ticketid
            WHERE c.deleted = 0
            GROUP BY t.status
            ORDER BY ticket_count DESC";

    $result = $adb->pquery($sql, []);
    $data = [];

    while ($row = $adb->fetchByAssoc($result)) {
        $row = decodeUTF8($row);
        $row['avg_age_hours'] = round($row['avg_age_hours'], 2);
        $data[] = $row;
    }

    return [
        'data' => $data,
        'total' => count($data)
    ];
}
```

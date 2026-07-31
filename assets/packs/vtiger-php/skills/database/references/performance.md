# Database Performance Reference

> MySQL optimization, indexing, caching, and query tuning for VTiger

## Query Analysis with EXPLAIN

### Basic EXPLAIN

```sql
EXPLAIN SELECT a.accountid, a.accountname
FROM vtiger_account a
INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
WHERE e.deleted = 0 AND a.accountname LIKE 'Test%';
```

### EXPLAIN Output Columns

| Column | Good Values | Bad Values | Meaning |
|--------|-------------|------------|---------|
| `type` | const, eq_ref, ref | index, ALL | Join type (const is best) |
| `possible_keys` | Shows indexes | NULL | Available indexes |
| `key` | Index name | NULL | Actually used index |
| `rows` | Low number | High number | Estimated rows scanned |
| `Extra` | Using index | Using filesort, Using temporary | Additional info |

### Type Values (Best to Worst)

1. **const**: Single row match via PRIMARY KEY or UNIQUE
2. **eq_ref**: One row per previous row combination (PRIMARY KEY/UNIQUE lookup)
3. **ref**: Multiple rows match indexed column
4. **range**: Index range scan (BETWEEN, IN, >, <)
5. **index**: Full index scan (better than ALL but still slow)
6. **ALL**: Full table scan (AVOID THIS)

### Red Flags in Extra Column

- **Using filesort**: Sorting without index (slow for large datasets)
- **Using temporary**: Creates temp table (expensive operation)
- **Using where with ALL type**: Full table scan with filter

## Indexing Strategies

### When to Add Indexes

```sql
-- Always index foreign keys
ALTER TABLE vtiger_contactdetails ADD INDEX idx_accountid (accountid);

-- Index WHERE clause columns
ALTER TABLE vtiger_crmentity ADD INDEX idx_setype_deleted (setype, deleted);

-- Index ORDER BY columns
ALTER TABLE vtiger_crmentity ADD INDEX idx_modifiedtime (modifiedtime);

-- Index JOIN conditions
ALTER TABLE vtiger_campaign ADD INDEX idx_social_campaign_id (social_campaign_id);
```

### Composite Index Leftmost Prefix Rule

```sql
-- Create composite index
ALTER TABLE vtiger_crmentity ADD INDEX idx_setype_deleted_created (setype, deleted, createdtime);

-- These queries use the index:
WHERE setype = 'Accounts'
WHERE setype = 'Accounts' AND deleted = 0
WHERE setype = 'Accounts' AND deleted = 0 AND createdtime > '2025-01-01'

-- This does NOT use the index (missing leftmost column):
WHERE deleted = 0 AND createdtime > '2025-01-01'
```

### Index Best Practices

1. **Index high-cardinality columns**: Columns with many unique values
2. **Avoid indexing low-cardinality**: status (2-3 values), boolean flags
3. **Limit index count**: Too many indexes slow INSERT/UPDATE
4. **Monitor index usage**: Remove unused indexes

```sql
-- Check index usage (MySQL 5.6+)
SELECT * FROM sys.schema_unused_indexes WHERE object_schema = 'vtiger';
```

## Query Optimization

### Avoid SELECT *

```php
// BAD - loads all columns
$sql = "SELECT * FROM vtiger_account a
        INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
        WHERE e.deleted = 0";

// GOOD - only needed columns
$sql = "SELECT a.accountid, a.accountname, a.website
        FROM vtiger_account a
        INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
        WHERE e.deleted = 0";
```

### Always JOIN with deleted = 0

```php
// BAD - returns deleted records
$sql = "SELECT * FROM vtiger_account WHERE accountid = ?";

// GOOD - filters soft-deleted
$sql = "SELECT a.* FROM vtiger_account a
        INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
        WHERE e.deleted = 0 AND a.accountid = ?";
```

### Use LIMIT for Pagination

```php
// Paginate results
$page = 1;
$limit = 20;
$offset = ($page - 1) * $limit;

$sql = "SELECT a.accountid, a.accountname
        FROM vtiger_account a
        INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
        WHERE e.deleted = 0
        ORDER BY a.accountname
        LIMIT ? OFFSET ?";
$result = $adb->pquery($sql, [$limit, $offset]);
```

### EXISTS vs IN for Subqueries

```php
// Prefer EXISTS for large subqueries (stops at first match)
$sql = "SELECT a.accountid FROM vtiger_account a
        WHERE EXISTS (
            SELECT 1 FROM vtiger_contactdetails c
            WHERE c.accountid = a.accountid AND c.email LIKE '%@example.com'
        )";

// IN works for small result sets
$sql = "SELECT a.accountid FROM vtiger_account a
        WHERE a.accountid IN (1, 2, 3, 4, 5)";
```

### Avoid Functions on Indexed Columns

```php
// BAD - breaks index usage
$sql = "SELECT * FROM vtiger_crmentity
        WHERE YEAR(createdtime) = 2025";

// GOOD - uses index
$sql = "SELECT * FROM vtiger_crmentity
        WHERE createdtime >= '2025-01-01 00:00:00'
        AND createdtime < '2026-01-01 00:00:00'";
```

### Use generateQuestionMarks for IN Clauses

```php
// Efficient IN clause with variable length
$ids = [1, 2, 3, 4, 5];
$marks = generateQuestionMarks($ids);

$sql = "SELECT * FROM vtiger_crmentity
        WHERE crmid IN ($marks) AND deleted = 0";
$result = $adb->pquery($sql, $ids);
```

### Use getOne() for Single Scalars

```php
// BAD - fetches unnecessary data
$result = $adb->pquery("SELECT COUNT(*) as count FROM vtiger_account", []);
$row = $adb->fetchByAssoc($result);
$count = $row['count'];

// GOOD - direct scalar retrieval
$count = $adb->getOne("SELECT COUNT(*) FROM vtiger_account", []);
```

## Batch Processing Patterns

### LIMIT + OFFSET Pattern (Simple)

```php
$limit = 100;
$offset = 0;

do {
    $sql = "SELECT crmid FROM vtiger_crmentity
            WHERE setype = ? AND deleted = 0
            LIMIT ? OFFSET ?";
    $result = $adb->pquery($sql, ['Accounts', $limit, $offset]);

    while ($row = $adb->fetchByAssoc($result)) {
        // Process record
        processRecord($row['crmid']);
    }

    $offset += $limit;
} while ($adb->num_rows($result) > 0);
```

### Keyset Pagination (Faster for Large Offsets)

```php
$limit = 100;
$lastId = 0;

do {
    $sql = "SELECT crmid FROM vtiger_crmentity
            WHERE setype = ? AND deleted = 0 AND crmid > ?
            ORDER BY crmid
            LIMIT ?";
    $result = $adb->pquery($sql, ['Accounts', $lastId, $limit]);

    $processedCount = 0;
    while ($row = $adb->fetchByAssoc($result)) {
        processRecord($row['crmid']);
        $lastId = $row['crmid'];
        $processedCount++;
    }
} while ($processedCount >= $limit);
```

### Batch INSERT Pattern

```php
global $adb;

$records = [
    ['name' => 'Account 1', 'website' => 'example1.com'],
    ['name' => 'Account 2', 'website' => 'example2.com'],
    // ... more records
];

$adb->startTransaction();

try {
    $sql = "INSERT INTO vtiger_account (accountid, accountname, website) VALUES (?, ?, ?)";

    foreach ($records as $record) {
        $id = generateRecordId();
        $adb->pquery($sql, [$id, $record['name'], $record['website']]);
    }

    $adb->completeTransaction();
}
catch (Exception $e) {
    $adb->rollbackTransaction();
    error_log('Batch insert failed: ' . $e->getMessage());
}
```

## Caching Strategies

### Redis Caching

```php
// Use RedisUtils for caching
require_once 'include/utils/RedisUtils.php';

$cacheKey = 'accounts:count:' . $userId;
$ttl = 3600; // 1 hour

// Try cache first
$count = RedisUtils::get($cacheKey);

if ($count === false) {
    // Cache miss - query database
    global $adb;
    $sql = "SELECT COUNT(*) FROM vtiger_account a
            INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
            WHERE e.deleted = 0 AND e.smownerid = ?";
    $count = $adb->getOne($sql, [$userId]);

    // Store in cache
    RedisUtils::setex($cacheKey, $ttl, $count);
}

return $count;
```

### Vtiger_Cache (Per-Request)

```php
// Per-request caching (not persistent)
$cacheKey = 'module_fields_' . $moduleName;

// Check cache
$fields = Vtiger_Cache::get('moduleFields', $cacheKey);

if ($fields === false) {
    // Cache miss - load from database
    $fields = getFieldsForModule($moduleName);

    // Store in cache
    Vtiger_Cache::set('moduleFields', $cacheKey, $fields);
}

return $fields;
```

### Cache Invalidation

```php
// Clear cache on data change
public function save() {
    parent::save();

    // Invalidate cache
    $cacheKey = 'accounts:count:' . $this->get('assigned_user_id');
    RedisUtils::del($cacheKey);
}
```

## Debugging & Monitoring

### Enable SQL Logging

```php
// In config file or debug mode
$GLOBALS['sql_logging'] = true;

// All queries logged to logs/sqltime.log
```

### MySQL Slow Query Log

```sql
-- Enable slow query log
SET GLOBAL slow_query_log = 'ON';
SET GLOBAL long_query_time = 2; -- Log queries > 2 seconds
SET GLOBAL slow_query_log_file = '/var/log/mysql/slow.log';

-- Check slow queries
SELECT * FROM mysql.slow_log ORDER BY query_time DESC LIMIT 10;
```

### Profile Query Execution

```sql
-- Enable profiling
SET profiling = 1;

-- Run query
SELECT * FROM vtiger_account WHERE accountname LIKE 'Test%';

-- Show profiles
SHOW PROFILES;

-- Show detailed profile
SHOW PROFILE FOR QUERY 1;
```

### Check Table Statistics

```sql
-- Analyze table for optimizer
ANALYZE TABLE vtiger_account;

-- Show table status
SHOW TABLE STATUS LIKE 'vtiger_account';

-- Show index cardinality
SHOW INDEX FROM vtiger_account;
```

## Real-World Optimization Examples

### Before: Full Table Scan

```php
$sql = "SELECT COUNT(*) FROM vtiger_campaign
        WHERE campaignname LIKE '%promo%'";
// EXPLAIN: type=ALL, rows=10000
```

### After: Indexed Prefix Search

```php
$sql = "SELECT COUNT(*) FROM vtiger_campaign
        WHERE campaignname LIKE 'promo%'"; // Prefix search uses index
// Add: ALTER TABLE vtiger_campaign ADD INDEX idx_campaignname (campaignname(50));
// EXPLAIN: type=range, rows=50
```

### Before: Correlated Subquery

```php
$sql = "SELECT a.accountid,
        (SELECT COUNT(*) FROM vtiger_contactdetails c
         WHERE c.accountid = a.accountid) as contact_count
        FROM vtiger_account a";
// Executes subquery for EACH row
```

### After: LEFT JOIN with COUNT

```php
$sql = "SELECT a.accountid, COUNT(c.contactid) as contact_count
        FROM vtiger_account a
        LEFT JOIN vtiger_contactdetails c ON c.accountid = a.accountid
        LEFT JOIN vtiger_crmentity ec ON ec.crmid = c.contactid AND ec.deleted = 0
        INNER JOIN vtiger_crmentity ea ON ea.crmid = a.accountid
        WHERE ea.deleted = 0
        GROUP BY a.accountid";
// Single query with aggregation
```

## Quick Checklist

- [ ] Use EXPLAIN to analyze slow queries
- [ ] Index foreign keys and WHERE columns
- [ ] Avoid SELECT *, specify needed columns only
- [ ] Always filter deleted = 0 on vtiger_crmentity
- [ ] Use LIMIT for pagination
- [ ] Prefer EXISTS over IN for large subqueries
- [ ] Avoid functions on indexed columns in WHERE
- [ ] Use generateQuestionMarks() for dynamic IN clauses
- [ ] Use getOne() for single scalar values
- [ ] Batch process with keyset pagination for large datasets
- [ ] Cache frequently accessed data with Redis
- [ ] Enable slow query log in production
- [ ] Run ANALYZE TABLE periodically

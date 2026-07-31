# DateTime Handling Reference

> Convert between database and user formats, handle timezones

## Format Standards

| Context | Format | Example |
|---------|--------|---------|
| Database | `Y-m-d H:i:s` | `2025-02-11 14:30:00` |
| Database (Date only) | `Y-m-d` | `2025-02-11` |
| User | User preference | `02-11-2025 2:30 PM` |
| Display | Depends on locale | `11/02/2025 14:30` |

## Current Date/Time

```php
// Current datetime
$now = date('Y-m-d H:i:s');
// Output: 2025-02-11 14:30:00

// Current date
$today = date('Y-m-d');
// Output: 2025-02-11

// Custom format
$formatted = date('Y-m-d\TH:i:s\Z'); // ISO 8601
// Output: 2025-02-11T14:30:00Z
```

## Database → User Format

### Using DateTimeField

```php
// Convert DB datetime to user format
$dbDateTime = '2025-02-11 14:30:00';
$userDateTime = DateTimeField::convertToUserFormat($dbDateTime);
// Output: 02-11-2025 (format depends on user preference)

// Convert DB date to user format
$dbDate = '2025-02-11';
$userDate = DateTimeField::convertToUserFormat($dbDate);
// Output: 02-11-2025
```

### With Timezone Conversion

```php
$currentUser = Users_Record_Model::getCurrentUserModel();

// Convert DB datetime to user timezone
$dbDateTime = '2025-02-11 14:30:00';
$userDateTime = DateTimeField::convertToUserTimeZone($dbDateTime, $currentUser);
// If user timezone is GMT+7, output: 2025-02-11 21:30:00
```

## User Format → Database

### Using DateTimeField

```php
// Convert user datetime to DB format
$userDateTime = '02-11-2025 2:30 PM';
$dbDateTime = DateTimeField::convertToDBFormat($userDateTime);
// Output: 2025-02-11 14:30:00

// Convert user date to DB format
$userDate = '02-11-2025';
$dbDate = DateTimeField::convertToDBFormat($userDate);
// Output: 2025-02-11
```

### With Timezone Conversion

```php
$currentUser = Users_Record_Model::getCurrentUserModel();

// Convert user timezone to DB timezone (UTC)
$userDateTime = '2025-02-11 21:30:00';
$dbDateTime = DateTimeField::convertToDBTimeZone($userDateTime, $currentUser);
// If user timezone is GMT+7, output: 2025-02-11 14:30:00
```

## Timezone Handling

### Get User Timezone

```php
$currentUser = Users_Record_Model::getCurrentUserModel();
$userTimeZone = $currentUser->get('time_zone');
// Output: Asia/Ho_Chi_Minh

// Set timezone
date_default_timezone_set($userTimeZone);
```

### Convert Between Timezones

```php
$dateTime = new DateTime('2025-02-11 14:30:00', new DateTimeZone('UTC'));
$dateTime->setTimezone(new DateTimeZone('Asia/Ho_Chi_Minh'));
$converted = $dateTime->format('Y-m-d H:i:s');
// Output: 2025-02-11 21:30:00
```

## Date Calculations

### Using strtotime

```php
// Add days
$tomorrow = date('Y-m-d', strtotime('+1 day'));
$nextWeek = date('Y-m-d', strtotime('+7 days'));
$nextMonth = date('Y-m-d', strtotime('+1 month'));

// Subtract days
$yesterday = date('Y-m-d', strtotime('-1 day'));
$lastWeek = date('Y-m-d', strtotime('-7 days'));

// Add hours/minutes
$inTwoHours = date('Y-m-d H:i:s', strtotime('+2 hours'));
$in30Minutes = date('Y-m-d H:i:s', strtotime('+30 minutes'));

// Combine date with calculation
$expiresIn = 3600; // seconds
$expiredDate = date('Y-m-d H:i:s', strtotime("+ {$expiresIn} seconds"));
```

### Using DateTime Class

```php
$date = new DateTime('2025-02-11 14:30:00');

// Add interval
$date->add(new DateInterval('P7D')); // Add 7 days
$date->add(new DateInterval('PT2H')); // Add 2 hours

// Subtract interval
$date->sub(new DateInterval('P1M')); // Subtract 1 month

// Format result
$result = $date->format('Y-m-d H:i:s');
```

### Date Difference

```php
$date1 = new DateTime('2025-02-11');
$date2 = new DateTime('2025-03-15');
$diff = $date1->diff($date2);

echo $diff->days; // Total days: 32
echo $diff->m;    // Months: 1
echo $diff->d;    // Days (in month): 4
```

## SQL Date Operations

### Current Date/Time

```sql
-- Current datetime
SELECT CURDATE(), CURTIME(), NOW();

-- Use in queries
UPDATE vtiger_cpadvertisingaccount
SET last_sync_datetime = NOW()
WHERE cpadvertisingaccountid = ?
```

### Date Arithmetic

```sql
-- Add/subtract intervals
SELECT DATE_ADD(closingdate, INTERVAL 7 DAY) as week_later
FROM vtiger_campaign;

SELECT DATE_SUB(createdtime, INTERVAL 1 MONTH) as month_ago
FROM vtiger_crmentity;

-- Date functions
SELECT YEAR(createdtime), MONTH(createdtime), DAY(createdtime)
FROM vtiger_crmentity;

-- Extract parts
SELECT EXTRACT(YEAR FROM createdtime) as year
FROM vtiger_crmentity;
```

### Date Comparisons

```sql
-- Between dates
WHERE createdtime BETWEEN '2025-01-01' AND '2025-12-31'

-- Greater than/less than
WHERE closingdate > NOW()
WHERE createdtime < DATE_SUB(NOW(), INTERVAL 30 DAY)

-- Null or empty check
WHERE closingdate IS NOT NULL
AND closingdate != '0000-00-00 00:00:00'
```

### Date Formatting in SQL

```sql
-- Format date
SELECT DATE_FORMAT(createdtime, '%Y-%m-%d') as formatted_date
FROM vtiger_crmentity;

-- Format patterns
-- %Y = 4-digit year
-- %m = 2-digit month
-- %d = 2-digit day
-- %H = 2-digit hour (24h)
-- %i = 2-digit minute
-- %s = 2-digit second
```

## Smarty Template Formatting

```smarty
{* Format datetime *}
{$RECORD->get('createdtime')|date_format:"%Y-%m-%d %H:%M:%S"}

{* Format with user format *}
{$RECORD->getDisplayValue('createdtime')}

{* Custom format *}
{$START_DATE|date_format:"%d/%m/%Y"}
```

## Common Patterns

### Check if Date is Valid

```php
// Check for empty or zero date
$isEmpty = in_array($date, ['', '0000-00-00', '0000-00-00 00:00:00']);

// Check if date is in past
if (strtotime($date) < time()) {
    // Date is in the past
}

// Check if date is within threshold
$daysThreshold = 2;
if (strtotime($expiredDate) < strtotime("+{$daysThreshold} days")) {
    // Date is within 2 days
}
```

### Token Expiration Pattern

```php
// From connector token renewal
$expiredDate = (string) $record->get('token_expired_date');
$daysThreshold = 2;

if (empty($expiredDate) || strtotime($expiredDate) < strtotime("+{$daysThreshold} days")) {
    // Renew token
    $expiresIn = (int) $tokenData['expires_in']; // Seconds
    $newExpiredDate = date('Y-m-d H:i:s', strtotime("+ {$expiresIn} seconds"));

    $record->set('token_expired_date', $newExpiredDate);
    $record->save();
}
```

### Date Range Query

```php
$startDate = date('Y-m-d', strtotime('-30 days'));
$endDate = date('Y-m-d');

$sql = "SELECT COUNT(*) FROM vtiger_campaign c
        INNER JOIN vtiger_crmentity e ON e.crmid = c.campaignid
        WHERE e.deleted = 0
        AND e.createdtime BETWEEN ? AND ?";
$count = $adb->getOne($sql, [$startDate . ' 00:00:00', $endDate . ' 23:59:59']);
```

## Best Practices

1. **Always store in UTC**: Store all dates in database as UTC, convert on display
2. **Use Y-m-d H:i:s format**: VTiger standard format for consistency
3. **Handle null/empty**: Check for `0000-00-00 00:00:00` before processing
4. **User timezone aware**: Always convert to user timezone for display
5. **Avoid direct comparison**: Use `strtotime()` or `DateTime` for comparisons
6. **SQL date functions**: Prefer SQL functions for DB-side calculations

# Database Operations Reference

> Core patterns for VTiger database operations using PearDatabase

## Database Instance

```php
// ALWAYS use this pattern
global $adb;

// NEVER use (causes issues in some contexts)
$adb = PearDatabase::getInstance();
```

## Basic Query Operations

### SELECT with pquery (Parameterized)

```php
global $adb;

// Single param
$sql = "SELECT * FROM vtiger_account WHERE accountid = ?";
$result = $adb->pquery($sql, [$accountId]);

// Multiple params
$sql = "SELECT * FROM vtiger_contactdetails WHERE firstname = ? AND lastname = ?";
$result = $adb->pquery($sql, [$firstName, $lastName]);

// IMPORTANT: Always type-cast external input
$recordId = (int) $request->get('record');
$moduleName = (string) $request->getModule();
```

### Fetch Methods

**Single Scalar Value (getOne)**

```php
$sql = "SELECT COUNT(*) FROM vtiger_crmentity WHERE setype = ? AND deleted = 0";
$count = $adb->getOne($sql, ['Accounts']);
```

**Single Row (fetchByAssoc + decodeUTF8)**

```php
$sql = "SELECT * FROM vtiger_account WHERE accountid = ?";
$result = $adb->pquery($sql, [$accountId]);
$row = $adb->fetchByAssoc($result);

// ALWAYS decode UTF-8 for non-ASCII characters
$row = decodeUTF8($row);

// Access columns
$accountName = $row['accountname'];
```

**Multiple Rows (Loop fetchByAssoc)**

```php
$sql = "SELECT accountid, accountname FROM vtiger_account
        INNER JOIN vtiger_crmentity ON crmid = accountid
        WHERE deleted = 0 LIMIT 100";
$result = $adb->pquery($sql, []);

$accounts = [];
while ($row = $adb->fetchByAssoc($result)) {
    $row = decodeUTF8($row);
    $accounts[] = $row;
}
```

**Row Count**

```php
$numRows = $adb->num_rows($result);

if ($numRows > 0) {
    // Process results
}
```

### INSERT Operations

**Using sql_insert_data Helper**

```php
global $adb;

$data = [
    'accountid' => $recordId,
    'accountname' => $accountName,
    'website' => $website,
];

$sql = $adb->sql_insert_data('vtiger_account', $data);
// Generates: INSERT INTO vtiger_account (accountid, accountname, website) VALUES (?, ?, ?)

$result = $adb->pquery($sql, array_values($data));
$lastId = $adb->getLastInsertID();
```

**Manual INSERT**

```php
$sql = "INSERT INTO vtiger_cpadvertisingaccount (account_id, account_name, status)
        VALUES (?, ?, ?)";
$adb->pquery($sql, [$accountId, $accountName, 'Active']);
```

### UPDATE Operations

```php
$sql = "UPDATE vtiger_cpadvertisingaccount
        SET last_sync_datetime = ?, status = ?
        WHERE cpadvertisingaccountid = ?";
$adb->pquery($sql, [date('Y-m-d H:i:s'), 'Active', $recordId]);

// Check affected rows
$affected = $adb->getAffectedRowCount($result);
```

### DELETE Operations

```php
// Hard delete (rarely used)
$sql = "DELETE FROM vtiger_customtable WHERE id = ?";
$adb->pquery($sql, [$id]);

// Soft delete (preferred for vtiger_crmentity records)
$sql = "UPDATE vtiger_crmentity SET deleted = 1 WHERE crmid = ?";
$adb->pquery($sql, [$recordId]);
```

### Dynamic IN Clause

```php
// For variable-length IN clauses
$ids = [1, 2, 3, 4, 5];
$marks = generateQuestionMarks($ids);

$sql = "SELECT * FROM vtiger_crmentity WHERE crmid IN ($marks) AND deleted = 0";
$result = $adb->pquery($sql, $ids);
```

## Transactions

```php
global $adb;

$adb->startTransaction();

try {
    $sql1 = "INSERT INTO vtiger_account (accountid, accountname) VALUES (?, ?)";
    $adb->pquery($sql1, [$id, $name]);

    $sql2 = "UPDATE vtiger_crmentity SET modifiedtime = ? WHERE crmid = ?";
    $adb->pquery($sql2, [date('Y-m-d H:i:s'), $id]);

    $adb->completeTransaction();
}
catch (Exception $e) {
    $adb->rollbackTransaction();
    error_log('Transaction failed: ' . $e->getMessage());
    throw $e;
}
```

## Real Codebase Examples

### Simple JOIN with Aggregate

```php
// From Accounts_Data_Helper::getSalesByAccount
public static function getSalesByAccount($accountId) {
    global $adb;

    $sql = "SELECT SUM(vtiger_invoice.total) as total
            FROM vtiger_invoice
            INNER JOIN vtiger_crmentity ON vtiger_crmentity.crmid = vtiger_invoice.invoiceid
            WHERE vtiger_crmentity.deleted = 0
            AND vtiger_invoice.accountid = ?";

    return $adb->getOne($sql, [$accountId]);
}
```

### GROUP_CONCAT for Batch IDs

```php
$sql = "SELECT GROUP_CONCAT(campaignid) as campaign_ids
        FROM vtiger_campaign
        INNER JOIN vtiger_crmentity ON crmid = campaignid
        WHERE deleted = 0 AND social_campaign_id IS NOT NULL";
$result = $adb->pquery($sql, []);
$row = $adb->fetchByAssoc($result);
$campaignIds = explode(',', $row['campaign_ids']);
```

### Complex Correlated Subquery

```php
// From HelpDesk modtracker
$sql = "SELECT vtiger_modtracker_basic.*
        FROM vtiger_modtracker_basic
        WHERE crmid = ? AND module = ?
        AND id = (
            SELECT MAX(id)
            FROM vtiger_modtracker_basic
            WHERE crmid = ? AND changedon <= ?
        )";
$result = $adb->pquery($sql, [$recordId, $module, $recordId, $timestamp]);
```

### UPDATE with COALESCE

```php
$sql = "UPDATE vtiger_campaign
        SET closingdate = COALESCE(?, closingdate),
            campaignstatus = ?
        WHERE campaignid = ?";
$adb->pquery($sql, [$endDate, $status, $campaignId]);
```

### UPDATE with JOIN

```php
$sql = "UPDATE vtiger_campaign c
        INNER JOIN vtiger_crmentity e ON e.crmid = c.campaignid
        SET c.campaignstatus = ?, e.modifiedtime = ?
        WHERE c.social_campaign_id = ? AND e.deleted = 0";
$adb->pquery($sql, ['Completed', date('Y-m-d H:i:s'), $socialCampaignId]);
```

### Soft Delete with JOIN

```php
$sql = "UPDATE vtiger_crmentity e
        INNER JOIN vtiger_campaign c ON c.campaignid = e.crmid
        SET e.deleted = 1
        WHERE c.social_campaign_id = ?";
$adb->pquery($sql, [$socialCampaignId]);
```

### fetchByAssoc + decodeUTF8 Pattern

```php
$sql = "SELECT * FROM vtiger_contactdetails WHERE contactid = ?";
$result = $adb->pquery($sql, [$contactId]);

if ($adb->num_rows($result) > 0) {
    $contact = $adb->fetchByAssoc($result);
    $contact = decodeUTF8($contact); // CRITICAL for UTF-8

    // Now safe to use Vietnamese, Chinese, etc.
    $fullName = $contact['firstname'] . ' ' . $contact['lastname'];
}
```

### Loop with UTF-8 Decode

```php
$accounts = [];
while ($row = $adb->fetchByAssoc($result)) {
    $row = decodeUTF8($row);
    $accounts[] = [
        'id' => $row['accountid'],
        'name' => $row['accountname'], // Preserves Vietnamese characters
    ];
}
```

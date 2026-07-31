# Database Structure Reference

> VTiger table architecture, naming conventions, and module structure

## Central Registry: vtiger_crmentity

**Core table for ALL CRM records**

```sql
CREATE TABLE vtiger_crmentity (
    crmid INT PRIMARY KEY,
    smcreatorid INT,
    smownerid INT,
    modifiedby INT,
    setype VARCHAR(30),      -- Module name
    description TEXT,
    createdtime DATETIME,
    modifiedtime DATETIME,
    viewedtime DATETIME,
    status VARCHAR(50),
    version INT,
    presence INT,
    deleted TINYINT,         -- Soft delete flag
    label VARCHAR(255)       -- Display label
);
```

**Golden Rule: ALWAYS filter deleted = 0**

```php
// CORRECT
$sql = "SELECT * FROM vtiger_account a
        INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
        WHERE e.deleted = 0 AND a.accountid = ?";

// WRONG - returns deleted records
$sql = "SELECT * FROM vtiger_account WHERE accountid = ?";
```

## Module Table Structure

Each module has multiple related tables:

| Property | Example (Contacts) | Purpose |
|----------|-------------------|---------|
| `$table_name` | `vtiger_contactdetails` | Main module data |
| `$table_index` | `contactid` | Primary key |
| `$tab_name` | `vtiger_contactdetails` | Usually same as `$table_name` |
| `$tab_name_index` | `contactid` | Usually same as `$table_index` |
| `$customFieldTable` | `vtiger_contactscf` | Custom fields (suffix: `cf`) |

## Standard Table Naming Patterns

### Core Pattern

```
vtiger_{module}           # Main data table
vtiger_{module}cf         # Custom fields
vtiger_{module}address    # Address fields (if applicable)
vtiger_{module}details    # Additional details (legacy)
```

### Example: Contacts Module

```sql
-- Main contact info
vtiger_contactdetails (contactid, firstname, lastname, email, phone, ...)

-- Additional contact data
vtiger_contactaddress (contactaddressid, mailingcity, mailingstreet, ...)
vtiger_contactsubdetails (contactsubscriptionid, homephone, otherphone, ...)

-- Custom fields
vtiger_contactscf (contactid, cf_custom1, cf_custom2, ...)

-- Customer relationship
vtiger_customerdetails (customerid, portal, support_start_date, ...)
```

### Example: HelpDesk Module

```sql
-- Main ticket data
vtiger_troubletickets (ticketid, title, description, priority, status, ...)

-- Custom fields
vtiger_ticketcf (ticketid, cf_resolution_notes, cf_escalation_level, ...)
```

### Example: Accounts Module

```sql
-- Main account data
vtiger_account (accountid, accountname, account_no, website, tickersymbol, ...)

-- Address info
vtiger_accountbillads (accountaddressid, bill_street, bill_city, bill_state, ...)
vtiger_accountshipads (accountaddressid, ship_street, ship_city, ship_state, ...)

-- Custom fields
vtiger_accountscf (accountid, cf_industry_code, cf_tax_id, ...)
```

## How to Find Module Tables

**Method 1: Open Module Entity File**

```php
// File: modules/Contacts/Contacts.php
class Contacts extends CRMEntity {
    public $table_name = 'vtiger_contactdetails';
    public $table_index = 'contactid';
    public $tab_name = Array('vtiger_crmentity', 'vtiger_contactdetails', 'vtiger_contactscf');
    public $tab_name_index = Array('crmid', 'contactid', 'contactid');
    public $customFieldTable = Array('vtiger_contactscf', 'contactid');
    // ...
}
```

**Method 2: Query vtiger_field**

```php
global $adb;

$sql = "SELECT tablename, columnname, fieldname
        FROM vtiger_field
        WHERE tabid = (SELECT tabid FROM vtiger_tab WHERE name = ?)";
$result = $adb->pquery($sql, ['Contacts']);

while ($row = $adb->fetchByAssoc($result)) {
    echo "{$row['tablename']}.{$row['columnname']} -> {$row['fieldname']}\n";
}
```

## Standard Query Pattern

**Always JOIN vtiger_crmentity**

```php
// Single module query
$sql = "SELECT a.accountid, a.accountname, e.createdtime, e.modifiedtime
        FROM vtiger_account a
        INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
        WHERE e.deleted = 0 AND e.setype = 'Accounts'";

// With custom fields
$sql = "SELECT a.accountid, a.accountname, acf.cf_tax_id
        FROM vtiger_account a
        INNER JOIN vtiger_crmentity e ON e.crmid = a.accountid
        LEFT JOIN vtiger_accountscf acf ON acf.accountid = a.accountid
        WHERE e.deleted = 0";

// Multiple modules (Contacts + Accounts)
$sql = "SELECT c.contactid, c.firstname, c.lastname,
               a.accountid, a.accountname
        FROM vtiger_contactdetails c
        INNER JOIN vtiger_crmentity ec ON ec.crmid = c.contactid
        LEFT JOIN vtiger_account a ON a.accountid = c.accountid
        LEFT JOIN vtiger_crmentity ea ON ea.crmid = a.accountid
        WHERE ec.deleted = 0
        AND (a.accountid IS NULL OR ea.deleted = 0)";
```

## Custom Module Pattern (CP* modules)

```sql
-- Main table
vtiger_cpadvertisingaccount (
    cpadvertisingaccountid INT PRIMARY KEY,
    account_id VARCHAR(255),
    account_name VARCHAR(255),
    status VARCHAR(50),
    last_sync_datetime DATETIME
)

-- Custom fields
vtiger_cpadvertisingaccountcf (
    cpadvertisingaccountid INT,
    cf_platform VARCHAR(100),
    cf_budget DECIMAL(10,2)
)

-- Always joined with crmentity
SELECT cpa.*, e.createdtime, e.smownerid
FROM vtiger_cpadvertisingaccount cpa
INNER JOIN vtiger_crmentity e ON e.crmid = cpa.cpadvertisingaccountid
WHERE e.deleted = 0 AND e.setype = 'CPAdvertisingAccount'
```

## Common Lookup Tables

```sql
-- Users
vtiger_users (id, user_name, first_name, last_name, email1, status)

-- Groups
vtiger_groups (groupid, groupname)

-- Picklists
vtiger_accounttype (accounttype, accounttypeid)  -- Account Type picklist
vtiger_ticketstatus (ticketstatus)               -- Ticket Status picklist

-- Module metadata
vtiger_tab (tabid, name, presence, tabsequence)
vtiger_field (fieldid, tabid, fieldname, columnname, tablename, uitype)
```

## Performance Notes

1. **Always index foreign keys** pointing to crmid
2. **Add indexes** on frequently filtered columns (status, date fields)
3. **Avoid SELECT *** from modules with many fields
4. **Use LEFT JOIN** for custom fields (may not exist for all records)
5. **Filter by setype** when querying vtiger_crmentity directly

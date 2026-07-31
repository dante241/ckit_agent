# Relationships Reference

> VTiger module relationships: 1:1, 1:N, N:N patterns

## Relationship Types

### 1:1 (One-to-One)

Foreign key in one table pointing to another.

```sql
-- Contacts → Accounts (one contact belongs to one account)
vtiger_contactdetails.accountid → vtiger_account.accountid

-- Example query
SELECT c.*, a.accountname
FROM vtiger_contactdetails c
LEFT JOIN vtiger_account a ON a.accountid = c.accountid
LEFT JOIN vtiger_crmentity ea ON ea.crmid = a.accountid AND ea.deleted = 0
INNER JOIN vtiger_crmentity ec ON ec.crmid = c.contactid
WHERE ec.deleted = 0
```

### 1:N (One-to-Many)

Multiple records in child table point to single parent record.

```sql
-- Accounts → Contacts (one account has many contacts)
vtiger_account.accountid ← vtiger_contactdetails.accountid

-- Get all contacts for an account
SELECT c.contactid, c.firstname, c.lastname
FROM vtiger_contactdetails c
INNER JOIN vtiger_crmentity e ON e.crmid = c.contactid
WHERE c.accountid = ? AND e.deleted = 0
```

### N:N (Many-to-Many)

Junction table connects two modules.

```sql
-- Products ↔ PriceBooks (many-to-many via vtiger_pricebookproductrel)
vtiger_products.productid ↔ vtiger_pricebookproductrel ↔ vtiger_pricebook.pricebookid

-- Junction table structure
CREATE TABLE vtiger_pricebookproductrel (
    pricebookid INT,
    productid INT,
    listprice DECIMAL(28,8),
    usedcurrency INT,
    PRIMARY KEY (pricebookid, productid)
);

-- Get all products in a pricebook
SELECT p.productid, p.productname, pbr.listprice
FROM vtiger_products p
INNER JOIN vtiger_pricebookproductrel pbr ON pbr.productid = p.productid
INNER JOIN vtiger_crmentity e ON e.crmid = p.productid
WHERE pbr.pricebookid = ? AND e.deleted = 0
```

## Create Relationships via UI

**Admin → Module Manager → Select Module → Relationships → Add Relationship**

- Select related module
- Choose relationship type (1:N or N:N)
- Define label for both directions
- System creates junction table automatically (for N:N)

## Create Relationships via Code

### Define in Extensions.php

```php
// File: modules/CPAdvertisingAccount/Extensions.php
<?php

return [
    'relationships' => [
        // 1:N relationship (CPAdvertisingAccount → Campaigns)
        [
            'relatedModule' => 'Campaigns',
            'label' => 'Campaigns',
            'relationshipType' => '1:N',
        ],

        // N:N relationship (CPAdvertisingAccount ↔ Contacts)
        [
            'relatedModule' => 'Contacts',
            'label' => 'Related Contacts',
            'relationshipType' => 'N:N',
        ],
    ],
];
```

### RelatedList Function in Entity

```php
// File: modules/Accounts/Accounts.php
class Accounts extends CRMEntity {

    public function get_contacts($id, $cur_tab_id, $rel_tab_id, $actions = false) {
        global $adb;

        $query = "SELECT vtiger_contactdetails.contactid, vtiger_contactdetails.firstname,
                         vtiger_contactdetails.lastname, vtiger_contactdetails.email
                  FROM vtiger_contactdetails
                  INNER JOIN vtiger_crmentity ON vtiger_crmentity.crmid = vtiger_contactdetails.contactid
                  WHERE vtiger_crmentity.deleted = 0
                  AND vtiger_contactdetails.accountid = ?";

        $result = $adb->pquery($query, [$id]);

        $returnData = [];
        while ($row = $adb->fetchByAssoc($result)) {
            $row = decodeUTF8($row);
            $returnData[] = $row;
        }

        return $returnData;
    }
}
```

## Register in vtiger_relatedlists

Relationships are stored in `vtiger_relatedlists` table:

```sql
INSERT INTO vtiger_relatedlists
(tabid, related_tabid, name, sequence, label, presence, actions)
VALUES
(?, ?, ?, ?, ?, 0, 'ADD,SELECT');
```

**Parameters:**
- `tabid`: Parent module ID
- `related_tabid`: Related module ID
- `name`: Function name in Entity (e.g., `get_contacts`)
- `sequence`: Display order
- `label`: Display label
- `presence`: 0=visible, 1=hidden
- `actions`: Comma-separated (ADD, SELECT, EDIT, DELETE)

## Access via Model

### Get Relation Model

```php
// Get relationship between two modules
$relationModel = Vtiger_Relation_Model::getInstance(
    Vtiger_Module_Model::getInstance('Accounts'),
    Vtiger_Module_Model::getInstance('Contacts')
);

if ($relationModel) {
    $relationId = $relationModel->getId();
    $label = $relationModel->get('label');
}
```

### Link Records (N:N)

```php
$sourceRecordModel = Vtiger_Record_Model::getInstanceById($accountId, 'Accounts');
$relatedRecordModel = Vtiger_Record_Model::getInstanceById($contactId, 'Contacts');

$relationModel = Vtiger_Relation_Model::getInstance(
    $sourceRecordModel->getModule(),
    $relatedRecordModel->getModule()
);

// Link records
$relationModel->addRelation($sourceRecordModel->getId(), $relatedRecordModel->getId());
```

### Unlink Records (N:N)

```php
// Unlink records
$relationModel->removeRelation($sourceRecordModel->getId(), $relatedRecordModel->getId());
```

### Get Related Records

```php
$recordModel = Vtiger_Record_Model::getInstanceById($accountId, 'Accounts');
$relationListView = Vtiger_RelationListView_Model::getInstance($recordModel, 'Contacts');

// Get paginated results
$pagingModel = new Vtiger_Paging_Model();
$pagingModel->set('page', 1);
$pagingModel->set('limit', 20);

$relatedRecords = $relationListView->getEntries($pagingModel);
```

## Migration Example

```php
// File: modules/CPMigration/migrations/2025.02.11.14.30.00_AddCampaignRelationship.php
<?php

return new class extends CPMigration_Base_Model {

    public function up(): int {
        // Get module IDs
        $adsAccountTabId = $this->getTabId('CPAdvertisingAccount');
        $campaignTabId = $this->getTabId('Campaigns');

        if (!$adsAccountTabId || !$campaignTabId) {
            return self::UP_FAILED;
        }

        // Check if relationship exists
        $sql = "SELECT 1 FROM vtiger_relatedlists
                WHERE tabid = ? AND related_tabid = ?";
        $result = $this->pquery($sql, [$adsAccountTabId, $campaignTabId]);

        if ($this->num_rows($result) > 0) {
            return self::UP_SUCCESS; // Already exists
        }

        // Get next sequence
        $sql = "SELECT MAX(sequence) as max_seq FROM vtiger_relatedlists WHERE tabid = ?";
        $maxSeq = (int) $this->getOne($sql, [$adsAccountTabId]);

        // Insert relationship
        $sql = "INSERT INTO vtiger_relatedlists
                (tabid, related_tabid, name, sequence, label, presence, actions)
                VALUES (?, ?, ?, ?, ?, 0, 'ADD,SELECT')";

        $this->pquery($sql, [
            $adsAccountTabId,
            $campaignTabId,
            'get_related_list', // Generic function name
            $maxSeq + 1,
            'Campaigns'
        ]);

        return self::UP_SUCCESS;
    }

    public function down(): int {
        return self::DOWN_NOT_SUPPORTED;
    }
};
```

## Quick Repair After Changes

After adding/modifying relationships programmatically:

1. Go to **Settings → Module Manager → {Module}**
2. Click **Quick Repair** button
3. System rebuilds relationship cache

Or via code:

```php
$moduleModel = Vtiger_Module_Model::getInstance('Accounts');
$moduleModel->updateCache();
```

## Common Patterns

### Campaign → Contacts (N:N via vtiger_campaigncontrel)

```sql
SELECT c.contactid, c.firstname, c.lastname, ccr.campaignrelstatusid
FROM vtiger_contactdetails c
INNER JOIN vtiger_campaigncontrel ccr ON ccr.contactid = c.contactid
INNER JOIN vtiger_crmentity e ON e.crmid = c.contactid
WHERE ccr.campaignid = ? AND e.deleted = 0
```

### Potentials → Contacts (1:N)

```sql
SELECT p.potentialid, p.potentialname, p.amount
FROM vtiger_potential p
INNER JOIN vtiger_crmentity e ON e.crmid = p.potentialid
WHERE p.related_to = ? AND e.deleted = 0
```

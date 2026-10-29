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

### Declare in RelationshipsRegister.php

Relations are declared per module in `modules/<Module>/RelationshipsRegister.php` (`Extensions.php` is unrelated — it holds addLink scripts). QuickRepair calls `RelationshipUtils::repairRelationships()`, which reads every register and creates missing `vtiger_relatedlists` rows; Settings → Layout Editor → Relation writes the same file.

```php
// File: modules/CPAutomaticTaskList/RelationshipsRegister.php (real)
<?php

$relationships = array(
    // N:N (junction vtiger_crmentityrel)
    array(
        'leftSideModule' => 'CPAutomaticTaskList',
        'rightSideModule' => 'Documents',
        'relationshipType' => 'N:N',
        'relationshipName' => 'LBL_DOCUMENT_LIST',
        'enabledActions' => array(
            'ADD',
            'SELECT'
        ),
        'listingFunctionName' => 'get_related_list',
        'leftSideReferenceFieldName' => null,
        'rightSideReferenceFieldName' => null
    ),
    // 1:N (reference field on the right-side module)
    array(
        'leftSideModule' => 'CPAutomaticTaskList',
        'rightSideModule' => 'CPEventPRMessageLog',
        'relationshipType' => '1:N',
        'relationshipName' => 'LBL_EVENT_PR_MESSAGE_LOG_LIST',
        'enabledActions' => array(),
        'listingFunctionName' => 'get_dependents_list',
        'leftSideReferenceFieldName' => null,
        'rightSideReferenceFieldName' => 'related_cpautomatictasklist'
    ),
);
```

### RelatedList Function in Entity

Generic relations use `get_related_list` (N:N) / `get_dependents_list` (1:N). Legacy core modules keep custom listing functions such as `Accounts::get_contacts` (`modules/Accounts/Accounts.php:191`): they build the related-list SQL and return `GetRelatedList($this_module, $related_module, $other, $query, $button, $returnset)` — not a row array. Do not copy their `$id` concatenation into new code; new relations should use the generic functions.

## Register in vtiger_relatedlists

Relationships are stored in `vtiger_relatedlists` table:

```sql
-- relation_id is not AUTO_INCREMENT: take it from $adb->getUniqueID('vtiger_relatedlists')
INSERT INTO vtiger_relatedlists
(relation_id, tabid, related_tabid, name, sequence, label, presence, actions)
VALUES
(?, ?, ?, ?, ?, ?, 0, 'ADD,SELECT');
```

**Parameters:**
- `relation_id`: from `getUniqueID('vtiger_relatedlists')`
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
$relationModel->deleteRelation($sourceRecordModel->getId(), $relatedRecordModel->getId());
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

Prefer adding the entry to `RelationshipsRegister.php` and letting QuickRepair create it. When a migration must insert the row directly, use `getTabid()` and `$this->db` (`CPMigration_Base_Model` only wraps `pquery()`):

```php
// File: modules/CPMigration/migrations/2025.02.11.14.30.00_AddCampaignRelationship.php
<?php

return new class extends CPMigration_Base_Model {

    public function up(): int {
        $adsAccountTabId = (int) getTabid('CPAdvertisingAccount');
        $campaignTabId = (int) getTabid('Campaigns');

        if (!$adsAccountTabId || !$campaignTabId) {
            return self::UP_FAILED;
        }

        // Idempotent: skip when the relation already exists
        $sql = "SELECT 1 FROM vtiger_relatedlists WHERE tabid = ? AND related_tabid = ?";
        $result = $this->pquery($sql, [$adsAccountTabId, $campaignTabId]);

        if ($this->db->num_rows($result) > 0) {
            return self::UP_SUCCESS;
        }

        $sql = "SELECT MAX(sequence) FROM vtiger_relatedlists WHERE tabid = ?";
        $maxSeq = (int) $this->db->getOne($sql, [$adsAccountTabId]);

        $sql = "INSERT INTO vtiger_relatedlists (relation_id, tabid, related_tabid, name, sequence, label, presence, actions)
                VALUES (?, ?, ?, ?, ?, ?, 0, 'ADD,SELECT')";

        $this->pquery($sql, [
            $this->db->getUniqueID('vtiger_relatedlists'),
            $adsAccountTabId,
            $campaignTabId,
            'get_related_list',
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

After adding/modifying relationships, run **Quick Repair** (it rebuilds relations from every `RelationshipsRegister.php` via `RelationshipUtils::repairRelationships()`). There is no `updateCache()` API.

## Common Patterns

### Campaign → Contacts (N:N via vtiger_campaigncontrel)

```sql
SELECT c.contactid, c.firstname, c.lastname, ccr.campaignrelstatusid
FROM vtiger_contactdetails c
INNER JOIN vtiger_campaigncontrel ccr ON ccr.contactid = c.contactid
INNER JOIN vtiger_crmentity e ON e.crmid = c.contactid
WHERE ccr.campaignid = ? AND e.deleted = 0
```

### Potentials by Account (1:N via related_to)

```sql
SELECT p.potentialid, p.potentialname, p.amount
FROM vtiger_potential p
INNER JOIN vtiger_crmentity e ON e.crmid = p.potentialid
WHERE p.related_to = ? AND e.deleted = 0
```

# Inventory Inheritance Reference

Real structure of the core inventory modules (`SalesOrder`, `Invoice`, `Quotes`, `PurchaseOrder`). Read the files themselves before copying — the snippets below are trimmed.

## Entity Class

**File:** `modules/{Module}/{Module}.php` — extends `CRMEntity` (there is no `Inventory` entity base class).

```php
class SalesOrder extends CRMEntity {

	var $table_name = 'vtiger_salesorder';
	var $table_index = 'salesorderid';
	var $tab_name = Array('vtiger_crmentity', 'vtiger_salesorder', 'vtiger_sobillads', 'vtiger_soshipads', 'vtiger_salesordercf', 'vtiger_invoice_recurring_info', 'vtiger_inventoryproductrel');
	var $customFieldTable = Array('vtiger_salesordercf', 'salesorderid');

	// Set to false to save the record without touching its line items
	var $isLineItemUpdate = true;

	function save_module($module) {
		// Line items are skipped for SaveAjax, MassEditSave, ProcessDuplicates and DETAILVIEW ajax saves
		if ($_REQUEST['action'] != 'SaveAjax' && $this->isLineItemUpdate != false /* ... */) {
			saveInventoryProductDetails($this, 'SalesOrder');
		}

		// currency_id + conversion_rate are written on the main table
	}
}
```

## Module Model

**File:** `modules/{Module}/models/Module.php`

```php
class SalesOrder_Module_Model extends Inventory_Module_Model {
	// override only what differs from Inventory_Module_Model
}
```

Taxes and currency come from core utils — do not query the tables from the model:

```php
$productTaxes = getAllTaxes('available');                 // vtiger_inventorytaxinfo, deleted = 0
$shippingTaxes = getAllTaxes('available', 'sh');          // vtiger_shippingtaxinfo
$currencyInfo = getInventoryCurrencyInfo('SalesOrder', $recordId); // currency_id, conversion_rate, currency_name, ...
```

Base currency, if you really need raw SQL (put it in a Data helper): `SELECT * FROM vtiger_currency_info WHERE defaultid = '-11'` (`defaultid` is varchar; other currencies have `'0'`).

## Record Model

**File:** `modules/{Module}/models/Record.php`

```php
class SalesOrder_Record_Model extends Inventory_Record_Model {
	// override only what differs from Inventory_Record_Model
}
```

### Reading line items

```php
$recordModel = Inventory_Record_Model::getInstanceById($recordId, 'SalesOrder');
$products = $recordModel->getProducts(); // 1-based: $products[1], $products[2], ...

foreach ($products as $i => $product) {
	$productId = $product["hdnProductId{$i}"];
	$quantity = $product["qty{$i}"];
	$listPrice = $product["listPrice{$i}"];
}

$finalDetails = $products[1]['final_details'] ?? [];
// keys: hdnSubTotal, discount_type_final, discount_amount_final, discount_percentage_final, discountTotal_final,
// taxtype, taxes, tax_totalamount, shipping_handling_charge, sh_taxes, shtax_totalamount, adjustment, grandTotal
```

`getProducts()` (over `getAssociatedProducts()` in `include/utils/EditViewUtils.php`) already applies individual/group tax, compound taxes and discounts — reuse its numbers instead of recalculating.

Header totals are normal record fields: `$recordModel->get('hdnSubTotal')`, `get('hdnS_H_Amount')` (shipping), `get('txtAdjustment')`, `get('hdnGrandTotal')`. Record models use **field names**; the DB columns are `subtotal`, `s_h_amount`, `adjustment`, `total`.

### Saving line items

**UI save:** `Inventory_Save_Action` → record save → `save_module()` → `saveInventoryProductDetails()`, which reads the line-item keys from `$_REQUEST` (`totalProductCount`, `hdnProductId{N}`, `qty{N}`, `listPrice{N}`, `discount_percentage{N}`, …). It also deletes old rows via `deleteInventoryProductDetails()` and updates stock via `updateInventoryProductRel()` — so never write a raw `DELETE` + `INSERT` into `vtiger_inventoryproductrel`.

**Save from code (API, import, cron):** `Inventory_Record_Model::saveWithItems()` fills those request keys for you (used by `IntegrationApiHandler::saveRecord()`):

```php
$recordModel = Vtiger_Record_Model::getCleanInstance('SalesOrder');
$recordModel->saveWithItems(
	[
		'subject' => $subject,
		'sostatus' => 'Created',
		'account_id' => $accountId,
		'total' => $subTotal,
		'discount_amount' => 0,
		'grand_total' => $grandTotal,
	],
	[
		[
			'productid' => $productId,
			'sequence_no' => 1,
			'quantity' => 2,
			'listprice' => 150000,
			'purchase_cost' => 100000,
			'discount_percent' => 0,
			'discount_amount' => 0,
			'taxes' => [],
		],
	]
);
```

**Header-only update (keep line items):** `$recordModel->getEntity()->isLineItemUpdate = false;` before `save()`.

## Edit View / Save Action

Inherit and override only what you need; the line-item UI and request parsing are already in the parents.

```php
class SalesOrder_Edit_View extends Inventory_Edit_View {

	public function checkPermission(Vtiger_Request $request) {
		parent::checkPermission($request);
		// extra checks
	}
}

class SalesOrder_Save_Action extends Inventory_Save_Action {
	// override process()/saveRecord() only for module-specific behaviour
}
```

Line-item templates: `layouts/v7/modules/Inventory/partials/LineItemsEdit.tpl` (edit) and `layouts/v7/modules/Inventory/LineItemsDetail.tpl` (detail).

## Database Tables

### Main Module Table (inventory-related columns)

```sql
-- vtiger_salesorder (excerpt; check SHOW COLUMNS for the full list)
salesorderid INT PRIMARY KEY,
subject VARCHAR(...),
sostatus VARCHAR(...),
subtotal DECIMAL(...),        -- field hdnSubTotal
s_h_amount DECIMAL(...),      -- field hdnS_H_Amount (shipping)
adjustment DECIMAL(...),      -- field txtAdjustment
total DECIMAL(...),           -- field hdnGrandTotal
currency_id INT,
conversion_rate DECIMAL(...)
```

### Custom Fields Table

```sql
CREATE TABLE vtiger_salesordercf (
	salesorderid INT PRIMARY KEY
);
```

### Shared Line Items Table

```sql
-- vtiger_inventoryproductrel (already exists, shared by all inventory modules)
lineitem_id INT AUTO_INCREMENT PRIMARY KEY,
id INT,                       -- parent crmid (salesorderid, invoiceid, ...), indexed
productid INT,
sequence_no INT,
section_num INT,
section_name VARCHAR(100),
quantity DECIMAL(25,3),
listprice DECIMAL(27,8),
discount_percent DECIMAL(7,3),
discount_amount DECIMAL(27,8),
comment TEXT,
description TEXT,
incrementondel INT,
purchase_cost DECIMAL(27,8),
margin DECIMAL(27,8),
tax1 ... tax7 DECIMAL(7,3),
image VARCHAR(2)
```

## Reference Implementations

**Study these core modules:**
- `modules/SalesOrder/` — complete inventory implementation
- `modules/Invoice/` — similar pattern with payment tracking
- `modules/Quotes/` — simpler inventory module
- `modules/PurchaseOrder/` — vendor-side inventory

**Key files:**
- `modules/Inventory/` — base models, views, actions
- `include/utils/InventoryUtils.php` — save/delete line items, taxes, currency
- `include/utils/EditViewUtils.php` — `getAssociatedProducts()`

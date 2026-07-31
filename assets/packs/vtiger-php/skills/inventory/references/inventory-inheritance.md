# Inventory Inheritance Reference

## Module Structure with Type Declarations

### Entity Class

**File:** `modules/{Module}/{Module}.php`

```php
<?php

class SalesOrder extends Inventory {

    public function __construct() {
        parent::__construct();
        $this->table_name = 'vtiger_salesorder';
        $this->table_index = 'salesorderid';
        $this->column_fields = ['...'];
    }

    public function getSupportedModules(): array {
        return ['Products', 'Services'];
    }
}
```

### Module Model

**File:** `modules/{Module}/models/Module.php`

```php
<?php

class SalesOrder_Module_Model extends Inventory_Module_Model {

    public function getInventoryFields(): array {
        return [
            'product' => ['label' => 'Product', 'uitype' => 10],
            'quantity' => ['label' => 'Qty', 'uitype' => 7],
            'listprice' => ['label' => 'Price', 'uitype' => 71],
            'discount' => ['label' => 'Discount', 'uitype' => 1],
            'tax' => ['label' => 'Tax', 'uitype' => 83],
            'total' => ['label' => 'Total', 'uitype' => 71],
        ];
    }

    public function getTaxes(): array {
        global $adb;
        $sql = "SELECT taxname, percentage FROM vtiger_inventorytaxinfo
                WHERE deleted = 0";
        $result = $adb->pquery($sql);

        $taxes = [];
        while ($row = $adb->fetchByAssoc($result)) {
            $taxes[] = [
                'taxname' => (string) $row['taxname'],
                'percentage' => (float) $row['percentage'],
            ];
        }
        return $taxes;
    }

    public function getDefaultCurrency(): array {
        $sql = "SELECT * FROM vtiger_currency_info WHERE defaultid = 1";
        global $adb;
        $result = $adb->pquery($sql);
        return $adb->fetchByAssoc($result);
    }
}
```

### Record Model

**File:** `modules/{Module}/models/Record.php`

```php
<?php

class SalesOrder_Record_Model extends Inventory_Record_Model {

    public function getLineItems(): array {
        global $adb;
        $recordId = (int) $this->getId();

        $sql = "SELECT * FROM vtiger_inventoryproductrel
                WHERE id = ? ORDER BY sequence_no ASC";
        $result = $adb->pquery($sql, [$recordId]);

        $lineItems = [];
        while ($row = $adb->fetchByAssoc($result)) {
            $lineItems[] = [
                'sequence_no' => (int) $row['sequence_no'],
                'productid' => (int) $row['productid'],
                'quantity' => (float) $row['quantity'],
                'listprice' => (float) $row['listprice'],
                'discount_amount' => (float) $row['discount_amount'],
                'discount_percent' => (float) $row['discount_percent'],
                'tax1' => (float) $row['tax1'],
                'tax2' => (float) $row['tax2'],
                'tax3' => (float) $row['tax3'],
                'comment' => (string) $row['comment'],
            ];
        }
        return $lineItems;
    }

    public function calculateTotals(): array {
        $lineItems = $this->getLineItems();
        $subtotal = 0;
        $totalDiscount = 0;
        $totalTax = 0;

        foreach ($lineItems as $item) {
            $itemSubtotal = $item['quantity'] * $item['listprice'];
            $itemDiscount = $item['discount_amount'] ?:
                            ($itemSubtotal * $item['discount_percent'] / 100);
            $taxableAmount = $itemSubtotal - $itemDiscount;
            $itemTax = $taxableAmount * ($item['tax1'] + $item['tax2'] + $item['tax3']) / 100;

            $subtotal += $itemSubtotal;
            $totalDiscount += $itemDiscount;
            $totalTax += $itemTax;
        }

        $shipping = (float) $this->get('hdnS_H_Amount');
        $adjustment = (float) $this->get('txtAdjustment');
        $grandTotal = $subtotal - $totalDiscount + $totalTax + $shipping + $adjustment;

        return [
            'subtotal' => $subtotal,
            'discount' => $totalDiscount,
            'tax' => $totalTax,
            'shipping' => $shipping,
            'adjustment' => $adjustment,
            'total' => $grandTotal,
        ];
    }

    public function saveLineItems(array $lineItems): void {
        global $adb;
        $recordId = (int) $this->getId();

        // Clear existing line items
        $adb->pquery("DELETE FROM vtiger_inventoryproductrel WHERE id = ?", [$recordId]);

        // Save new line items
        $sequence = 1;
        foreach ($lineItems as $item) {
            $sql = "INSERT INTO vtiger_inventoryproductrel
                    (id, sequence_no, productid, quantity, listprice, discount_amount,
                     discount_percent, tax1, tax2, tax3, comment)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";
            $adb->pquery($sql, [
                $recordId,
                $sequence++,
                (int) $item['productid'],
                (float) $item['quantity'],
                (float) $item['listprice'],
                (float) ($item['discount_amount'] ?? 0),
                (float) ($item['discount_percent'] ?? 0),
                (float) ($item['tax1'] ?? 0),
                (float) ($item['tax2'] ?? 0),
                (float) ($item['tax3'] ?? 0),
                (string) ($item['comment'] ?? ''),
            ]);
        }
    }
}
```

## Edit View

**File:** `modules/{Module}/views/Edit.php`

```php
<?php

class SalesOrder_Edit_View extends Inventory_Edit_View {

    public function process(Vtiger_Request $request): void {
        $recordId = (int) $request->get('record');

        if ($recordId > 0) {
            $recordModel = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');
            $lineItems = $recordModel->getLineItems();
        } else {
            $lineItems = [];
        }

        $viewer = $this->getViewer($request);
        $viewer->assign('LINE_ITEMS', $lineItems);
        $viewer->view('EditView.tpl', 'SalesOrder');
    }
}
```

## Save Action

**File:** `modules/{Module}/actions/Save.php`

```php
<?php

class SalesOrder_Save_Action extends Inventory_Save_Action {

    public function process(Vtiger_Request $request): void {
        $recordId = (int) $request->get('record');

        if ($recordId > 0) {
            $recordModel = Vtiger_Record_Model::getInstanceById($recordId, 'SalesOrder');
            $recordModel->set('mode', 'edit');
        } else {
            $recordModel = Vtiger_Record_Model::getCleanInstance('SalesOrder');
        }

        // Save main record fields
        $recordModel->set('subject', $request->get('subject'));
        $recordModel->set('sostatus', $request->get('sostatus'));
        $recordModel->save();

        // Save line items
        $lineItems = $this->getLineItemsFromRequest($request);
        $recordModel->saveLineItems($lineItems);

        $response = new Vtiger_Response();
        $response->setResult(['record' => $recordModel->getId()]);
        $response->emit();
    }

    protected function getLineItemsFromRequest(Vtiger_Request $request): array {
        $productIds = $request->get('productid') ?: [];
        $quantities = $request->get('quantity') ?: [];
        $listPrices = $request->get('listprice') ?: [];

        $lineItems = [];
        foreach ($productIds as $index => $productId) {
            $lineItems[] = [
                'productid' => (int) $productId,
                'quantity' => (float) $quantities[$index],
                'listprice' => (float) $listPrices[$index],
            ];
        }
        return $lineItems;
    }
}
```

## Database Tables

### Main Module Table

```sql
CREATE TABLE vtiger_salesorder (
    salesorderid INT PRIMARY KEY,
    subject VARCHAR(255),
    sostatus VARCHAR(100),
    hdnS_H_Amount DECIMAL(25,8),  -- Shipping
    txtAdjustment DECIMAL(25,8),  -- Adjustment
    currency_id INT,
    conversion_rate DECIMAL(10,3)
);
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
CREATE TABLE vtiger_inventoryproductrel (
    id INT,  -- Foreign key to main module (salesorderid, invoiceid, etc.)
    sequence_no INT,
    productid INT,
    quantity DECIMAL(25,3),
    listprice DECIMAL(25,8),
    discount_amount DECIMAL(25,8),
    discount_percent DECIMAL(25,3),
    comment TEXT,
    tax1 DECIMAL(7,3),
    tax2 DECIMAL(7,3),
    tax3 DECIMAL(7,3)
);
```

## Reference Implementations

**Study these core modules:**
- `modules/SalesOrder/` — complete inventory implementation
- `modules/Invoice/` — similar pattern with payment tracking
- `modules/Quotes/` — simpler inventory module
- `modules/PurchaseOrder/` — vendor-side inventory

**Key files:**
- `modules/Inventory/` — base classes
- `include/utils/InventoryUtils.php` — shared utilities
- `vtlib/Vtiger/Inventory.php` — entity base class

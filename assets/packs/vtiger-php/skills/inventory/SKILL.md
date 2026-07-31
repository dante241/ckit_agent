---
name: inventory
description: "VTiger inventory modules — line items, SalesOrder/Invoice/Quote, vtiger_inventoryproductrel, tax/currency. Use when: đơn hàng, hoá đơn, báo giá, line item, sản phẩm trong đơn, thuế, tiền tệ."
user-invocable: false
---

# VTiger Inventory Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

Use this skill when:
- Creating modules with line items (products/services)
- Building order-like entities (quotes, invoices, purchase orders)
- Implementing tax/discount calculations
- Adding currency support to modules
- Working with inventory product relations
- Creating custom inventory-based modules

## Inheritance Pattern

**Entity inheritance:**
```php
modules/{Module}/{Module}.php extends Inventory
```

**Model inheritance:**
```php
// Module Model
class Module_Module_Model extends Inventory_Module_Model { }

// Record Model
class Module_Record_Model extends Inventory_Record_Model { }
```

**View/Action inheritance:**
```php
// Views
class Module_Edit_View extends Inventory_Edit_View { }
class Module_Detail_View extends Inventory_Detail_View { }

// Actions
class Module_Save_Action extends Inventory_Save_Action { }
class Module_Delete_Action extends Inventory_Delete_Action { }
```

## Module Structure

```
modules/SalesOrder/
├── SalesOrder.php                # Entity extends Inventory
├── models/
│   ├── Module.php                # extends Inventory_Module_Model
│   └── Record.php                # extends Inventory_Record_Model
├── views/
│   ├── Edit.php                  # extends Inventory_Edit_View
│   └── Detail.php                # extends Inventory_Detail_View
└── actions/
    ├── Save.php                  # extends Inventory_Save_Action
    └── Delete.php                # extends Inventory_Delete_Action
```

## Line Items Table

**Core table:** `vtiger_inventoryproductrel`

**Shared by all inventory modules** — columns:
- `id` — primary key (NOT module-specific crmid)
- `sequence_no` — line item order
- `productid` — related product/service crmid
- `quantity` — item quantity
- `listprice` — unit price
- `discount_amount` — discount per item
- `discount_percent` — discount percentage
- `comment` — line item notes
- `tax1` — first tax rate
- `tax2` — second tax rate
- `tax3` — third tax rate

**Critical:** Use `id` column for foreign key, NOT the module's crmid.

## Module Model Methods

### getInventoryFields(): array

```php
public function getInventoryFields(): array {
    return [
        'product' => ['label' => 'Product', 'uitype' => 10],
        'quantity' => ['label' => 'Quantity', 'uitype' => 7],
        'listprice' => ['label' => 'List Price', 'uitype' => 71],
        'discount' => ['label' => 'Discount', 'uitype' => 1],
        'tax' => ['label' => 'Tax', 'uitype' => 83],
        'total' => ['label' => 'Total', 'uitype' => 71],
    ];
}
```

### getTaxes(): array

```php
public function getTaxes(): array {
    global $adb;
    $sql = "SELECT * FROM vtiger_inventorytaxinfo WHERE deleted = 0";
    $result = $adb->pquery($sql);

    $taxes = [];
    while ($row = $adb->fetchByAssoc($result)) {
        $taxes[] = [
            'taxname' => $row['taxname'],
            'percentage' => $row['percentage'],
        ];
    }
    return $taxes;
}
```

## Record Model Methods

### getLineItems(): array

```php
public function getLineItems(): array {
    global $adb;
    $recordId = $this->getId();

    $sql = "SELECT * FROM vtiger_inventoryproductrel
            WHERE id = ? ORDER BY sequence_no";
    $result = $adb->pquery($sql, [$recordId]);

    $lineItems = [];
    while ($row = $adb->fetchByAssoc($result)) {
        $lineItems[] = $row;
    }
    return $lineItems;
}
```

### calculateTotals(): array

```php
public function calculateTotals(): array {
    $lineItems = $this->getLineItems();
    $subtotal = 0;
    $totalTax = 0;

    foreach ($lineItems as $item) {
        $itemTotal = $item['quantity'] * $item['listprice'];
        $itemTotal -= $item['discount_amount'];
        $subtotal += $itemTotal;

        $taxAmount = $itemTotal * ($item['tax1'] + $item['tax2']) / 100;
        $totalTax += $taxAmount;
    }

    return [
        'subtotal' => $subtotal,
        'tax' => $totalTax,
        'total' => $subtotal + $totalTax,
    ];
}
```

## Line Items Template Pattern

**Location:** `layouts/v7/modules/{Module}/EditView.tpl`

```smarty
{include file="InventoryItems.tpl"}

<div class="inventory-items">
    {foreach from=$LINE_ITEMS item=ITEM}
        <div class="line-item">
            <input type="hidden" name="sequence_no[]" value="{$ITEM.sequence_no}">
            <input type="text" name="productid[]" value="{$ITEM.productid}">
            <input type="number" name="quantity[]" value="{$ITEM.quantity}">
            <input type="text" name="listprice[]" value="{$ITEM.listprice}">
        </div>
    {/foreach}
</div>
```

## Tax/Currency/Calculation

**Calculation order:**
1. Subtotal = quantity × listprice
2. Discount = subtotal × discount_percent / 100 OR discount_amount
3. Taxable = subtotal - discount
4. Tax = taxable × (tax1 + tax2 + tax3) / 100
5. Shipping (from module field)
6. Adjustment (from module field)
7. Grand total = taxable + tax + shipping + adjustment

**Currency support:**
- Store `conversion_rate` in main module table
- All amounts stored in base currency
- Convert on display using stored rate

**Tax support:**
- Reuse `vtiger_inventorytaxinfo` table
- Multiple tax rates per line item
- Compound or simple tax calculation

## Critical Pitfalls

1. **MUST extend Inventory classes** — no standalone inventory modules
2. **vtiger_inventoryproductrel shared** — use `id` column, not crmid
3. **sequence_no for order** — preserve line item sequence
4. **Store conversion_rate** — currency changes don't affect historical records
5. **Reuse tax logic** — don't recreate tax calculations
6. **Batch line items** — save all at once, not one-by-one
7. **Idempotent delete** — clear old line items before re-saving

## Reference Files

- [Inventory Inheritance](references/inventory-inheritance.md) — Complete module structure guide

## Quick Example

```php
// Record Model
class CustomOrder_Record_Model extends Inventory_Record_Model {

    public function getLineItems(): array {
        global $adb;
        $sql = "SELECT * FROM vtiger_inventoryproductrel
                WHERE id = ? ORDER BY sequence_no";
        $result = $adb->pquery($sql, [$this->getId()]);

        $items = [];
        while ($row = $adb->fetchByAssoc($result)) {
            $items[] = $row;
        }
        return $items;
    }

    public function calculateTotals(): array {
        $items = $this->getLineItems();
        $subtotal = 0;

        foreach ($items as $item) {
            $subtotal += $item['quantity'] * $item['listprice'];
        }

        return ['subtotal' => $subtotal];
    }
}
```

## Exemplars (PENDING REVIEW by user)

> ⚠️ Chưa tìm được exemplar thuần Tín Bùi/Tùng Nguyễn cho domain này — file dưới là code tác giả khác, dùng tạm đến khi user chỉ định file chuẩn.

- Inventory record model (line items): `modules/Inventory/models/Record.php`
- Ajax inventory: `modules/Inventory/actions/BasicAjax.php`

## Verify

```bash
# Tạo SalesOrder/Invoice test qua UI với >=2 line item + tax + discount
# Check tổng tiền các tầng: line total, subtotal, tax, grand total khớp tay tính
mysql <db> -e "SELECT * FROM vtiger_inventoryproductrel WHERE id=<crmid>"
```

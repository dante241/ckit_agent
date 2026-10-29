---
name: inventory
description: "VTiger inventory modules — line items, SalesOrder/Invoice/Quote, vtiger_inventoryproductrel, tax/currency. Use when: đơn hàng, hoá đơn, báo giá, line item, sản phẩm trong đơn, thuế, tiền tệ."
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

**Entity:** there is NO `Inventory` entity class. Inventory entities extend `CRMEntity` directly and call the inventory utils from `save_module()`:
```php
class SalesOrder extends CRMEntity { /* ... */ } // modules/SalesOrder/SalesOrder.php
```

**Model inheritance:**
```php
class SalesOrder_Module_Model extends Inventory_Module_Model { }
class SalesOrder_Record_Model extends Inventory_Record_Model { }
```

**View/Action inheritance** (only the classes that exist in `modules/Inventory/`):
```php
// Views: Detail, Edit, List, Popup, ProductsPopup, ServicesPopup, SendEmail, ...
class SalesOrder_Edit_View extends Inventory_Edit_View { }
class SalesOrder_Detail_View extends Inventory_Detail_View { }

// Actions: BasicAjax, ExportPDF, GetTaxes, MassSave, Save, SaveAjax
class SalesOrder_Save_Action extends Inventory_Save_Action { }
```

There is no `Inventory_Delete_Action` — delete goes through `Vtiger_Delete_Action`.

## Module Structure (real `modules/SalesOrder/`, trimmed)

```
modules/SalesOrder/
├── SalesOrder.php                # Entity extends CRMEntity; save_module() saves line items
├── models/
│   ├── Module.php                # extends Inventory_Module_Model
│   └── Record.php                # extends Inventory_Record_Model
├── views/
│   ├── Edit.php                  # extends Inventory_Edit_View
│   └── Detail.php                # extends Inventory_Detail_View
└── actions/
    ├── Save.php                  # extends Inventory_Save_Action
    └── SaveAjax.php
```

## Line Items Table

**Core table:** `vtiger_inventoryproductrel` — shared by all inventory modules.

| Column | Meaning |
|---|---|
| `lineitem_id` | **primary key** (auto increment) |
| `id` | parent record crmid (salesorderid, invoiceid, quoteid, …) — indexed, not unique |
| `productid` | product/service crmid |
| `sequence_no` | line order |
| `section_num`, `section_name` | line item section grouping |
| `quantity`, `listprice` | qty and unit price |
| `discount_percent`, `discount_amount` | per-line discount |
| `comment`, `description` | line notes |
| `incrementondel` | stock flag: `deleteInventoryProductDetails()` queues these lines' quantities so `updateInventoryProductRel()` can put them back to stock on re-save |
| `purchase_cost`, `margin` | cost / margin per line |
| `tax1` … `tax7` | per-line tax percentages (one column per `vtiger_inventorytaxinfo.taxname`) |
| `image` | `varchar(2)`, not written by the core save path |

Read the live schema before writing SQL: `SHOW COLUMNS FROM vtiger_inventoryproductrel`.

## Core APIs — reuse, don't reimplement

| Need | API |
|---|---|
| Line items of a record (with taxes, totals in `[1]['final_details']`) | `Inventory_Record_Model::getProducts()` (wraps `getAssociatedProducts()` in `include/utils/EditViewUtils.php`) |
| Product / shipping taxes of a record | `$recordModel->getProductTaxes()`, `getShippingTaxes()` |
| All tax definitions | `getAllTaxes($available = 'all', $sh = '', $mode = '', $id = '')` in `include/utils/InventoryUtils.php` |
| Record currency | `$recordModel->getCurrencyInfo()` / `getInventoryCurrencyInfo($module, $id)` |
| Save line items | Standard record save: `save_module()` → `saveInventoryProductDetails($focus, $module)` reads the `$_REQUEST` line-item keys (`hdnProductId1`, `qty1`, `listPrice1`, …, `totalProductCount`) |
| Save from code (API, import) | `Inventory_Record_Model::saveWithItems(array $recordData, array $items)` — builds those request keys via `populateItemDataToRequest()` and saves |

`saveInventoryProductDetails()` also handles stock (`updateInventoryProductRel`, `deleteInventoryProductDetails`) and subproducts — a hand-written `DELETE` + `INSERT` into `vtiger_inventoryproductrel` skips all of that.

## Line Items Templates

Edit and detail line items are rendered by the Inventory layout partials — extend or override these instead of writing new markup:

- Edit: `layouts/v7/modules/Inventory/partials/LineItemsEdit.tpl` (included from `partials/EditViewContents.tpl`, rows in `partials/LineItemsContent.tpl`)
- Detail: `layouts/v7/modules/Inventory/LineItemsDetail.tpl` (included from `DetailViewFullContents.tpl`)

```smarty
{include file="partials/LineItemsEdit.tpl"|@vtemplate_path:'Inventory'}
```

## Tax/Currency/Calculation

**Calculation order:**
1. Line total = quantity × listprice
2. Line discount = line total × discount_percent / 100 OR discount_amount
3. Taxable = line total − line discount
4. Line tax = taxable × (sum of the line's tax columns) / 100 (individual tax mode) — or group tax on the pre-tax total
5. Shipping (`hdnS_H_Amount` field → `s_h_amount` column)
6. Adjustment (`txtAdjustment` field → `adjustment` column)
7. Grand total (`hdnGrandTotal` field → `total` column)

**Currency support:**
- `currency_id` + `conversion_rate` are stored on the main module table
- Amounts are stored in the **record's own currency**; reports convert to base by dividing by the record's `conversion_rate`
- The base currency row in `vtiger_currency_info` has `defaultid = '-11'` (varchar)

**Tax support:**
- Reuse `vtiger_inventorytaxinfo` (product taxes) / `vtiger_shippingtaxinfo` (shipping)
- Each active tax maps to a `taxN` column on `vtiger_inventoryproductrel`

## Critical Pitfalls

1. **Models extend `Inventory_*`; the entity extends `CRMEntity`** — there is no `Inventory` entity class
2. **`lineitem_id` is the PK**; `id` is the parent crmid (many rows per record)
3. **Save line items through `saveInventoryProductDetails()` / `saveWithItems()`** — never raw DELETE + INSERT (skips stock + subproducts)
4. **`SaveAjax` / mass edit do NOT re-save line items** — `save_module()` skips them for those actions
5. **sequence_no for order** — preserve line item sequence
6. **Store conversion_rate** — currency changes don't affect historical records
7. **Reuse tax logic** — `getAllTaxes()` / `getProductTaxes()`, don't recreate tax calculations
8. **Custom SQL goes in the module's Data helper**, not in Record/Module models (see `module` skill)

## Reference Files

- [Inventory Inheritance](references/inventory-inheritance.md) — entity, models, views, save flow with real code

## Exemplars

Not authored by the preferred owners — core code, read it to learn the API:

- Inventory record model (line items, `getProducts()`, `saveWithItems()`): `modules/Inventory/models/Record.php`
- Ajax inventory: `modules/Inventory/actions/BasicAjax.php`

## Verify

```bash
# Tạo SalesOrder/Invoice test qua UI với >=2 line item + tax + discount
# Check tổng tiền các tầng: line total, subtotal, tax, grand total khớp tay tính
mysql <db> -e "SELECT lineitem_id, sequence_no, productid, quantity, listprice, tax1 FROM vtiger_inventoryproductrel WHERE id=<crmid> ORDER BY sequence_no"
```

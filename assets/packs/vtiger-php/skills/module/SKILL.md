---
name: module
description: "VTiger module structure — MVC, Record/Module model, Data/Logic helper, CustomModules đăng ký, quick_repair. Use when: tạo module mới, entity, model, helper Data/Logic; field lẻ → skill field."
user-invocable: false
---

# VTiger Module Structure

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use This Skill

- Creating new VTiger modules
- Understanding module directory structure
- Working with Record/Module models
- Implementing Data/Logic helpers
- Extending CRMEntity base class

## Creating a New Module — Minimal Path

> Full recipe (with CLI commands, expected output, troubleshooting): see [`references/create-module.md`](references/create-module.md).

Most modules need only TWO steps. Do not pre-create empty model skeletons, empty `BlocksAndFieldsRegister.php`, or empty language files — they are not required for the module to register and run.

### Step 1: Register in `include/Extensions/CustomModules.php`

```php
'CPDemo' => array(
    'moduleName' => 'CPDemo',
    'displayNameEn' => 'CP Demo',
    'displayNameVn' => 'CP Demo',
    'menu' => 'TOOLS',
    'isExtension' => false,
    'hasActivities' => false,
    'isPerson' => false,
    'createdBy' => 'base'
),
```

| Key | Type | Description |
|-----|------|-------------|
| `moduleName` | string | Must match folder name in `modules/` |
| `displayNameEn` | string | English display name |
| `displayNameVn` | string | Vietnamese display name |
| `menu` | string | Menu group: `TOOLS`, `SALES`, `MARKETING`, `SUPPORT`, `OPERATION`, `CUSTOMERS`, `DEBITS`, `HRM`, `CONFIGS`, `UTILITIES` |
| `isExtension` | bool | `true` = no entity table (config/dashboard). `false` = has entity with DB tables |
| `hasActivities` | bool | `true` = can have Calendar activities linked |
| `isPerson` | bool | `true` = person-type module (firstname/lastname fields) |
| `createdBy` | string | `base` (R&D), `dev` (DEV team), `cus` (customer) |

### Step 2: Run Quick Repair

```bash
php cli_tool.php quick_repair
```

This auto-creates `vtiger_<module>` + `vtiger_<module>cf` tables, registers in `vtiger_tab`, wires the menu entry. That's it — the module is now usable in the UI.

### When you actually need extra files

Add **only what you need**, never empty placeholders:

| You need... | Add file |
|---|---|
| Custom fields beyond `<module>id` | `modules/<Module>/BlocksAndFieldsRegister.php` + re-run quick_repair |
| Custom table columns / indexes / FK / UNIQUE constraints | CRMEntity at `modules/<Module>/<Module>.php` extending `Vtiger_CRMEntity` (define `$table_name`, `$customFieldTable`, `$list_fields`, `$mandatory_fields`) — or write a `CPMigration` migration |
| UI labels / translations | `languages/en_us/<Module>.php` + `languages/vn_vn/<Module>.php` |
| Custom DetailView / ListView buttons or behavior | `modules/<Module>/models/<Record\|Module\|ListView\|DetailView>.php` subclass overriding specific methods — do NOT create empty subclasses, framework falls back to `Vtiger_*_Model` parents automatically |
| AJAX endpoints | `modules/<Module>/actions/<Name>.php` |
| Custom pages | `modules/<Module>/views/<Name>.php` + TPL in `modules/<Module>/tpls/<Name>.tpl` |
| Event handlers | `modules/<Module>/handlers/<Name>.php` + entry in `HandlersRegister.php` |

The framework autoloader treats missing `<Module>_<Component>_Model` classes as "use parent" — empty subclass files add noise without behavior.

## Module Directory Layout

| Directory | Purpose |
|-----------|---------|
| `modules/{Module}/` | Core module logic (Models, Actions, Views) |
| `modules/{Module}/models/` | Record_Model, Module_Model, custom models |
| `modules/{Module}/actions/` | AJAX/JSON endpoints |
| `modules/{Module}/views/` | HTML page controllers |
| `modules/{Module}/helpers/` | Data.php (DB), Logic.php (business) |
| `layouts/v7/modules/{Module}/` | Smarty TPL templates |
| `modules/{Module}/resources/` | CSS files (always here), JS for custom views |
| `layouts/v7/modules/{Module}/resources/` | JS for core views (List, Edit, Detail) |
| `languages/en_us/{Module}.php` | Language strings (LBL_*, JS_*) |

## Key Module Classes

### 1. CRMEntity (Entity Definition)
- Located: `modules/{Module}/{Module}.php`
- Purpose: Define tables, fields, indexes, relationships
- Extends: `CRMEntity` base class

### 2. Record Model (Instance Operations)
- Located: `modules/{Module}/models/Record.php`
- Purpose: CRUD operations on individual records
- Extends: `Vtiger_Record_Model`

### 3. Module Model (Module-Level Operations)
- Located: `modules/{Module}/models/Module.php`
- Purpose: Module-wide operations, metadata, search
- Extends: `Vtiger_Module_Model`

## Helpers Pattern

### Data Helper (Database Layer)
- File: `modules/{Module}/helpers/Data.php`
- Pattern: `{Module}_Data_Helper`
- Responsibility: Database queries ONLY (pquery, fetchByAssoc, decodeUTF8)
- Rule: NEVER call from View/Action directly

### Logic Helper (Business Logic)
- File: `modules/{Module}/helpers/Logic.php`
- Pattern: `{Module}_Logic_Helper`
- Responsibility: Business rules, validation, orchestration
- Rule: Calls Data helper for DB access

## Critical Pitfalls

1. **Always decodeUTF8** on fetchByAssoc results
2. **Always pquery with params**, never concatenate SQL
3. **set('mode','edit')** before updating records
4. **Entity tab_name MUST include vtiger_crmentity**
5. **Data=DB only, Logic=business** — never mix layers

## References

- [create-module.md](references/create-module.md) — End-to-end minimal recipe with `cli_tool.php quick_repair` command, expected output, troubleshooting
- [module-structure.md](references/module-structure.md) — Directory layout, file naming
- [mvc-pattern.md](references/mvc-pattern.md) — Request flow, Controller types
- [model-entity.md](references/model-entity.md) — CRMEntity, Record/Module models
- [helpers-pattern.md](references/helpers-pattern.md) — Data vs Logic separation

## Exemplars (ưu tiên code Tín Bùi / Tùng Nguyễn — PENDING REVIEW by user)

- Module CP hoàn chỉnh 100% tung.nguyen (actions/views/helpers/handlers/BFR/resources): `modules/CPMasterPlan/`
- Record model (Tin Bui 2/2): `modules/CPAdvertisingAccount/models/Record.php`

## Verify

```bash
php cli_tool.php quick_repair
curl -s 'http://localhost/vtiger/index.php?module=<Module>&view=List' -H 'Cookie: PHPSESSID=<sid>' | grep -c listview
# Kỳ vọng: module mở được List view không fatal
```

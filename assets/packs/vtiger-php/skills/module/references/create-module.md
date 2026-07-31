# Creating a VTiger Module — Minimal Recipe

> **Loaded by:** [SKILL.md](../SKILL.md) Step 1-2 reference.

The whole module-creation flow in this codebase is two commands. Everything else (CRMEntity class, BlocksAndFieldsRegister, models, language files) is optional and added on-demand. Pre-creating empty stubs is anti-pattern in this codebase.

---

## TL;DR

```bash
# 1. Edit include/Extensions/CustomModules.php — add the module entry
# 2. Register + create base tables:
php cli_tool.php -c quick_repair
```

After step 2, the module is registered in `vtiger_tab`, base tables `vtiger_<module>` + `vtiger_<module>cf` exist, the menu wiring is live, and the module is usable in the UI.

---

## Step-by-step

### 1. Add the entry in `include/Extensions/CustomModules.php`

```php
'CPDemo' => array(
    'moduleName'    => 'CPDemo',           // MUST match modules/<Folder>
    'displayNameEn' => 'CP Demo',
    'displayNameVn' => 'CP Demo',
    'menu'          => 'TOOLS',             // Group: TOOLS, SALES, MARKETING, SUPPORT, OPERATION, CUSTOMERS, DEBITS, HRM, CONFIGS, UTILITIES
    'isExtension'   => false,               // false = entity with DB tables; true = config/dashboard only
    'hasActivities' => false,               // true = Calendar activity linkage
    'isPerson'      => false,               // true = person-type (firstname/lastname schema)
    'createdBy'     => 'base',              // base (R&D), dev (DEV team), cus (customer)
),
```

Place the new entry alphabetically next to similar modules (e.g. between `CPSocialFeedback` and `CPSocialIntegration` for a social-domain entity).

### 2. Run Quick Repair

```bash
php cli_tool.php -c quick_repair
```

Expected output:

```
Finished!
```

This invokes `Vtiger_QuickRepair_Action::process()` (`cli_tool.php:72-85`) which calls `ModuleBuilder::build()` (`include/ModuleBuilder/ModuleBuilder.php:78`). For each new entry in `CustomModules.php` it:

- inserts a row into `vtiger_tab` for the new module
- **copies module folder from VTiger template** (`copyFolder()` line 139) — generates `modules/<Module>/` with default `<Module>.php` CRMEntity, model skeletons, `BlocksAndFieldsRegister.php`
- **generates language files** for `en_us` + `vn_vn` (`replaceModuleLang()` lines 144-148)
- creates `vtiger_<modulename>` table (entity table) + `vtiger_<modulename>cf` table (custom fields table) via `$moduleInstance->initTables()` (line 169)
- creates default block + standard fields (name, assigned_user_id, etc.) via `addBlockAndFields()` (line 171)
- sets default sharing + initializes webservice + enables Import/Export/Merge (lines 174-179)
- syncs `BlocksAndFieldsRegister.php` from DB if missing (lines 184-186 — `syncToRegisterFile`)
- adds the module to its menu (line 191 — `Settings_MenuEditor_Module_Model::addModuleToApp`)
- registers the menu entry per `menu` group
- clears Smarty + module caches

**Key point:** because `copyFolder()` short-circuits when `modules/<Module>/` already exists (line 138), pre-creating any file under that path BLOCKS the template copy. Either commit the full template-equivalent files yourself, or commit only the `CustomModules.php` entry and let `quick_repair` generate the folder.

### 3. Verify

- Login as admin → confirm the module appears under its menu group with an empty List view
- Check tables: `SHOW TABLES LIKE 'vtiger_<modulename>%';` → two rows expected
- Check tab row: `SELECT * FROM vtiger_tab WHERE name = '<ModuleName>';`

---

## When (and only when) to add extra files

| Need | File | Action |
|---|---|---|
| Custom fields (text, picklist, lookup, datetime, etc.) | `modules/<Module>/BlocksAndFieldsRegister.php` | Declare `$blocks` + `$fields` arrays. Re-run `quick_repair` to materialize columns + `vtiger_field` metadata. |
| Custom table columns / indexes / FK / UNIQUE constraints / data backfill | `modules/CPMigration/migrations/YYYY.MM.DD.HH.mm.ss_<Name>.php` | Write a CPMigration migration with `pquery()` DDL. Idempotent short-circuit returns `self::UP_SUCCESS`. |
| Override entity table name or `$customFieldTable` | `modules/<Module>/<Module>.php` | Subclass `Vtiger_CRMEntity`; set `$table_name`, `$customFieldTable`, `$list_fields`, `$mandatory_fields`. Required only when the framework defaults need overriding — otherwise skip. |
| UI labels / translations | `languages/en_us/<Module>.php`, `languages/vn_vn/<Module>.php` | Vietnamese locale dir is `vn_vn` (NOT `vi_vn`). Both files must return `$languageStrings = [...]`. |
| Override DetailView / ListView / Module behavior | `modules/<Module>/models/<Record\|Module\|ListView\|DetailView>.php` | Subclass `Vtiger_<Component>_Model` and override only the methods you need. Do NOT create empty subclasses — VTiger autoloader falls back to the parent automatically when the subclass file is absent. |
| AJAX endpoint | `modules/<Module>/actions/<Name>.php` | `<Module>_<Name>_Action extends Vtiger_Action_Controller` |
| Custom page | `modules/<Module>/views/<Name>.php` + `modules/<Module>/tpls/<Name>.tpl` | View extends `CustomView_Base_View` (NOT `Vtiger_Index_View`); see [`view/references/custom-view-base.md`](../../view/references/custom-view-base.md). |
| Event handler | `modules/<Module>/handlers/<Name>.php` + entry in `modules/<Module>/HandlersRegister.php` | See `handler` skill. |

---

## Common pitfalls

- **Empty model subclasses** (`class CPDemo_Record_Model extends Vtiger_Record_Model {}` with no body) — DELETE these. They add noise without behavior. Framework autoloader uses the parent automatically when the file is missing.
- **`isExtension: true` confusion** — set this only for config/dashboard pages with no entity table. If unsure, use `false`.
- **`moduleName` ≠ folder name** — `'moduleName' => 'CPDemo'` MUST match `modules/CPDemo/` exactly. Case-sensitive on Linux deploys.
- **Forgetting to re-run `quick_repair`** after editing `BlocksAndFieldsRegister.php` — the file is read by Quick Repair, not lazily at request time. New fields won't appear in UI until repair runs.
- **Wrong Vietnamese locale dir** — the directory is `languages/vn_vn/`, NOT `languages/vi_vn/`. Files in `vi_vn` are orphaned.
- **Adding `assigned_user_id` as a custom column on the entity table** — ownership lives on `vtiger_crmentity.smownerid`. Use UIType 53 in `BlocksAndFieldsRegister.php`; the join is automatic.
- **`createdtime` / `modifiedtime` / `deleted`** — auto-provided by `vtiger_crmentity`. Do NOT register them as custom fields; Quick Repair will create duplicate metadata rows.

---

## Related skills

- `field` — Custom field types, UITypes, picklists, layout (EditView/DetailView/QuickCreate)
- `migration` — Schema migrations for columns/indexes beyond what Quick Repair provides
- `language` — Translation files, `vtranslate()`, 3-tier loading (base → dev → cus)
- `handler` — Event handlers, `HandlersRegister.php`
- `view` — Core vs custom views, `CustomView_Base_View`

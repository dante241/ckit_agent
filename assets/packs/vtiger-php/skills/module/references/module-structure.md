# VTiger Module Structure

## Directory Layout

### Core VTiger Directories
```
modules/{Module}/               # Module root
├── {Module}.php               # CRMEntity definition
├── models/                    # Model classes
│   ├── Record.php            # Record_Model
│   ├── Module.php            # Module_Model
│   └── ListView.php          # ListView_Model (optional)
├── actions/                   # AJAX/JSON endpoints
├── views/                     # HTML page controllers
├── helpers/                   # Data & Logic helpers
│   ├── Data.php              # Database layer
│   └── Logic.php             # Business logic
├── handlers/                  # Event handlers
└── custom/                    # Custom field layouts
    ├── EditView.php
    ├── DetailView.php
    └── QuickCreate.php
```

### Custom Module Extensions
```
modules/{Module}/
├── Extensions.php             # Run-once setup script executed by Quick Repair
├── HandlersRegister.php       # Event handler registration
├── BlocksAndFieldsRegister.php # Field definitions (synced by Quick Repair)
└── RelationshipsRegister.php  # Module relationships
```

### Layout Directories
```
layouts/v7/modules/{Module}/
├── *.tpl                      # Smarty templates
└── resources/
    ├── *.js                   # JavaScript controllers
    └── *.css                  # Module styles
```

### Language Files
```
languages/en_us/
├── {Module}.php               # Base strings (R&D)
├── dev/{Module}.php           # Dev team customizations
└── cus/{Module}.php           # Customer strings
```

## File Naming Conventions

| File Type | Convention | Example |
|-----------|------------|---------|
| PHP Class | PascalCase | `Record.php`, `DetailView.php` |
| Template | PascalCase | `Detail.tpl`, `EditView.tpl` |
| JavaScript | PascalCase | `DetailView.js` |
| CSS | PascalCase | `Detail.css` |
| Helper | PascalCase | `Data.php`, `Logic.php` |

## Important Core Files

### 1. CRMEntity (`{Module}.php`)
Defines module structure (extends `Vtiger_CRMEntity`):
- `$table_name` — Primary table
- `$tab_name` — Array of tables (MUST include `vtiger_crmentity`)
- `$table_index` — Primary key field
- `$customFieldTable` — Custom fields table
- `$list_fields_name` — ListView columns

### 2. Extensions.php
A plain PHP script, not a config array. `Vtiger_QuickRepair_Action::loadExtensions()` `include_once`s every `modules/*/Extensions.php` on each Quick Repair, so it must be idempotent (create-if-missing). Example `modules/CPLogAPI/Extensions.php`:
```php
require_once('modules/CPLogAPI/CPLogAPI.php');

$logAPIEntity = new CPLogAPI();
$logAPIEntity->initTableLog();
$logAPIEntity->initCustomModule();
```

### 3. HandlersRegister.php
Declares variables that `loadHandlers()` reads after `include_once` (see `modules/CPMasterPlan/HandlersRegister.php`):
```php
$registeredEvents = array(
	'vtiger.entity.beforesave',
	'vtiger.entity.aftersave',
	'vtiger.entity.beforedelete',
	'vtiger.entity.afterdelete',
);

$handlerName = '{Module}Handler'; // class in modules/{Module}/handlers/{Module}Handler.php
// $batchHandlerName = '{Module}BatchHandler'; // for vtiger.batchevent.* events
```

### 4. BlocksAndFieldsRegister.php
Module blocks/fields as code. Quick Repair generates it for a new module (`syncToRegisterFile`) and syncs it to the DB on every run (`repairBlocksAndFields()` → `syncToDatabase`). Dev/customer overlays: `BlocksAndFieldsRegister.dev.php` / `.cus.php`.

### 5. RelationshipsRegister.php
Define related list relationships between modules (applied by Quick Repair `loadRelationships()`).

## Core VTiger Key Files

- `include/utils/CommonUtils.php` — Utility functions
- `include/Webservice/` — REST API utilities
- `modules/Vtiger/models/Record.php` — Base Record_Model
- `modules/Vtiger/models/Module.php` — Base Module_Model
- `vtlib/Vtiger/Module.php` — Module creation/management
- `include/events/VTEventHandler.php` — Event handler base
- `include/database/PearDatabase.php` — Database access

## Critical Rules

1. **Entity tab_name** MUST include `vtiger_crmentity` for proper CRM behavior
2. **File naming** follows PascalCase for PHP classes and templates
3. **Custom directory** for field layout customization (NOT class-based)
4. **Helpers separation**: Data.php=DB, Logic.php=business
5. **Language files** cascade: base → dev → customer (later overrides)

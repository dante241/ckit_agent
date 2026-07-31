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
├── Extensions.php             # Custom UITypes registration
├── HandlersRegister.php       # Event handler registration
├── BlocksAndFieldsRegister.php # Field definitions
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
Defines module structure:
- `$table_name` — Primary table
- `$tab_name` — Array of tables (MUST include `vtiger_crmentity`)
- `$table_index` — Primary key field
- `$column_fields` — Field definitions
- `$customFieldTable` — Custom fields table
- `$list_fields_name` — ListView columns

### 2. Extensions.php
Register custom UITypes:
```php
return [
    'vtlib.uitype.handler.extended' => [
        'MyCustom_UIType_Model' => 'modules/{Module}/uitypes/MyCustom.php',
    ],
];
```

### 3. HandlersRegister.php
Register event handlers:
```php
return [
    'vtiger.entity.aftersave' => '{Module}_EntitySave_Handler',
    'vtiger.entity.beforedelete' => '{Module}_EntityDelete_Handler',
];
```

### 4. BlocksAndFieldsRegister.php
Define module fields programmatically (used in migrations).

### 5. RelationshipsRegister.php
Define related list relationships between modules.

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

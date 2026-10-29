# System Config (Database)

> Layer 1: Global configuration stored in `vtiger_config` table

## Settings_Vtiger_Config_Model

**Location**: `modules/Settings/Vtiger/models/Config.php`

**Table**: `vtiger_config`
- `category` (varchar(100)) — config group identifier
- `value` (mediumtext) — JSON-encoded array

### Core Methods (real signatures — untyped, static)

#### saveConfig($category, $config)

`static function saveConfig($category, $config)` — returns nothing. Checks existence with `$adb->getOne("SELECT COUNT(category) ...")`, then UPDATE or INSERT `json_encode($config, JSON_UNESCAPED_UNICODE)` via `pquery`.

#### loadConfig($category, $toArray = false)

`static function loadConfig($category, $toArray = false)` — reads the value with `$adb->getOne(...)` and returns `json_decode(html_entity_decode($config, ENT_COMPAT | ENT_HTML401), $toArray)`. A missing category decodes `''` → `null`, so callers add `?? []`.

**Usage**:
```php
// Return as array
$config = Settings_Vtiger_Config_Model::loadConfig('module_settings', true) ?? [];
// ['enabled' => true, 'api_key' => 'abc123']

// Return as stdClass
$config = Settings_Vtiger_Config_Model::loadConfig('module_settings');
```

### Real Example: Call Center Config

`modules/Settings/Vtiger/actions/SaveCallCenterConfig.php` saves `callcenter_config` / `callcenter_gateway`; readers include `modules/PBXManager/actions/OutgoingCall.php`:

```php
Settings_Vtiger_Config_Model::saveConfig('callcenter_config', $generalConfig);

$callCenterConfig = Settings_Vtiger_Config_Model::loadConfig('callcenter_config');
```

## Config Helper Pattern

Use the static-cache helper from `SKILL.md` (§ Config Helper Pattern) — `loadConfig()` hits the DB on every call, a static variable reduces N queries per request to 1.

## Admin Settings View Pattern

View, Save action (admin gate + `validateRequest` → `validateWriteAccess()`), JS and the render convention `$viewer->display('modules/Settings/Vtiger/tpls/<ConfigName>.tpl')` are in [config-view.md](./config-view.md).

### Template (`modules/Settings/Vtiger/tpls/<ConfigName>.tpl`)

Smarty output is NOT auto-escaped in this codebase — escape stored values.

```smarty
<div class="container-fluid">
    <form name="configs">
        <div class="form-group">
            <label>
                <input type="checkbox" name="enabled" {if $CONFIGS.enabled}checked{/if}>
                {vtranslate('LBL_ENABLE_FEATURE', $MODULE_NAME)}
            </label>
        </div>

        <div class="form-group">
            <label>{vtranslate('LBL_API_KEY', $MODULE_NAME)}</label>
            <input type="text" name="api_key" value="{$CONFIGS.api_key|escape}" class="form-control">
        </div>

        <button type="submit" class="btn btn-success">
            {vtranslate('LBL_SAVE', $MODULE_NAME)}
        </button>
    </form>
</div>
```

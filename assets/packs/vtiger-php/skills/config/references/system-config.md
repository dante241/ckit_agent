# System Config (Database)

> Layer 1: Global configuration stored in `vtiger_config` table

## Settings_Vtiger_Config_Model

**Location**: `modules/Settings/Vtiger/models/Config.php`

**Table**: `vtiger_config`
- `category` (varchar) — config group identifier
- `value` (text) — JSON-encoded array

### Core Methods

#### saveConfig($category, $configArray)

Upserts configuration to database.

```php
/**
 * Save configuration to vtiger_config table
 * @param string $category Config category/group name
 * @param array $configArray Associative array of config values
 * @return bool Success status
 */
public static function saveConfig(string $category, array $configArray): bool {
    $db = PearDatabase::getInstance();

    // CRITICAL: Use JSON_UNESCAPED_UNICODE to preserve Unicode characters
    $value = json_encode($configArray, JSON_UNESCAPED_UNICODE);

    // Check if category exists
    $result = $db->pquery("SELECT 1 FROM vtiger_config WHERE category = ?", [$category]);

    if ($db->num_rows($result) > 0) {
        // Update existing
        $db->pquery("UPDATE vtiger_config SET value = ? WHERE category = ?", [$value, $category]);
    } else {
        // Insert new
        $db->pquery("INSERT INTO vtiger_config (category, value) VALUES (?, ?)", [$category, $value]);
    }

    return true;
}
```

**Internal Flow**:
1. JSON encode array with `JSON_UNESCAPED_UNICODE` flag
2. Check existence with `SELECT 1` (faster than `SELECT *`)
3. Upsert logic: UPDATE if exists, INSERT if new
4. Use prepared statements (`pquery`) for security

#### loadConfig($category, $toArray = false)

Loads configuration from database.

```php
/**
 * Load configuration from vtiger_config table
 * @param string $category Config category name
 * @param bool $toArray Return as array (true) or stdClass (false)
 * @return array|stdClass|null Decoded config or null if not found
 */
public static function loadConfig(string $category, bool $toArray = false) {
    $db = PearDatabase::getInstance();

    $result = $db->pquery("SELECT value FROM vtiger_config WHERE category = ?", [$category]);

    if ($db->num_rows($result) === 0) {
        return null;
    }

    $row = $db->fetchByAssoc($result);
    $value = $row['value'];

    // Decode JSON
    return json_decode($value, $toArray);
}
```

**Usage**:
```php
// Return as array
$config = Settings_Vtiger_Config_Model::loadConfig('module_settings', true);
// ['enabled' => true, 'api_key' => 'abc123']

// Return as stdClass
$config = Settings_Vtiger_Config_Model::loadConfig('module_settings');
// object { enabled: true, api_key: 'abc123' }
```

### Real Example: Call Center Config

```php
// Save call center config
$config = [
    'enable' => true,
    'provider' => 'stringee',
    'auto_popup' => false,
    'record_calls' => true
];
Settings_Vtiger_Config_Model::saveConfig('callcenter_config', $config);

// Load call center config
$config = Settings_Vtiger_Config_Model::loadConfig('callcenter_config', true);
if (!empty($config['enable'])) {
    // Feature enabled
}
```

## Config Helper Pattern

For modules with complex config, use static helper class with caching:

```php
class PBXManager_Config_Helper {

    /**
     * Check if call center is enabled (file config)
     */
    public static function isCallCenterEnabled(): bool {
        global $callCenterConfig;

        if (empty($callCenterConfig['enable'])) {
            return false;
        }

        return true;
    }

    /**
     * Get call center config from DB with static cache
     */
    public static function getCallCenterConfig(): array {
        static $config;

        if (!empty($config)) {
            return $config;
        }

        $config = Settings_Vtiger_Config_Model::loadConfig('callcenter_config', true) ?? [];

        return $config;
    }

    /**
     * Get specific config value with default
     */
    public static function getConfigValue(string $key, $default = null) {
        $config = self::getCallCenterConfig();
        return $config[$key] ?? $default;
    }
}
```

**Why Static Cache?**
- `loadConfig()` hits DB every call
- Static variable persists for request lifetime
- Reduces DB queries from N to 1

## Admin Settings View Pattern

### View Controller

```php
class Settings_ModuleName_ConfigView_View extends Settings_Vtiger_BaseConfig_View {

    public function process(Vtiger_Request $request) {
        $viewer = $this->getViewer($request);
        $moduleName = $request->getModule();

        // Load current config
        $config = Settings_Vtiger_Config_Model::loadConfig('module_config', true) ?? [];

        // Assign to template
        $viewer->assign('CONFIG', $config);
        $viewer->assign('MODULE', $moduleName);

        $viewer->view('ConfigView.tpl', $moduleName);
    }
}
```

### Action Controller

```php
class Settings_ModuleName_SaveConfig_Action extends Settings_Vtiger_Basic_Action {

    public function __construct() {
        parent::__construct();
        $this->exposeMethod('save');
    }

    public function save(Vtiger_Request $request) {
        $config = [
            'enabled' => (bool) $request->get('enabled'),
            'api_key' => (string) $request->get('api_key'),
            'threshold' => (int) $request->get('threshold')
        ];

        Settings_Vtiger_Config_Model::saveConfig('module_config', $config);

        $response = new Vtiger_Response();
        $response->setResult([
            'success' => true,
            'message' => vtranslate('LBL_CONFIG_SAVED', 'Settings:Vtiger')
        ]);
        $response->emit();
    }
}
```

### Template (ConfigView.tpl)

```smarty
<div class="container-fluid">
    <form id="configForm">
        <div class="form-group">
            <label>
                <input type="checkbox" name="enabled" {if $CONFIG.enabled}checked{/if}>
                {vtranslate('LBL_ENABLE_FEATURE', $MODULE)}
            </label>
        </div>

        <div class="form-group">
            <label>{vtranslate('LBL_API_KEY', $MODULE)}</label>
            <input type="text" name="api_key" value="{$CONFIG.api_key}" class="form-control">
        </div>

        <button type="submit" class="btn btn-success">
            {vtranslate('LBL_SAVE', $MODULE)}
        </button>
    </form>
</div>
```

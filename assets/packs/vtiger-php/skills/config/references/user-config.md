# User Preferences (Per-User Config)

> Layer 2: User-specific settings stored in `vtiger_user_preferences` table

## Users_Preferences_Model

**Location**: `modules/Users/models/Preferences.php`

**Table**: `vtiger_user_preferences`
- `user_id` (int) — user record ID
- `category` (varchar) — preference group identifier
- `value` (text) — JSON-encoded array

### Core Methods

#### savePreferences($userId, $category, $preferencesArray)

Upserts user preferences to database.

```php
/**
 * Save user preferences
 * @param int $userId User record ID
 * @param string $category Preference category/group
 * @param array $preferencesArray Associative array of preference values
 * @return bool Success status
 */
public static function savePreferences(int $userId, string $category, array $preferencesArray): bool {
    $db = PearDatabase::getInstance();

    // CRITICAL: Use JSON_UNESCAPED_UNICODE
    $value = json_encode($preferencesArray, JSON_UNESCAPED_UNICODE);

    // Check if user preference exists
    $result = $db->pquery(
        "SELECT 1 FROM vtiger_user_preferences WHERE user_id = ? AND category = ?",
        [$userId, $category]
    );

    if ($db->num_rows($result) > 0) {
        // Update existing
        $db->pquery(
            "UPDATE vtiger_user_preferences SET value = ? WHERE user_id = ? AND category = ?",
            [$value, $userId, $category]
        );
    } else {
        // Insert new
        $db->pquery(
            "INSERT INTO vtiger_user_preferences (user_id, category, value) VALUES (?, ?, ?)",
            [$userId, $category, $value]
        );
    }

    return true;
}
```

#### loadPreferences($userId, $category, $toArray = false)

Loads user preferences from database.

```php
/**
 * Load user preferences
 * @param int $userId User record ID
 * @param string $category Preference category
 * @param bool $toArray Return as array (true) or stdClass (false)
 * @return array|stdClass|null Decoded preferences or null if not found
 */
public static function loadPreferences(int $userId, string $category, bool $toArray = false) {
    $db = PearDatabase::getInstance();

    $result = $db->pquery(
        "SELECT value FROM vtiger_user_preferences WHERE user_id = ? AND category = ?",
        [$userId, $category]
    );

    if ($db->num_rows($result) === 0) {
        return null;
    }

    $row = $db->fetchByAssoc($result);
    $value = $row['value'];

    return json_decode($value, $toArray);
}
```

**Usage**:
```php
$currentUser = Users_Record_Model::getCurrentUserModel();
$userId = $currentUser->getId();

// Save user preferences
$prefs = [
    'notifications_enabled' => true,
    'theme' => 'dark',
    'sidebar_collapsed' => false
];
Users_Preferences_Model::savePreferences($userId, 'ui_settings', $prefs);

// Load user preferences
$prefs = Users_Preferences_Model::loadPreferences($userId, 'ui_settings', true);
// ['notifications_enabled' => true, 'theme' => 'dark', 'sidebar_collapsed' => false]
```

## Real Example: Call Center User Config

### View Controller

```php
class Settings_Vtiger_CallCenterUserConfig_View extends Settings_Vtiger_Index_View {

    /**
     * IMPORTANT: User config views allow all users (not just admins)
     */
    public function checkPermission(Vtiger_Request $request): bool {
        return true;
    }

    public function process(Vtiger_Request $request) {
        $viewer = $this->getViewer($request);
        $currentUser = Users_Record_Model::getCurrentUserModel();
        $userId = (int) $currentUser->getId();

        // Load user's call center config
        $userConfig = Users_Preferences_Model::loadPreferences(
            $userId,
            'callcenter_user_config',
            true
        ) ?? [];

        $viewer->assign('USER_CONFIG', $userConfig);
        $viewer->assign('USER_ID', $userId);

        $viewer->view('CallCenterUserConfig.tpl', 'Settings:Vtiger');
    }
}
```

### Action Controller

```php
class Settings_Vtiger_SaveCallCenterUserConfig_Action extends Settings_Vtiger_Basic_Action {

    /**
     * User config actions allow all users
     */
    public function checkPermission(Vtiger_Request $request): bool {
        return true;
    }

    public function __construct() {
        parent::__construct();
        $this->exposeMethod('save');
    }

    public function save(Vtiger_Request $request) {
        $currentUser = Users_Record_Model::getCurrentUserModel();
        $userId = (int) $currentUser->getId();

        // Collect user preferences
        $userConfig = [
            'auto_answer' => (bool) $request->get('auto_answer'),
            'ringtone' => (string) $request->get('ringtone'),
            'show_customer_info' => (bool) $request->get('show_customer_info'),
            'default_status' => (string) $request->get('default_status')
        ];

        // Save to database
        Users_Preferences_Model::savePreferences($userId, 'callcenter_user_config', $userConfig);

        $response = new Vtiger_Response();
        $response->setResult([
            'success' => true,
            'message' => vtranslate('LBL_PREFERENCES_SAVED', 'Settings:Vtiger')
        ]);
        $response->emit();
    }
}
```

## System Config vs User Preferences

| Aspect | System Config | User Preferences |
|---|---|---|
| **Table** | `vtiger_config` | `vtiger_user_preferences` |
| **Model** | `Settings_Vtiger_Config_Model` | `Users_Preferences_Model` |
| **Scope** | Global (all users) | Per-user |
| **Keys** | `category` | `user_id`, `category` |
| **Save Method** | `saveConfig($category, $array)` | `savePreferences($userId, $category, $array)` |
| **Load Method** | `loadConfig($category, $toArray)` | `loadPreferences($userId, $category, $toArray)` |
| **Permission Check** | `isAdmin()` required | All users allowed (`return true`) |
| **Use Cases** | Feature flags, API keys, thresholds | UI settings, notifications, user-specific behavior |

## Common Patterns

### Get Current User Preferences

```php
$currentUser = Users_Record_Model::getCurrentUserModel();
$userId = (int) $currentUser->getId();

$prefs = Users_Preferences_Model::loadPreferences($userId, 'module_prefs', true);

// With default fallback
$prefs = Users_Preferences_Model::loadPreferences($userId, 'module_prefs', true) ?? [
    'default_view' => 'list',
    'records_per_page' => 20
];
```

### Update Single Preference Value

```php
$userId = (int) Users_Record_Model::getCurrentUserModel()->getId();

// Load existing
$prefs = Users_Preferences_Model::loadPreferences($userId, 'ui_prefs', true) ?? [];

// Update single value
$prefs['sidebar_collapsed'] = true;

// Save back
Users_Preferences_Model::savePreferences($userId, 'ui_prefs', $prefs);
```

### Merge with System Defaults

```php
// System-wide defaults
$systemConfig = Settings_Vtiger_Config_Model::loadConfig('module_defaults', true) ?? [];

// User overrides
$userPrefs = Users_Preferences_Model::loadPreferences($userId, 'module_prefs', true) ?? [];

// Merge: user prefs override system defaults
$effectiveConfig = array_merge($systemConfig, $userPrefs);
```

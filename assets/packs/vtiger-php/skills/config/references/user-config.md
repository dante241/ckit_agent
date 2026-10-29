# User Preferences (Per-User Config)

> Layer 2: User-specific settings stored in `vtiger_user_preferences` table

## Users_Preferences_Model

**Location**: `modules/Users/models/Preferences.php`

**Table**: `vtiger_user_preferences`
- `user_id` (int) — user record ID
- `category` (varchar) — preference group identifier
- `value` (text) — JSON-encoded array

### Core Methods (real signatures — untyped, static)

- `static function savePreferences($userId, $category, $preferences)` — returns nothing. Existence check via `$adb->getOne("SELECT 1 ...")`, then UPDATE or INSERT `json_encode($preferences, JSON_UNESCAPED_UNICODE)`.
- `static function loadPreferences($userId, $category, $toArray = false)` — `$adb->getOne(...)` then `json_decode(html_entity_decode($preferences, ENT_COMPAT | ENT_HTML401), $toArray)`. Missing row → `null`, so callers add `?? []`.

**Usage**:
```php
$userId = (int) Users_Record_Model::getCurrentUserModel()->getId();

$prefs = [
	'notifications_enabled' => true,
	'sidebar_collapsed' => false,
];
Users_Preferences_Model::savePreferences($userId, 'ui_settings', $prefs);

$prefs = Users_Preferences_Model::loadPreferences($userId, 'ui_settings', true) ?? [];
```

## Real Example: Call Center User Config

Per-user Settings page reachable by non-admins. Files: `modules/Settings/Vtiger/views/CallCenterUserConfig.php`, `modules/Settings/Vtiger/actions/SaveCallCenterUserConfig.php`, `modules/Settings/Vtiger/tpls/CallCenterUserConfig.tpl`. Category is `'callcenter_config'` (per-user row in `vtiger_user_preferences`, distinct from the system `callcenter_config` row in `vtiger_config`).

### View Controller (excerpt)

```php
class Settings_Vtiger_CallCenterUserConfig_View extends Settings_Vtiger_BaseConfig_View {

	// Per-user page: open to all users (overrides the admin gate of Settings_Vtiger_Index_View)
	function checkPermission(Vtiger_Request $request) {
		return true;
	}

	public function process(Vtiger_Request $request) {
		checkAccessForbiddenFeature('CallCenterIntegration');
		global $current_user;

		$moduleName = $request->getModule(false);
		$callCenterUserConfig = Users_Preferences_Model::loadPreferences($current_user->id, 'callcenter_config', true) ?? [];

		$viewer = $this->getViewer($request);
		$viewer->assign('VENDOR_CONFIG', $callCenterUserConfig);
		$viewer->assign('MODULE_NAME', $moduleName);
		$viewer->display('modules/Settings/Vtiger/tpls/CallCenterUserConfig.tpl');
	}
}
```

### Action Controller (excerpt)

```php
class Settings_Vtiger_SaveCallCenterUserConfig_Action extends Settings_Vtiger_Basic_Action {

	// Per-user save: any user, but only for their own $current_user->id
	function checkPermission(Vtiger_Request $request) {
		return true;
	}

	function validateRequest(Vtiger_Request $request) {
		$request->validateWriteAccess();
	}

	function process(Vtiger_Request $request) {
		checkAccessForbiddenFeature('CallCenterIntegration');
		global $current_user;

		$requestData = $request->getAll();
		$settings = $requestData['settings'] ?? [];
		$callCenterUserConfig = $settings['vendor'] ?? [];

		Users_Preferences_Model::savePreferences($current_user->id, 'callcenter_config', $callCenterUserConfig);

		$response = new Vtiger_Response();
		$response->setResult(true);
		$response->emit();
	}
}
```

`checkPermission() { return true; }` is acceptable ONLY because the action writes the caller's own preferences; never use it on an action that writes `vtiger_config` (system scope). Comparison of the config layers: see the table in `SKILL.md`.

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

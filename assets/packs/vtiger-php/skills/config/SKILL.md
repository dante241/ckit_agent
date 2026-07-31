---
name: config
description: "VTiger configuration — system config DB, user preferences, config_override, feature toggle. Use when: trang cấu hình, settings module, config, tuỳ chọn hệ thống, bật tắt tính năng."
user-invocable: false
---

# VTiger Configuration Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

- Managing module settings (enable/disable features)
- Storing global configuration (API keys, endpoints, thresholds)
- Handling user-specific preferences (UI settings, notifications)
- Feature toggles and flags
- Admin settings pages
- Reading/writing system or file configs

## Configuration Layers Comparison

| Aspect | System Config (DB) | File Config | User Preferences |
|---|---|---|---|
| **Storage** | `vtiger_config` table | PHP file (`config_override.php`) | `vtiger_user_preferences` table |
| **Model** | `Settings_Vtiger_Config_Model` | `CustomConfigUtils` | `Users_Preferences_Model` |
| **Scope** | Global (all users) | Global (`$GLOBALS`) | Per-user |
| **Use for** | Module settings, feature flags | Server config, static values | User-specific settings |
| **Persistence** | Database (JSON) | File system (PHP array) | Database (JSON) |
| **Cache** | None (query each time) | `$GLOBALS` at boot | None (query each time) |
| **Access** | `loadConfig($category)` | `$GLOBALS['key']['subkey']` | `loadPreferences($userId, $category)` |
| **Update** | `saveConfig($category, $array)` | `saveCustomConfigs(['dot.notation' => val])` | `savePreferences($userId, $category, $array)` |

## Config Helper Pattern

For modules with complex config logic, use static helper class:

```php
class ModuleName_Config_Helper {
    /**
     * Get module config with static cache
     */
    public static function getConfig(): array {
        static $config;
        if (!empty($config)) return $config;

        $config = Settings_Vtiger_Config_Model::loadConfig('module_config', true) ?? [];
        return $config;
    }

    /**
     * Check if feature is enabled
     */
    public static function isFeatureEnabled(): bool {
        $config = self::getConfig();
        return !empty($config['feature_enabled']);
    }
}
```

**Benefits**: Static cache prevents repeated DB queries, centralized config logic, type-safe access methods.

## Config View Pattern

All config view files live under `modules/Settings/Vtiger/` (NOT `layouts/v7/`):

| File | Naming |
|------|--------|
| View | `views/<ConfigName>.php` extends `BaseConfig_View` |
| Action | `actions/Save<ConfigName>.php` extends `Basic_Action` |
| Template | `tpls/<ConfigName>.tpl` |
| JS | `resources/<ConfigName>.js` extends `CustomView_BaseController_Js` |
| CSS | `resources/<ConfigName>.css` |

`BaseConfig_View` auto-loads JS/CSS matching the view name — no need to override `getHeaderScripts()`/`getHeaderCss()`.

See [Config View reference](./references/config-view.md) for full code examples (View, Action, JS, toggle pattern, exposeMethod).

## Critical Pitfalls

1. **Missing JSON_UNESCAPED_UNICODE**: ALWAYS use when encoding config arrays (Unicode chars get escaped otherwise)
2. **No Static Cache**: Repeated `loadConfig()` calls hit DB each time — use Config Helper pattern
3. **File Config Overuse**: Don't store dynamic data in file config — use DB config instead
4. **Direct Table Access**: NEVER query `vtiger_config` directly — use model methods
5. **Permission Checks**: System config views MUST check `isAdmin()`, user prefs views return `true` in `checkPermission()`

## Reference Files

- [Config View](./references/config-view.md) — BaseConfig_View, View/Action/JS patterns, exposeMethod, toggle
- [System Config (DB)](./references/system-config.md) — Settings_Vtiger_Config_Model, saveConfig/loadConfig patterns
- [User Preferences](./references/user-config.md) — Users_Preferences_Model, per-user settings
- [Config Globals](./references/config-globals.md) — $GLOBALS, vglobal(), config files

## Exemplars (PENDING REVIEW by user)

> ⚠️ Chưa tìm được exemplar thuần Tín Bùi/Tùng Nguyễn cho domain này — file dưới là code tác giả khác, dùng tạm đến khi user chỉ định file chuẩn.

- Config module hoàn chỉnh (view + helper + ajax): `modules/CPChatbotConfig/` (views/ChatbotConfig.php, helpers/Logic.php, views/ChatbotConfigAjax.php)

## Verify

```bash
rm -f test/templates_c/*.php
curl -s 'http://localhost/vtiger/index.php?module=<M>&view=<Config>' -H 'Cookie: PHPSESSID=<sid>' | grep -c '<form'
# Save config qua UI → reload → giá trị giữ nguyên
```

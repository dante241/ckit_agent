---
name: testing
description: "VTiger verify/testing — 3 tier verify-only/verify-sql/must-test, smoke test PHP standalone, chrome-devtools E2E, browser cache quirk. Use when: test, verify, kiểm tra tính năng sau code, viết test script."
user-invocable: false
---

# VTiger Testing Skill

> **Conventions:** Follow `.omp/rules/cloudgo-development-rules.md`

## When to Use

- Writing tests for new features or bug fixes
- Verifying VTiger components (Actions, Models, APIs, Webhooks, Event Handlers)
- Generating test case documentation
- Creating standalone test scripts
- Validating database operations, API responses, or business logic

## Context: VTiger Testing Environment

**Critical:** VTiger has **NO PHPUnit** framework. All tests are standalone PHP scripts that bootstrap VTiger runtime.

## Testing Flow

1. **Feature Spec** → Understand requirements and expected behavior
2. **Test Case Doc** → Generate structured test cases (see `references/test-case-generation.md`)
3. **Test Script** → Write standalone PHP test script (see `references/standalone-test-script.md`)
4. **Run & Verify** → Execute script and verify results (see `references/verification-patterns.md`)
5. **Report** → Document test results and coverage

## Quick Reference

### Test Case Template
```markdown
# Test Cases: {Feature Name}
## Module: {ModuleName}
### TC-01: {Scenario Name}
- **Type:** Happy path / Edge case / Error case
- **Precondition:** {setup}
- **Steps:** 1. ... 2. ...
- **Expected:** {outcome}
- **Actual:** [ ] Pass / [ ] Fail
```

### Standalone Script Pattern
```php
<?php
chdir(dirname(__FILE__) . '/../');
require_once('config.php');
require_once('include/utils/VtlibUtils.php');
require_once('includes/runtime/EntryPoint.php');

global $adb, $current_user;
$current_user = CRMEntity::getInstance('Users');
$current_user->retrieveCurrentUserInfoFromFile(1);

// Test functions here
```

### Test File Location
`test/test-{component}-{module}-{feature}.php`

Examples:
- `test/test-action-accounts-save.php`
- `test/test-model-cpgoal-calculation.php`
- `test/test-api-zalo-webhook.php`

## Component-Specific Strategies

| Component | Key Test Areas |
|-----------|----------------|
| **Action Controller** | Permission checks, parameter validation, CSRF, feature gates |
| **Helper/Model** | Normal input, empty input, boundary cases, SQL injection, XSS |
| **API Handler** | Auth (no token/invalid/valid), CRUD operations, error handling |
| **Webhook Connector** | Valid payload, malformed data, duplicate events, auth |
| **Event Handler** | afterSave create/edit, field changes, related records |

## Critical Pitfalls

1. **No PHPUnit** — Don't use PHPUnit syntax or annotations
2. **Bootstrap Required** — Always include VTiger bootstrap in test scripts
3. **User Context** — Tests run as admin (user ID 1) by default
4. **Cleanup** — Always clean up test records after execution
5. **Database State** — Tests may affect real database; use caution
6. **Vietnamese Text** — Always test with Vietnamese characters for text fields
7. **XSS/SQL Injection** — Include security test cases for user input

## References

- [Test Case Generation](references/test-case-generation.md) — Test case document templates and generation rules
- [Standalone Test Script](references/standalone-test-script.md) — Full test script template and patterns
- [Verification Patterns](references/verification-patterns.md) — Database, API, and log verification methods

## Verify tiers (chuẩn CloudGo — repo KHÔNG có test suite tự động)

1. **verify-only** (đổi nhỏ, no logic): `php -l` + mở trang liên quan không fatal.
2. **verify-sql**: chạy SQL thật trên DB dev, EXPLAIN nếu đụng bảng lớn.
3. **must-test** (logic/UI): thao tác flow thật qua chrome-devtools MCP (mở trang → thao tác → screenshot + console check). JS đổi → hard-refresh vì cache-buster tĩnh.

Báo xong PHẢI kèm evidence tier đã chạy (output lệnh / screenshot).

## Test cases BẮT BUỘC (mọi feature/bugfix chạm logic)

Trước khi code (Phase plan) → **liệt kê test case dạng bảng** vào `plan.md`/`todolist.md`. Không được code logic khi chưa có danh sách test case.

Mỗi case tối thiểu: **# · Kịch bản (input/điều kiện) · Kỳ vọng · Trạng thái** (`chưa` / `PASS` / `FAIL`).

**Bắt buộc phủ đủ:**
1. **Happy path** — hành vi mới đúng như yêu cầu.
2. **Regression** — hành vi CŨ không đổi (giá trị/quyền/nhánh cũ vẫn như trước).
3. **Negative/security** — case KHÔNG được cấp quyền/không được qua (đặc biệt logic permission: chứng minh không over-grant, và gate cấp trên — profile/ACL — vẫn chặn).
4. **Boundary/null** — dữ liệu cũ/thiếu field, giá trị biên, guard null.
5. **Mọi nhánh code mới** — nếu thêm code ở N nhánh (ROLE/GROUP, def_org 0/1/3, related-module…), phải có case chạm từng nhánh; nhánh chưa test → ghi rõ "CHƯA cover" trong plan, không được ngầm bỏ.

Hoàn thành: tick từng case + đính evidence (output script / screenshot). Case "CHƯA cover" còn lại phải nêu minh bạch trong Completion Notes, KHÔNG được tuyên bố "full coverage".

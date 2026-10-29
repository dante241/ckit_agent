---
name: testing
description: "VTiger verify/testing — 3 tier verify-only/verify-sql/must-test, smoke test PHP standalone cho code legacy, PHPUnit cho domains/*/tests, E2E bằng omp browser. Use when: test, verify, kiểm tra tính năng sau code, viết test script."
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

**Hai loại test:**
- **PHPUnit** — chỉ cho `domains/*/tests` (`phpunit.xml.dist`, chạy bằng `tools/vendor/bin/phpunit` hoặc `.cache/phpunit.phar`). Quy ước: `rule://domain-tests`, skill `vtiger-phpunit-unit-test`.
- **Standalone PHP script** — cho code legacy (`modules/`, `include/`, `api/`…): script trong `test/` tự bootstrap VTiger runtime và tự assert.

## Testing Flow

1. **Feature Spec** → Understand requirements and expected behavior
2. **Test Case Doc** → Generate structured test cases (see `references/test-case-generation.md`)
3. **Test Script** → Write standalone PHP test script (see `references/standalone-test-script.md`)
4. **Run & Verify** → Execute script and verify results (see `references/verification-patterns.md`)
5. **Report** → Document test results and coverage

## Quick Reference

- Template test case: `references/test-case-generation.md`
- Template script standalone (bootstrap, assert, cleanup): `references/standalone-test-script.md`

### Test File Location
Script standalone đặt trong `test/`, theo đúng 2 kiểu tên đang có:
- `test/test-#<ticket>-<feature>.php` — vd `test/test-#10432-save-integration.php`, `test/test-#11814-import-data-flow.php`
- `test/test-<component>-<module>-<feature>.php` — vd `test/test-model-cpnotifications-muted-modules.php`, `test/test-model-reports-chart-date-formatting.php`

## Component-Specific Strategies

| Component | Key Test Areas |
|-----------|----------------|
| **Action Controller** | Permission checks, parameter validation, CSRF, feature gates |
| **Helper/Model** | Normal input, empty input, boundary cases, SQL injection, XSS |
| **API Handler** | Auth (no token/invalid/valid), CRUD operations, error handling |
| **Webhook Connector** | Valid payload, malformed data, duplicate events, auth |
| **Event Handler** | afterSave create/edit, field changes, related records |

## Critical Pitfalls

1. **PHPUnit chỉ ở `domains/*/tests`** — script standalone cho legacy không dùng PHPUnit syntax/annotation
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

## Verify tiers (chuẩn CloudGo — PHPUnit chỉ phủ `domains/*/tests`; code legacy verify theo tier dưới)

1. **verify-only** (đổi nhỏ, no logic): `php -l` + mở trang liên quan không fatal.
2. **verify-sql**: chạy SQL thật trên DB dev, EXPLAIN nếu đụng bảng lớn.
3. **must-test** (logic/UI): thao tác flow thật qua omp `browser` tool (mở trang → thao tác → screenshot + console check). JS/CSS load qua `vresource_url()` tự có `?v=<version>.<mtime>` nên không cần hard-refresh; Smarty compile cache ở `cache/templates_c/v7/`.

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

# Security Rules (OWASP) — MANDATORY for all PHP

> Path-scoped sibling of `cloudgo-development-rules.md`. Loads when editing `**/*.php`.
> `code-reviewer` MUST load this before reviewing PHP. Every item is a review checklist row — a violation is a CRITICAL/HIGH finding, not a style nit.

## Checklist (tick every box on review)

### SQL Injection
- [ ] **ALL** queries use `pquery($sql, [$params])` with `?` placeholders. NO string concatenation / interpolation of user data into SQL.
- [ ] Id-set / `IN (...)` filters use **placeholder expansion**, never `implode(',', $ids)` into the SQL. Cast each id: `array_map('intval', $ids)` then bind `?` per element (or use the framework condition builder).
- [ ] When using `QueryGenerator`/`EnhancedQueryGenerator`: the ACL **sharing WHERE is auto-injected by `getQuery()`** — do NOT re-add it manually, and call `getQuery()` **exactly once** (add all conditions before that call, then splice cursor/ORDER/LIMIT onto the returned SQL).
- [ ] `JOIN vtiger_crmentity` always includes `AND deleted = 0`.

### XSS
- [ ] User-supplied **string** values written to records pass `vtlib_purify()` before `$record->set()`.
- [ ] JS renders dynamic content with `.text()` not `.html()`. (See `javascript-rules.md`.)
- [ ] API responses don't echo raw request fragments back to an unauthenticated caller (no reflected enumeration).

### CSRF / Write Access
- [ ] Write Actions call `$request->validateWriteAccess()` (CSRF token `__vtrftk`).
- [ ] State-changing API routes (POST/PATCH/DELETE) are not reachable without auth (deny-by-default router).

### Authorization (the v1 acceptance bar — permission fidelity)
- [ ] Module-level: `isPermitted(module, action)` / `checkPermission()` before any read or write.
- [ ] Record-level (IDOR): `isPermitted(module, 'EditView'|'Delete', $recordId)` **before** `getInstanceById()` mutation.
- [ ] Field-level: strip/reject fields the profile cannot edit; never write a field the user lacks edit permission for.
- [ ] Admin-only pages/endpoints check `isAdminUser()`.
- [ ] Resolve the gate from the **actual request module/record**, not a hard-coded one. (See cautionary example CR-02 below.)

### Input Handling
- [ ] Type-cast **every** `$request->get()` / external value: `(int)`, `(string)`, `(bool)` (see `php-conventions.md` → Type Casting).
- [ ] Validate path params (`{module}`, `{id}`) against existence → 404, before use.
- [ ] Pre-save validation gate returns structured errors (422) **before** touching the model on bad input.

### Secrets
- [ ] No secrets (`.env`, API keys, JWT secrets, passwords) in committed code. Secrets live in `config_override.cus.php` (tracked, value NOT committed). Enforced by `pretooluse-protect-secrets.sh`.
- [ ] HMAC/signature code **fails closed** on empty/placeholder secret — never sign with an empty key.

## Cautionary examples (real bugs caught in V1 Phase 3 review)

- **CR-02 — ACL bypass via duplicate route registration.** A `foreach ($modules as $m)` loop registered the SAME dynamic pattern `POST /{module}` once per module with a per-module permission gate. Router is first-match-wins → only the FIRST module's gate ever fired; users were wrongly 403'd / wrongly allowed. **Rule:** register a dynamic catch-all route ONCE with a baseline gate; do the real per-module/record ACL inside the handler from the resolved request module.
- **CR-04 — forgeable cursor on empty secret.** `hash_hmac('sha256', $json, $secret)` with `$secret = ''` produces a deterministic signature with a known empty key → attacker forges cursors. **Rule:** fail closed (`throw`) when the signing secret is empty/placeholder; on the decode side, treat empty secret as "reset to page 1", never verify against an empty key.

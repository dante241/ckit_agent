---
description: "OWASP security — SQLi/XSS/CSRF/permission/cast/threat model. BẮT BUỘC khi code PHP. Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(**/*.php), tool:write(**/*.php), tool:edit(**/*.inc), tool:write(**/*.inc), tool:edit(**/*.service), tool:write(**/*.service)"
condition: ".*"
---

# Security Rules (OWASP) — MANDATORY for all PHP

Every item is a review checklist row — a violation is a CRITICAL/HIGH finding, not a style nit.

**Threat model first:** before applying a security or robustness finding, identify what the code actually stores, protects, or exposes. Fix real failure modes. Document non-issues briefly. Ask when the risk is plausible but depends on product intent.

## Checklist (tick every box on review)

### SQL Injection
- [ ] Prepared statements only (`pquery` + `?`, see php-conventions → Database & SQL Conventions).
- [ ] Id-set / `IN (...)` filters use **placeholder expansion**, never `implode(',', $ids)` into the SQL. Cast each id: `array_map('intval', $ids)` then bind `?` per element (or use the framework condition builder).
- [ ] When using `QueryGenerator`/`EnhancedQueryGenerator`: the ACL **sharing WHERE is auto-injected by `getQuery()`** — do NOT re-add it manually, and call `getQuery()` **exactly once** (add all conditions before that call, then splice cursor/ORDER/LIMIT onto the returned SQL).

### XSS
- [ ] User-supplied **string** values written to records pass `vtlib_purify()` before `$record->set()`.
- [ ] API responses don't echo raw request fragments back to an unauthenticated caller (no reflected enumeration).

### CSRF / Write Access
- [ ] Write Actions call `$request->validateWriteAccess()` (CSRF token `__vtrftk`).
- [ ] State-changing API routes (POST/PATCH/DELETE) are not reachable without auth (deny-by-default router).

### Authorization (permission fidelity)
- [ ] Module-level: `isPermitted(module, action)` / `checkPermission()` before any read or write.
- [ ] Record-level (IDOR): php83 P1/P2 — `isPermitted(module, 'EditView'|'Delete', $recordId)` **before** `getInstanceById()` mutation.
- [ ] Field-level: strip/reject fields the profile cannot edit; never write a field the user lacks edit permission for.
- [ ] Admin-only pages/endpoints check `isAdminUser()`.
- [ ] Resolve the gate from the **actual request module/record**, not a hard-coded one. Router is first-match-wins: register a dynamic catch-all route (`POST /{module}`) ONCE with a baseline gate, never once per module; do the real per-module/record ACL inside the handler from the resolved request module.

### Input Handling
- [ ] Validate path params (`{module}`, `{id}`) against existence → 404, before use.
- [ ] Pre-save validation gate returns structured errors (422) **before** touching the model on bad input.

### Secrets
- [ ] Secrets live in `config_override.cus.php` (tracked, value NOT committed). No hook enforces this — check the staged diff.
- [ ] HMAC/signature code **fails closed** (`throw`) on empty/placeholder secret — never sign or verify with an empty key; on the decode side treat an empty secret as "reset to page 1".

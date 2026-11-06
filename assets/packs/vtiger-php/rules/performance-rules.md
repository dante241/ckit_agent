---
description: "Performance — tránh N+1, bounded query + LIMIT, index. BẮT BUỘC khi code PHP có SQL/vòng lặp/load record. Auto-load (TTSR) khi edit thêm SQL, loop hoặc load record."
scope: "tool:edit(**/*.php), tool:write(**/*.php), tool:edit(**/*.inc), tool:write(**/*.inc), tool:edit(**/*.service), tool:write(**/*.service)"
condition: ["\\b(SELECT|select|UPDATE|update|DELETE\\s+FROM|delete\\s+from|INSERT\\s+INTO|insert\\s+into|LIMIT|limit)\\b", "pquery\\(|->query\\(|getInstanceById\\(|getQuery\\(", "\\b(foreach|while|for)\\s*\\("]
---

# Performance Rules — MANDATORY for PHP touching SQL, loops or record loads

A clear violation (N+1 on a list path, unbounded query) is a HIGH/PERF finding.

## Checklist

### Avoid N+1
- [ ] On **list/collection** paths, do NOT call `Vtiger_Record_Model::getInstanceById()` (or any full-record load) **per row**. One SQL fetch should return the row data; resolve per-row needs from the row itself.
- [ ] For per-row permission flags (edit/delete), use `Users_Privileges_Model::isPermitted($module, 'EditView'|'Delete', $id)` (lightweight) instead of loading the full record model just to call `isEditable()`/`isDeletable()`.
- [ ] Batch lookups (owners, reference labels) outside the row loop where possible — collect ids, query once, map back.

### Bounded queries
- [ ] Every list endpoint applies a `LIMIT` / page size (clamped to a sane max, e.g. 1..100). No unbounded `SELECT`.
- [ ] Prefer **keyset (cursor) pagination** over `OFFSET` for large/append-heavy tables; offset only where arbitrary sort needs it. Deterministic sort needs a unique tie-breaker (`crmid`).
- [ ] QueryGenerator `getQuery()` once (security-rules → SQL Injection); re-running it per request is wasteful.

### Index-backed access
- [ ] WHERE/ORDER columns are index-backed. On `vtiger_crmentity` rely on `deleted`, `modifiedtime`, `smownerid`, `setype`; avoid sorting/filtering on un-indexed text columns at scale.
- [ ] Keyset sort columns (`modifiedtime`/`createdtime`/`crmid`) match the WHERE comparison column — never compare a `modifiedtime` cursor value against a `createdtime` column.

### Caching / repeated work
- [ ] Cache expensive per-request lookups (module/field metadata, role-aware field permissions). Respect existing cache-key discipline — e.g. `preFetchModuleFieldPermission` is keyed `{tabId}_{accessMode}` and is NOT `current_view`-aware, so detail-view vs edit-view permission passes must use distinct keys / reset between passes.
- [ ] Don't re-resolve the same module/field model repeatedly in a loop; hoist it.

### Direct SQL for hot paths
- [ ] Direct `pquery` UPDATE (skips save handlers) only as a deliberate, documented exception for high-frequency no-event updates; entity writes go through `Vtiger_Record_Model::save()`/`delete()`.

# Performance Rules — MANDATORY for all PHP

> Path-scoped sibling of `cloudgo-development-rules.md`. Loads when editing `**/*.php`.
> `code-reviewer` MUST load this before reviewing PHP. A clear violation (N+1 on a list path, unbounded query) is a HIGH/PERF finding.

## Checklist

### Avoid N+1
- [ ] On **list/collection** paths, do NOT call `Vtiger_Record_Model::getInstanceById()` (or any full-record load) **per row**. One SQL fetch should return the row data; resolve per-row needs from the row itself.
- [ ] For per-row permission flags (edit/delete), use `Users_Privileges_Model::isPermitted($module, 'EditView'|'Delete', $id)` (lightweight) instead of loading the full record model just to call `isEditable()`/`isDeletable()`.
- [ ] Batch lookups (owners, reference labels) outside the row loop where possible — collect ids, query once, map back.

### Bounded queries
- [ ] Every list endpoint applies a `LIMIT` / page size (clamped to a sane max, e.g. 1..100). No unbounded `SELECT`.
- [ ] Prefer **keyset (cursor) pagination** over `OFFSET` for large/append-heavy tables; offset only where arbitrary sort needs it. Deterministic sort needs a unique tie-breaker (`crmid`).
- [ ] `getQuery()` (QueryGenerator) is called **once** — add all conditions first, then splice cursor WHERE + ORDER BY + LIMIT onto the returned SQL. Re-running the generator per request is wasteful and risks double ACL.

### Index-backed access
- [ ] WHERE/ORDER columns are index-backed. On `vtiger_crmentity` rely on `deleted`, `modifiedtime`, `smownerid`, `setype`; avoid sorting/filtering on un-indexed text columns at scale.
- [ ] Keyset sort columns (`modifiedtime`/`createdtime`/`crmid`) match the WHERE comparison column — never compare a `modifiedtime` cursor value against a `createdtime` column.

### Caching / repeated work
- [ ] Cache expensive per-request lookups (module/field metadata, role-aware field permissions). Respect existing cache-key discipline — e.g. `preFetchModuleFieldPermission` is keyed `{tabId}_{accessMode}` and is NOT `current_view`-aware, so detail-view vs edit-view permission passes must use distinct keys / reset between passes.
- [ ] Don't re-resolve the same module/field model repeatedly in a loop; hoist it.

### Direct SQL for hot paths (when handlers add no value)
- [ ] For high-frequency, no-event-needed updates (e.g. last-sync timestamps), a direct `pquery` UPDATE is acceptable and preferred over a full `$record->save()` that fires every handler — but this is a deliberate, documented exception, never the default for entity writes (entity writes go through `Vtiger_Record_Model::save()`/`delete()` per CRUD rules).

## Reference
- Live findings: `docs/perf-analysis-2026-05-08.md`, `docs/perf-analysis-2026-06-06.md` (notification badge N+1, missing `vtiger_crmentity`/`vtiger_tab` indexes, metadata re-query).
- V1 Phase 3 WR-02: list rows triggered one `getInstanceById` per row solely for edit/delete flags — defeats the single SQL fetch. Resolve perms without the full model load.

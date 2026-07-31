---
paths:
  - "include/Webservice/**/*.php"
  - "modules/CPAPIIntegration/**/*.php"
  - "modules/CPOTTIntegration/**/*.php"
  - "modules/CPMauticIntegration/**/*.php"
---

# API Connector Patterns

> Loads when editing API connector / integration files (`include/Webservice/**`, `modules/CP*Integration/**`).

## V1 REST API (`/api/v1`) — Request Transport Rules (MANDATORY)

These apply to the public `/api/v1` gateway (`include/Webservice/V1/**`). LOCKED project decision — later phases MUST comply.

- **No JSON payload in a GET (or DELETE) query.** A GET/DELETE request MUST NOT carry a JSON-encoded value in a query parameter (e.g. `?filters=<json>`). GET is for path params + simple scalar/flat query params only (`?limit=25`, `?sort=-modifiedtime`, `?view=detail`).
- **Structured / JSON input → POST with a JSON body.** Any endpoint needing structured input (filter sets, bulk ids, search criteria, nested objects) MUST be a POST with the data in the request body. Example: list+filter is `POST /{module}/list` with body `{filters, sort, cursor, fields, limit, cvid}` — NOT `GET /{module}?filters=...`.
- **Reads that take only path/scalar params stay GET** (idempotent, cacheable): `GET /{module}/{id}`, `GET /{module}/{id}/related/{relatedModule}`, `GET /meta/modules/{module}?view=detail`.
- **Rationale:** JSON-in-query is brittle (encoding, length limits, log noise, cache poisoning) and breaks REST/proxy/cache semantics. A "list with filters" is conceptually a search → POST is correct.
- **API documentation (MANDATORY):** every endpoint section in `include/Webservice/V1/API-DOCUMENTATION.md` MUST show the COMPLETE request (headers + every body/param field) AND a REAL response captured from a live CRM call (curl against the running system, real data — never a mock/hand-written sample), plus error cases (401/403/404/422). Both payload and response = real system data.

### API doc change markers — keep the contract diff-able for the UI agent (MANDATORY)

`API-DOCUMENTATION.md` is the single contract the SPA / UI agent reads to generate and update its client. It is large; the UI agent must be able to detect *that* something changed and jump *to* the changed part without re-reading the whole file. So every change to the contract MUST leave three markers:

1. **Bump `Doc version` (semver) at the top of the file**, on EVERY contract change:
   - `MAJOR` — breaking change (removed/renamed endpoint, field, or error code; changed transport/shape a client can't ignore).
   - `MINOR` — backward-compatible addition or behavior tightening (new endpoint, new field, new query value, new ACL/validation that returns a new status).
   - `PATCH` — doc-only fix (re-captured example, typo, clarified prose) with no behavior change.
2. **Add one row to the `## Changelog` table** (reverse-chronological): `version | date | one-line change | affected §/anchor links`. The UI agent compares its last-integrated version against this table to get the exact list of sections to re-read.
3. **Put a callout at the TOP of each changed section:**
   > 🆕 **UPDATED `<date>` (v`<version>`):** one-line summary of what changed.

   Use `🆕 NEW` for a brand-new endpoint or field, `🆕 UPDATED` for a behavior/shape change on an existing one. A new endpoint = append its section AND a changelog row AND (optionally) a ToC entry. The `🆕` glyph is the agreed grep token — `grep '🆕'` must list everything recently touched.

   Keep older `🆕` callouts until they age out of relevance (roughly a milestone); they are cheap and let the UI agent see recent history at a glance.

A doc edit that changes the contract but skips the version bump / changelog row / section callout is an incomplete change — `code-reviewer` flags it the same as a missing real-response capture.

### Reuse existing business logic — survey BEFORE writing a V1 endpoint (MANDATORY)

A V1 endpoint is a thin REST/JSON adapter over logic VTiger ALREADY has. Do NOT re-implement create/update/delete/list/permission/serialization logic from scratch. Before writing ANY V1 handler method, find the existing native code that does the same thing and reuse it.

**Step 1 — locate the native equivalent.** For the operation your endpoint performs, find the matching native code path:
| V1 operation | Native source to study + reuse |
|--------------|--------------------------------|
| Create / update a record | `modules/<Module>/actions/Save.php` (or `modules/Vtiger/actions/Save.php`) — esp. `getRecordModelFromRequest()`, `saveRecord()`. Reuse `Vtiger_Record_Model::getCleanInstance()/getInstanceById()` + `->set()` + `->save()` (D-11), NOT raw pquery. |
| Delete (soft) | `modules/Vtiger/actions/Delete.php` → `$recordModel->delete()`. |
| List / filter / paginate | `modules/Vtiger/models/ListView.php` (`getListViewEntries` builds models via `getRecordFromArray($row)` — NO per-row `getInstanceById`, avoids N+1) + `Vtiger_QueryGenerator` for ACL-injected SQL. |
| Related list | `modules/Vtiger/models/RelationListView.php::getEntries` (same `getRecordFromArray` row pattern). |
| Field metadata / structure | `Vtiger_RecordStructure_Model::getInstanceForModule($m, $mode)` (per-view field set) + `Vtiger_Field_Model` flags. |
| Picklist values (role-aware) | `Vtiger_Field_Model::getAssignedPicklistValues()` / module `getPicklistValuesDetails()`. |
| Permissions (module/action/record/field) | `Users_Privileges_Model::isPermitted($module,$action,$recordId)`, `Vtiger_Record_Model::isEditable()/isDeletable()`, `$field->getPermissions()`. NEVER hand-roll ACL SQL. |
| Login / credentials | `Users::doLogin()` (covers crypt_type/LDAP/AD) — see `V1_AuthController` precedent. |
| Custom view / saved filter | `CustomView_Record_Model` / `CustomView::getAllFilterByModule()`. |

Use Serena (`find_symbol`, `get_symbols_overview`, `search_for_pattern`) or Grep on `modules/<Module>/actions/` and `modules/Vtiger/models/` to find these before coding.

**Step 2 — reuse the model/helper, adapt only the edges.** Take the native model methods and event pipeline as-is; in the V1 layer add ONLY: JSON (de)serialization (`V1_FieldSerializer`), the envelope (`V1_Response`), cursor pagination, and a pre-save validation gate. Do not duplicate save/delete/permission/query logic that the model already encapsulates.

**Step 3 — what NOT to copy from Action controllers.** Action controllers are request/response (HTML/redirect/`Vtiger_Response`) bound and assume web-session + CSRF (`__vtrftk`) state. Reuse their *business logic* (the model calls, the field-mapping, the validation order), NOT their transport (no `$response->emit()`, no redirects, no Smarty, no session reliance) — the V1 layer is stateless JWT + `V1_Response`. When a native method is IonCube-encoded or session-coupled, replicate the minimal model calls it makes rather than invoking it directly.

**Trigger for `code-reviewer`:** a V1 handler that issues raw `pquery` INSERT/UPDATE/DELETE on entity tables, hand-rolls ACL/permission SQL, re-implements picklist/field-structure resolution, or builds list SQL without `QueryGenerator` — flag as "reinvents existing logic; reuse native model/helper" (DRY / Framework-First violation).



## Config Initialization

```php
protected function initConfigs() {
    $config = Config_Model::getInstance()->getChannelConfig($this->channel);
    $this->host         = (string) $config['host'];
    $this->apiVersion   = (string) $config['api_version'];
    $this->clientId     = (string) $config['client_id'];
    $this->clientSecret = (string) $config['client_secret'];
}
```

## Token Renewal Logic

Standard refresh flow — check expiry threshold, call refresh API, persist new token + new expiry, update connector status:

```php
public function renewAccessToken(Record_Model $record): string {
    $accessToken   = $record->getAccessToken();
    $expiredDate   = (string) $record->get('token_expired_date');
    $daysThreshold = 2;

    // Refresh if missing expiry OR within threshold of expiring
    if (empty($expiredDate) || strtotime($expiredDate) < strtotime("+{$daysThreshold} days")) {
        $result = $this->makePostRequest($url, __FUNCTION__, $params);

        if (!empty($result['access_token'])) {
            $expiresIn   = (int) $result['expires_in'];
            $accessToken = (string) $result['access_token'];

            $record->set('access_token', $accessToken);
            $record->set('token_expired_date', date('Y-m-d H:i:s', strtotime("+ $expiresIn seconds")));
            $record->set('status', 'valid');
        }
        else {
            $record->set('status', 'expired');
        }

        $record->set('mode', 'edit');
        $record->save();
    }

    return $accessToken;
}
```

**Key rules:**

- Always cast incoming token / expiry values: `(string)`, `(int)`
- Set `status = 'valid'` on success, `'expired'` on failure
- Never log raw access tokens — see `## Logging` below
- Use `__FUNCTION__` when calling `makePostRequest` so logs identify the caller

## Type Casting (Security — REQUIRED)

```php
$this->host = (string) $config['host'];
$this->clientId = (string) $config['client_id'];
$this->expiresIn = (int) $token['expires_in'];
```

## Logging

Use `saveLog()` or `LoggerManager::getLogger($channel)` for inbound/outbound traces. Never log raw access tokens / secrets.

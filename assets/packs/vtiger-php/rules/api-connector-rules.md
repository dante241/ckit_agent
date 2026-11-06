---
description: "Legacy API connector/integration (include/Webservice, modules/CP*Integration): config init, token renewal, logging. V1 /api/v1 ở api-v1.md. Auto-load (TTSR) khi sửa file khớp."
scope: "tool:edit(include/Webservice/**/*.php), tool:write(include/Webservice/**/*.php), tool:edit(modules/CP*Integration/**/*.php), tool:write(modules/CP*Integration/**/*.php)"
condition: ".*"
---

# API Connector Patterns

V1 `/api/v1` transport/doc/reuse rules: `api-v1.md` (fires on `include/Webservice/V1/**`).

## Config Initialization

Cast every config value when reading it into connector properties. Example: `modules/CPSocialAdsIntegration/BaseConnector.php` `initConfigs()`.

## Token Renewal

Check expiry threshold → call refresh API → persist new token + new expiry → update connector status. Example: `modules/CPSocialAdsIntegration/connectors/FacebookAds.php` `renewAccessToken()`.

- Always cast incoming token / expiry values: `(string)`, `(int)`
- Set the token status field to `'valid'` on success, `'expired'` on failure
- Never log raw access tokens — see `## Logging` below
- Pass `__FUNCTION__` to `makeGetRequest`/`makePostRequest` so logs identify the caller

## Logging

Use `saveLog()` or `LoggerManager::getLogger($channel)` for inbound/outbound traces. Never log raw access tokens / secrets.

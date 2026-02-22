# OA-WEBPARITY-082 Retired Chat Header Cleanup

Date: 2026-02-22  
Status: pass  
Issue: OA-WEBPARITY-082

## Deliverables

1. Removed retired legacy chat headers from compatibility JSON response helper:
   - `apps/openagents.com/service/src/lib.rs`
2. Replaced obsolete tests that asserted retired-mode headers:
   - `apps/openagents.com/service/src/lib.rs`
3. Updated OpenAPI compatibility stream response example to final stream semantics (no retired-mode payload):
   - `apps/openagents.com/service/src/openapi.rs`
   - `apps/openagents.com/service/openapi/openapi.json`

## Behavior After Cleanup

1. Active compatibility stream success responses do not include retired-mode headers.
2. Legacy chat JSON compatibility responses no longer emit `x-oa-legacy-chat-retired` / `x-oa-legacy-chat-canonical`.
3. OpenAPI no longer advertises retired stream response semantics for active stream aliases.

## Verification Executed

```bash
./apps/openagents.com/service/scripts/verify-openapi-json.sh
./apps/openagents.com/scripts/run-full-parity-regression.sh
```

Regression artifact:
- `apps/openagents.com/storage/app/parity-regression/20260222T183316Z/summary.json`

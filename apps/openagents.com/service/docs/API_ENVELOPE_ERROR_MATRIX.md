# API Envelope and Error Code Matrix

Status: active  
Tracking: OA-WEBPARITY-005

Rust control-service now uses shared envelope/error helpers in:

- `apps/openagents.com/service/src/api_envelope.rs`

## Canonical Response Shapes

Success envelope:

```json
{
  "data": { "...": "..." }
}
```

Error envelope:

```json
{
  "message": "Human readable summary",
  "error": {
    "code": "machine_readable_code",
    "message": "Detailed message"
  },
  "errors": {
    "field": ["Validation detail"]
  }
}
```

`errors` is only present for validation-style responses.

## Error Matrix

| Code | HTTP Status | Laravel-equivalent behavior |
| --- | ---: | --- |
| `invalid_request` | 422 | validation failure |
| `unauthorized` | 401 | unauthenticated |
| `forbidden` | 403 | unauthorized action |
| `not_found` | 404 | missing route/resource |
| `conflict` | 409 | conflict |
| `invalid_scope` | 422 | invalid scope request |
| `service_unavailable` | 503 | upstream/provider unavailable |
| `sync_token_unavailable` | 503 | sync token service unavailable |
| `static_asset_error` | 500 | static asset read/serve failure |
| `legacy_route_unavailable` | 503 | legacy split target unavailable |
| `internal_error` | 500 | internal server error |

The matrix is source-of-truth in code via `api_error_matrix()`.

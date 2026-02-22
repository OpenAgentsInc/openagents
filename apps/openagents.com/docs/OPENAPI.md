# OpenAPI in Rust Control-Service Era

Status: active

OpenAPI publication is now Rust-native for the control service.

Canonical sources:

- Route-contract + OpenAPI generator module:
  - `apps/openagents.com/service/src/openapi.rs`
- Runtime document endpoint:
  - `GET /openapi.json`
- Generated snapshot:
  - `apps/openagents.com/service/openapi/openapi.json`
- Snapshot generator:
  - `apps/openagents.com/service/scripts/generate-openapi-json.sh`
- Snapshot verifier:
  - `apps/openagents.com/service/scripts/verify-openapi-json.sh`

CI enforcement:

- `.github/workflows/web-openapi-rust.yml`
- Validates OpenAPI-generation tests and snapshot parity.

Historical Laravel documentation (archived):

- `apps/openagents.com/docs/archived/legacy-laravel-deploy/OPENAPI.md`

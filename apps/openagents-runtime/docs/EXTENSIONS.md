# Extension Manifest Validation

Runtime extension activation uses a two-stage validation path:

1. Base extension manifest contract:
   - `docs/protocol/extensions/extension-manifest.schema.v1.json`
   - Validator: `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/manifest_validator.ex`
2. Tool-pack specialization (for comms):
   - `docs/protocol/comms/integration-manifest.schema.v1.json`
   - `docs/protocol/comms/tool-pack-contract.v1.json`
   - Validator: `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/comms_manifest_validator.ex`

Activation entrypoint:

- `apps/openagents-runtime/lib/openagents_runtime/tools/extensions/manifest_registry.ex`

Validation outcomes are surfaced to operators/control-plane integration points via telemetry:

- `[:openagents_runtime, :tools, :extensions, :manifest_validation]`
- metadata includes:
  - `outcome` (`accepted` or `rejected`)
  - `tool_pack`
  - `extension_id`
  - `reason_code`
  - `error_count`

Validation failures are machine-readable and reason-coded:

- `reason_code: manifest_validation.invalid_schema`

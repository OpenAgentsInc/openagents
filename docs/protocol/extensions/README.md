# Extensions Protocol

Canonical runtime extension manifest contracts:

- `docs/protocol/extensions/extension-manifest.schema.v1.json`

Tool-pack specific specializations:

- `docs/protocol/comms/integration-manifest.schema.v1.json`
- `docs/protocol/comms/tool-pack-contract.v1.json`
- `docs/protocol/coding/integration-manifest.schema.v1.json`
- `docs/protocol/coding/tool-pack-contract.v1.json`

Runtime validation path:

- Base validator: `apps/runtime/lib/openagents_runtime/tools/extensions/manifest_validator.ex`
- Activation registry: `apps/runtime/lib/openagents_runtime/tools/extensions/manifest_registry.ex`

Validation errors are machine-readable and use runtime reason taxonomy:

- `reason_code: manifest_validation.invalid_schema`

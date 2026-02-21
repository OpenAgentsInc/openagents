# Coding Tool Pack Contracts

Versioned coding contracts for OpenAgents runtime coding operations and Laravel control-plane integration authoring.

Artifacts:
- `docs/protocol/extensions/extension-manifest.schema.v1.json` (base extension manifest contract)
- `docs/protocol/coding/tool-pack-contract.v1.json`
- `docs/protocol/coding/integration-manifest.schema.v1.json`

Runtime implementation surfaces:
- `apps/runtime/lib/openagents_runtime/tools/coding/kernel.ex`
- `apps/runtime/lib/openagents_runtime/tools/coding/providers/github_adapter.ex`
- `apps/runtime/lib/openagents_runtime/tools/extensions/coding_manifest_validator.ex`

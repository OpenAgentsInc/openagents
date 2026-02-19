# Comms Ownership Split (Runtime vs Laravel)

## Runtime (canonical execution)

- Validate comms integration manifests before execution.
- Execute send-side effects (`comms.send`) through provider adapters.
- Enforce consent/suppression/policy gate decisions.
- Emit receipt-visible outcomes and reason codes.
- Record normalized delivery events into canonical runtime event log.

## Laravel (control-plane authored state)

- Author integration records and manifest payloads from settings UI.
- Store provider keys encrypted and expose runtime-scoped secret retrieval.
- Receive provider webhooks, verify signatures, normalize payloads, and forward to runtime ingestion.
- Provide operator/admin views for integration health and audit trails.

## Shared Contract Boundary

- Manifest shape: `docs/protocol/comms/integration-manifest.schema.v1.json`
- Tool pack contract: `docs/protocol/comms/tool-pack-contract.v1.json`
- Reason taxonomy: `docs/protocol/reasons/runtime-policy-reason-codes.v1.json`

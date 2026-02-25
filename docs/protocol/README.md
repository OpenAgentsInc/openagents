# Protocol Docs

This directory is the Rust-era protocol system-of-record.

## Canonical Entry Points

- `docs/protocol/PROTOCOL_SURFACE.md`
- `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`
- `docs/protocol/OA_SYNC_WS_MAPPING.md`
- `docs/protocol/control-auth-session-v1.md`
- `docs/protocol/codex-worker-events-v1.md`
- `docs/protocol/codex-worker-control-v1.md`
- `docs/protocol/codex-khala-vercel-sse-mapping-v1.md`
- `docs/protocol/client-telemetry-v1.md`
- `docs/protocol/nexus-nostr-bridge-v1.md`
- `docs/protocol/marketplace-commerce-grammar-v1.md`
- `docs/protocol/lightning-wallet-executor-receipt-v1.md`

## Fixtures and Taxonomy

- `docs/protocol/fixtures/`
- `docs/protocol/reasons/runtime-policy-reason-codes.v1.json`

## Archived (Historical)

- `docs/protocol/archived/onyx-integration-contract-v1.md`

## Proto Authority

- `proto/openagents/control/v1/auth.proto`
- `proto/openagents/runtime/v1/orchestration.proto`
- `proto/openagents/runtime/v1/commerce.proto`
- `proto/openagents/codex/v1/events.proto`
- `proto/openagents/codex/v1/workers.proto`
- `proto/openagents/codex/v1/auth.proto`
- `proto/openagents/sync/v1/client_telemetry.proto`
- `proto/openagents/sync/v2/sync.proto`
- `proto/openagents/sync/v2/errors.proto`
- `proto/openagents/sync/v2/streams.proto`
- `proto/openagents/sync/v2/checkpoints.proto`
- `proto/openagents/sync/v2/auth.proto`
- `proto/openagents/lightning/v1/control_plane.proto`
- `proto/openagents/lightning/v1/wallet_executor.proto`

Generated Rust wire crate:

- `crates/openagents-proto/`

## Related

- `docs/adr/`
- `docs/core/GLOSSARY.md`
- `proto/README.md`
- `proto/PACKAGE_MAP.md`

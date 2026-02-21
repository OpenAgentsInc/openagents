# Rust-Era Proto Package Map (OA-RUST-007)

Status: Active  
Last updated: 2026-02-21  
Scope: cross-process and client/server contracts only

## Canonical Package Ownership

| Package | Domain | Owner lane | Contains | Does not contain |
| --- | --- | --- | --- | --- |
| `openagents.control.v1` | Control-plane authority | `owner:openagents.com` | Auth/session/device, org membership, policy/comms intents, sync-token minting contracts | Runtime run state, WS replay internals |
| `openagents.runtime.v1` | Runtime authority | `owner:runtime` | Run lifecycle, worker lifecycle, authority events, receipts/replay artifacts | Identity/session authority records |
| `openagents.codex.v1` | Codex surface contracts | `owner:runtime` (with `owner:desktop` + `owner:ios` consumers) | Codex worker envelopes, sandbox bindings, Codex auth/worker projections consumed by web/desktop/iOS | Generic runtime-agnostic auth/session contracts |
| `openagents.sync.v1` | Khala transport/replay | `owner:khala` | WS envelope, topic/cursor semantics, replay/bootstrap errors, subscription contracts | Authority mutation payloads |
| `openagents.lightning.v1` | Lightning control/payments | `owner:infra` | Lightning policy/executor control-plane contract payloads | Codex/runtime generic orchestration |
| `openagents.protocol.v1` | Transitional legacy namespace | `owner:contracts-docs` | Migration shims only for contracts not yet moved to canonical package | Any new contract additions |

## Placement Rules

1. Every new cross-boundary message must be added in its canonical package domain above.
2. New contracts are forbidden in `openagents.protocol.v1`.
3. `openagents.sync.v1` carries delivery/replay protocol only; authority writes remain control/runtime HTTP contracts.
4. Domain auth/session contracts live in `openagents.control.v1`; Codex runtime auth state envelopes live in `openagents.codex.v1`.
5. Runtime authority events/receipts are `openagents.runtime.v1`, even when streamed via Khala.

## Versioning and Upgrade Strategy

1. `v1` packages are additive-only.
2. Breaking changes require a versioned package bump (`v2`) and migration notes.
3. Namespace split from `openagents.protocol.v1` follows this sequence:
   - add canonical message in target package with stable field IDs,
   - dual-read in consumers during transition,
   - remove legacy usage after all consumers migrate,
   - reserve removed legacy field/message identifiers.
4. Buf compatibility gates (`buf lint`, `buf breaking`) remain mandatory before merge.

## Current Tree Audit and Canonical Target Home

| Current file | Current package | Canonical package home | Owner lane | Follow-on issue |
| --- | --- | --- | --- | --- |
| `proto/openagents/control/v1/auth.proto` | `openagents.control.v1` | `openagents.control.v1` | `owner:openagents.com` | OA-RUST-010 |
| `proto/openagents/runtime/v1/orchestration.proto` | `openagents.runtime.v1` | `openagents.runtime.v1` | `owner:runtime` | OA-RUST-009 |
| `proto/openagents/codex/v1/sandbox.proto` | `openagents.codex.v1` | `openagents.codex.v1` | `owner:runtime` | OA-RUST-011 |
| `proto/openagents/codex/v1/events.proto` | `openagents.codex.v1` | `openagents.codex.v1` | `owner:runtime` | OA-RUST-011 |
| `proto/openagents/codex/v1/workers.proto` | `openagents.codex.v1` | `openagents.codex.v1` | `owner:runtime` | OA-RUST-011 |
| `proto/openagents/codex/v1/auth.proto` | `openagents.codex.v1` | `openagents.codex.v1` | `owner:runtime` | OA-RUST-011 |
| `proto/openagents/protocol/v1/codex_sandbox.proto` | `openagents.protocol.v1` | `openagents.codex.v1` | `owner:contracts-docs` | transitional legacy shim; retire after codex.v1 cutover |
| `proto/openagents/protocol/v1/codex_events.proto` | `openagents.protocol.v1` | `openagents.codex.v1` | `owner:contracts-docs` | transitional legacy shim; retire after codex.v1 cutover |
| `proto/openagents/protocol/v1/codex_workers.proto` | `openagents.protocol.v1` | `openagents.codex.v1` | `owner:contracts-docs` | transitional legacy shim; retire after codex.v1 cutover |
| `proto/openagents/protocol/v1/codex_auth.proto` | `openagents.protocol.v1` | `openagents.codex.v1` | `owner:contracts-docs` | transitional legacy shim; retire after codex.v1 cutover |
| `proto/openagents/protocol/v1/events.proto` | `openagents.protocol.v1` | `openagents.runtime.v1` | `owner:runtime` | transitional legacy; retire after runtime.v1 consumer cutover |
| `proto/openagents/protocol/v1/receipts.proto` | `openagents.protocol.v1` | `openagents.runtime.v1` | `owner:runtime` | transitional legacy; retire after runtime.v1 consumer cutover |
| `proto/openagents/protocol/v1/comms.proto` | `openagents.protocol.v1` | `openagents.control.v1` | `owner:openagents.com` | transitional legacy; retire after control.v1 cutover |
| `proto/openagents/protocol/v1/reasons.proto` | `openagents.protocol.v1` | `openagents.runtime.v1` (runtime reason set), referenced by control/codex as needed until split complete | `owner:contracts-docs` | OA-RUST-009 / OA-RUST-010 / OA-RUST-011 |
| `proto/openagents/sync/v1/topics.proto` | `openagents.sync.v1` | `openagents.sync.v1` | `owner:khala` | OA-RUST-008 |
| `proto/openagents/sync/v1/errors.proto` | `openagents.sync.v1` | `openagents.sync.v1` | `owner:khala` | OA-RUST-008 |
| `proto/openagents/sync/v1/sync.proto` | `openagents.sync.v1` | `openagents.sync.v1` | `owner:khala` | OA-RUST-008 |
| `proto/openagents/sync/v1/client_telemetry.proto` | `openagents.sync.v1` | `openagents.sync.v1` | `owner:khala` | OA-RUST-093 |
| `proto/openagents/lightning/v1/control_plane.proto` | `openagents.lightning.v1` | `openagents.lightning.v1` | `owner:infra` | OA-RUST-095 / OA-RUST-101 / OA-RUST-102 |

## Verification

```bash
buf lint
```

## References

- `proto/README.md`
- `docs/protocol/README.md`
- `docs/ARCHITECTURE-RUST.md`
- `docs/ARCHITECTURE-RUST-ROADMAP.md`

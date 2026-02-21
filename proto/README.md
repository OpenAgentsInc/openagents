# OpenAgents Layer-0 Proto Contracts

`proto/` is the canonical Layer-0 contract source for shared runtime/control-plane/client protocol semantics.

## Decision Lock (2026-02-21)

Proto remains the source of truth for all boundary-crossing contracts, including the Rust-only endstate.

Required policy:

1. Cross-process and client/server contracts are proto-first.
2. Rust wire types are generated from proto.
3. Rust-native types are domain-layer only and must map to/from proto types explicitly.
4. Serde JSON types are never the authority contract (debug and interoperability use only).

Recommended layering:

1. Wire layer (`proto` generated types).
2. Domain layer (Rust invariants/state machine types).
3. Explicit conversion boundary (`TryFrom`/`From` mappings).

Khala-specific requirement:

- Use an explicit envelope model with stable fields (topic, sequence, kind, payload bytes, schema version) to preserve replay and compatibility semantics.

## Versioning Policy

- Additive-only changes in-place for v1 packages:
  - add new messages, enums, fields, or enum values
  - reserve field numbers/names when removing fields
- Breaking changes require a package version bump (`v2` namespace).
- Buf `breaking` check is required in CI against `main`.

## Package Layout

Rust-era package ownership and placement rules are defined in:

- `proto/PACKAGE_MAP.md`

Current files:

- `proto/openagents/control/v1/auth.proto`
- `proto/openagents/runtime/v1/orchestration.proto`
- `proto/openagents/protocol/v1/reasons.proto`
- `proto/openagents/protocol/v1/events.proto`
- `proto/openagents/protocol/v1/receipts.proto`
- `proto/openagents/protocol/v1/comms.proto`
- `proto/openagents/protocol/v1/codex_sandbox.proto`
- `proto/openagents/protocol/v1/codex_events.proto`
- `proto/openagents/protocol/v1/codex_workers.proto`
- `proto/openagents/protocol/v1/codex_auth.proto`
- `proto/openagents/sync/v1/topics.proto`
- `proto/openagents/sync/v1/errors.proto`
- `proto/openagents/sync/v1/sync.proto`
- `proto/openagents/lightning/v1/control_plane.proto`

`openagents.protocol.v1` is transitional legacy namespace only. New contracts must be placed using `proto/PACKAGE_MAP.md`.

## Codegen

Initial generation targets configured in `buf.gen.yaml`:
- TypeScript (`generated/ts`)
- PHP (`generated/php`)
- Rust (required by policy; generation path to remain configured in lockstep with Rust service/client rollout)

Policy:
- Generated artifacts under `generated/` are **not checked into git**.
- CI enforces generation viability for TS/PHP targets using `scripts/verify-proto-generate.sh`.

Local verification command (canonical):

```bash
./scripts/verify-proto-generate.sh
```

Optional manual local generation (for inspection only):

```bash
buf generate
```

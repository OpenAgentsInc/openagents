# OpenAgents Layer-0 Proto Contracts

`proto/` is the canonical Layer-0 contract source for shared runtime/control-plane/client protocol semantics.

## Versioning Policy

- Additive-only changes in-place for v1 packages:
  - add new messages, enums, fields, or enum values
  - reserve field numbers/names when removing fields
- Breaking changes require a package version bump (`v2` namespace).
- Buf `breaking` check is required in CI against `main`.

## Package Layout

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

## Codegen

Initial generation targets configured in `buf.gen.yaml`:
- TypeScript (`generated/ts`)
- PHP (`generated/php`)

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

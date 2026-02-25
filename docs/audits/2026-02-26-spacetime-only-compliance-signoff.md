# 2026-02-26 Spacetime-Only Compliance Signoff

Date: 2026-02-26
Status: signed off
Owner lanes: Runtime, Desktop, Control, Protocol, Docs

## Scope

Final signoff for Spacetime-only retained sync transport posture:

1. runtime publish path uses real Spacetime client writes in retained runtime bootstrap,
2. desktop transport/parser use Spacetime subscribe semantics (no Phoenix frame lane),
3. control exposes canonical sync token route only,
4. active docs/runbooks align with implemented behavior,
5. retained-surface regression guards exist and are executable.

## Retained-Surface Symbol Guard

Command:

```bash
./scripts/spacetime/verify-spacetime-only-symbols.sh
```

Guard checks retained runtime/desktop/control/docs surfaces for blocked legacy transport symbols:

1. `/sync/socket/websocket`
2. `/api/spacetime/token`
3. `/api/v1/sync/token`
4. `/api/v1/spacetime/token`
5. `phx_join`
6. `phx_reply`
7. `sync:update_batch`

Result: pass.

## Runtime + Desktop E2E Gate

Command:

```bash
./scripts/spacetime/runtime-desktop-e2e.sh
```

Gate components:

1. runtime publisher idempotency + sequence conflict + durable outbox behavior,
2. retired runtime route guard remains 404,
3. desktop parser rejects legacy frame protocol,
4. desktop stream extraction + handshake dedupe behavior remains deterministic.

Result: pass.

## Maincloud Two-Client Handshake Gate

Command:

```bash
./scripts/spacetime/maincloud-handshake-smoke.sh --db "$OA_SPACETIME_DEV_DATABASE"
```

Pass condition:

1. active count increases by at least +2 during concurrent subscriptions,
2. final count returns to baseline after both subscriptions close.

Result: pass in active dev Maincloud lane.

## Chaos Suite Evidence

Command:

```bash
./scripts/spacetime/run-chaos-drills.sh
```

Result: pass.

Artifact directory from verification run:

1. `output/chaos/spacetime/20260225T204011Z/`

## Core Verification Commands Executed

1. `cargo check -p openagents-proto -p autopilot-spacetime -p autopilot-desktop -p openagents-runtime-service -p openagents-control-service`
2. `cargo test -p autopilot-spacetime --lib`
3. `cargo test -p openagents-proto --lib`
4. `cargo test -p openagents-runtime-service spacetime_publisher::tests -- --nocapture`
5. `cargo test -p autopilot-desktop runtime_codex_proto::tests -- --nocapture`
6. `cargo test -p openagents-control-service sync_token -- --nocapture`

Result: pass.

## Active-Docs Alignment Completed

Updated canonical docs/runbooks to match code:

1. `docs/core/ARCHITECTURE.md`
2. `docs/protocol/SPACETIME_SYNC_TRANSPORT_MAPPING.md`
3. `docs/sync/README.md`
4. `docs/sync/SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`
5. `docs/sync/RUNTIME_CODEX_CUTOVER_RUNBOOK.md`
6. `docs/sync/SPACETIME_RUNTIME_PUBLISH_MIRROR.md`
7. `docs/sync/SPACETIME_OBSERVABILITY_AND_ALERTS.md`
8. `apps/openagents.com/README.md`
9. `apps/openagents.com/deploy/smoke-control.sh`

## Conclusion

Retained OpenAgents sync surfaces are now Spacetime-only by implementation, docs, and automated regression guard posture.

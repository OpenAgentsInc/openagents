# 2026-02-25 Maincloud Bootstrap and Handshake Audit

Date: 2026-02-25
Status: active
Owner lanes: Runtime, Desktop, Control, Infra, Docs

## Scope

Audit what is now concretely set up for Spacetime Maincloud, what was verified live, and what remains to reach full OpenAgents Spacetime integration.

## What Is Now Set Up

### 1) Live Maincloud database is active

Database identity:

1. `c2003d5910b79ed770057c5ccff7af287ba3463569b04e9d827b5cdc949294f9`

The module currently published includes:

1. `person` table (baseline bootstrap table)
2. `active_connection` table keyed by `connection_id`
3. lifecycle reducers:
   1. `client_connected` (insert active connection)
   2. `client_disconnected` (remove active connection)

### 2) In-repo docs and config scaffolding are now present

1. `docs/sync/SPACETIME_MAINCLOUD_MANAGED_DEPLOYMENT.md`
2. `docs/sync/SPACETIME_MAINCLOUD_HANDSHAKE_SMOKE_TEST.md`
3. `docs/sync/examples/maincloud-dev.envvars` (includes current dev database identity)
4. `AGENTS.md` now points agents at the managed deployment + handshake docs

## Live Verification Evidence

### 1) Publish and schema verification

Verified with:

1. `spacetime publish ... --server maincloud`
2. `spacetime describe <db> --server maincloud --json`
3. `spacetime logs <db> --server maincloud --num-lines 60`

Observed:

1. Module updates succeeded.
2. `active_connection` table was created.
3. Lifecycle reducer logs show connect/disconnect events with identity + connection ID.

### 2) Immediate two-client handshake test (connected users)

Executed two concurrent anonymous subscriptions against `active_connection` and queried counts during and after:

1. During test: `SELECT COUNT(*) FROM active_connection` returned `2`.
2. After both clients disconnected: count returned to `0`.

Conclusion:

1. Basic shared-database handshake visibility is working now.
2. Connection lifecycle cleanup behavior is working for this bootstrap lane.

## Key Constraints Observed

1. In this environment, Spacetime `2.0.1` module publish required `rustc 1.93.0`.
2. First publish path involved a 1.x -> 2.x upgrade acknowledgement.

## Gap Analysis: Current Bootstrap vs Full OpenAgents Integration

### Current state (good for immediate validation)

1. Real Maincloud database exists and is reachable.
2. Reducers and lifecycle hooks execute on a real host.
3. Client handshake/presence signal can be measured immediately.

### Remaining for full integration

1. Runtime publish path:
   1. Move from current in-process/mock-style publisher behavior to real reducer calls against configured Spacetime host per environment.
2. Desktop integration:
   1. Replace remaining legacy websocket semantics with Spacetime subscribe + reconnect/resume behavior in retained desktop paths.
3. Control token posture:
   1. Ensure control-issued token semantics and env values are consistent with this Maincloud lane for dev/staging.
4. Contract convergence:
   1. Keep stream/checkpoint/replay contracts aligned with `docs/plans/spacetimedb-full-integration.md` and retained protocol docs.
5. Operational hardening:
   1. Add smoke test automation that runs the two-client handshake check in CI/local gate scripts.

## Recommended Buildout Order From This Point

1. Phase A: Lock dev lane
   1. Standardize dev env loading from `docs/sync/examples/maincloud-dev.envvars` shape.
   2. Add a script wrapper for one-command handshake smoke test.
2. Phase B: Desktop live path
   1. Route desktop sync connect/subscription to Spacetime path in dev mode.
   2. Surface connected-session count in desktop diagnostics pane.
3. Phase C: Runtime/control convergence
   1. Switch runtime projection writes to live reducer calls.
   2. Validate token mint/refresh and claim scope in end-to-end tests.
4. Phase D: Promotion gates
   1. Run replay/resume + chaos drill gates against hosted Spacetime endpoints.
   2. Publish staged evidence and promote.

## Immediate Operator Outcome

If the goal is to test handshake between two clients right now with visible connected count, that capability is available and verified today via:

1. `docs/sync/SPACETIME_MAINCLOUD_HANDSHAKE_SMOKE_TEST.md`

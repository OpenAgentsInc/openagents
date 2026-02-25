# Spacetime Maincloud Managed Deployment Playbook

Date: 2026-02-25
Status: active planning + operator guide
Owner lanes: Infra, Runtime, Control, Desktop

## Purpose

Define the managed-hosting deployment lane for SpacetimeDB using Maincloud, and give a concrete operator flow for a newly created database that currently has no tables/module.

This complements `docs/sync/SPACETIME_GCLOUD_DEPLOYMENT_CONSIDERATIONS.md` (self-host/BYO-cloud lane).

Companion docs:

1. `docs/sync/SPACETIME_MAINCLOUD_HANDSHAKE_SMOKE_TEST.md`
2. `docs/sync/examples/maincloud-dev.envvars`
3. `scripts/spacetime/maincloud-handshake-smoke.sh`

## What Maincloud Gives Us

Maincloud is SpacetimeDB managed hosting:

1. Managed infrastructure and scaling.
2. Database lifecycle controls (running/paused via dashboard).
3. Publish/update/delete via CLI against `--server maincloud`.
4. Standard client URI base: `https://maincloud.spacetimedb.com`.

Operational implication:

1. For near-term bring-up, Maincloud removes most infra work.
2. We can focus first on module schema/reducers, auth boundaries, client subscribe behavior, and replay correctness.

## What "No Tables Yet" Means

A database created in the UI has no schema until a module is published to it.

Practical rule:

1. UI creation creates the database container.
2. `spacetime publish <db> --server maincloud` (with a module path) creates schema/reducers and runs `init` reducer logic if defined.

## Immediate Operator Flow For Your Existing Maincloud Database

Use your database name when available (preferred). Database identity is also valid.

1. Authenticate CLI:

```bash
spacetime login
```

2. Sanity check server config:

```bash
spacetime server list
```

3. Use canonical in-repo module path:

```bash
MODULE_PATH="spacetime/modules/autopilot-sync/spacetimedb"
```

4. Publish module to your existing DB:

```bash
spacetime publish <your-db-name> --server maincloud --module-path "$MODULE_PATH"
```

5. Verify schema/log health:

```bash
spacetime describe <your-db-name> --server maincloud --json
spacetime logs <your-db-name> --server maincloud --follow
```

6. For iterative updates:

```bash
spacetime publish <your-db-name> --server maincloud --module-path <module-path>
```

7. Only when intentionally resetting data:

```bash
spacetime publish <your-db-name> --server maincloud --delete-data on-conflict
```

## Live Bootstrap Status (2026-02-25)

Maincloud bootstrap and validation were run against:

1. `OA_SPACETIME_DEV_DATABASE=c2003d5910b79ed770057c5ccff7af287ba3463569b04e9d827b5cdc949294f9`

Verified outcomes:

1. Module publish succeeded on Maincloud.
2. Schema includes baseline table `person`.
3. Schema includes `active_connection` presence table with lifecycle reducers for connect/disconnect.
4. Two-client handshake smoke test passed (`connected_clients=2` during concurrent subscriptions; returns to `0` after disconnect).

Important toolchain note:

1. Current Spacetime 2.0.1 publish path required `rustc 1.93.0` in this environment.
2. If default toolchain is older, publish using:

```bash
RUSTUP_TOOLCHAIN=1.93.0-aarch64-apple-darwin spacetime publish <db> --server maincloud --module-path <module-path> -y
```

## OpenAgents Environment Mapping (Managed Dev Lane)

For a managed dev/staging lane backed by Maincloud:

1. `OA_SPACETIME_<ENV>_HTTP_BASE_URL=https://maincloud.spacetimedb.com`
2. `OA_SPACETIME_<ENV>_DATABASE=<your-db-name>`
3. `OA_SPACETIME_<ENV>_WEBSOCKET_PATH=/v1/database/{database}/subscribe`
4. Keep token issuer/audience/signing keys aligned with `docs/sync/SPACETIME_TOKEN_SCOPE_AND_ROTATION.md`.

Tracked example for current dev bootstrap:

1. `docs/sync/examples/maincloud-dev.envvars`

## BitCraftPublic Patterns Worth Borrowing

Observed in `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer`:

1. CI publish lifecycle by branch/PR database naming, including PR-ephemeral databases.
2. Non-interactive auth with CLI token (`spacetime login --token ...`) for automation.
3. Server safety checks before publish (`spacetime server fingerprint ...`, then `set-default`).
4. Publish retry strategy for schema conflicts:
   1. first normal publish,
   2. then conflict-reset publish if needed.
5. Cleanup automation that deletes PR databases on PR close.
6. Module-level auth gates in reducers (`init`, `client_connected`, role checks) rather than relying only on outer services.
7. Scheduled reducer tables for background loops and periodic work.
8. Explicit idempotency counters for cross-module message processing.

OpenAgents adaptation:

1. Keep deterministic stream/replay invariants as first gate.
2. Use non-destructive publish by default; allow reset only in explicitly disposable environments.
3. Add ephemeral DB naming and cleanup for preview lanes.
4. Preserve control-issued token boundaries even when hosted on Maincloud.

## BitCraftPublic Reference Files Reviewed

1. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/.github/workflows/upload-module.yml`
2. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/.github/workflows/delete-module.yml`
3. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/.github/install-spacetimedb-github-actions.sh`
4. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/packages/game/Cargo.toml`
5. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/packages/game/publish.sh`
6. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/packages/game/src/lib.rs`
7. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/packages/game/src/messages/authentication.rs`
8. `/Users/christopherdavid/code/BitCraftPublic/BitCraftServer/packages/game/src/game/handlers/authentication.rs`

## Security Notes

1. Database identity is address material, not equivalent to a signing secret.
2. Do not commit auth tokens, private keys, or session credentials in docs/repo.
3. Prefer using database name in routine commands and operator docs.

## Decision Guidance: Maincloud vs BYO GCP

Use Maincloud first when:

1. We need fastest path to real Spacetime behavior and client integration.
2. We are validating schema/reducer/auth/subscription semantics.

Use BYO GCP/self-host path when:

1. We need custom networking/compliance boundaries.
2. We need dedicated infra ownership and infra-level tuning.

For BYO track requirements, see `docs/sync/SPACETIME_GCLOUD_DEPLOYMENT_CONSIDERATIONS.md`.

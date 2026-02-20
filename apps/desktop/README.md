# OpenAgents Desktop (EP212 Shell)

This app is the desktop execution boundary for Lightning flows in EP212.

## Purpose

- Desktop hosts wallet/payment execution boundaries.
- `openagents.com` remains the chat/orchestration UI.
- Khala is the runtime/Codex sync lane for migrated desktop status surfaces.

## Current Scope

- Electron app shell with Effect service graph.
- Renderer is fully Effuse-based and mounted in Effuse panes (same pane system family as web).
- Auth linkage strategy: sign in with the same email used on `openagents.com` to map to the same OpenAgents user id.
- Connectivity probes for OpenAgents API and Khala sync lane.
- Background executor loop consumes Lightning tasks and writes deterministic transitions/results.
- L402 execution path uses `@openagentsinc/lightning-effect` + `@openagentsinc/lnd-effect`.
- Operational panes for node runtime/sync, wallet state + spend/balance availability, executor queue failure taxonomy, and payment/invoice history.

## Commands

```bash
cd apps/desktop
npm run dev
npm run typecheck
npm test
npm run lnd:prepare
npm run smoke:lnd-binary -- --json
npm run smoke:lnd-runtime -- --json
npm run smoke:lnd-runtime-real -- --json
npm run smoke:lnd-wallet -- --json
npm run smoke:l402-convex-executor -- --json
npm run smoke:l402-panes -- --json
npm run test:l402-local-node-smoke -- --json
```

`test:l402-local-node-smoke` writes a machine-readable artifact to
`output/l402-local-node-smoke-artifact.json` (override with `--artifact <path>`).

## Environment

Optional environment variables:

- `OA_DESKTOP_OPENAGENTS_BASE_URL` (default: `https://openagents.com`)
- `OA_DESKTOP_KHALA_SYNC_ENABLED` (default: `false`)
- `OA_DESKTOP_KHALA_SYNC_URL` (default derived from `OA_DESKTOP_OPENAGENTS_BASE_URL`)
- `OA_DESKTOP_EXECUTOR_TICK_MS` (default: `2000`)
- `OA_DESKTOP_LND_TARGET` (optional override for bundled target, e.g. `darwin-arm64`)
- `OA_DESKTOP_LND_DEV_BINARY_PATH` (optional local dev override binary path)
- `OA_DESKTOP_LND_DEV_BINARY_SHA256` (optional checksum gate for local dev override)
- `OA_DESKTOP_LND_P2P_LISTEN` (optional `listen` override; default `127.0.0.1:19735` to avoid common `9735` conflicts)
- `OA_LND_TARGETS` (comma-separated target list for `npm run lnd:prepare`)

## Security Notes

- Renderer runs with `contextIsolation: true`, `sandbox: true`, `nodeIntegration: false`.
- Auth token state is memory-only in this phase.
- No Lightning key material is handled in the current implementation.
- LND binary path resolution and checksum checks execute in main process only.

## LND Binary Packaging (Phase N3)

- Pinned artifacts are declared in `lnd/lnd-artifacts.json` (`v0.20.0-beta`).
- `npm run lnd:prepare` downloads release artifacts from Lightning Network releases, verifies archive + binary checksums, and stages binaries under `build-resources/lnd`.
- Forge bundles `build-resources/lnd` via `packagerConfig.extraResource`, so packaged apps receive `resources/lnd/<target>/lnd`.
- Runtime checksum validation uses `resources/lnd/runtime-manifest.json` and fails closed in packaged mode on mismatch.
- Main process boots an Effect-managed LND lifecycle manager (start/stop/restart + health/backoff state machine) and exposes renderer-safe status via preload bridge.
- Main process also runs wallet lifecycle management (init/unlock/restore/backup-ack) with renderer-safe projection and no secret-bearing fields.

### Local Source Override (No hardcoded path)

For local development with a binary built from `/Users/christopherdavid/code/lnd/`, use an environment variable override rather than embedding that path in code:

```bash
OA_DESKTOP_LND_DEV_BINARY_PATH=/Users/christopherdavid/code/lnd/lnd \
npm run dev
```

If you want checksum enforcement in dev mode:

```bash
OA_DESKTOP_LND_DEV_BINARY_PATH=/Users/christopherdavid/code/lnd/lnd \
OA_DESKTOP_LND_DEV_BINARY_SHA256=<sha256> \
npm run dev
```

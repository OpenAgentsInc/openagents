# Deploying the gateway

How a decision-API deployment is packaged, configured, and verified.
Only the lanes documented here are tested; a topology that differs from
these is untested until [`scripts/verify-gateway-install.sh`](../../../scripts/verify-gateway-install.sh)
and a real backend pass on it.

## Lanes

The gateway is the same binary in every lane. What differs is the
registry's bindings, the listener's exposure, and which backend stands
behind each door.

| Lane | What it is | Example config |
| --- | --- | --- |
| Shared | One gateway and one backend serving every tenant's bound door; tenants share capacity under per-tenant quota. | `deploy/gateway/gateway.shared.json` |
| Dedicated | One door bound to one tenant's own backend; the process still multi-tenants the API, but that tenant's calls forward only to its backend. | `deploy/gateway/gateway.dedicated.json` |
| Self-hosted | The operator runs gateway and backend on one host, binds localhost, and keeps the registry local. | `deploy/gateway/gateway.self-hosted.json` |
| On-device | The Lev lane: `laya-serve` or the Lev bridge answers locally; nothing leaves the host. | `deploy/gateway/gateway.on-device.json` |

A lane is a property of the registry manifest — `tenancy`'s
`Binding.lane` — not of the gateway config. The config names which
doors exist and where they forward; the manifest names which lane a
door serves and which tenants may name it.

## Prerequisites

- The pinned Rust toolchain in `rust-toolchain.toml` builds `gateway`,
  `tenant-keys`, and the serving backend (`kev-serve`, `laya-serve`, or
  the Lev bridge helper built by `./scripts/build-lev-bridge.sh`).
- Model weights are fetched out of band — they are not in git. Kev
  weights live under `~/work/kev-artifacts/`, Laya under
  `~/work/laya-artifacts/`; a deployment copies them to its own store
  and pins each door to the artifact digest the backend publishes on
  `GET /v1/models`. The digest is the check: the gateway refuses a call
  whose backend publishes a different identity.
- A TLS terminator (the `deploy/caddy` or `deploy/nginx` shape) fronts
  the public listener. The gateway speaks plain HTTP and expects the
  backend on a private interface — `Door.endpoint` documents the same
  boundary. There is no TLS inside the gateway.

## Secrets

`keys.json` holds digests only; the cleartext `oak_<id>.<secret>`
exists once, on the line `tenant-keys issue` prints. The service needs
no other secret: backend endpoints are localhost URLs, and the
`gateway.env` template is intentionally empty. File permissions are the
boundary — the registry directory is `0750` under the service user.

## Health and readiness

`GET /healthz` is process liveness only. Readiness is per call: the
gateway fetches the backend's card at request time and refuses
`identity_mismatch` or `unavailable` rather than serving a door whose
backend is absent or wrong. A load balancer should gate on `/healthz`;
a caller should treat a refused call, not a health flag, as the door's
real state.

## Backup and restore

The registry directory is the whole state: `registry.json` (manifest
and revisions), `keys.json`, `quota-ledger.jsonl`, `receipts.jsonl`,
`feedback/`, `updates/`, `jobs/`, and `skills/` where configured. Back
it up as a directory copy while the service is stopped — the ledger
holds a lock file while a writer runs. Restore is the reverse: stop,
replace the directory, start. No export or transform step exists.

## Upgrade and rollback

Releases live as immutable directories under `/opt/openagents-gateway/`
with one `current` symlink; `openagents-gateway.service` runs
`current/gateway`. Upgrade: unpack the new release, repoint `current`,
restart. Rollback: repoint `current` to the previous release, restart.
The registry's on-disk formats are versioned and append-only — a
rollback can read everything the newer binary wrote, but a manifest
revision is history, not a downgrade hazard.

## Fresh-install verification

`./scripts/verify-gateway-install.sh` proves the path end to end against
`deploy/gateway/stub-backend.py`: bootstrap a registry, issue a key,
serve `GET /v1/models` and one bounded `POST /v1/systemone` call,
confirm the receipt and quota ledger persisted, restart, restore from a
backup copy, roll back through the `current` symlink, and delete. A run
prints PASS/FAIL per step; the 2026-09-22 run passed all fourteen
checks in seconds. Run it after any host, network, or config change —
a green build proves nothing about a deployment.

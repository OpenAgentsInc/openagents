# SpacetimeDB Admin Runbook

Date: 2026-06-17
Status: live self-hosted OpenAgents world database operator runbook

## Scope

This runbook covers the first self-hosted SpacetimeDB instance for OpenAgents
world-state experiments. The service is a projection and interaction layer for
game/world clients. It is not authority for settlement, payout, training truth,
product promises, receipts, wallet state, private prompts, private repos, or
provider credentials.

## Live Endpoint

```text
Primary: https://spacetime.openagents.com
Fallback: https://spacetime.34.28.177.95.sslip.io
Database/module name: openagents-world
Static IP: 34.28.177.95
```

Use `openagents-world` for the SpacetimeDB database name. The earlier
`openagents_world` spelling is invalid for SpacetimeDB HTTP database routes.
Table names can still use underscores.

## GCP Inventory

```text
Project: openagentsgemini
Region: us-central1
Zone: us-central1-a
Instance: spacetimedb-world-1
Machine: e2-standard-4
Network: oa-lightning
Subnet: oa-lightning-us-central1
Internal IP: 10.42.0.52
External static IP: 34.28.177.95
Static IP resource: spacetimedb-world-ip
Service account: spacetimedb-world@openagentsgemini.iam.gserviceaccount.com
Network tag: spacetimedb-world
HTTP/S firewall rule: oa-allow-spacetimedb-world-http-https
SSH access: IAP TCP forwarding through the existing OpenAgents IAP SSH rule
```

## Runtime Layout

```text
SpacetimeDB root: /stdb
Standalone binary: /stdb/bin/2.6.0/spacetimedb-standalone
CLI binary: /stdb/bin/2.6.0/spacetimedb-cli
System user: spacetimedb
Systemd unit: spacetimedb.service
Listen address: 127.0.0.1:3000
Reverse proxy: nginx
TLS renewal: certbot.timer
```

Do not use `/stdb/spacetime` in operator runbooks. The wrapper exists on the
VM, but its HOME-relative `current` lookup was inconsistent under `sudo`. Use
the versioned binaries above.

## DNS And TLS

`spacetime.openagents.com` has an `A` record pointing at `34.28.177.95`.

The current Let's Encrypt certificate is the existing fallback lineage expanded
to include both names:

```text
Certificate name: spacetime.34.28.177.95.sslip.io
Domains: spacetime.openagents.com, spacetime.34.28.177.95.sslip.io
Full chain: /etc/letsencrypt/live/spacetime.34.28.177.95.sslip.io/fullchain.pem
Private key: /etc/letsencrypt/live/spacetime.34.28.177.95.sslip.io/privkey.pem
```

Check the certificate:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo certbot certificates'
```

Dry-run renewal:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo certbot renew --dry-run'
```

Reissue only if the public hostnames change:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo certbot --nginx -d spacetime.openagents.com -d spacetime.34.28.177.95.sslip.io --non-interactive --agree-tos --register-unsafely-without-email --redirect --expand'
```

## Public HTTP Boundary

Nginx exposes only these public routes:

```text
location ~ ^/v1/database/[^/]+/subscribe$
location /v1/identity
```

`/` and unlisted routes return `403`. Module publishing and admin operations
must happen over IAP SSH against the VM-local server.

Expected public probes:

```bash
curl -sS -o /tmp/spacetime-root.txt -w 'root %{http_code} %{ssl_verify_result} %{remote_ip}\n' \
  https://spacetime.openagents.com/

curl -sS -o /tmp/spacetime-identity.txt -w 'identity %{http_code} %{ssl_verify_result} %{remote_ip}\n' \
  https://spacetime.openagents.com/v1/identity

curl -sS -o /tmp/spacetime-subscribe.txt -w 'subscribe %{http_code} %{ssl_verify_result} %{remote_ip}\n' \
  https://spacetime.openagents.com/v1/database/openagents-world/subscribe
```

Expected output:

```text
root 403 0 34.28.177.95
identity 405 0 34.28.177.95
subscribe 426 0 34.28.177.95
```

Interpretation:

- `403` on `/` means the default public route is denied.
- `405` on `/v1/identity` means Nginx reaches SpacetimeDB, but the probe uses
  the wrong method for identity creation.
- `426` on `/v1/database/openagents-world/subscribe` means SpacetimeDB is
  reachable and the endpoint expects a WebSocket client.
- `ssl_verify_result=0` means the certificate validates.

## SSH And Service Checks

Open an operator shell:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap
```

Check VM identity and IPs:

```bash
gcloud compute instances describe spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --format='table(name,status,machineType.basename(),networkInterfaces[0].networkIP,networkInterfaces[0].accessConfigs[0].natIP,tags.items)'
```

Check services:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo systemctl is-active spacetimedb nginx certbot.timer'
```

Expected service states are three `active` lines.

## Publishing A Module

The source for the first world module lives in the separate top-level app:

```text
apps/openagents-world-spacetimedb
```

Build the SpacetimeDB module locally, then copy the WASM to the VM:

```bash
rustup target add wasm32-unknown-unknown
cargo build --manifest-path apps/openagents-world-spacetimedb/Cargo.toml \
  --target wasm32-unknown-unknown \
  --release
```

```bash
gcloud compute scp \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  apps/openagents-world-spacetimedb/target/wasm32-unknown-unknown/release/openagents_world.wasm \
  spacetimedb-world-1:/tmp/openagents-world.wasm
```

Publish on the VM-local server:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo -u spacetimedb /stdb/bin/2.6.0/spacetimedb-cli publish -s local --bin-path /tmp/openagents-world.wasm --yes=all openagents-world'
```

Issue #5236 published the initial module on 2026-06-17. The created database
identity was:

```text
c2003001c2b7d8f00db5ba85210abfefc8f8dfea110207f85f917d41faa89847
```

Only service identities should call reducers that create or mutate authority
projection rows. Browser clients may subscribe to public rows and call
interaction reducers only after those reducers are explicitly modeled as safe.

## Projecting The Public Tassadar Summary

The bridge source lives next to the module:

```text
apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs
apps/openagents-world-spacetimedb/scripts/tassadar-summary-transform.mjs
```

The bridge reads only the public Worker/D1 projection:

```text
https://openagents.com/api/public/tassadar-run-summary
```

Dry-run the projection locally:

```bash
bun apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs
```

Run the transform coverage:

```bash
bun test apps/openagents-world-spacetimedb/scripts/tassadar-summary-transform.test.mjs
```

Apply the projection through IAP SSH:

```bash
bun apps/openagents-world-spacetimedb/scripts/project-tassadar-summary.mjs --apply-vm
```

The apply path calls VM-local service reducers with the `spacetimedb` user's
local identity. It upserts `training_run`, `run_entity`, `world_edge`,
`proof_ref`, `settlement_ref`, and `projection_cursor`, appends only missing
`world_event` refs, and records `bridge_health` through
`record_bridge_success`.

Issue #5237 projected canonical run `run.tassadar.executor.20260615` on
2026-06-17. The dry-run planned 182 reducer calls:

```text
upsert_training_run: 1
upsert_run_entity: 23
append_world_event: 17
upsert_proof_ref: 123
upsert_world_edge: 16
upsert_settlement_ref: 1
record_projection_cursor: 1
```

The live table counts after apply and replay were:

```text
training_run: 1
run_entity: 16
world_edge: 16
proof_ref: 58
settlement_ref: 1
world_event: 17
projection_cursor: 1
bridge_health: 1
```

`run_entity` and `proof_ref` counts are lower than reducer-call counts because
the bridge de-duplicates public refs through table primary keys. Replaying the
bridge left `world_event` at 17 rows.

Verify live counts:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='set -e; for table in training_run run_entity world_edge proof_ref settlement_ref world_event projection_cursor bridge_health; do printf "\n%s\n" "$table"; sudo -u spacetimedb /stdb/bin/2.6.0/spacetimedb-cli sql -s local openagents-world "SELECT COUNT(*) AS count FROM $table"; done'
```

## Logs

SpacetimeDB logs:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo journalctl -u spacetimedb -n 200 --no-pager'
```

Nginx logs:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo tail -n 200 /var/log/nginx/access.log; sudo tail -n 200 /var/log/nginx/error.log'
```

## Restart And Recovery

Validate and reload Nginx:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo nginx -t && sudo systemctl reload nginx'
```

Restart SpacetimeDB:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo systemctl restart spacetimedb && sudo systemctl is-active spacetimedb'
```

If a bad module publish breaks the world surface, keep the public Nginx
boundary in place, stop the service if needed, and roll back by publishing the
last known good WASM over IAP SSH. Do not expose additional admin routes
publicly to recover.

## Security Notes

- SpacetimeDB rows are public projection data unless a stricter policy is
  modeled and enforced in the module.
- Do not store wallet mnemonics, service tokens, private prompts, private repo
  contents, provider payloads, raw customer data, or raw shell logs in module
  rows.
- User reducers may update presence, viewport, selection, and other explicitly
  modeled interaction state only.
- Service reducers may project already-public run, proof, pylon, settlement,
  and receipt refs from existing OpenAgents authority surfaces.
- The Worker/D1 projection remains the source of truth for `/tassadar` until a
  later invariant update explicitly changes that boundary.

## Hardening Backlog

- Add a GCP uptime check for `https://spacetime.openagents.com/v1/identity`
  expecting `405`.
- Snapshot the boot disk before meaningful world data accumulates.
- Move persistent SpacetimeDB data under a separate persistent disk before
  production gameplay state exists.
- Add log-based alerts for repeated `5xx` proxy errors and SpacetimeDB service
  restarts.
- Create a narrow Cloudflare DNS automation credential if future deploys should
  manage `spacetime.openagents.com` without owner intervention.

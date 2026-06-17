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
Data disk: spacetimedb-world-data-1 (100 GB pd-balanced, /stdb)
Service account: spacetimedb-world@openagentsgemini.iam.gserviceaccount.com
Network tag: spacetimedb-world
HTTP/S firewall rule: oa-allow-spacetimedb-world-http-https
SSH access: IAP TCP forwarding through the existing OpenAgents IAP SSH rule
```

## Runtime Layout

```text
SpacetimeDB root: /stdb
Dedicated data disk: /dev/disk/by-id/google-spacetimedb-world-data-1
Data disk UUID: 21a7f95e-9d61-4a42-a93f-268fc99ee557
Standalone binary: /stdb/bin/2.6.0/spacetimedb-standalone
CLI binary: /stdb/bin/2.6.0/spacetimedb-cli
System user: spacetimedb
Systemd unit: spacetimedb.service
Listen address: 127.0.0.1:3000
Reverse proxy: nginx
TLS renewal: certbot.timer
Telemetry: google-cloud-ops-agent 2.68.0
```

Do not use `/stdb/spacetime` in operator runbooks. The wrapper exists on the
VM, but its HOME-relative `current` lookup was inconsistent under `sudo`. Use
the versioned binaries above.

`/stdb` was moved off the boot disk in issue #5239. The original boot-disk copy
remains on the VM at `/stdb.boot-20260617-pre-data-disk` as a local rollback
aid. The persistent mount is:

```fstab
UUID=21a7f95e-9d61-4a42-a93f-268fc99ee557 /stdb ext4 defaults,nofail 0 2
```

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

## Interaction TTLs And Browser Reducers

The live module accepts browser/user reducers only for explicit interaction
state. These reducers do not create run, proof, settlement, receipt, payout, or
product-claim truth:

```text
join_region
leave_region
set_avatar_position
focus_pylon
clear_pylon_focus
send_local_message
send_pylon_message
send_emote
set_agent_intent
```

The module clamps avatar coordinates to the initial run region:

```text
x: -8..8
y: 0..4
z: -6..6
```

It also enforces a minimum 100 ms interval between accepted
`set_avatar_position` writes and rejects impossible movement over
14 meters/second. The `/tassadar` browser client is more conservative: it sends
position updates at most every 250 ms, sends a 5 second idle keepalive while
connected, and emits pylon attention at most every 1 second.

Chat reducers are also bounded. `send_local_message` and `send_pylon_message`
accept plain text only, cap bodies at 280 characters, and reject writes from the
same avatar inside a 1 second window. The first web HUD sends either a local
8 meter message or a pylon-targeted message, depending on the current pylon
selection.

Cleanup is service-owned. Run `expire_interaction_rows` from an authorized
service identity to remove:

```text
stale non-service avatar positions after 20 seconds
pylon_attention rows after 8 seconds
chat_bubble rows after 8 seconds
local_emote rows after 8 seconds
agent_intent rows after 15 seconds
local_chat_message rows after 90 seconds
```

Pylon-agent and service-agent avatar positions are intentionally preserved by
expiry; guest/human positions are not.

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

Check the data-disk mount:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='findmnt /stdb; df -h /stdb; grep " /stdb " /etc/fstab'
```

Expected mount:

```text
/stdb  /dev/sdb  ext4  rw,relatime
UUID=21a7f95e-9d61-4a42-a93f-268fc99ee557 /stdb ext4 defaults,nofail 0 2
```

## Snapshots And Rollback Points

Issue #5239 created these rollback points before production gameplay state:

```text
Boot disk snapshot: spacetimedb-world-1-boot-20260617-pre-world-hardening
Data disk snapshot: spacetimedb-world-data-1-20260617-post-migration
Storage location: us-central1
```

List them:

```bash
gcloud compute snapshots list \
  --project openagentsgemini \
  --filter='name=("spacetimedb-world-1-boot-20260617-pre-world-hardening","spacetimedb-world-data-1-20260617-post-migration")' \
  --format='table(name,status,storageLocations,sourceDisk.basename(),creationTimestamp)'
```

Create a fresh data-disk snapshot before risky module or VM surgery:

```bash
gcloud compute snapshots create spacetimedb-world-data-1-$(date -u +%Y%m%d-%H%M%S) \
  --project openagentsgemini \
  --source-disk=spacetimedb-world-data-1 \
  --source-disk-zone=us-central1-a \
  --storage-location=us-central1 \
  --labels=service=spacetimedb-world,purpose=world-data
```

Restore from the data-disk snapshot by stopping the VM, creating a replacement
disk from the snapshot, attaching it as `spacetimedb-world-data-1`, and starting
the VM:

```bash
gcloud compute instances stop spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a

gcloud compute instances detach-disk spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --disk=spacetimedb-world-data-1

gcloud compute disks create spacetimedb-world-data-1-restored \
  --project openagentsgemini \
  --zone us-central1-a \
  --source-snapshot=spacetimedb-world-data-1-20260617-post-migration \
  --type=pd-balanced

gcloud compute instances attach-disk spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --disk=spacetimedb-world-data-1-restored \
  --device-name=spacetimedb-world-data-1

gcloud compute instances start spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a
```

If the data-disk mount itself is broken but the boot disk is healthy, the local
pre-migration copy can recover the service quickly:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo systemctl stop spacetimedb; sudo umount /stdb; sudo sed -i.bak "/ \\/stdb /s/^/#/" /etc/fstab; sudo rm -rf /stdb; sudo mv /stdb.boot-20260617-pre-data-disk /stdb; sudo systemctl start spacetimedb; sudo systemctl is-active spacetimedb'
```

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
`proof_ref`, `settlement_ref`, `projection_cursor`, `pylon_station`,
`agent_avatar`, and `avatar_position`, appends only missing `world_event` refs,
and records `bridge_health` through `record_bridge_success`.

The script inserts a `--` separator before reducer arguments because station
coordinates can be negative and the SpacetimeDB CLI otherwise parses values
such as `-2.35` as options.

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

Issue #5262 extended the same bridge on 2026-06-17 to seed pylon stations and
pylon-agent avatars from public leaderboard pylon refs. The dry-run planned 194
reducer calls:

```text
upsert_training_run: 1
upsert_run_entity: 23
append_world_event: 17
upsert_proof_ref: 123
upsert_world_edge: 16
upsert_pylon_station_from_projection: 6
ensure_pylon_agent_avatar: 6
upsert_settlement_ref: 1
record_projection_cursor: 1
```

After publishing the #5261 schema and applying/replaying the #5262 bridge, the
live interaction counts were:

```text
pylon_station: 6
agent_avatar: 6
avatar_position: 6
pylon_attention: 0
local_chat_message: 0
chat_bubble: 0
local_emote: 0
agent_intent: 0
```

`world_event` stayed at 17 after replay, confirming that replay did not create
duplicate projection events.

Verify live counts:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='set -e; for table in training_run run_entity world_edge proof_ref settlement_ref world_event projection_cursor bridge_health; do printf "\n%s\n" "$table"; sudo -u spacetimedb /stdb/bin/2.6.0/spacetimedb-cli sql -s local openagents-world "SELECT COUNT(*) AS count FROM $table"; done'
```

Issue #5261 added public interaction tables for the shared world layer. Verify
those tables too:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='set -e; for table in pylon_station agent_avatar avatar_position pylon_attention local_chat_message chat_bubble local_emote agent_intent; do printf "\n%s\n" "$table"; sudo -u spacetimedb /stdb/bin/2.6.0/spacetimedb-cli sql -s local openagents-world "SELECT COUNT(*) AS count FROM $table"; done'
```

Before issue #5264/#5265 browser interaction is enabled, `pylon_attention`,
`local_chat_message`, `chat_bubble`, `local_emote`, and `agent_intent` may
legitimately be empty.

## Browser Subscription Adapter

Issue #5238 added the feature-flagged `/tassadar` browser adapter. The page
still fetches `/api/public/tassadar-run-summary` first and treats that Worker/D1
summary as the base snapshot. SpacetimeDB is an additive subscription layer
only.

The route enables the adapter by passing public data attributes to
`oa-tassadar-run`:

```text
data-spacetime-world-url="https://spacetime.openagents.com"
data-spacetime-database="openagents-world"
```

Omit those attributes to run the page in Worker-summary-only mode. No private
tokens or service identities are passed to the browser. The browser subscribes
only to these public tables for the canonical run:

```text
training_run
run_entity
world_edge
proof_ref
settlement_ref
world_event
```

Issue #5261 generated bindings for the interaction tables, but the live
browser adapter should not subscribe to them until the corresponding visual
layers land. Issue #5263 is the first planned subscription/rendering step for
`pylon_station`, `agent_avatar`, and `avatar_position`.

The generated TypeScript bindings are checked into:

```text
apps/openagents.com/apps/web/src/scene/spacetimeWorldBindings
```

Regenerate them after module schema changes with a SpacetimeDB CLI matching the
module version:

```bash
~/.local/bin/spacetime generate \
  --lang typescript \
  --out-dir apps/openagents.com/apps/web/src/scene/spacetimeWorldBindings \
  --module-path apps/openagents-world-spacetimedb
```

The generated `index.ts` carries a `// @ts-nocheck` header because
`spacetimedb@2.6.0` generated schema generics currently conflict with this
repo's `exactOptionalPropertyTypes` TypeScript setting. Keep hand-written
adapter code typechecked.

## Logs

Cloud Logging ingestion is enabled through `google-cloud-ops-agent`. The agent
keeps the default syslog pipeline and adds parsed Nginx access/error log
receivers.

Check agent health:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo systemctl is-active google-cloud-ops-agent google-cloud-ops-agent-fluent-bit google-cloud-ops-agent-opentelemetry-collector'
```

Cloud Logging queries:

```bash
gcloud logging read \
  'resource.type="gce_instance" AND resource.labels.instance_id="1980115011797729631" AND log_id("nginx_access")' \
  --project openagentsgemini \
  --freshness=10m \
  --limit=20 \
  --format='table(timestamp,httpRequest.status,httpRequest.requestUrl,httpRequest.userAgent)'

gcloud logging read \
  'resource.type="gce_instance" AND resource.labels.instance_id="1980115011797729631" AND log_id("syslog") AND textPayload:"spacetimedb.service"' \
  --project openagentsgemini \
  --freshness=24h \
  --limit=20 \
  --format='table(timestamp,textPayload)'
```

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

## Monitoring And Alerting

Uptime check:

```text
Name: SpacetimeDB world identity 405
Resource: projects/openagentsgemini/uptimeCheckConfigs/spacetimedb-world-identity-405-ZXbnN1mTEVs
URL: https://spacetime.openagents.com/v1/identity
Expected status: 405
Period: 60s
Regions: usa-iowa, usa-oregon, usa-virginia
TLS validation: enabled
```

Alert policies:

```text
SpacetimeDB world identity uptime failure
projects/openagentsgemini/alertPolicies/10740697432845517968

SpacetimeDB world nginx proxy 5xx spike
projects/openagentsgemini/alertPolicies/10740697432845517398

SpacetimeDB world service restart loop
projects/openagentsgemini/alertPolicies/1795782802307036790
```

Log-based metrics:

```text
spacetime_nginx_proxy_5xx
  resource.type="gce_instance" AND resource.labels.instance_id="1980115011797729631" AND log_id("nginx_access") AND httpRequest.status>=500

spacetime_spacetimedb_service_restart
  resource.type="gce_instance" AND resource.labels.instance_id="1980115011797729631" AND log_id("syslog") AND textPayload=~"spacetimedb\\.service: (Scheduled restart job|Main process exited|Failed with result|Start request repeated too quickly|Start operation timed out)"
```

The project had no configured Cloud Monitoring notification channels when issue
#5239 ran. The policies are enabled and will create Cloud Monitoring incidents;
attach an email, Slack, PagerDuty, or webhook notification channel before
production gameplay state if external paging is required.

Verify monitoring resources:

```bash
gcloud monitoring uptime list-configs \
  --project openagentsgemini \
  --format='table(name,displayName,httpCheck.path,httpCheck.acceptedResponseStatusCodes[0].statusValue,period)'

gcloud logging metrics list \
  --project openagentsgemini \
  --format='table(name,filter)' | grep spacetime_

gcloud monitoring policies list \
  --project openagentsgemini \
  --format='table(name,displayName,enabled,conditions[0].displayName)' | grep 'SpacetimeDB world'
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

- Attach external notification channels to the enabled Cloud Monitoring alert
  policies before production gameplay state requires paging.
- Create a narrow Cloudflare DNS automation credential if future deploys should
  manage `spacetime.openagents.com` without owner intervention.

# SpacetimeDB GCP Deployment Receipt

Date: 2026-06-17
Issue: `OpenAgentsInc/openagents#5228`
Status: deployed on the primary OpenAgents subdomain.

## Live Endpoint

Primary verified HTTPS endpoint:

```text
https://spacetime.openagents.com
```

Fallback DNS-over-IP endpoint:

```text
https://spacetime.34.28.177.95.sslip.io
```

`spacetime.openagents.com` resolves to the static GCP IP
`34.28.177.95`. The original deployment used the `sslip.io` fallback because
the local Cloudflare token available during deployment returned `403` for DNS
mutation. The owner-created `A` record is now live, so the OpenAgents subdomain
is canonical and the fallback remains only an emergency verification endpoint.

## GCP Resources

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
HTTP firewall rule: oa-allow-spacetimedb-world-http-https
```

The VM follows the existing OpenAgents GCP pattern: Ubuntu 24.04, GCE, systemd,
IAP SSH for operator access, and a narrow public HTTP/S boundary.

## Runtime

```text
SpacetimeDB root: /stdb
SpacetimeDB standalone binary: /stdb/bin/2.6.0/spacetimedb-standalone
SpacetimeDB CLI binary: /stdb/bin/2.6.0/spacetimedb-cli
SpacetimeDB service: spacetimedb.service
Listen address: 127.0.0.1:3000
Reverse proxy: nginx
Certificate: Let's Encrypt lineage spacetime.34.28.177.95.sslip.io
Certificate SANs: spacetime.openagents.com, spacetime.34.28.177.95.sslip.io
Certificate path: /etc/letsencrypt/live/spacetime.34.28.177.95.sslip.io/fullchain.pem
Certificate renewal: certbot.timer
```

The `/stdb/spacetime` wrapper was installed but should not be used in runbooks
because its HOME-relative `current` lookup was inconsistent under `sudo`.
Prefer the versioned binaries above.

## Public Boundary

Nginx exposes only:

- `location ~ ^/v1/database/[^/]+/subscribe$`
- `location /v1/identity`

`/` and all unlisted routes are denied publicly by Nginx. Publish/admin work
should happen through IAP SSH and the VM-local SpacetimeDB server.

## Verification

GCP host:

```bash
gcloud compute instances describe spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --format='table(name,status,machineType.basename(),networkInterfaces[0].networkIP,networkInterfaces[0].accessConfigs[0].natIP,tags.items)'
```

Observed:

```text
spacetimedb-world-1  RUNNING  e2-standard-4  10.42.0.52  34.28.177.95  ['spacetimedb-world']
```

Public route probes:

```bash
curl -sS -o /tmp/spacetime-root.txt -w '%{http_code} %{ssl_verify_result} %{remote_ip}\n' \
  https://spacetime.openagents.com/

curl -sS -o /tmp/spacetime-identity.txt -w '%{http_code} %{ssl_verify_result} %{remote_ip}\n' \
  https://spacetime.openagents.com/v1/identity

curl -sS -o /tmp/spacetime-subscribe.txt -w '%{http_code} %{ssl_verify_result} %{remote_ip}\n' \
  https://spacetime.openagents.com/v1/database/openagents-world/subscribe
```

Observed:

```text
/ -> 403 0 34.28.177.95
/v1/identity -> 405 0 34.28.177.95
/v1/database/openagents-world/subscribe -> 426 0 34.28.177.95
```

Interpretation:

- `403` on `/` proves the public default route is blocked.
- `405` on `/v1/identity` proves Nginx reaches the SpacetimeDB identity route
  but the probe used the wrong HTTP method for identity creation.
- `426` on `/v1/database/openagents-world/subscribe` proves the subscribe route
  reaches SpacetimeDB and requires a WebSocket client, as expected.
- `ssl_verify_result=0` confirms the certificate validates.

VM-local service checks:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command='sudo systemctl is-active spacetimedb nginx certbot.timer'
```

Observed services:

```text
spacetimedb.service active
nginx.service active
certbot.timer active
```

SpacetimeDB startup log reported:

```text
spacetimedb-standalone version: 2.6.0
Starting SpacetimeDB listening on 127.0.0.1:3000
PostgreSQL wire protocol server disabled
```

## Module Name

Use this initial database/module name:

```text
openagents-world
```

The originally proposed `openagents_world` was rejected by SpacetimeDB's HTTP
route parser because underscores are invalid in database names.

## Operator Notes

SSH:

```bash
gcloud compute ssh spacetimedb-world-1 \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap
```

Publish from a VM-local WASM path:

```bash
sudo -u spacetimedb /stdb/bin/2.6.0/spacetimedb-cli publish \
  -s local \
  --bin-path /tmp/openagents-world.wasm \
  openagents-world
```

The initial module source now lives at:

```text
apps/openagents-world-spacetimedb
```

Runbook follow-up:

- keep `spacetime.openagents.com` as the canonical public endpoint;
- keep the published `openagents-world` module schema minimal until the bridge
  proves the row contract;
- wire a service bridge from the public Tassadar projection to SpacetimeDB rows;
- add uptime checks and snapshots before meaningful world data accumulates.
- add DNS automation only after a Cloudflare credential with the right zone
  scope is available.

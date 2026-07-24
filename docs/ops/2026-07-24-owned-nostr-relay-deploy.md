# Owned Nostr relay deploy runbook (SARAH-NR-03)

- Date: 2026-07-24
- Class: procedure
- Packet: `SARAH-NR-03`
- OpenAgents issue: [OpenAgentsInc/openagents#9220](https://github.com/OpenAgentsInc/openagents/issues/9220)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](../omega/2026-07-24-sarah-workroom-mvp-spec.md) §23 and §24.4
- Hosting decision: Option A — host from `OpenAgentsInc/nostr-effect`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: operator runbook. Production live proof is owner-gated when secrets or DNS are missing

## 1. Purpose

This runbook tells an operator how to deploy the owned Nostr relay for Sarah.

The relay service lives in the `nostr-effect` repository (Option A).
This monorepo owns the load-proof harness, the deploy index, and the
operator notes. Do not recreate `apps/nostr-relay` here.

## 2. Authority and stack

| Item | Value |
| --- | --- |
| Project | `openagentsgemini` |
| Region | `us-central1` |
| Runtime | Node 24 |
| Package manager | pnpm |
| Host library | `nostr-effect` Node host |
| Store | Cloud SQL Postgres |
| Secrets | Google Secret Manager |
| Public host | `relay.openagents.com` |
| DNS | Cloudflare DNS-only records to Google Cloud |

Rules:

1. Do not use Bun as a runtime, package manager, test runner, or build tool.
2. Do not use Cloudflare Workers, Durable Objects, D1, or R2.
3. Do not put a secret in a repository file, an event, a tag, or a log.
4. Relay acceptance is never an OpenAgents admission.

## 3. Pin and Node entry

Pin `nostr-effect` at commit:

```text
77073343c68f159f3dea80ddbe9e9896b1f052f2
```

Documented package exports (Option A host):

| Export | Role |
| --- | --- |
| `nostr-effect/relay` | Host-agnostic relay core (no Bun, no Cloudflare) |
| `nostr-effect/relay/node` | Node 24 host (`node:http` + `ws`) and `startTestRelay` |
| `nostr-effect/relay/node/sqlite` | `node:sqlite` store for local durable proofs |
| `nostr-effect/relay/node/postgres` | Postgres / Cloud SQL `EventStore` |
| `src/relay/main.ts` | Thin process entry: `startRelay({ port })` |

Local smoke from a `nostr-effect` checkout at the pin:

```sh
node --import tsx scripts/node-relay-smoke.ts
# or
node --import tsx src/relay/main.ts
```

Default port is `PORT` or `8080`.
`startTestRelay` uses `MemoryEventStoreLive` for zero-dependency local tests.
Production must compose `PostgresStoreLive` from
`nostr-effect/relay/node/postgres`.

## 4. Required relay behavior

| Requirement | Carrier |
| --- | --- |
| Capability advertisement | NIP-11 information document |
| Client authentication | NIP-42 challenge and verify |
| Closed membership | policy pipeline plus attested agent keys |
| Protected events | NIP-70 |
| Expiration | NIP-40 |
| Gap reconciliation | NIP-77 negentropy |
| Operator control | NIP-86 management API |
| Ephemeral routing | kinds 20000 to 29999 without storage |

Advertise custom kinds through `supported_extensions`, never through
`supported_nips`.

## 5. Google Cloud resources

Create these resources once. Keep names stable.

### 5.1 Cloud SQL

1. Create a Postgres 17 instance for the relay, or a dedicated database on an
   admitted instance.
2. Create database `nostr_relay_prod` (and `nostr_relay_staging` if needed).
3. Create role `nostr_relay_app` with a password stored only in Secret Manager.
4. Prefer the Cloud SQL Auth Connector. Attach the instance on Cloud Run with
   `--add-cloudsql-instances=openagentsgemini:us-central1:<instance>`.
5. Use the authority-less URL form that the monorepo already uses for
   postgres.js sockets when the connector is active.

Suggested secret ids:

| Secret id | Content |
| --- | --- |
| `nostr-relay-database-url` | Postgres URL or authority-less URL |
| `nostr-relay-pgpassword` | Password only, when the connector uses `PGPASSWORD` |

`PostgresStoreLive` opens the store and creates the `events` table on first
start. Confirm the schema after the first healthy revision.

### 5.2 Cloud Run service

Suggested service name: `openagents-nostr-relay`.

Shape for a WebSocket host:

| Setting | Starting value | Note |
| --- | --- | --- |
| CPU | 2 | Measure under load |
| Memory | 1 Gi | Measure under load |
| Concurrency | 80–200 | Long-lived sockets. Measure under load |
| Min instances | 1 | Avoid cold-start disconnects |
| Max instances | 3 | Raise only after multi-replica notes |
| Timeout | 3600s | Long WebSocket sessions |
| Session affinity | enabled | Keep a client on one instance when multi-replica |
| HTTP/2 | enabled if TLS end-to-end requires it | Verify with NIP-11 and WS |

Container entry (from `nostr-effect` at the pin):

```sh
node --import tsx src/relay/main.ts
```

Or an equivalent `vp pack` Node build that still calls the Node host with
`PostgresStoreLive` and `DATABASE_URL` / connector env.

Mount secrets with `--set-secrets`. Do not bake secrets into the image.

### 5.3 Example deploy command

Run from a clean `nostr-effect` checkout at the pin. Adjust image build to the
repository Dockerfile when present.

```sh
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
export PROJECT=openagentsgemini
export REGION=us-central1
export SERVICE=openagents-nostr-relay
export SQL_INSTANCE=openagentsgemini:us-central1:<instance>

gcloud run deploy "$SERVICE" \
  --project="$PROJECT" \
  --region="$REGION" \
  --platform=managed \
  --allow-unauthenticated \
  --port=8080 \
  --cpu=2 \
  --memory=1Gi \
  --concurrency=100 \
  --min-instances=1 \
  --max-instances=3 \
  --timeout=3600 \
  --session-affinity \
  --add-cloudsql-instances="$SQL_INSTANCE" \
  --set-secrets="DATABASE_URL=nostr-relay-database-url:latest" \
  --image="<artifact-registry-image-at-pin>"
```

Record the Cloud Run URL. DNS for `relay.openagents.com` points at this
service (or at a Google load balancer in front of it).

## 6. DNS for `relay.openagents.com`

Cloudflare remains the DNS authority for `openagents.com`.

1. Create or update a DNS-only (grey-cloud) record for `relay.openagents.com`.
2. Point it at the Cloud Run service host or at the Google load balancer that
   fronts the service.
3. Do not enable the Cloudflare HTTP proxy for this WebSocket service without a
   new owner decision.
4. Do not move nameservers to Cloud DNS without a new owner decision.

Verify:

```sh
dig +short relay.openagents.com
curl -fsS -H 'Accept: application/nostr+json' https://relay.openagents.com/
```

NIP-11 must return JSON. WebSocket upgrade must succeed on
`wss://relay.openagents.com`.

## 7. Load proof

An unproven relay is not admitted as the Sarah record.

### 7.1 Local harness (this monorepo)

```sh
pnpm --dir packages/sarah run test -- src/relay-load-proof/relay-load-proof.test.ts
pnpm --dir packages/sarah run load-proof
```

The CLI starts `nostr-effect` `startTestRelay` when the package is installed.
If that import is not available, it uses the package mock relay.
Write a report file with `LOAD_PROOF_OUT=./relay-load-proof.json`.

Environment:

| Variable | Purpose |
| --- | --- |
| `RELAY_URL` | Measure a remote host (`wss://…`) instead of a local host |
| `LOAD_PROOF_MOCK=1` | Force the package mock host |
| `LOAD_PROOF_DURATION_MS` | Measurement window (default 5000) |
| `LOAD_PROOF_PUBLISHERS` | Concurrent publishers (default 4) |
| `LOAD_PROOF_SUBSCRIBERS` | Concurrent subscribers (default 2) |
| `LOAD_PROOF_OUT` | Path for the JSON report |

### 7.2 Remote load proof

After the service is live:

```sh
RELAY_URL=wss://relay.openagents.com \
LOAD_PROOF_DURATION_MS=30000 \
pnpm --dir packages/sarah run load-proof
```

Report must include:

- publish and subscribe rate (events per second)
- median and p99 latency
- error classes
- pass or fail against remote thresholds
- host mode `remote`

### 7.3 Thresholds

| Profile | Min publish RPS | Min subscribe RPS | Max median ms | Max p99 ms | Max error rate |
| --- | --- | --- | --- | --- | --- |
| Local (`startTestRelay` / mock) | 40 | 20 | 150 | 750 | 2% |
| Remote (Cloud Run + Postgres) | 20 | 10 | 400 | 2000 | 5% |

Raise production targets only with a new measured report.

### 7.4 Overload observation

Increase publishers and subscribers until:

1. OK false rates rise, or
2. connect or operation timeouts dominate, or
3. Cloud Run CPU or memory saturates, or
4. Postgres connection or write latency dominates.

Record the first failure mode. Do not raise concurrency caps without that
record.

## 8. Health monitoring

`apps/openagents.com/workers/api/src/relay-health.ts` probes the market relay
with a NIP-11 leg and a WebSocket EOSE leg.

After the owned relay is live and load-proven:

1. Point a **separate** owned-relay monitor target at
   `wss://relay.openagents.com`.
2. Keep the third-party market relay (`wss://nos.lol` or the current
   `MARKET_RELAY_URL`) as its own monitored target while the market lane still
   uses it.
3. Do not flip Sarah coordination traffic until the remote load report passes
   and operator notes below are filled.

## 9. Operator notes

### 9.1 Backup

1. Enable automated Cloud SQL backups for the relay instance.
2. Keep at least seven daily backups and one weekly backup for thirty days.
3. Store the backup schedule and retention in the instance description.

### 9.2 Restore

1. Restore to a new instance or clone.
2. Point a staging Cloud Run revision at the restored database.
3. Run NIP-11, a single EVENT/OK, a REQ/EOSE, and the remote load proof.
4. Only then repoint production.

### 9.3 Key rotation

Relay process secrets (database password, admin NIP-86 material if used) rotate
through Secret Manager version adds and a Cloud Run redeploy.
Sarah signing material rotation is owned by `SARAH-NR-04`
(`docs/omega/2026-07-24-sarah-nostr-identity-contract.md`).
Do not place Sarah private keys in the relay service.

### 9.4 Multi-replica

1. Start with `min-instances=1` until a single instance passes load proof.
2. Enable session affinity before `max-instances` greater than 1.
3. Confirm NIP-77 and subscription fan-out under two instances before you raise
   production traffic.
4. Shared durability is the Postgres store. Do not run multi-replica with only
   memory stores.

## 10. Exit criteria

Local exit (this packet, without production secrets):

1. Runbook is in the deploy index.
2. Node entry pin and exports are documented.
3. `packages/sarah` load-proof tests pass.
4. `pnpm --dir packages/sarah run load-proof` exits 0 on a local host.

Production exit (owner-gated when blocked):

1. Cloud Run service is live in `openagentsgemini`.
2. `https://relay.openagents.com/` serves NIP-11.
3. `wss://relay.openagents.com` accepts EVENT/OK and REQ/EOSE.
4. Remote load-proof report passes and is retained as a public-safe receipt.
5. Owned-relay health monitor is configured separately from the market relay.

## 11. NEEDS_OWNER (only if truly blocked)

Record a narrow owner action when one of these is irreducible:

| Blocker | Owner action |
| --- | --- |
| No Cloud SQL instance or database for the relay | Create instance/database and grant the runtime SA `roles/cloudsql.client` |
| Secret Manager secrets absent | Create `nostr-relay-database-url` (and password secret if required) |
| DNS change for `relay.openagents.com` | Add or update the DNS-only record to the Cloud Run target |
| Production traffic flip | Admit remote load report and cut Sarah coordination to the owned relay |

Continue every independent packet while those items wait.

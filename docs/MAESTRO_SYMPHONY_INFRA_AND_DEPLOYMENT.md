# Maestro Symphony Infrastructure and Deployment Guide

This document is a detailed operations and infrastructure guide for running Maestro Symphony as a self-hosted Bitcoin indexer/API service.

Scope:
- Symphony architecture and runtime behavior
- Infra required to run it reliably (Bitcoin Core, storage, network, process model)
- Deployment options (Docker Compose, native/systemd, Kubernetes reference)
- Capacity planning, backup/restore, hardening, troubleshooting

Out of scope:
- L402 or x402 payment gatewaying
- Commercial API policy and billing

Date context:
- This guide was written on March 2, 2026.
- Upstream snapshot filenames in Symphony docs currently include 2025 dates. Treat those as examples and verify current snapshot indexes before production cutover.

## 1. What Symphony Is

Maestro Symphony is a Bitcoin indexing pipeline plus HTTP API server with:
- Supported networks: `mainnet`, `testnet4`, `regtest`
- Built-in indexers:
  - `TxCountByAddress`
  - `UtxosByAddress`
  - `Runes`
- Mempool-aware query mode via `?mempool=true`

The codebase couples sync/index and serve in one binary (`maestro-symphony`) and supports runtime commands:
- `sync` (index only)
- `serve` (API only)
- `run` (sync + serve)
- `docs` (regenerate `docs/openapi.json`)

## 2. Runtime Architecture

### 2.1 Core components

1. Bitcoin Core node
- P2P endpoint for chain data
- RPC endpoint for block/template queries
- Must run with tx index enabled for full utility

2. Symphony sync pipeline
- Pull stage reads chain events/mempool state from Bitcoin Core
- Index stage applies transaction indexers and writes RocksDB state
- Handles reorganizations with rollback buffer semantics

3. Symphony API server
- Reads indexed state and serves HTTP endpoints under `/addresses` and `/runes`
- Includes utility endpoints `/`, `/tip`, `/dump`

4. RocksDB state
- Primary local persistence for indexed data
- Timestamped read model for confirmed/mempool views

### 2.2 Data flow

1. Bitcoin Core updates chain/mempool.
2. Symphony pull stage fetches blocks and optional mempool snapshot.
3. Index stage updates RocksDB tables and tip metadata.
4. API server serves stable confirmed view or mempool-augmented view (`mempool=true`).

### 2.3 Trust and boundary model

- Symphony assumes trusted access to Bitcoin RPC/P2P.
- API layer is plaintext HTTP by default and has no built-in auth middleware.
- `dump` endpoint exposes raw key/value internals and should be treated as privileged.

## 3. API Surface and Query Model

### 3.1 OpenAPI endpoints

Primary OpenAPI paths:
- `GET /addresses/{address}/utxos`
- `GET /addresses/{address}/tx_count`
- `GET /addresses/{address}/runes/utxos`
- `GET /addresses/{address}/runes/utxos/{rune}`
- `GET /addresses/{address}/runes/balances`
- `GET /addresses/{address}/runes/balances/{rune}`
- `GET /addresses/{address}/runes/txs/{txid}`
- `POST /runes/info`
- `GET /runes/{rune}`
- `GET /runes/{rune}/utxos/{utxo}/balance`

Most endpoints support `mempool` query as optional boolean.

### 3.2 Non-OpenAPI utility endpoints

These are present in server routing and useful operationally:
- `GET /` -> static `"Symphony API Server"`
- `GET /tip` -> best known indexed tip (height/hash)
- `GET /dump` -> low-level key/value dump (debug only)

Operational recommendation:
- Restrict `/dump` to loopback/admin network only.
- Do not expose `/dump` on public ingress.

### 3.3 Response metadata model

API responses generally include `indexer_info`:
- `chain_tip`: indexed confirmed tip hash/height
- `mempool_timestamp`: present when mempool snapshot is in play
- `estimated_blocks`: projected block heights from mempool template context

This metadata should be used by clients to reason about freshness and whether mempool context was included.

## 4. Configuration Model

### 4.1 Config loading precedence

Symphony loads config from:
1. `symphony.toml` (if present, optional)
2. Explicit config path passed as positional arg
3. Environment overrides with prefix `SYMPHONY` and `_` separator

In practice, use explicit config file path and keep env overrides for secrets or CI/CD overrides.

### 4.2 Key configuration blocks

Top-level:
- `db_path`: path to RocksDB directory (default if omitted: `./tmp/symphony`)
- `storage`: RocksDB memory tuning
- `sync`: node access + indexer behavior
- `server`: listen address

`[sync.node]`:
- `p2p_address`
- `rpc_address`
- `rpc_user`
- `rpc_pass`

`[sync]`:
- `network`: `mainnet | testnet4 | regtest`
- `mempool`: bool
- `utxo_cache_size`: GB, optional
- `block_page_size`: optional, default `50`
- `stage_queue_size`: optional, default `20`
- `stage_timeout_secs`: optional, default `600`
- `max_rollback`: optional, code default `16`
- `safe_mode`: optional, default `false`
- `stop_after`: optional (useful for controlled indexing tests)

`[sync.indexers]`:
- `transaction_indexers`: list of indexer definitions

`[storage]`:
- `rocksdb_memory_budget`: GB, optional

`[server]`:
- `address`: optional, default `0.0.0.0:8080`

### 4.3 Important defaults (operational impact)

1. UTXO cache default
- If `sync.utxo_cache_size` is omitted, code chooses ~40% of available memory.
- This is implemented in index worker code and materially affects memory planning.

2. RocksDB budget default
- If `storage.rocksdb_memory_budget` is omitted, code chooses ~25% of available memory.
- Default behavior may leave index/filter block memory less tightly bounded than explicit budget mode.

3. Rollback depth default
- Code default is `max_rollback = 16` unless configured.
- Example configs often set larger values (for example `32` or `256`), which may be preferable for unstable environments.

4. Network mode
- `testnet4` is used in default test config; this is not legacy testnet3.

### 4.4 Example production-leaning config (single node)

```toml
db_path = "/var/lib/symphony"

[storage]
rocksdb_memory_budget = 8.0

[sync.node]
p2p_address = "127.0.0.1:8333"
rpc_address = "http://127.0.0.1:8332"
rpc_user = "symphony"
rpc_pass = "REPLACE_ME"

[sync]
network = "mainnet"
mempool = true
utxo_cache_size = 16.0
max_rollback = 64
block_page_size = 50
stage_queue_size = 20
stage_timeout_secs = 600

[sync.indexers]
transaction_indexers = [
  { type = "TxCountByAddress" },
  { type = "UtxosByAddress" },
  { type = "Runes", start_height = 840000, index_activity = true }
]

[server]
address = "0.0.0.0:8080"
```

## 5. Infrastructure Requirements

### 5.1 Baseline resource profile

From upstream guidance:
- Testnet4:
  - Disk: ~4 GB (Symphony) + ~50 GB (Bitcoin node snapshot guidance)
  - CPU: 2 cores
  - RAM: 4 GB
  - Sync: ~30 minutes (context dependent)
- Mainnet:
  - Disk: ~100 GB (Symphony) + ~600 GB (Bitcoin node snapshot guidance)
  - CPU: 4 cores
  - RAM: 12 GB
  - Sync: ~4 days from scratch (context dependent)

Treat these as starting points; actual requirements depend on enabled indexers, mempool mode, and IO throughput.

### 5.2 Ports and connectivity

Default ports in provided compose files:
- Bitcoin P2P: `8333/tcp`
- Bitcoin RPC: `8332/tcp`
- Symphony API: `8080/tcp`

Guidance:
- Keep RPC private to Symphony hosts.
- Expose API through reverse proxy/ingress with TLS and auth controls.

### 5.3 Storage characteristics

- Both Bitcoin Core and Symphony are write-heavy during sync bootstrap.
- Use SSD/NVMe; avoid slow networked disks for initial sync.
- Separate data volumes are recommended:
  - `/var/lib/bitcoin`
  - `/var/lib/symphony`

## 6. Deployment Options

For a concrete `gcloud`-first Google Cloud path, see Section 15.

### 6.1 Option A: Docker Compose (recommended for fast start)

Symphony repo includes:
- `docker-compose.yml` (testnet4)
- `docker-compose.mainnet.yml`
- `docker-compose.regtest.yml`

Compose characteristics:
- Runs `bitcoin/bitcoin:29`
- Builds Symphony image from local Dockerfile
- Binds config and data directories
- Uses `run` mode (`sync + serve`)

Commands:

```bash
# testnet4
make COMPOSE_FILE=docker-compose.yml compose-up

# mainnet
make COMPOSE_FILE=docker-compose.mainnet.yml compose-up

# regtest
make COMPOSE_FILE=docker-compose.regtest.yml compose-up

make COMPOSE_FILE=docker-compose.mainnet.yml docker-ps
make COMPOSE_FILE=docker-compose.mainnet.yml compose-down
```

Operational notes:
- Compose maps API to host `:8080`.
- Data persists under `./tmp/bitcoin-data` and `./tmp/symphony-data`.
- `symphony` service runs with `user: "${UID}:${GID}"`; set those env vars in shells/automation to avoid file ownership surprises.

### 6.2 Option B: Native binary + systemd (recommended for production hosts)

Build:

```bash
cargo build --release
sudo install -m 0755 target/release/maestro-symphony /usr/local/bin/maestro-symphony
```

Run modes:

```bash
# sync + serve
RUST_LOG=info maestro-symphony /etc/symphony/mainnet.toml run

# sync only
RUST_LOG=info maestro-symphony /etc/symphony/mainnet.toml sync

# serve only
RUST_LOG=info maestro-symphony /etc/symphony/mainnet.toml serve
```

Example `systemd` unit (`/etc/systemd/system/symphony.service`):

```ini
[Unit]
Description=Maestro Symphony
After=network-online.target bitcoind.service
Wants=network-online.target

[Service]
Type=simple
User=symphony
Group=symphony
WorkingDirectory=/var/lib/symphony
Environment=RUST_LOG=info
ExecStart=/usr/local/bin/maestro-symphony /etc/symphony/mainnet.toml run
Restart=always
RestartSec=5
LimitNOFILE=65536
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/symphony

[Install]
WantedBy=multi-user.target
```

### 6.3 Option C: Kubernetes reference architecture (not upstream-provided)

This pattern is inferred from runtime behavior and is a reference, not official Symphony manifests.

Suggested baseline:
- StatefulSet `bitcoind` with PVC
- StatefulSet `symphony` with PVC
- Internal Service for Bitcoin RPC/P2P
- Internal Service for Symphony API
- Ingress/Gateway for external API with TLS and auth

Key guidance:
- Prefer single Symphony pod per indexed dataset.
- Co-locate Symphony and Bitcoin in same cluster/zone to reduce RPC latency.
- Use anti-affinity and storage classes that provide predictable IOPS.
- Add PodDisruptionBudgets for controlled maintenance.

When not to split sync/serve:
- Symphony `serve` read-only mode relies on RocksDB secondary refresh semantics.
- For simplicity and correctness, run `run` mode in one process unless you have a tested multi-process data-sharing design.

## 7. Snapshot Bootstrap Strategy

Using snapshots dramatically reduces bootstrap time.

Upstream snapshot guide currently lists examples with explicit dates:
- Bitcoin mainnet snapshot example filename: `20250826.tar.lz4`
- Bitcoin testnet snapshot example filename: `20250827.tar.lz4`
- Symphony mainnet/testnet snapshot example filename: `20250927.tar.lz4`

Testnet bootstrap sequence:

```bash
mkdir -p ./tmp/{symphony-data,bitcoin-data}

curl -L https://snapshots.gomaestro.org/bitcoin-node/testnet/snapshots/20250827.tar.lz4 | \
  lz4 -d | tar -xf - -C ./tmp/bitcoin-data

curl -L https://snapshots.gomaestro.org/symphony/testnet/snapshots/20250927.tar.lz4 | \
  lz4 -d | tar -xf - -C ./tmp/symphony-data

make COMPOSE_FILE=docker-compose.yml compose-up
```

After startup:
- Verify `/tip`
- Query a known endpoint and inspect `indexer_info`

## 8. Operational Runbook

### 8.1 Startup checklist

1. Bitcoin RPC responds with expected chain.
2. Symphony config points to correct node endpoints and credentials.
3. `db_path` volume has expected free space.
4. Symphony process starts with desired mode (`run`, `sync`, or `serve`).
5. API port reachable from intended network segment.

### 8.2 Health checks

Basic:

```bash
curl -s http://127.0.0.1:8080/
curl -s http://127.0.0.1:8080/tip | jq .
```

Indexer-level:

```bash
curl -s "http://127.0.0.1:8080/addresses/<ADDRESS>/utxos" | jq '.indexer_info'
curl -s "http://127.0.0.1:8080/addresses/<ADDRESS>/utxos?mempool=true" | jq '.indexer_info'
```

Expected behavior:
- `mempool_timestamp` may be `null` if no mempool snapshot is indexed yet.
- `estimated_blocks` may be empty even when healthy.

### 8.3 Logging and diagnostics

- Configure log verbosity via `RUST_LOG` (default examples use `info`).
- Use structured logs for:
  - node connection status
  - tip progression
  - mempool ingestion
  - rollback events

### 8.4 OpenAPI refresh

Regenerate API spec:

```bash
make openapi
# or
maestro-symphony /etc/symphony/mainnet.toml docs
```

## 9. Performance and Capacity Tuning

### 9.1 Memory budgeting strategy

Plan memory in three buckets:
1. OS + process overhead
2. UTXO cache (`sync.utxo_cache_size`, default ~40% if omitted)
3. RocksDB (`storage.rocksdb_memory_budget`, default ~25% if omitted)

For constrained hosts:
- Explicitly set both budgets.
- Consider disabling UTXO cache (`utxo_cache_size = 0`) only after benchmarking latency impact.

### 9.2 Throughput knobs

- `sync.block_page_size`: larger can increase batch throughput, but also memory and downstream burst pressure.
- `sync.stage_queue_size`: controls in-flight event buffering between pull and index stages.
- `sync.stage_timeout_secs`: worker timeout/retry policy budget.

Tune with staged load tests, not in production first.

### 9.3 Rollback behavior knobs

- `sync.max_rollback`: depth of rollback safety window.
- `sync.safe_mode`: stricter rollback action consistency checks.

If your chain source is unstable or you need stronger safety diagnostics, increase rollback depth and consider safe mode in non-latency-critical environments.

## 10. Security and Hardening

### 10.1 Network security

Recommended stance:
- Bitcoin RPC (`8332`) private-only
- Bitcoin P2P (`8333`) exposed only as required
- Symphony API (`8080`) behind reverse proxy with TLS termination

### 10.2 API exposure model

Symphony server code does not include built-in API auth/rate-limiting middleware.
Add these at the edge:
- mTLS or JWT/authn at gateway
- request rate limits
- IP allowlists for admin endpoints

### 10.3 Endpoint hardening

- Restrict `/dump` to admin-only paths or disable by network policy.
- Prefer not to expose root and debug endpoints publicly without controls.

### 10.4 Secrets

- Keep RPC credentials outside committed config.
- Inject via environment or secret mounts.
- Rotate credentials and segment by environment.

## 11. Backup, Restore, and Disaster Recovery

### 11.1 Data classes

1. Bitcoin node data directory
2. Symphony RocksDB directory
3. Config and deployment manifests

### 11.2 Backup approaches

Approach A (faster RTO):
- Snapshot both Bitcoin and Symphony volumes together after controlled stop.

Approach B (minimal storage):
- Keep only config + automation + optional periodic Symphony snapshots; rebuild from Bitcoin node when needed.

### 11.3 Restore workflow

1. Restore data volumes.
2. Validate ownership/permissions.
3. Start Bitcoin node first, then Symphony.
4. Verify `/tip` and API query consistency.

### 11.4 DR drills

Run regular restore drills in staging:
- time-to-serving objective
- tip catch-up time
- API correctness on known addresses/runes

## 12. Extending Symphony (Custom Indexers)

For product-specific metadata indexing, Symphony supports custom indexers.

Critical invariants:
- `TransactionIndexer` enum values are encoded as `u8`.
- Never reorder or delete existing variants in a live deployment.
- Add new variants only at the end.

Workflow summary:
1. Add custom module under `src/sync/stages/index/indexers/custom/`.
2. Register enum variant + factory wiring.
3. Define tables via `define_indexer_table!`.
4. Implement `ProcessTransaction`.
5. Rebuild and reindex (or run migration strategy with fresh DB when schema/encoding changes).

## 13. Common Failure Modes and Fixes

1. Symphony starts but API is stale
- Cause: serve-only read mode not refreshing from active writer path.
- Fix: prefer `run` mode or ensure read-only secondary path and refresh semantics are valid for your filesystem/process model.

2. High memory pressure / OOM
- Cause: implicit large defaults for UTXO cache and RocksDB under small host memory.
- Fix: explicitly set `utxo_cache_size` and `rocksdb_memory_budget` to bounded values.

3. No mempool data in responses
- Cause: mempool disabled or no valid mempool snapshot at current tip.
- Fix: set `sync.mempool = true`, verify RPC node health and block template availability.

4. Rollback panic in strict environments
- Cause: rollback buffer invariant violations with `safe_mode = true`.
- Fix: inspect logs, verify data consistency assumptions, increase rollback depth, and validate node reliability.

5. Slow startup after restore
- Cause: old snapshots or limited IOPS.
- Fix: refresh snapshots and move DBs to faster storage.

## 14. Recommended Production Blueprint (Single Region)

1. One Bitcoin Core instance per environment (mainnet/testnet segregated).
2. One Symphony `run` process per network/index profile.
3. Dedicated SSD volumes for Bitcoin and Symphony data.
4. Reverse proxy (TLS + auth + rate limiting) in front of Symphony API.
5. Continuous metrics/log shipping plus restore-tested backups.

This keeps operational complexity low while preserving clear isolation boundaries.

## 15. Google Cloud Deployment (gcloud-first)

This section assumes Google Cloud is the target environment and `gcloud` is the operator interface.

### 15.1 Hosting decision on GCP

Recommended canonical path for Symphony:
- Compute Engine VM(s) with attached Persistent Disk, running Docker Compose or systemd.

Why:
- Symphony + Bitcoin Core are stateful and disk-heavy.
- Symphony writes local RocksDB and expects durable local storage characteristics.
- Cloud Run is not a good fit for this stateful pair due to ephemeral local filesystem model and process lifecycle.

Secondary path:
- GKE with StatefulSets for `bitcoind` and `symphony` when you need Kubernetes-native operations.

### 15.2 Operator preflight and defaults

Use the same environment variable style used in existing OpenAgents GCP runbooks.

```bash
export GCP_PROJECT="${GCP_PROJECT:-openagentsgemini}"
export GCP_REGION="${GCP_REGION:-us-central1}"
export GCP_ZONE="${GCP_ZONE:-us-central1-a}"
```

Authenticate and pin project:

```bash
gcloud auth login
gcloud config set project "${GCP_PROJECT}"
gcloud config get-value project
```

Optional but recommended auth checks:

```bash
gcloud auth list
gcloud config list --format='text(core.account,core.project,compute.region,compute.zone)'
```

Enable required services:

```bash
gcloud services enable \
  compute.googleapis.com \
  artifactregistry.googleapis.com \
  cloudbuild.googleapis.com \
  secretmanager.googleapis.com \
  logging.googleapis.com \
  monitoring.googleapis.com \
  --project "${GCP_PROJECT}"
```

### 15.3 Build and publish Symphony container image

Create Artifact Registry repo once:

```bash
gcloud artifacts repositories create openagents-symphony \
  --repository-format=docker \
  --location="${GCP_REGION}" \
  --description="Maestro Symphony images" \
  --project="${GCP_PROJECT}"
```

Build from local Symphony checkout:

```bash
export SYMPHONY_SRC="/Users/christopherdavid/code/maestro/maestro-symphony"
export TAG="$(git -C "${SYMPHONY_SRC}" rev-parse --short HEAD)"
export SYMPHONY_IMAGE="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/openagents-symphony/symphony:${TAG}"

gcloud builds submit "${SYMPHONY_SRC}" \
  --tag "${SYMPHONY_IMAGE}" \
  --project "${GCP_PROJECT}"
```

Optional moving tag:

```bash
gcloud artifacts docker tags add \
  "${SYMPHONY_IMAGE}" \
  "${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/openagents-symphony/symphony:latest"
```

### 15.4 Canonical GCP deployment: Compute Engine

Create dedicated service account for VM workloads:

```bash
gcloud iam service-accounts create symphony-mainnet \
  --display-name="Symphony Mainnet SA" \
  --project "${GCP_PROJECT}"
```

Grant minimal practical roles (tighten as needed):

```bash
gcloud projects add-iam-policy-binding "${GCP_PROJECT}" \
  --member="serviceAccount:symphony-mainnet@${GCP_PROJECT}.iam.gserviceaccount.com" \
  --role="roles/logging.logWriter"

gcloud projects add-iam-policy-binding "${GCP_PROJECT}" \
  --member="serviceAccount:symphony-mainnet@${GCP_PROJECT}.iam.gserviceaccount.com" \
  --role="roles/monitoring.metricWriter"

gcloud projects add-iam-policy-binding "${GCP_PROJECT}" \
  --member="serviceAccount:symphony-mainnet@${GCP_PROJECT}.iam.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"
```

Create data disks:

```bash
gcloud compute disks create symphony-bitcoin-mainnet \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --type pd-ssd \
  --size 2048GB

gcloud compute disks create symphony-data-mainnet \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --type pd-ssd \
  --size 512GB
```

Create VM with attached disks:

```bash
gcloud compute instances create symphony-mainnet-1 \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --machine-type n2-standard-8 \
  --image-family ubuntu-2204-lts \
  --image-project ubuntu-os-cloud \
  --boot-disk-size 100GB \
  --boot-disk-type pd-ssd \
  --service-account "symphony-mainnet@${GCP_PROJECT}.iam.gserviceaccount.com" \
  --scopes cloud-platform \
  --disk name=symphony-bitcoin-mainnet,device-name=bitcoin-data,mode=rw,auto-delete=no \
  --disk name=symphony-data-mainnet,device-name=symphony-data,mode=rw,auto-delete=no \
  --tags symphony-api,bitcoin-p2p
```

Reserved internal IP (optional, recommended for stable private addressing):

```bash
gcloud compute addresses create symphony-mainnet-ip \
  --project "${GCP_PROJECT}" \
  --region "${GCP_REGION}" \
  --subnet default
```

Then add `--private-network-ip=<RESERVED_IP>` when creating the VM.

Firewall rules (example):

```bash
# Restrict API ingress to trusted CIDRs only.
gcloud compute firewall-rules create symphony-api-ingress \
  --project "${GCP_PROJECT}" \
  --network default \
  --direction INGRESS \
  --priority 1000 \
  --action ALLOW \
  --rules tcp:8080 \
  --source-ranges "<TRUSTED_CIDR_1>,<TRUSTED_CIDR_2>" \
  --target-tags symphony-api

# Optional: expose Bitcoin P2P if you want inbound peers.
gcloud compute firewall-rules create symphony-bitcoin-p2p-ingress \
  --project "${GCP_PROJECT}" \
  --network default \
  --direction INGRESS \
  --priority 1000 \
  --action ALLOW \
  --rules tcp:8333 \
  --source-ranges "0.0.0.0/0" \
  --target-tags bitcoin-p2p
```

Bootstrap host:

1. Install Docker + Compose plugin.
2. Mount `/dev/disk/by-id/google-bitcoin-data` to `/var/lib/bitcoin`.
3. Mount `/dev/disk/by-id/google-symphony-data` to `/var/lib/symphony`.
4. Pull/clone `maestro-symphony`.
5. Use compose mainnet config with host-bound data directories.
6. Start stack and verify `/tip`.

Use Secret Manager for sensitive values:

```bash
gcloud secrets create symphony-rpc-pass --replication-policy=automatic --project "${GCP_PROJECT}"
printf '%s' '<STRONG_RPC_PASSWORD>' | \
  gcloud secrets versions add symphony-rpc-pass --data-file=- --project "${GCP_PROJECT}"
```

Read secret on VM at deploy time:

```bash
gcloud secrets versions access latest \
  --secret=symphony-rpc-pass \
  --project "${GCP_PROJECT}"
```

### 15.5 Environment matrix (dev/staging/prod)

Use explicit per-environment naming and avoid ad hoc resource names.

Suggested defaults:
- `dev`:
  - network: `testnet4`
  - VM: `symphony-dev-1`
  - zone: `${GCP_REGION}-a`
  - disks: `symphony-bitcoin-dev`, `symphony-data-dev`
  - API exposure: private only
- `staging`:
  - network: `testnet4` (or mainnet shadow if you need production-shape performance tests)
  - VM: `symphony-staging-1`
  - disks: `symphony-bitcoin-staging`, `symphony-data-staging`
  - API exposure: restricted CIDR
- `prod`:
  - network: `mainnet`
  - VM: `symphony-mainnet-1`
  - disks: `symphony-bitcoin-mainnet`, `symphony-data-mainnet`
  - API exposure: behind controlled ingress/LB + explicit allowlists

Treat promotion as `dev -> staging -> prod`, with explicit health/tip checks at each step.

### 15.6 Release gates and go/no-go checklist

Use a deterministic gate model similar to existing OpenAgents GCP runbooks.

Pre-deploy gates:
1. `gcloud` auth/project/region/zone are set as expected.
2. New image exists in Artifact Registry and digest is recorded.
3. Config diff reviewed (`mainnet.toml`/`testnet.toml` changes).
4. Backup snapshot created before any destructive maintenance.

Deploy gates:
1. VM/service process starts cleanly.
2. `GET /tip` responds and tip progresses.
3. Representative API endpoints return valid payloads.
4. Error logs stay below agreed threshold over observation window.

Post-deploy gates:
1. Snapshot and deploy receipt captured.
2. Rollback command path tested in staging.
3. On-call/operator notes updated with image tag + timestamp + checks.

### 15.7 Optional GKE deployment path

Use this only if your team already operates stateful workloads on GKE.

Reference pattern from historical OpenAgents runtime deploys:
- Build with Cloud Build and push to Artifact Registry.
- Deploy StatefulSets + Services + NetworkPolicy.
- Use environment overlays (dev/staging/prod) and `kubectl apply -k`.

Baseline GKE commands:

```bash
gcloud container clusters create symphony-mainnet \
  --project "${GCP_PROJECT}" \
  --region "${GCP_REGION}" \
  --machine-type n2-standard-8 \
  --num-nodes 3

gcloud container clusters get-credentials symphony-mainnet \
  --region "${GCP_REGION}" \
  --project "${GCP_PROJECT}"
```

Recommended K8s objects:
- StatefulSet `bitcoind` + PVC
- StatefulSet `symphony` + PVC
- ClusterIP Service for Bitcoin RPC/P2P
- Internal Service/Ingress for Symphony API
- NetworkPolicy limiting RPC and API ingress
- PodDisruptionBudget for both stateful services

### 15.8 Deploy gates and verification (gcloud)

Minimum post-deploy checks:

```bash
gcloud compute instances list --project "${GCP_PROJECT}" --filter="name=symphony-mainnet-1"

gcloud compute ssh symphony-mainnet-1 \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --command "curl -sf http://127.0.0.1:8080/tip | jq ."

gcloud logging read \
  'resource.type="gce_instance" AND severity>=ERROR' \
  --project "${GCP_PROJECT}" \
  --freshness=30m \
  --limit=100
```

Inventory commands (same operator style used by existing OpenAgents GCP runbooks):

```bash
gcloud run services list --platform=managed --region="${GCP_REGION}" --project="${GCP_PROJECT}"
gcloud run jobs list --region="${GCP_REGION}" --project="${GCP_PROJECT}"
gcloud artifacts repositories list --location="${GCP_REGION}" --project="${GCP_PROJECT}"
gcloud secrets list --project="${GCP_PROJECT}"
```

### 15.9 Backup and restore on GCP

Use disk snapshots as the default rapid-recovery mechanism.

Create snapshots (prefer controlled stop of write-heavy services first):

```bash
gcloud compute disks snapshot symphony-bitcoin-mainnet \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --snapshot-names "symphony-bitcoin-mainnet-$(date -u +%Y%m%dT%H%M%SZ)"

gcloud compute disks snapshot symphony-data-mainnet \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --snapshot-names "symphony-data-mainnet-$(date -u +%Y%m%dT%H%M%SZ)"
```

Restore disk from snapshot:

```bash
gcloud compute disks create symphony-data-mainnet-restore \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --source-snapshot "<SNAPSHOT_NAME>"
```

Attach restored disk to recovery VM and validate `/tip` plus representative API queries before promotion.

Scheduled snapshots (recommended):

```bash
gcloud compute resource-policies create snapshot-schedule symphony-daily-7d \
  --project "${GCP_PROJECT}" \
  --region "${GCP_REGION}" \
  --max-retention-days 7 \
  --daily-schedule \
  --start-time 03:00

gcloud compute disks add-resource-policies symphony-bitcoin-mainnet \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --resource-policies "symphony-daily-7d"

gcloud compute disks add-resource-policies symphony-data-mainnet \
  --project "${GCP_PROJECT}" \
  --zone "${GCP_ZONE}" \
  --resource-policies "symphony-daily-7d"
```

### 15.10 Example operator receipt template

Store deploy evidence in backroom archive (or your ops artifact store):

```json
{
  "timestamp_utc": "2026-03-02T00:00:00Z",
  "project": "openagentsgemini",
  "region": "us-central1",
  "zone": "us-central1-a",
  "environment": "prod",
  "vm": "symphony-mainnet-1",
  "image": "us-central1-docker.pkg.dev/openagentsgemini/openagents-symphony/symphony:<TAG>",
  "snapshot_before": {
    "bitcoin": "symphony-bitcoin-mainnet-<STAMP>",
    "symphony": "symphony-data-mainnet-<STAMP>"
  },
  "health_checks": {
    "tip_ok": true,
    "api_sample_ok": true,
    "error_log_scan_ok": true
  },
  "operator": "<name>"
}
```

## 16. Source Pointers

Primary upstream files used for this guide:
- `maestro-symphony/README.md`
- `maestro-symphony/Makefile`
- `maestro-symphony/docker-compose.yml`
- `maestro-symphony/docker-compose.mainnet.yml`
- `maestro-symphony/docker-compose.regtest.yml`
- `maestro-symphony/examples/*.toml`
- `maestro-symphony/docs/openapi.json`
- `maestro-symphony/docs/guides/setup-with-snapshot.md`
- `maestro-symphony/docs/guides/add-a-custom-index.md`
- `maestro-symphony/docs/rocksdb.md`
- `maestro-symphony/src/main.rs`
- `maestro-symphony/src/serve/mod.rs`
- `maestro-symphony/src/sync/mod.rs`
- `maestro-symphony/src/sync/pipeline.rs`
- `maestro-symphony/src/storage/kv_store.rs`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/docs/sync/SPACETIME_GCLOUD_DEPLOYMENT_CONSIDERATIONS.md`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/runtime/docs/DEPLOY_GCP.md`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/apps/runtime/deploy/cloudrun/check-migration-drift.sh`
- `/Users/christopherdavid/code/backroom/openagents-prune-20260225-205724-wgpui-mvp/docs/core/DEPLOYMENT_RUST_SERVICES.md`

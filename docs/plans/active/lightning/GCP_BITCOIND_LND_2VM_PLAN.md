# GCP Self-Hosted Bitcoin Core + LND (2-VM) Plan

Last updated: 2026-02-13

This is the "clean" self-hosted foundation for OpenAgents Lightning + L402: run our own full Bitcoin node (`bitcoind`) and Lightning node (`lnd`) on Google Cloud, with minimal moving parts and clear operational boundaries.

This plan intentionally assumes **two stateful VMs**:

1. **VM #1:** Bitcoin Core full node (`bitcoind`)
2. **VM #2:** LND (`lnd`) using `bitcoind` as its chain backend

Everything else (Aperture, web services, control plane, ops tooling) can be layered on without changing the core chain/node architecture.

## 0) Goals / Non-Goals

Goals:

- Operate a production-grade (or at least production-shaped) **mainnet** Bitcoin Core + LND pair on GCP.
- Keep RPC surfaces **private** (VPC-only) and rely on IAP/OS Login for admin access.
- Provide a clear path to:
  - bootstrap liquidity (inbound/outbound) for receiving L402 payments,
  - pay invoices (for buyer flows and operator actions),
  - integrate the node with **Aperture** (L402 reverse proxy) and OpenAgents control plane.
- Be able to verify the system end-to-end with deterministic, copy/paste commands.

Non-goals (explicitly out of scope for the first pass):

- Remote signer (adds a third key-holding system). We can harden later with remote signing once the baseline is stable.
- Multi-region active/active nodes.
- "Routing node profitability" optimization. We only need "works reliably for L402".
- Kubernetes for stateful `bitcoind`/`lnd` (we can revisit later; VMs are simpler).

## 1) Architecture (What Runs Where)

### 1.1 Core node boundary (2 VMs)

**`oa-bitcoind` VM**

- Runs `bitcoind` mainnet full node.
- Exposes:
  - P2P: `8333/tcp` (optional inbound; required outbound)
  - RPC: `8332/tcp` (VPC-only)
  - ZMQ: `28332/tcp`, `28333/tcp` (VPC-only; no auth)

**`oa-lnd` VM**

- Runs `lnd` mainnet with chain backend = `bitcoind`.
- Exposes:
  - Lightning P2P: `9735/tcp` (public inbound if we want peers to connect)
  - gRPC: `10009/tcp` (VPC-only)
  - REST: `8080/tcp` (VPC-only; optional)

### 1.2 How this ties into OpenAgents L402

Seller / paywall:

- **Aperture** needs access to LND gRPC + macaroons + TLS to:
  - create invoices,
  - validate preimages (or otherwise verify payment),
  - manage LSAT/macaroons depending on our policy.
- Recommended: keep Aperture on **Cloud Run** (stateless), and connect to LND over a **Serverless VPC Access connector**. LND remains private.

Buyer:

- If we want OpenAgents to pay external L402 endpoints, we can implement payment execution in:
  - Convex/Worker-backed service (if keys can safely live there), or
  - a dedicated "wallet executor" service in Cloud Run/GCE with strict secret boundaries.
- This doc focuses on the node foundation. The buyer executor is layered on top (see existing `docs/plans/active/lightning/*`).

### 1.3 Network diagram (high level)

```
                 (public internet)
                        |
                        |  HTTPS
                        v
               [Aperture (Cloud Run)]
                        |
                        |  gRPC (VPC-only)
                        v
                  [oa-lnd (GCE VM)]
                        |
                        |  RPC + ZMQ (VPC-only)
                        v
               [oa-bitcoind (GCE VM)]
```

## 2) Sizing Recommendations (Mainnet)

Sizing is opinionated and biased toward "this works reliably and won't paint us into a corner".

### 2.1 Bitcoin Core (`oa-bitcoind`)

Disk:

- **2 TB** Persistent Disk (recommended baseline for 2026+ headroom).
- Disk type:
  - `pd-ssd` if we want faster initial sync and better latency under load.
  - `pd-balanced` if we want lower cost and can accept slower sync.

Compute:

- Recommended starting point: `n2-standard-8` (8 vCPU / 32 GB RAM) for initial sync.
- After initial sync: resize down to `n2-standard-4` (4 vCPU / 16 GB RAM) if costs matter and we are not running heavy indexing beyond `txindex`.

Bitcoin Core config knobs:

- `dbcache`: with 32 GB RAM, `dbcache=12000` to `dbcache=16000` can materially improve IBD. (Keep OS headroom.)

### 2.2 LND (`oa-lnd`)

Disk:

- **200 GB** Persistent Disk is typically plenty for LND data + logs (even with a large graph).
- Disk type: `pd-balanced` is fine.

Compute:

- `e2-standard-4` (4 vCPU / 16 GB RAM) is a solid default.
- If we expect heavier operations (Aperture + higher throughput), use `n2-standard-4`.

### 2.3 Bitcoin blockchain size assumptions

As a coarse planning reference, the Bitcoin blockchain was ~693 GB in Oct 2025 and grows steadily. Plan disk with large headroom (2 TB) so we do not need emergency storage migrations. (Disk resize is possible later, but operationally annoying.)

## 3) Networking and Security Model

### 3.1 VPC layout

- Create a dedicated VPC + subnet for Lightning infrastructure.
- Give both VMs **private IPs** in the same subnet.
- Prefer:
  - no external IP on `oa-bitcoind`,
  - optional external IP on `oa-lnd` (needed if we want inbound peers; otherwise can also be NAT-only).

### 3.2 Admin access

- Use **OS Login** and **IAP TCP forwarding** for SSH.
- Disable password auth; use SSH keys through OS Login.
- No public RPC ports exposed.

### 3.3 Firewall rules (suggested)

Inbound to `oa-bitcoind`:

- Deny all by default.
- Allow from `oa-lnd` private IP CIDR to:
  - `8332/tcp` (RPC)
  - `28332/tcp` and `28333/tcp` (ZMQ)
- Optional: allow `8333/tcp` from the internet if we want inbound P2P.

Inbound to `oa-lnd`:

- Deny all by default.
- Allow `9735/tcp` from the internet (if we want inbound Lightning peers).
- Allow from Aperture (via VPC connector subnet) to:
  - `10009/tcp` (gRPC)
  - `8080/tcp` (REST, optional)

### 3.4 Secrets and key material

Baseline (no remote signer):

- LND wallet seed stays in operator custody (offline storage).
- LND macaroons and TLS certs live on disk on `oa-lnd` and are copied into Secret Manager only when strictly required (e.g. Aperture).
- Prefer mounting secrets into Cloud Run via Secret Manager rather than embedding into images.

Future hardening (optional):

- Remote signer architecture (third system), or HSM/KMS-backed signing if we adopt an implementation that supports it.

## 4) Step-by-Step Implementation (Operator Runbook)

This is written as if we are doing it manually once. After we converge on the shape, we should codify it in Terraform.

### 4.1 Prereqs

- GCP project and billing.
- Enable APIs:
  - `compute.googleapis.com`
  - `iap.googleapis.com`
  - `secretmanager.googleapis.com`
  - `cloudresourcemanager.googleapis.com`

### 4.2 Copy/paste: GCP provisioning skeleton

This section is intentionally explicit so we can repeat it without relying on a vendor dashboard UI.

Set common vars:

```bash
export PROJECT_ID="openagents-prod"
export REGION="us-central1"
export ZONE="us-central1-a"

export OA_VPC="oa-lightning"
export OA_SUBNET="oa-lightning-${REGION}"
export OA_SUBNET_CIDR="10.42.0.0/24"

# Used if we keep Cloud Run (Aperture, wallet executor) and need VPC reachability.
export OA_SERVERLESS_CONNECTOR="oa-serverless-${REGION}"
export OA_SERVERLESS_CIDR="10.42.8.0/28"
```

Enable APIs:

```bash
gcloud config set project "$PROJECT_ID"
gcloud services enable compute.googleapis.com iap.googleapis.com secretmanager.googleapis.com
```

Create VPC + subnet:

```bash
gcloud compute networks create "$OA_VPC" --subnet-mode=custom
gcloud compute networks subnets create "$OA_SUBNET" \
  --network="$OA_VPC" \
  --region="$REGION" \
  --range="$OA_SUBNET_CIDR"
```

Optional: create Cloud NAT so `oa-bitcoind` can have no external IP:

```bash
gcloud compute routers create "oa-nat-router-${REGION}" --network="$OA_VPC" --region="$REGION"
gcloud compute routers nats create "oa-nat-${REGION}" \
  --router="oa-nat-router-${REGION}" \
  --region="$REGION" \
  --nat-all-subnet-ip-ranges \
  --auto-allocate-nat-external-ips
```

Optional: Serverless VPC Access connector (Cloud Run -> VPC):

```bash
gcloud compute networks vpc-access connectors create "$OA_SERVERLESS_CONNECTOR" \
  --region="$REGION" \
  --network="$OA_VPC" \
  --range="$OA_SERVERLESS_CIDR"
```

Create firewall rules (tight defaults; adjust as needed):

```bash
# Allow SSH via IAP (required for gcloud compute ssh --tunnel-through-iap).
gcloud compute firewall-rules create "oa-allow-iap-ssh" \
  --network="$OA_VPC" \
  --allow=tcp:22 \
  --source-ranges="35.235.240.0/20"

# Optional: allow inbound Bitcoin P2P to bitcoind (not required for outbound-only).
gcloud compute firewall-rules create "oa-allow-bitcoin-p2p" \
  --network="$OA_VPC" \
  --allow=tcp:8333 \
  --target-tags="oa-bitcoind" \
  --source-ranges="0.0.0.0/0"

# Allow LND Lightning P2P inbound (recommended if we want inbound channels/peers).
gcloud compute firewall-rules create "oa-allow-lnd-p2p" \
  --network="$OA_VPC" \
  --allow=tcp:9735 \
  --target-tags="oa-lnd" \
  --source-ranges="0.0.0.0/0"

# VPC-only rules: bitcoind RPC/ZMQ reachable only from lnd + (optionally) serverless connector CIDR.
gcloud compute firewall-rules create "oa-allow-bitcoind-backend" \
  --network="$OA_VPC" \
  --allow=tcp:8332,tcp:28332,tcp:28333 \
  --target-tags="oa-bitcoind" \
  --source-ranges="$OA_SUBNET_CIDR"

# VPC-only rules: LND gRPC/REST reachable only within VPC (Aperture/wallet services).
gcloud compute firewall-rules create "oa-allow-lnd-rpc" \
  --network="$OA_VPC" \
  --allow=tcp:10009,tcp:8080 \
  --target-tags="oa-lnd" \
  --source-ranges="$OA_SUBNET_CIDR,$OA_SERVERLESS_CIDR"
```

Create VMs:

```bash
# bitcoind VM (no external IP by default; relies on NAT)
gcloud compute instances create "oa-bitcoind" \
  --zone="$ZONE" \
  --machine-type="n2-standard-8" \
  --network-interface="subnet=$OA_SUBNET,no-address" \
  --tags="oa-bitcoind" \
  --boot-disk-size="100GB" \
  --boot-disk-type="pd-balanced" \
  --create-disk="name=oa-bitcoind-data,size=2000GB,type=pd-ssd,auto-delete=no"

# lnd VM (public IP so peers can connect; remove external IP if you want outbound-only)
gcloud compute instances create "oa-lnd" \
  --zone="$ZONE" \
  --machine-type="e2-standard-4" \
  --network-interface="subnet=$OA_SUBNET" \
  --tags="oa-lnd" \
  --boot-disk-size="100GB" \
  --boot-disk-type="pd-balanced" \
  --create-disk="name=oa-lnd-data,size=200GB,type=pd-balanced,auto-delete=no"
```

Notes:

- Disk and machine types are safe defaults; resize later if needed.
- Consider reserving static external IP(s) for `oa-lnd` if you want stable node identity.

### 4.3 Provision the VPC

- Create `oa-lightning` VPC.
- Create `oa-lightning-us-central1` subnet (pick a region and stick to it).
- Create a Serverless VPC Access connector subnet for Cloud Run (if we will keep Aperture on Cloud Run).

### 4.4 Create `oa-bitcoind` VM

- Ubuntu LTS image.
- Boot disk: 50-100 GB.
- Data disk: 2 TB (`pd-ssd` recommended).
- No external IP (prefer) + Cloud NAT for outbound.

Install Bitcoin Core:

- Prefer official Bitcoin Core binaries with signature verification.
- Alternatively build from source if we need deterministic reproducibility, but it slows ops.

`bitcoin.conf` (example, mainnet):

```ini
server=1
txindex=1
disablewallet=1

# P2P
listen=1
port=8333

# RPC (bind private)
rpcbind=10.0.0.10
rpcallowip=10.0.0.0/24
rpcport=8332

# Use rpcauth (recommended) or rpcuser/rpcpassword (simpler).
# rpcauth=<generated>

# ZMQ (no auth; keep VPC-only)
zmqpubrawblock=tcp://10.0.0.10:28332
zmqpubrawtx=tcp://10.0.0.10:28333
```

Systemd unit should:

- ensure the data disk is mounted (e.g. `/var/lib/bitcoin`)
- run `bitcoind -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoin`

Verify:

- `bitcoin-cli -rpcconnect=127.0.0.1 getblockchaininfo`
- Confirm `initialblockdownload` transitions to `false` after IBD.

### 4.5 Create `oa-lnd` VM

- Ubuntu LTS image.
- Boot disk: 50-100 GB.
- Data disk: 200 GB (`pd-balanced`).
- External IP optional:
  - If yes: open `9735/tcp` and set LND `externalip=<public_ip>`.
  - If no: still usable for outbound-only connections, but peers cannot easily connect inbound.

Install LND:

- Prefer official release binaries; verify checksums/signatures.
- Keep version pinned (avoid auto-upgrading during demos).

`lnd.conf` (example skeleton, mainnet + bitcoind backend):

```ini
[Application Options]
debuglevel=info
maxpendingchannels=5
alias=oa-lnd

# Bind gRPC/REST on the private interface only.
rpclisten=10.0.0.20:10009
restlisten=10.0.0.20:8080

# Lightning P2P
listen=0.0.0.0:9735
# externalip=<static_public_ip>:9735

[Bitcoin]
bitcoin.mainnet=true
bitcoin.node=bitcoind

[Bitcoind]
bitcoind.rpchost=10.0.0.10:8332
bitcoind.rpcuser=<rpc_user_or_unused_if_cookie>
bitcoind.rpcpass=<rpc_pass>
bitcoind.zmqpubrawblock=tcp://10.0.0.10:28332
bitcoind.zmqpubrawtx=tcp://10.0.0.10:28333

# If we use Cloud Run to talk to LND, ensure the TLS cert includes a SAN for
# the hostname/IP Cloud Run connects to (private DNS name recommended).
# tlsextraip=10.0.0.20
# tlsextradomain=oa-lnd.internal
```

Initialize wallet:

- `lncli --network=mainnet create`
- Store seed phrase offline.
- (Optional) Set wallet password policy for operators.

Verify chain sync from LND:

- `lncli getinfo`
- Confirm:
  - `synced_to_chain: true`
  - `synced_to_graph: true` (may take time after startup)

### 4.6 Create first channel (minimum viable)

To receive L402 payments, we need inbound liquidity. Outbound-only channels are not enough.

Minimum viable bootstrap:

1. Fund the LND on-chain wallet.
2. Connect to a high-availability peer.
3. Open an initial channel (outbound).
4. Convert some outbound to inbound by:
   - spending (organic), or
   - using Loop Out (if we adopt it), or
   - leasing inbound liquidity (Pool) once we meet its prerequisites.

## 5) Liquidity Strategy (Practical, Demo-Oriented)

For L402 seller infrastructure, the node must be able to **receive** Lightning payments reliably:

- That means the node needs **inbound capacity**.
- Opening channels ourselves gives us **outbound**; inbound must be acquired.

Pragmatic sequence:

1. Open 1-3 outbound channels to well-connected nodes (stability over fee games).
2. Generate inbound:
   - Easiest: spend from the node (outbound drains to inbound).
   - Faster: use a swap/loop out method (moves liquidity to inbound while keeping capital).
   - Programmatic: use a marketplace approach (Pool), but it has prerequisites and operational overhead.
3. For the first EP212-style demo: do not aim for "routing". Aim for "payments succeed".

## 6) Aperture Integration Notes (Seller)

If we keep Aperture on Cloud Run:

1. Create a Serverless VPC Access connector in the same region as Cloud Run.
2. Ensure Cloud Run egress uses the connector.
3. Allow connector subnet CIDR to reach `oa-lnd:10009`.
4. Provide Aperture:
   - `tls.cert`
   - the appropriate macaroon(s)
   - LND address `oa-lnd.internal:10009`

If we move Aperture to the `oa-lnd` VM:

- We avoid Cloud Run networking, but we take on host-level process management (systemd/Docker).
- This can be "cleaner" for a small system, but is more coupled operationally.

## 7) Backups and Disaster Recovery

Bitcoin Core:

- Can be re-synced; do not rely on it for keys (use `disablewallet=1`).
- Snapshotting the disk can help recovery time, but is not strictly required.

LND:

- Required backups:
  - wallet seed (offline)
  - Static Channel Backup file (`channel.backup`) exported periodically
- Store backups in a versioned GCS bucket with tight IAM and KMS encryption.

## 8) Monitoring / Alerting (Minimum)

Set up Cloud Ops Agent on both VMs and alert on:

- `bitcoind` not running
- `bitcoind` IBD stuck (no height progress for N minutes)
- `lnd` not running
- `lnd` not synced to chain
- disk usage > 80%
- memory pressure / OOM events

## 9) Verification Checklist (Copy/Paste)

On `oa-bitcoind`:

- `bitcoin-cli getblockchaininfo`
- `bitcoin-cli getnetworkinfo`
- `bitcoin-cli getzmqnotifications` (confirm ZMQ endpoints are set)

On `oa-lnd`:

- `lncli getinfo`
- `lncli walletbalance`
- `lncli channelbalance`

End-to-end seller sanity:

1. Create invoice: `lncli addinvoice --amt 1000 --memo "smoke"`
2. Pay it from an external wallet
3. Confirm settled:
   - `lncli lookupinvoice <rhash>`
   - `lncli listinvoices --max_invoices 20`

## 10) Phased Rollout (Recommended)

Phase 0 (local/dev):

- Run `bitcoind` + `lnd` in regtest on a laptop for protocol-level tests.

---

## Work Log (2026-02-13)

This section is an execution log for the initial GCP bring-up using `gcloud` in the
current project.

### GCP Project + Region

- Project: `openagentsgemini`
- Region: `us-central1`
- Zone: `us-central1-a`

### Provisioned Resources

Created:

- VPC: `oa-lightning`
- Subnet: `oa-lightning-us-central1` (`10.42.0.0/24`)
- Cloud Router: `oa-nat-router-us-central1`
- Cloud NAT: `oa-nat-us-central1` (egress for private instances)
- Serverless VPC Access connector: `oa-serverless-us-central1` (`10.42.8.0/28`)
- Firewall rules:
  - `oa-allow-iap-ssh` (IAP to tcp/22)
  - `oa-allow-bitcoin-p2p` (tcp/8333 -> tag `oa-bitcoind`)
  - `oa-allow-lnd-p2p` (tcp/9735 -> tag `oa-lnd`)
  - `oa-allow-bitcoind-backend` (tcp/8332,28332,28333 -> tag `oa-bitcoind` from `10.42.0.0/24`)
  - `oa-allow-lnd-rpc` (tcp/10009,8080 -> tag `oa-lnd` from `10.42.0.0/24` + `10.42.8.0/28`)
- Static external IPv4 (regional): `oa-lnd-ip` = `34.61.69.143`
- VMs:
  - `oa-bitcoind` (no external IP)
    - internal IP: `10.42.0.2`
    - boot: `pd-balanced` 100GB
    - data: `oa-bitcoind-data` `pd-ssd` 2000GB (not auto-deleted)
  - `oa-lnd`
    - internal IP: `10.42.0.3`
    - external IP: `34.61.69.143` (static)
    - boot: `pd-balanced` 100GB
    - data: `oa-lnd-data` `pd-balanced` 200GB (not auto-deleted)

### Node Software Installed

On `oa-bitcoind`:

- Bitcoin Core `30.2` (`bitcoind`, `bitcoin-cli`, etc) installed to `/usr/local/bin` from official tarball.
- Data disk formatted/mounted at `/var/lib/bitcoin`.
- Config: `/etc/bitcoin/bitcoin.conf`
  - RPC bound to `127.0.0.1` and `10.42.0.2` (VPC-only), port `8332`
  - ZMQ bound to `10.42.0.2:28332` and `10.42.0.2:28333` (VPC-only)
  - `txindex=1`, `disablewallet=1`
- systemd: `/etc/systemd/system/bitcoind.service`

On `oa-lnd`:

- LND `v0.20.1-beta` (`lnd`, `lncli`) installed to `/usr/local/bin` from GitHub release tarball + manifest sha256 verification.
- Data disk formatted/mounted at `/var/lib/lnd`.
- Config: `/etc/lnd/lnd.conf`
  - `externalip=34.61.69.143:9735`
  - `rpclisten=127.0.0.1:10009` and `rpclisten=10.42.0.3:10009` (VPC-only)
  - `restlisten=127.0.0.1:8080` and `restlisten=10.42.0.3:8080` (VPC-only)
  - `bitcoind.rpchost=10.42.0.2:8332`
  - `bitcoind.zmqpubrawblock=tcp://10.42.0.2:28332`
  - `bitcoind.zmqpubrawtx=tcp://10.42.0.2:28333`
  - Auto-unlock enabled: `wallet-unlock-password-file=/etc/lnd/wallet.password`
- systemd: `/etc/systemd/system/lnd.service`

### Secrets Stored (GCP Secret Manager)

Created and stored (DO NOT commit/copy these into the repo):

- `oa-lnd-mainnet-wallet-password`
- `oa-lnd-mainnet-seed-words`
- `oa-lnd-mainnet-tls-cert`
- `oa-lnd-mainnet-admin-macaroon`
- `oa-lnd-mainnet-invoice-macaroon`
- `oa-bitcoind-rpc-creds` (contains `rpcuser=` and `rpcpassword=`)

### Current Status (at time of log)

- `bitcoind` is syncing mainnet (IBD in progress). Example observed state:
  - headers: ~936k
  - blocks: increasing (e.g. `346,516 -> 348,021` over ~20s during IBD)
  - connections: `10` outbound (NAT-only; no inbound peers yet)
- `lnd` wallet initialized, auto-unlock working, connected to bitcoind RPC+ZMQ, syncing to chain.
- LND node URI (public P2P):
  - `032d6a983895eee81d950b7237c00d13d839a378819d57fb759b18a892fb0c227b@34.61.69.143:9735`

### Operator Commands (Copy/Paste)

SSH (IAP):

```bash
gcloud compute ssh oa-bitcoind --zone us-central1-a --tunnel-through-iap
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap
```

Check sync:

```bash
# oa-bitcoind
sudo -u bitcoin /usr/local/bin/bitcoin-cli -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoin getblockchaininfo

# oa-lnd
sudo lncli --lnddir=/var/lib/lnd --network=mainnet getinfo
```

### How To Tell When Bitcoin Core Is "Done" (IBD complete)

Bitcoin Core is fully synced when **both** are true:

- `initialblockdownload` is `false`
- `blocks == headers` (it has validated up to the best known header)

You can check this with:

```bash
sudo -u bitcoin /usr/local/bin/bitcoin-cli -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoin \
  getblockchaininfo | jq '{blocks,headers,verificationprogress,initialblockdownload}'
```

### Confirm Blocks Are Actively Downloading (Progress Watch)

To confirm IBD is progressing, poll `blocks` and ensure the number increases over time:

```bash
while true; do
  date -u +"%Y-%m-%dT%H:%M:%SZ"
  sudo -u bitcoin /usr/local/bin/bitcoin-cli -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoin \
    getblockchaininfo | jq '{blocks,headers,verificationprogress,initialblockdownload}'
  sleep 20
done
```

Tail logs:

```bash
# oa-bitcoind
sudo journalctl -u bitcoind --no-pager -n 200

# oa-lnd
sudo journalctl -u lnd --no-pager -n 200
```

Phase 1 (GCP testnet):

- Stand up the exact same 2-VM topology on testnet.
- Validate IBD/sync, wallet ops, and Aperture connectivity without mainnet risk.

Phase 2 (GCP mainnet, minimal):

- Stand up mainnet nodes.
- Acquire minimal inbound liquidity.
- Route first paid L402 requests through Aperture.

Phase 3 (hardening):

- Remote signer (if we accept an extra component).
- Watchtower client + external watchtowers.
- Key rotation and emergency controls.

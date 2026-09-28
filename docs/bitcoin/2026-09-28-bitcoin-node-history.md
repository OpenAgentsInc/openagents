# Bitcoin and Lightning node history

Date: 2026-09-28

Scope: a historical survey of every Bitcoin and Lightning node that OpenAgents
has run, the code and runbooks that remain, and what they imply for running a
node again. It is an audit, not a runbook. Commands and file paths from deleted
history are cited so that you can recover them with `git show`; they are not
current instructions.

This document omits credential values, private addresses, and node public
IP addresses. Where a secret exists, it names the secret's purpose only.

## Summary

OpenAgents has run its own mainnet nodes on Google Cloud since
2026-02-12. Before that date, and again for the current `openagents wallet`,
it relied on hosted services.

| Period | What ran | Where | Status on 2026-09-28 |
| --- | --- | --- | --- |
| 2023-12 to 2024 | Hosted LND at Voltage, LNbits, and Alby for L402 experiments and user balances | Vendors | Gone |
| 2024-12 to 2026-05 | Breez SDK and Spark wallets (Onyx, the web wallet, Nexus treasury) | Vendors plus a wallet store on a GCE disk | Replaced |
| 2026-02-12 onward | `oa-bitcoind`: Bitcoin Core 30.2, mainnet, archival, `txindex=1`, no wallet | GCE `n2-standard-8`, 2 TB `pd-ssd` | **Still running** |
| 2026-02-13 to 2026-07-30 | `oa-lnd`: LND v0.20.1-beta on mainnet, chain backend `oa-bitcoind` | GCE `e2-standard-4`, 200 GB `pd-balanced` | Swept, deleted, final disk snapshots kept |
| 2026-03-02 | `symphony-mainnet-1`: Maestro Symphony indexer on `oa-bitcoind` | GCE `n2-standard-8`, 512 GB `pd-ssd` | Gone |
| 2026-05-17 to 2026-07-30 | `nexus-ldk-mainnet-1`: LDK Server on mainnet, chain backend `oa-bitcoind` over RPC | GCE `e2-standard-4`, 200 GB `pd-ssd` | Swept, deleted, final disk snapshots kept |
| 2026-06 to 2026-07 | MoneyDevKit (MDK) `lightning-js` treasury node, chain data from MDK | Cloudflare Containers, then Cloud Run | Payout gates off |
| 2026-08-07 onward | Immortal public regtest sandbox: two Bitcoin Core regtest nodes and three Core Lightning nodes | GCE `e2-standard-4`, 100 GB | Running |
| 2026-09-27 onward | `openagents wallet`: `ldk-node` with an Esplora chain source | Each user's machine, public Esplora | Current code |

The main facts:

- **Implementation:** Bitcoin Core only. OpenAgents never ran btcd, Knots,
  Fulcrum, or a production electrs or Esplora server. electrs appeared only in
  local regtest harnesses.
- **Network:** mainnet in production. Testnet, signet, Mutinynet, and regtest
  appear only in tests, harnesses, and the Immortal sandbox.
- **Lightning:** LND (`oa-lnd`) and LDK Server (`nexus-ldk-mainnet-1`) ran
  against the node. Core Lightning ran only on regtest.
- **Consumers:** the L402 seller gateway `l402.openagents.com` (Episode 212),
  the Symphony indexer, a Charms CAST trial, the Nexus LDK treasury that paid
  Pylon accepted work, and the August 2026 Coldcard forensic scans.
- **Why it stopped:** Pylon and treasury payments moved to MoneyDevKit on
  2026-06-07 (issue #4504), which made the self-run Lightning nodes a
  fallback. Both Lightning nodes were swept and deleted in a cost cleanup
  around 2026-07-30. The owner explicitly kept `oa-bitcoind`, which became an
  archival chain source for forensic work.

The most useful finding for a relaunch is that **the node still exists**. A
read-only `gcloud compute instances list` on 2026-09-28 shows `oa-bitcoind`
running with its original disks. A new launch can start from a synced
archival node instead of a new initial block download.

## Timeline

### 2023 to 2025: hosted Lightning

- Episode 037 (2023-12-22) modeled the flow of funds on Voltage.
  Episode 042 recalls that GPUtopia payouts every few seconds overloaded a
  Voltage-hosted LNbits instance, which is why later balances lived in a
  database and users withdrew on demand.
- Episodes 062 to 070 (2024-01 to 2024-02) built L402 payments with Alby and a
  hosted LND node.
- Episodes 143, 169, 173, 207, 208, 212, and 214 used the Breez SDK and then
  Spark. Issues #845 and #1016 chose Spark so that OpenAgents would not run
  nodes.

### February 2026: `oa-bitcoind` and `oa-lnd`

Commit `73a084962d` (2026-02-13) added
`docs/lightning/plans/GCP_BITCOIND_LND_2VM_PLAN.md`: "run our own full Bitcoin
node (`bitcoind`) and Lightning node (`lnd`) on Google Cloud." Its work log
records what was built:

- **Network:** a dedicated VPC `oa-lightning` with one subnet, Cloud NAT for
  egress, a Serverless VPC Access connector for Cloud Run, and IAP-only SSH.
- **Firewall rules:** `oa-allow-bitcoin-p2p` (8333), `oa-allow-lnd-p2p`
  (9735), `oa-allow-bitcoind-backend` (8332, 28332, 28333 from the subnet),
  and `oa-allow-lnd-rpc` (10009, 8080 from the subnet and the connector).
- **`oa-bitcoind`:** `n2-standard-8`, no external IP, 100 GB `pd-balanced`
  boot disk, and a 2,000 GB `pd-ssd` data disk `oa-bitcoind-data` with
  `auto-delete=no`.
  - Software: Bitcoin Core 30.2 from the official tarball under
    `/usr/local/bin`.
  - Config: `/etc/bitcoin/bitcoin.conf` with `txindex=1`,
    `disablewallet=1`, RPC bound to loopback and the private interface, and
    ZMQ `rawblock` and `rawtx` publishers on the private interface.
  - Service: `/etc/systemd/system/bitcoind.service`, data at
    `/var/lib/bitcoin`.
  - The plan suggested `dbcache` of 12,000 to 16,000 MB for initial block
    download, then a resize to `n2-standard-4`. The resize never happened.
- **`oa-lnd`:** `e2-standard-4` with a static external IP, a 100 GB boot disk,
  and a 200 GB `pd-balanced` data disk.
  - Software: LND v0.20.1-beta, verified against the release manifest
    checksum.
  - Config: `/etc/lnd/lnd.conf` with the `bitcoind` backend over RPC and ZMQ,
    gRPC and REST on the private interface only, and auto-unlock from a
    password file.
- **Secrets:** Secret Manager held the LND wallet password, seed words, TLS
  certificate, admin and invoice macaroons, and the `bitcoind` RPC
  credentials (`oa-bitcoind-rpc-creds`).

Liquidity was minimal (`docs/lightning/status/20260214-ep212-liquidity-bootstrap-log.md`,
commit `910d9ff573` era):

- `oa-lnd` received two 20,000-sat deposits.
- One peer refused because its minimum channel size was 500,000 sats.
- A 30,000-sat private channel opened with a push amount, and a 21-sat L402
  payment succeeded.
- The status snapshot calls this "oriented around 'EP212 works once', not
  long-term reliability."

Issue #1632 (closed 2026-02-15) moved `l402.openagents.com` from Voltage to
Aperture on Cloud Run backed by `oa-lnd`. Aperture needed a patch
(`docs/lightning/deploy/aperture-private-invoices.patch`) that forces private
invoices with route hints. Without route hints, payers failed with
`NO_PATH_FOUND` because the channels were private. Issue #2173 (closed
2026-02-24) pointed a liquidity-pool backend at LND REST.

On 2026-02-21 the Lightning docs were archived out of the monorepo. Copies
remain in `backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/`.

### March 2026: indexer and Charms reads

- Issues #2738 to #2741 (closed 2026-03-02) deployed Maestro Symphony, a
  Bitcoin indexer and HTTP API, as `symphony-mainnet-1` against
  `oa-bitcoind`. The scripts `scripts/deploy/symphony/01-*.sh` to `07-*.sh`
  and `rotate-rpc-creds.sh` included a restore drill, and
  `docs/reports/symphony/20260302T175315Z-deploy-receipt.json` recorded a
  one-block lag behind `bitcoind`. The receipt's height, 886,552, is well
  below the 2026 tip. It suggests the node was still in initial block
  download on 2026-03-02, about 18 days after it started. This reading is an
  inference.
- `docs/charms/CAST_OA_BITCOIND_CONNECTION_RUNBOOK.md` (commit `a8de1b5189`,
  2026-03-04) reached the node over an IAP SSH tunnel. By then the node was
  synced and `txindex` was complete. Read and prove steps worked. Signing
  failed with `Method not found` because the node has `disablewallet=1`.
- A Psionic audit (`psionic/docs/audits/2026-03-22-openagentsgemini-gpu-training-pilot-audit.md`)
  still lists both VMs running on 2026-03-22.

### April to June 2026: Nexus treasury from Spark to LDK to MDK

- In April the Nexus treasury ran a Breez Spark wallet on the
  `nexus-mainnet-1` VM, not a self-run node. The workspace-root analysis
  `docs/2026-04-09-nexus-payout-continuity-failure-analysis.md` and issues
  #4193, #4198, #4321, and #4409 record payout stalls, balance misreads from an
  old SDK pin, and wallet-refresh timeouts.
- `docs/2026-05-15-ldk-nexus-treasury-transition-audit.md` (commit
  `7679951e05`) decided to move the treasury to LDK because Spark had put slow
  wallet sync and leaf spendability on the payout critical path.
- Issues #4480 to #4503 built the LDK path:
  - #4481 added `NEXUS_LDK_NETWORK` and `NEXUS_LDK_CHAIN_BACKEND` (`bitcoind`,
    `electrum`, or `esplora`).
  - #4483 added a local two-node regtest harness (commit `a01a52980e`).
  - #4485 added the GCP topology (commit `0d109615a1`).
- `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md` and `scripts/deploy/nexus/22-*.sh`
  to `28-*.sh` described `nexus-ldk-mainnet-1`:
  - Host: `e2-standard-4`, Ubuntu 22.04, no external IP, and a 200 GB
    `pd-ssd` data disk.
  - Build: `ldk-server` built from a pinned ref, with private gRPC on 3536
    protected by an HMAC and a TLS pin.
  - Chain: the `[bitcoind]` chain source pointed at `oa-bitcoind` over private
    RPC. The rule `oa-allow-oa-bitcoind-rpc-from-nexus-ldk-host` allowed only
    port 8332 from the LDK host tag.
  - Peers: Lightning P2P was private by default and open only to `oa-lnd`.
  - Backups: scripts `25-backup-ldk-server-state.sh` and
    `26-restore-ldk-server-drill.sh` wrote to a Cloud Storage bucket plus disk
    snapshots.
- Incident #4503 (2026-05-17): the related issues had closed before any LDK VM
  or disk existed, and `nexus.openagents.com` returned Cloudflare 530. The
  issues were reopened and the host was built.
- `docs/reports/nexus/2026-05-18-current-system-status-audit.md` (commit
  `d39de54325`) recorded the production proof. It showed `ldk_network:
  bitcoin`, `ldk_chain_backend: bitcoind`, one channel, and a 25-sat
  accepted-work payout that settled.
- Issue #4509 used `oa-lnd` as the funding node for LDK channels and ended at
  three channels. The LDK node itself stayed near one channel and about
  2,000 sats outbound, below its readiness floor of two channels and
  20,000 sats.
- Issues #4548 and #4550 record a fix that could not deploy because
  interactive `gcloud` reauthentication blocked the headless deploy. The
  workspace now uses an automation service account for this reason.
- Commit `229895ded4` and issue #4504 (2026-06-07) moved the Pylon v0.2
  release gate to MoneyDevKit. The GCP-hosted native Nexus and LDK deployment
  became "historical production context."

### June to July 2026: MDK treasury

Issues #4698 to #4700 ran an MDK `lightning-js` node as
`MdkTreasuryContainer` on Cloudflare Containers. Issue #8531 moved it to Cloud
Run. MDK's platform and LSP provide chain data and liquidity, so no
OpenAgents chain backend was involved. Dispatch stays behind a flag that is
off by default.

### Late July 2026: teardown

The forensic document (below) states that `oa-bitcoind` "until this week was
carrying $634/month as backing for a Lightning stack that has since been swept
and decommissioned," as part of a cleanup that removed about $23,700 a month
of infrastructure. Read-only listing on 2026-09-28 confirms these final
snapshots, all dated 2026-07-30:

- `oa-lnd-boot-final-20260730` (100 GB) and `oa-lnd-data-final-20260730`
  (200 GB).
- `nexus-ldk-final-20260730` (100 GB) and `nexus-ldk-data-final-20260730`
  (200 GB).

No GitHub issue records the teardown itself. The firewall rules for `oa-lnd`
and `nexus-ldk-host` still exist and target tags that no running instance
carries.

### August 2026: forensic archive

`docs/coldcard/2026-08-01-bitcoin-node-forensic-capability.md` (commits
`8ea08d645a`, `b6cb0767a0`, `b725e0861d`; issue #9298) measured the node
live on 2026-08-01:

| Property | Value |
| --- | --- |
| Version | `/Satoshi:30.2.0/` |
| Chain | mainnet, blocks equal to headers at 960,596 |
| Pruned | no |
| `txindex` | enabled |
| Size on disk | 864 GB of 2 TB (48%) |
| Peers | 10 (outbound only, behind NAT) |
| RPC | loopback and private subnet, `disablewallet=1` |
| Cost | about $634 a month: $284 machine, $340 disk, $10 boot |

It scanned 1,701 blocks (7.12 million transactions) through `getblock` and
`getrawtransaction` and froze the evidence as content-addressed bundles in
Cloud Storage. It recommended keeping the node, cutting the machine to
`n2-standard-4` if cost mattered (about $140 a month saved), and never pruning.
The 2026-09-18 commit `dabc08102f` removed that document and most of the older
tree from the monorepo. Recover it with
`git show dabc08102f^:docs/coldcard/2026-08-01-bitcoin-node-forensic-capability.md`.

### August to September 2026: Immortal sandbox and the CLI wallet

- The `immortal` repository's public regtest sandbox (Immortal issues #41 and
  #46) runs on `immortal-public-regtest-1`. Immortal issue #54 states that
  OpenAgents does not run a funded mainnet provider.
- `crates/wallet` (issue #9777, commit `2003a31acf`) runs `ldk-node` with an
  Esplora chain source against public servers. Issue #9813 added Olympus
  LSPS1, and issue #9829 (commit `3086f8557d`) added MoneyDevKit LSPS4 on a
  fork of `ldk-node` 0.7.0. The mainnet LSPS4 check, issue #9832, is open.

## Code and configuration that exist today

### In this repository (current `main`)

| Path | What it does | State |
| --- | --- | --- |
| `crates/wallet/src/config.rs` | Networks (`bitcoin`, `testnet`, `signet`, `regtest`), default Esplora URLs (Blockstream for mainnet and testnet, mempool.space for signet, none for regtest), LSP presets (Olympus LSPS1, MDK LSPS4, Mutinynet Esplora override) | Maintained |
| `crates/wallet/src/ldk.rs` | Builds the `ldk-node` node; calls `set_chain_source_esplora` only | Maintained |
| `crates/wallet/tests/testnet.rs` | Ignored test that reaches public testnet Esplora | Maintained, manual |
| `crates/wallet/Cargo.toml` | Pins MoneyDevKit's `ldk-node` fork (0.7.0 plus the LSPS4 client) | Maintained |
| `crates/openagents-cli/src/wallet.rs`, `src/x402.rs` | `openagents wallet` and `openagents x402` commands | Maintained |
| `docs/cli/README.md` | Wallet and x402 usage, including `init --esplora URL` | Maintained |

No current code talks to Bitcoin Core RPC, and nothing deploys a node.

### In this repository's history (deleted, recoverable with `git show`)

| Path | Last commit with the file | What it holds |
| --- | --- | --- |
| `docs/lightning/plans/GCP_BITCOIND_LND_2VM_PLAN.md` | `73a084962d` (moved to `docs/plans/active/lightning/` in `6b83384aff`) | The full plan, `gcloud` provisioning, configuration keys, and work log |
| `docs/lightning/status/*.md`, `docs/lightning/runbooks/*.md`, `docs/lightning/deploy/*` | `910d9ff573` | Liquidity log, Aperture cutover, Aperture and wallet-executor Dockerfiles, Cloud Build files |
| `apps/lightning-ops/` | `910d9ff573` | TypeScript Aperture route compiler and EP212 smoke programs |
| `docs/deploy/SYMPHONY_GCP_RUNBOOK.md`, `scripts/deploy/symphony/` | `2485b8e76c` | Symphony provisioning, hardening, restore drill, and RPC credential rotation |
| `docs/charms/CAST_OA_BITCOIND_CONNECTION_RUNBOOK.md` | `a8de1b5189` | IAP tunnel to node RPC |
| `docs/deploy/NEXUS_LDK_GCP_RUNBOOK.md` | `da197710e4` | LDK Server topology on GCP |
| `scripts/deploy/nexus/22-*.sh` to `28-*.sh`, `common.sh`, `test-*-guards.sh` | `f5919c7669^` | Provision, install (writes `ldk-server.toml` with a `[bitcoind]` section and a systemd unit), read-only smoke, backup, restore drill, and shell guard tests |
| `apps/nexus-control/src/treasury_provider.rs` | `f5919c7669^` | Nexus client for LDK Server gRPC |
| `apps/pylon/tests/ldk_wallet_regtest_harness.rs`, `docs/pylon/LDK_WALLET_REGTEST_HARNESS.md`, `scripts/pylon/ldk-wallet-regtest-harness.sh` | `6163a5d42b` | Regtest harness that starts `bitcoind` and electrs through the `electrsd` crate and two LDK nodes |
| `docs/coldcard/2026-08-01-bitcoin-node-forensic-capability.md`, `tools/coldcard/` | `dabc08102f^` | Node posture, cost, scan scripts |

None of these build on current `main`. The TypeScript and Nexus trees are
gone, so treat them as reference only.

### In sibling repositories

| Path | What it does | State |
| --- | --- | --- |
| `immortal/docs/deployment/runbook-provider-debian.md` | Operator runbook: Debian 13, Postgres 17, Bitcoin Core with loopback RPC, Core Lightning with the Boltz `hold` plugin v0.3.3 or LND v0.20.1-beta over REST | Maintained |
| `immortal/crates/immortal-provider/src/bitcoind.rs`, `src/contract.rs` | Hand-written JSON-RPC client and the declared RPC method list | Maintained |
| `immortal/deploy/systemd/immortal-provider.service` | Requires `postgresql`, `bitcoind`, and `lightningd` services | Maintained |
| `immortal/scripts/test-provider-funded.sh`, `scripts/support/provider-funded/Dockerfile.bitcoin` | Regtest harness; pins Bitcoin Core 31.1 with checksums | Maintained |
| `immortal/docs/deployment/runbook-public-regtest.md`, `deploy/public-regtest/compose.yaml` | Public regtest sandbox | Running |
| `tap-ldk/crates/tap-ldk-core/src/ldk_baseline.rs`, `scripts/regtest-bitcoin.sh` | `set_chain_source_bitcoind_rpc` against a Docker `bitcoin/bitcoin:30.0` regtest node | Experimental |
| `ldk-node/tests/common/mod.rs`, `tests/docker/` | Upstream test harness: `electrsd`, `blockstream/bitcoind`, `mempool/electrs` | Fork; OpenAgents changes touch assets only |
| `backroom/openagents-doc-archive/2026-02-21-*/docs/lightning/` | Archived copies of the February Lightning docs | Archive |

## Operations knowledge

- **Sync time:** The 2026-03-02 Symphony receipt implies initial block
  download was still running after about 18 days on `n2-standard-8` with
  `pd-ssd`, NAT-only outbound peers, and `txindex=1`. The node was synced by
  2026-03-04. Treat two to three weeks as the observed bound for this
  configuration. This estimate is inferred from two data points.
- **Disk growth:** 864 GB at block 960,596 (2026-08-01), including `txindex`.
  The plan assumed about 693 GB of chain in October 2025. A 2 TB disk gives
  several years of headroom. A 1 TB disk is already too small for an
  archival node with `txindex`.
- **Costs:** about $634 a month for `oa-bitcoind`. The Lightning VMs cost
  less; no per-VM figure was recorded.
- **Security posture:**
  - `oa-bitcoind` has no external IP. RPC listens on loopback and the private
    subnet, and firewall rules admit only the subnet and the LDK host tag.
  - ZMQ has no authentication, so it stays inside the VPC.
  - The node has no wallet and no keys.
  - Admin access is IAP SSH only.
  - RPC credentials live in Secret Manager and, per the Symphony runbook, are
    rotated with a script.
  - LND used an unlock password file on disk; LDK Server used HMAC plus a TLS
    pin.
- **Backups:**
  - Bitcoin Core state was treated as re-syncable. Its data disk has
    `auto-delete=no`, and a snapshot would cut recovery time.
  - LND backups meant the seed plus periodic `channel.backup` exports to a
    versioned bucket.
  - LDK Server used backup and restore drill scripts plus disk snapshots.
- **Monitoring:** The plan named a minimum alert set: node not synced,
  `lnd` not synced to chain, and disk usage above 80%. The Nexus stack
  projected `ldk_chain_backend`, channel counts, and capacity through
  `/api/stats`. There is no evidence of a standing alert on `oa-bitcoind`
  today.
- **Incidents:**
  - #4503 (2026-05-17): issues closed before the LDK host existed.
  - #4548 and #4550 (2026-05 to 2026-06): deploys blocked by interactive
    reauthentication.
  - April 2026 Spark treasury incidents: #4193, #4198, #4321, #4409.
  - Liquidity was the recurring operational failure. Every self-run Lightning
    node sat at or near one small channel, and peers with 500,000-sat minimums
    refused smaller channels.

## Relaunch considerations

### What current code requires

1. **`openagents wallet` (`crates/wallet`)** needs an Esplora HTTP API. It
   defaults to Blockstream and mempool.space, so every user's chain view and
   broadcast go through a third party. A self-hosted Esplora endpoint is a
   drop-in change through `init --esplora URL`. `ldk-node` also offers
   `set_chain_source_bitcoind_rpc` and `set_chain_source_electrum`. Adding a
   `bitcoind` or Electrum option to `WalletConfig` would be a small code
   change, and Nexus already modeled the same three options in #4481.
2. **The x402 validator** admits only mainnet (`bc`) and testnet (`tb`)
   invoices. MDK's signet LSP runs on Mutinynet, which needs its own Esplora.
   A signet node therefore does not help x402 acceptance testing.
3. **The Immortal provider** needs Bitcoin Core on loopback with these RPC
   methods: `getbestblockhash`, `getblockheader`, `getblock`, `getrawmempool`,
   `gettxout`, `getblockchaininfo`, `estimatesmartfee`, `getrawtransaction`,
   `gettxspendingprevout`, `getmempoolentry`, `scantxoutset`, and
   `sendrawtransaction`. It does not use ZMQ. It calls `getrawtransaction`
   without a block hash, which needs `txindex=1` for confirmed transactions.
   Its test harness sets `txindex=1`. Pruning is undocumented and likely
   unsafe. It also needs Core Lightning with the `hold` plugin, or LND. The
   loopback rule means the provider must run on the same host as `bitcoind`
   or reach it through a local tunnel.
4. **LDK Server or `ldk-node` with a `bitcoind` chain source** works over
   private RPC, as `nexus-ldk-mainnet-1` showed.

### Architecture options

| Option | Serves | Cost and effort | Tradeoffs |
| --- | --- | --- | --- |
| A. Reuse `oa-bitcoind` as is | Forensics, any GCP service over private RPC, LDK Server | Already paid, about $634 a month | No new sync. No Esplora API, so it cannot serve `openagents wallet` without a code change or an indexer |
| B. Option A plus an Esplora indexer on the same VPC | Everything in A plus `openagents wallet --esplora` for our own hosts | A second VM or a larger disk. The Blockstream `electrs` Esplora fork needs roughly the chain size again in index; the `romanz/electrs` Electrum server is much smaller but speaks Electrum, not Esplora. Estimate, not measured | Serving it publicly needs TLS, rate limits, and abuse controls; keep it private at first |
| C. Option A plus a `bitcoind` chain source in `crates/wallet` | Our own hosts over a private network such as the tailnet | Small code change, no new infrastructure | RPC credentials on each client; unsuitable for arbitrary users |
| D. Owner-operated machine, such as a workstation with 2 TB NVMe | Development, Immortal regtest-to-mainnet trials, a second independent node | Hardware only; a new sync that takes days on fast NVMe (estimate) | Residential uptime and bandwidth; a failure domain separate from GCP |
| E. Pruned node plus LDK with a `bitcoind` chain source | One Lightning node | Small disk (under 100 GB) | Incompatible with Immortal's `getrawtransaction` needs, forensics, and indexers |

A recommended path, based on the evidence above:

1. Keep `oa-bitcoind` and stop treating it as residual. Snapshot
   `oa-bitcoind-data` before any change, and add the three alerts from the
   2026-02 plan.
2. Consider an in-place upgrade from Bitcoin Core 30.2 to the version that
   Immortal tests against (31.1), after reading the release notes.
3. Resize to `n2-standard-4` once no initial block download or wide scan is
   planned.
4. Decide what consumes the node first. For `openagents wallet`, prefer
   option C for our own hosts and defer a public Esplora service (option B)
   until users need it. For an Immortal provider, put the provider and
   Core Lightning on the node's host or a sibling VM with a tunnel, and start
   on mainnet with small caps. Signet adds little for the current MDK and x402
   flows.
5. Remove the stale firewall rules for `oa-lnd` and `nexus-ldk-host`, or
   document why they stay.
6. Plan liquidity before relaunching a Lightning node. Every earlier node
   failed on liquidity, not on the chain backend. The LSPS1 and LSPS4 work in
   `crates/wallet` is the current answer to that problem.

### Risks

- A single node in one zone is a single point of failure for every service
  that uses it. Earlier Nexus outages came from the host and the deploy
  path, not from Bitcoin Core.
- Opening RPC to more services widens the credential blast radius. The
  forensic document's rule applies: sandboxes receive frozen bundles, not RPC
  access.
- Running a funded Lightning node or an Immortal provider carries custody,
  backup, and liveness duties. Earlier decisions, including issue #29's open
  spending-rail choice, show the owner has not yet accepted them for the CLI.
- Late-July teardown left no issue trail. A relaunch should record its
  lifecycle in an issue.

### Open questions

1. Is `oa-bitcoind` still at the chain tip? This survey did not open an SSH
   session. Check `getblockchaininfo` over IAP before planning around it.
2. Which consumer justifies the relaunch: the CLI wallet, an Immortal mainnet
   provider, forensics, or a Lightning service node?
3. Should `crates/wallet` gain a `bitcoind` or Electrum chain source, or
   should OpenAgents run Esplora?
4. Do the 2026-07-30 snapshots need to be kept, and for how long? They hold
   channel state for closed nodes; confirm every channel closed on chain
   before deleting them.
5. Should a second node run off GCP on owner hardware for independence?

## Sources

Monorepo commits:

- `73a084962d`: GCP `bitcoind` and LND plan.
- `910d9ff573` and `6b83384aff`: Lightning status and structure.
- `2485b8e76c`: Symphony deploy.
- `a8de1b5189`: CAST runbook.
- `7679951e05`: LDK transition audit.
- `a01a52980e`, `6f4fb2cd8f`, `0d109615a1`, `f2980ff3c6`, `eba63a6139`,
  `c977306eb9`, and `daa60b4ca7`: Nexus LDK.
- `d39de54325` and `18e0b56563`: LDK production proof.
- `6163a5d42b` and `930a1ed29f`: Pylon LDK harness and runtime.
- `229895ded4`: MDK retarget.
- `6ff78c854f`: `docs/cloud/2026-07-06-cloudflare-to-google-consolidation-audit.md`.
- `8ea08d645a`, `b6cb0767a0`, and `b725e0861d`: forensic node document.
- `86625916df`, `f5919c7669`, and `dabc08102f`: deletions.
- `2003a31acf`, `b7f2caea20`, `032922cba0`, `e95f292fa3`, and `3086f8557d`:
  current wallet.

OpenAgents issues: #845, #1016, #1605 to #1615, #1632, #2133, #2173, #2738 to
#2741, #4193, #4198, #4321, #4409, #4480 to #4504, #4509, #4533, #4534,
#4536, #4548, #4550, #4698 to #4700, #8531, #9298, #9777, #9787, #9813,
#9815, #9829, #9830, #9832, and #29.

Immortal issues: #41, #46, and #54.

Transcripts: `docs/transcripts/037.md`, `042.md`, `062.md`, `063.md`, `096.md`,
`143.md`, `169.md`, `173.md`, `208.md`, `212.md`, `214.md`, `235.md`, and
`267.md`.

Sibling repositories and workspace documents:

- `immortal/AGENTS.md`, `immortal/docs/deployment/runbook-provider-debian.md`,
  `runbook-public-regtest.md`, and `swap-network-infrastructure.md`.
- `immortal/crates/immortal-provider/src/bitcoind.rs`, `src/contract.rs`, and
  `src/config.rs`.
- `tap-ldk/crates/tap-ldk-core/src/ldk_baseline.rs` and
  `tap-ldk/scripts/regtest-bitcoin.sh`.
- `ldk-node/tests/common/mod.rs`.
- `backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/docs/plans/active/lightning/GCP_BITCOIND_LND_2VM_PLAN.md`
  and the `2026-02-21-oa-rust-113/docs/lightning/status/` logs.
- `psionic/docs/audits/2026-03-22-openagentsgemini-gpu-training-pilot-audit.md`.
- Workspace root: `docs/2026-04-09-nexus-payout-continuity-failure-analysis.md`,
  `2026-05-26-moneydevkit-liquidity-offload-analysis.md`, and `NEEDS_OWNER.md`
  ("Choose the CLI wallet's spending rail").

Read-only checks on 2026-09-28 with the automation service account: `gcloud
compute instances list`, `disks list`, `snapshots list`, and `firewall-rules
list` in the production project.

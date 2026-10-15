# LND as our main Lightning backend, driven from Rust: the `lnd-sdk` plan

Analysis, 2026-10-15. Status: **proposal; no code yet.** It asks whether
OpenAgents should make LND its main server-side Lightning backend sooner
rather than later, driven from Rust through a new `crates/lnd-sdk`, and how
to get there without breaking a payout.

It is separate from the LayerTree research. That work's
[integration plan](../research/layertree/integration-plan.md#lightning-node-path-ldk-in-rust-or-a-rust-lnd-sdk-driving-lnd)
already weighs LDK against an `lnd-sdk` and sketches the crate (modules,
first RPCs, testing). This document builds on that section rather than
repeating it. The LayerTree plan's position was "build `lnd-sdk`, prove it on
regtest, promote it only for a concrete need". This one argues the need is
already here, and lays out the phases.

## Contents

1. [Summary](#summary)
2. [Today's state: every place we touch Lightning](#1-todays-state-every-place-we-touch-lightning)
3. [LND against LDK and MDK](#2-lnd-against-ldk-and-mdk)
4. [The `lnd-sdk` crate](#3-the-lnd-sdk-crate)
5. [The integration path, phased](#4-the-integration-path-phased)
6. [Risks and mitigations](#5-risks-and-mitigations)
7. [Cost and time](#6-cost-and-time)
8. [Decisions needed from the owner](#7-decisions-needed-from-the-owner)
9. [Sources](#sources)

## Summary

- **Recommendation: yes, start now, server-side only.** Build
  `crates/lnd-sdk` against a pinned LND release, prove it on regtest (P0),
  then put one production LND node on GCP that receives agent credit top-ups
  and Pro (Lightning) payments (P1). Move outbound payouts to it only after a
  shadow run and a canary with a one-switch rollback (P2). Add Loop, an LSP
  and `tapd` Taproot Assets after that (P3).
- **Why sooner.** Every payment we receive today goes through MoneyDevKit's
  LSPS4 just-in-time channel and pays its fee of about 2%. Mainnet LSPS4
  receive was never formally proven (#9832 closed as not planned). Our
  Taproot Assets path (`tap-ldk`) is a demo that has not moved since
  2026-05-30. LND has the liquidity tooling (Loop, LSPs, Lightning Terminal)
  and `tapd` today. We ran LND on mainnet from February to July 2026, and the
  archival `oa-bitcoind` it used is still running.
- **What stays.** LDK stays wherever the node must be embedded: the
  per-machine `openagents x402 node`, agent wallets and mobile. Spark stays
  the person's Wallet. The `MdkTreasuryContainer` is not touched by any
  phase here.
- **The main costs.** A Go daemon to operate (database, backups, upgrades),
  macaroon custody, and inbound liquidity capital we now provide ourselves
  instead of renting per payment from MDK.

## 1. Today's state: every place we touch Lightning

The LayerTree plan has
[a shorter version of this table](../research/layertree/integration-plan.md#where-our-bitcoin-and-lightning-code-lives-today).
This one adds who holds the keys and what each surface would become.

| Surface | Code | Node or library | Keys held by | Becomes under this plan |
| --- | --- | --- | --- | --- |
| **Central receiver** (`payTo` for every x402 call, plugin invocations, hosted resources) | [`crates/wallet`](../../crates/wallet) as `openagents wallet serve` on `oa-pay-1` ([pay host](../deployment/pay-host.md)) | MoneyDevKit's `ldk-node` fork (rev `799e5be2`, [`crates/wallet/Cargo.toml`](../../crates/wallet/Cargo.toml)) with MDK's LSPS4 JIT liquidity | Us: seed in Secret Manager `openagents-pay-wallet-seed`, written to tmpfs at start. MDK's LSP is the channel counterparty, not a key holder | P1 runs an LND receiver beside it; P2 makes LND the receiver of record; the LDK receiver is retired or kept as a warm fallback |
| **Agent credit top-ups** (#11630) | A hosted resource on the pay front, credited through `POST /v1/agents/credit` ([self-registration](../agents/self-registration.md#paying-lightning-only)) | Same receiver | Same | P1's first LND traffic |
| **Pro over Lightning** (plan) | [Pro in Bitcoin](../pro/bitcoin.md): an MDK checkout whose invoice our receiver issues and registers with MDK | Same receiver; MDK platform for checkout, renewals and webhooks | Same; MDK never holds the money | P1, provided MDK's `register_invoice` accepts an LND-issued invoice (check first) |
| **Splits and payouts** (plugin authors, hosted-resource owners, bonuses) | [`crates/pay-ledger`](../../crates/pay-ledger) payout worker, `Rails` trait ([`payout.rs:385`](../../crates/pay-ledger/src/payout.rs)) | Lightning addresses paid from the receiver wallet; Spark addresses from the payout Spark wallet, refilled from the receiver | Us: receiver seed, plus `openagents-pay-spark-seed` for the payout Spark wallet (Spark operators co-sign transfers) | P2: an `lnd-sdk` implementation of `Rails` |
| **Earnings statements** | [`crates/gateway/src/earnings.rs`](../../crates/gateway/src/earnings.rs) | None; reads the ledger, never starts or retries a payment | n/a | Unchanged |
| **Affiliate payouts** (#11698 done, [#11727](https://github.com/OpenAgentsInc/openagents/issues/11727) open) | Commission terms and a payouts-due list in the gateway | None yet: the owner sends affiliate payouts by hand | The owner's own wallet | P2: drained by the payout worker like any other share |
| **Pylon provider payouts** | `pay_ledger::pylon`, `Policy::pylon_sweeps` ([Pylon paid jobs](../compute/pylon.md#paid-jobs-p3-test-sats)) | `TestLightning` fixture and test sats only; the real wallet path is not live | n/a today | P2, when Pylon paid jobs go to mainnet |
| **Per-machine x402 node** | `openagents x402 node` ([`x402_node.rs`](../../crates/openagents-cli/src/x402_node.rs)) | `ldk-node`, Esplora chain source, optional LSP | The person, on their machine | Stays LDK (P4) |
| **The person's Wallet** (phone, computers) | [`crates/spark-wallet`](../../crates/spark-wallet), [Breez docs](../breez/README.md) | Breez SDK, Spark | The person | Stays Spark |
| **Campaign treasury** | `MdkTreasuryContainer` (source removed in `21e82ce829`; Cloud Run) | MDK `lightning-js` | Us; mnemonic in Secret Manager and a local backup that **no agent may read, print, or run a second node on** | Untouched. Moving its balance is a separate owner decision |
| **Taproot Assets** | [`tap-ldk`](https://github.com/OpenAgentsInc/tap-ldk), [`ldk-node` fork](https://github.com/OpenAgentsInc/ldk-node), OpenAgentsInc `rust-lightning` fork | Native LDK asset channels, demo-grade | Demo keys only | P3: `tapd` server-side; `tap-ldk` becomes embedded-wallet research |
| **Swap provider** | [`immortal-provider`](https://github.com/OpenAgentsInc/immortal) (`src/lnd.rs`, `src/lightning.rs`) | Operator-declared bitcoind plus CLN or a feature-gated LND REST rail with a pinned certificate | The operator | Unchanged; keeps its own minimal REST client (see [where it lives](#where-it-lives-and-why-not-immortal)) |
| **Chain backend** | `oa-bitcoind` (Bitcoin Core 30.2, archival, `txindex=1`, no wallet) | GCE `n2-standard-8`, 2 TB `pd-ssd` | No keys | P1's chain source over private RPC and ZMQ, as for `oa-lnd` before |
| **History** | [Bitcoin node history](../bitcoin/2026-09-28-bitcoin-node-history.md) | `oa-lnd` (LND v0.20.1, Feb to Jul 2026), `nexus-ldk-mainnet-1` (LDK Server) | Swept and deleted 2026-07-30; disk snapshots kept | The runbook we rebuild from |

Two things worth stating plainly:

- **"The MDK/Nexus payout bridge" is, in today's code, the MDK-backed
  receiver plus Spark.** Nexus is gone. The workspace rule that "outbound
  payout stays on the MDK/Nexus bridge" maps to the pay host's
  `crates/wallet` node (MDK's fork, MDK's LSP) and the payout Spark wallet.
  P2 is the step that changes that rule, so it needs the owner's sign-off
  and an edit to the workspace `CLAUDE.md`.
- **We already hold the keys for every server-side balance.** LND does not
  move us from non-custodial to custodial. Agent prepaid balances and owed
  earnings are already custodial ledger liabilities. What changes is which
  daemon holds the hot key, and that the liquidity is ours.

## 2. LND against LDK and MDK

Three options for the server side:

- **LND**: Lightning Labs' Go node, run by us as a separate process.
- **LDK (embedded)**: what `crates/wallet` is now, with our own LSP choice.
- **MDK (hosted liquidity)**: what we run today, LDK plus MoneyDevKit's
  LSPS4 channels and platform.

### At a glance

| Factor | LND (via `lnd-sdk`) | LDK embedded (`crates/wallet`, any LSP) | MDK (`crates/wallet --lsp mdk`, today) |
| --- | --- | --- | --- |
| Maturity | Most deployed implementation; years at exchanges, LSPs, merchants | Solid library; we carry forks (MDK's `ldk-node`, our own `rust-lightning`) | MDK's fork at a pinned rev; mainnet LSPS4 receive not formally proven (#9832) |
| Liquidity | Loop In/Out, Lightning Terminal, LSPS1 purchase from many LSPs, channel-lease markets, the widest peer set | LSPS1/2 clients; we source liquidity ourselves | None to manage; about 2% per JIT-forwarded payment |
| Taproot Assets | `tapd` v0.8 today: mint, send, universe, asset channels and RFQ through `litd` | `tap-ldk` demo; force-close, sweep recovery, a proof courier, grouped assets not done | None |
| Operations | A Go binary, its database, SCB, watchtower, upgrades | One Rust binary we already deploy | Same as LDK, plus a dependency on MDK's LSP being up |
| Keys | Seed in `lnd`, or split with a remote signer; macaroons for access | Seed in our process | Seed in our process |
| Embedding (mobile, per-machine) | No | Yes | Yes |
| Language | Go daemon, Rust driver | Rust | Rust |
| Bitcoin-only | Yes | Yes | Yes |

### Maturity and battle-testing

- **LND** has the longest production record of any implementation and the
  largest share of public nodes. Most large LSPs and exchanges run it.
- It has had real incidents, and the record matters as much as the uptime.
  In October and November 2022 two btcd wire-parsing bugs made LND nodes
  reject valid blocks with large witnesses until they upgraded. That is the
  failure mode to plan for: a consensus-adjacent bug in a dependency, fixed
  fast upstream, which hurts whoever upgrades slowly.
- **We have run it.** `oa-lnd` served `l402.openagents.com` through Aperture
  from February to July 2026. The lessons are recorded: private channels
  need route hints on invoices (the Aperture patch), and a bootstrap
  "oriented around 'EP212 works once', not long-term reliability" is not
  operations.
- **LDK** is mature as a library, but our two forks put us one step away
  from upstream. Every fork bump is our own release engineering.

### Liquidity

This is the strongest argument for LND.

- **Inbound for receiving.** On MDK we rent inbound per payment (the JIT
  fee). On LND we buy it once: an LSPS1 channel purchase from an LSP, a
  channel lease, or a Loop Out that turns our outbound into inbound.
- **Loop** (submarine swaps, `loop` v0.34) moves balance between on-chain
  and channels without closing anything. A Loop Out to a cold-storage
  address both restores inbound and sweeps the hot wallet. That one move
  covers two needs: liquidity and a hot-wallet cap.
- **Lightning Terminal** (`litd` v0.17) bundles lnd, Loop, Pool, Faraday
  (fee and channel accounting) and `tapd`, with **accounts**: off-chain
  sub-balances whose macaroon can spend only that balance. That is a
  ready-made hard spending limit for the payout worker.
- **Lightning Node Connect** (LNC) gives remote, end-to-end-encrypted access
  without opening the gRPC port. Useful for an owner-only console later; not
  needed for P1.
- **Autopilot-style tooling.** LND's built-in `autopilotrpc` is basic and we
  would not turn it on. Channel policy should be ours: a small, typed rebalance
  and fee policy in `lnd-sdk`, fed by Faraday's channel reports.
- **Channel leases.** Pool's market has thinned since its launch; check its
  current volume before planning around it. LSPS1 purchases from several
  LSPs are the safer default.

### Taproot Assets and stablecoins on Bitcoin

- `tapd` (taproot-assets v0.8) is the reference implementation: minting,
  on-chain transfers, universes for proof distribution, and, in `litd`'s
  integrated mode, asset channels with RFQ edge nodes.
- Tether announced USDT on Taproot Assets in 2025. If a dollar stablecoin
  on Bitcoin rails reaches us, it will most likely arrive through `tapd`
  edge nodes first.
- `tap-ldk` already proved interop with `litd` both ways, which de-risks a
  mixed future: `tapd` on the server, LDK asset channels on embedded
  wallets, if `tap-ldk` ever finishes.
- This fits the [Bitcoin-only rails rule](agent-payments.md): Lightning and
  Taproot Assets, no USDC, Base, Solana, EVM or Tempo.

### Operations

| Concern | What LND needs | Our answer |
| --- | --- | --- |
| The binary | A Go release, verified against the signed manifest | Pin the version; the same checksum check `oa-lnd` used |
| Database | bbolt by default (single file, compaction needs downtime) or Postgres (`db.backend=postgres`, with native SQL tables for invoices in recent releases) | **Postgres.** Native tables, Cloud SQL backups, no compaction outages. See the warning below |
| Channel backups | Static channel backups (`channel.backup`, and `SubscribeChannelBackups` to stream every change) | Stream each SCB change to a bucket from `lnd-sdk`, encrypted with age the way the pay host's backups are |
| Watchtowers | Built-in tower client and server | Two independent towers: our own small tower in a different project, plus one third-party tower |
| Upgrades | Stop, swap the binary, start; database migrations run once and are one-way | A regtest rehearsal per release, a disk snapshot first, and a version gate in `lnd-sdk` |
| HA and failover | Single writer. LND supports leader election for active-passive clusters (check the current release notes for which backends it supports) | Start with one node, managed Postgres and a tested restore runbook. Add active-passive only if downtime starts to cost real money |

**The one rule to never break:** a Lightning node must never start from
an old copy of its channel state. Broadcasting a revoked commitment loses the
channel's whole balance to the peer's penalty transaction. So a database
point-in-time restore is never a recovery tool for a live node. The recovery
tool is SCB, which asks peers to force-close and returns our balance
on-chain. The same rule is why no second node may ever run on the treasury
mnemonic.

### Macaroons, attack surface and cost

- **Macaroons are bearer credentials.** `admin.macaroon` is full spend
  authority. Bake one macaroon per role, store each in Secret Manager, and
  give each service only its own:

  | Role | Permissions | Used by |
  | --- | --- | --- |
  | `invoice` | `invoices:read`, `invoices:write`, `info:read` | the pay front (receive) |
  | `readonly` | `*:read` | health, reconciliation, dashboards |
  | `payout` | `offchain:read`, `offchain:write`, `info:read`, scoped to a `litd` account with a balance cap | the payout worker |
  | `ops` | channel open and close, `onchain:write` | an owner-run console only; never on a service |

  Add the `ipaddr` and `timeout` caveats where the caller has a stable
  address. Rotate by baking new macaroons and deleting the old root key ID.
- **Attack surface.** gRPC (10009) and REST (8080) bind to the private
  interface only, as on `oa-lnd`. Only peer-to-peer (9735) is public. LND is
  memory-safe Go, but its surface is bigger than an embedded library: a
  network daemon, a database, and credentials that work from any host that
  holds them.
- **Cost.** Infrastructure is cheap next to liquidity; see
  [section 6](#6-cost-and-time).

### The team's Rust-first preference

- Everything we write stays Rust. `lnd-sdk` turns LND into one more typed
  service with the same traits callers use today (`pay_ledger::payout::Rails`,
  `openagents_wallet::LightningWallet`). No caller learns Go, gRPC or
  macaroons.
- What we give up: we can't patch LND ourselves easily, and we follow
  Lightning Labs' release cadence. That is the trade we already accept for
  Bitcoin Core.
- What we keep: our Rust Lightning code where embedding matters, and our
  BOLT, BIP and x402 validators, which check LND's invoices exactly as they
  check LDK's.

### Verdict

LND wins server-side on liquidity, Taproot Assets and track record. LDK wins
anywhere the node must live inside our process or on a device. MDK wins on
"nothing to operate", and costs about 2% of everything we receive. At low
volume that fee is small, but so is the effort of P0 and P1. The case for
going sooner is that the work is bounded, while both the fee and the
dependence on one LSP grow with revenue.

## 3. The `lnd-sdk` crate

The module list and the first RPCs are in the
[LayerTree plan](../research/layertree/integration-plan.md#the-lnd-sdk-crate).
This section settles the open choices and adds what that sketch left out.

### Modules

| Module | LND or Lightning Labs service | Phase |
| --- | --- | --- |
| `lightning` | `lnrpc.Lightning`: info, balances, channels, peers, `AddInvoice`, `LookupInvoice` | P0 |
| `invoices` | `invoicesrpc`: hold invoices, `SubscribeSingleInvoice`, cancel and settle | P0 (subscribe), P3 (hold) |
| `router` | `routerrpc`: `SendPaymentV2` with fee limit and timeout, `TrackPaymentV2`, `EstimateRouteFee`, mission-control reads | P0 |
| `wallet` | `walletrpc` (WalletKit): addresses, UTXOs, `SendCoins`, PSBT funding, `BumpFee`, fee estimates | P0 (reads), P1 (sweeps) |
| `chain` | `chainrpc` (ChainNotifier): confirmation and spend notifications for our own on-chain sends | P1 |
| `signer` | `signrpc`: for the remote-signer setup and for any message signing | P1 |
| `state` | `StateService`, `WalletUnlocker`: readiness and unlock | P0 |
| `tap` | taproot-assets `taprpc`, `assetwalletrpc`, `universerpc`, `tapchannelrpc`, `rfqrpc` | P3 |
| `loop` | `looprpc`: Loop Out and Loop In with quotes, swap tracking | P3 |
| `litd` | `litrpc`: accounts (balance-capped macaroons), session management | P2 |
| `macaroon` | bake, constrain (`ipaddr`, `timeout`), verify, and load from Secret Manager | P0 |
| `supervisor` | spawn or attach to `lnd`, `tapd`, `litd`, `loopd` for dev and regtest | P0 |
| `service` | the traits callers use, typed errors, and adapters to `Rails` and `LightningWallet` | P0 |
| `fake` | in-process fakes of each trait | P0 |

### gRPC and protos

- **`tonic` + `prost`.** `tonic` 0.12.3 is already in our `Cargo.lock`.
- **Vendored protos, versioned against one release.** Copy the `.proto`
  files from tagged releases into `crates/lnd-sdk/proto/<project>/<tag>/`:
  lnd `v0.21.1-beta`, taproot-assets `v0.8.0`, loop `v0.34.0-beta`,
  lightning-terminal `v0.17.0-alpha` (the newest tags in our
  reference clones under `~/work/projects/lightninglabs/repos/`). They are MIT
  licensed; keep each upstream licence beside its protos.
- **Check in the generated Rust** and regenerate it with a script, so a
  normal build needs no `protoc`. A test fails if regeneration from the
  vendored protos would change the checked-in output.
- **A version gate.** On connect, `GetInfo`/`GetVersion` must report the
  pinned major and minor release. A mismatch refuses to start unless an
  explicit override is set.
- **Existing crates** (`tonic_lnd` and its forks, `fedimint-tonic-lnd`,
  `lnd_grpc_rust`) are references, not dependencies. We want control over
  the proto version and the dependency set, and none of them covers `tapd`,
  `litd` and Loop together.

### Macaroons, TLS and secrets

- **TLS:** rustls with the node's certificate pinned by SHA-256 fingerprint,
  the same approach as `immortal-provider`'s LND REST rail. No system roots.
- **Macaroons:** a `tonic` interceptor adds the hex macaroon to metadata.
  The macaroon type is a newtype whose `Debug` and `Display` print
  `macaroon(<role>, <root key id>)` and never the bytes.
- **Secrets:** loaded from Secret Manager at start by a configured name
  (`lnd-<node>-macaroon-<role>`, `lnd-<node>-tls-cert`), or from a root-only
  file the systemd unit writes to tmpfs, as the pay host does for its seed.
  The seed and wallet password are never read by `lnd-sdk`; only the
  supervisor's dev mode handles them, and only on regtest.

### Errors, retries, streaming, health

- **Typed errors.** One enum per service, mapped from gRPC status codes and
  LND's failure reasons: `NoRoute`, `InsufficientBalance`,
  `IncorrectPaymentDetails`, `AlreadyPaid`, `InFlight`, `Timeout`,
  `PermissionDenied`, `Unavailable`, `VersionMismatch`, `NotSynced`.
- **Retries only for idempotent calls.** Reads retry with backoff.
  `AddInvoice` is not retried blind: on an unknown outcome, it looks up by
  our own payment hash or creates a new invoice. **`SendPaymentV2` is never
  retried by the transport.** LND refuses a second payment to a hash that
  succeeded or is in flight, and our payout worker already resolves an
  unknown state by lookup, never by re-sending. `lnd-sdk` keeps both rules.
- **Streaming.** `SubscribeInvoices` (resuming from `settle_index`, which we
  persist), `TrackPaymentV2` per in-flight payment, `SubscribeChannelEvents`,
  `SubscribeChannelBackups`, `SubscribeState`. Each stream reconnects with
  backoff and resumes from its saved index, so a reconnect can't drop a
  settlement. A gap is closed by a paged `ListInvoices` from the saved index.
- **Health.** A loop reports synced to chain, synced to graph, active
  channel count, peer count, and inbound and outbound balance. It feeds the
  pay host's existing health file and timer.
- **Sync interface.** `LightningWallet` is synchronous today. The adapter
  wraps the async client with a dedicated runtime handle; callers don't
  change.

### The dev and regtest supervisor

- Feature `supervisor`, off in production builds.
- It starts `bitcoind`, two `lnd`, and `tapd` (or `litd`) from pinned
  binaries or containers. It creates and unlocks wallets, mines, funds, opens
  channels, waits for `SERVER_ACTIVE`, and tears everything down.
- Production never spawns LND from Rust. systemd owns the process there, and
  `lnd-sdk` only attaches.

### Testing

| Layer | What runs | When |
| --- | --- | --- |
| Unit | `fake` services; a `tonic` mock server for transport, macaroon, TLS-pin and error mapping | every `cargo test -p lnd-sdk` |
| Property | `proptest` on the typed layer: msat/sat conversions never overflow or round up; fee-cap arithmetic; invoice-subscription resume never skips or repeats a `settle_index`; error mapping is total | every test run |
| Proto | regenerate from vendored protos; no diff | every test run |
| Regtest | `scripts/lnd-sdk-regtest.sh`, docker compose with bitcoind, lnd ×2, tapd; a fixture harness that funds, opens a channel, runs invoice, pay, track, a forced reconnect mid-stream, and an asset send | opt-in (`#[ignore]`), and in CI nightly |
| Contract | the payout worker's state-machine tests against the `lnd-sdk` `Rails` adapter on regtest, including `unknown`-state recovery without a second send; the x402 validator against LND-issued description-hash invoices | opt-in, and before P1/P2 |
| Interop | [`crates/x402/tests/fixtures/lnget`](../../crates/x402/tests/fixtures/lnget) (Lightning Labs' `lnget`) paying an LND-backed `402` | before P1 |

### Where it lives, and why not immortal

**`openagents/crates/lnd-sdk`.** Payments, wallet and payouts are owned by
`openagents`. The consumers (`pay-ledger`, `crates/wallet`, the gateway, the
pay front) are all here.

Not `immortal`: its [AGENTS.md](https://github.com/OpenAgentsInc/immortal/blob/main/AGENTS.md)
gives every crate an owner-approved dependency allowlist (no `tonic`, no
`prost`) and forbids third-party Bitcoin crates. `immortal-provider` already
has its own minimal, feature-gated LND REST rail with a pinned certificate
(approved 2026-08-05). That rail fits its rules, and it should stay as it
is. Not `tap-ldk`: its stated purpose is Taproot Assets *without* LND or
`tapd`.

## 4. The integration path, phased

Each phase has acceptance checks; none starts until the one before passes.

### P0: the regtest crate (1 to 2 weeks)

**Builds:** `crates/lnd-sdk` with `lightning`, `invoices` (subscribe),
`router`, `wallet` (reads), `state`, `macaroon`, `supervisor`, `service`
and `fake`. Adapters for `LightningWallet` and `Rails`. The regtest script.

**Replaces:** nothing. No production change.

**Acceptance:**

- On regtest: balance, `AddInvoice` with a description hash for an exact
  amount, pay with a fee cap, track, and lookup in both directions.
- The x402 validator (`crates/nostr/src/x402/invoice.rs`) accepts an
  LND-issued invoice signed by the LND node as `payTo`, and refuses one with
  an inline description.
- Kill the invoice stream mid-test: after reconnect, every settlement is
  seen exactly once.
- The payout worker's recovery test passes against the adapter: an
  interrupted pay resolves by lookup, never by a second send.
- Version gate: a node of the wrong release is refused.

### P1: one production LND node receiving credits and Pro (2 to 3 weeks)

**Builds:**

- **Host `oa-lnd-1`** on our own GCE VM (not GKE: one stateful process
  doesn't need a cluster). `e2-standard-4`, Shielded VM, deletion
  protection, a static external IP for 9735 only, IAP-only SSH, private
  gRPC reachable only from `oa-pay-1`.
- **Data:** LND on Cloud SQL Postgres (private IP, automated backups), plus
  a 50 GB `pd-ssd` for `lnd`'s directory and logs.
- **Chain:** `oa-bitcoind` over private RPC and ZMQ, as `oa-lnd` did.
  Re-verify it is still running and synced first.
- **Backups:** SCB streamed on every change to an age-encrypted bucket that
  the host can write but not read, matching the pay host's pattern.
- **Watchtower:** client on, two towers.
- **Monitoring:** the `lnd-sdk` health loop into the existing health timer,
  plus alerts for "not synced", "zero active channels", "pending force
  close", "inbound below N", and "hot balance above cap".
- **Liquidity:** buy inbound with an LSPS1 purchase from one LSP, plus one
  self-funded channel to a large, well-connected node.
- **Traffic:** the pay front gets a second `Receiver`. Agent credit top-ups
  (#11630) and Pro (Lightning) invoices are issued by LND. Plugin calls and
  hosted resources stay on the LDK receiver for now.

**Replaces:** MDK LSPS4 for credits and Pro only.

**Acceptance:**

- An agent buys credit end to end with a real wallet. The gateway credits it
  exactly once (`agent-credit.jsonl`), and reconciliation matches the
  ledger to LND's `LookupInvoice`.
- The MDK Pro checkout accepts an LND-issued invoice through
  `register_invoice`, and the `subscription.created` webhook starts Pro. If
  MDK refuses invoices from a non-LSPS4 node, Pro stays on the LDK receiver
  and the finding is recorded.
- Restore drill on a regtest copy: restore from seed plus SCB, and recover
  the balance through peer force-closes.
- Restart `lnd` during an open invoice: the payment settles after restart,
  and the stream resumes without a gap.
- Payments with route hints work for private channels (the Aperture lesson).

### P2: outbound payouts move to LND (2 weeks build, 2 to 4 weeks dual run)

**Builds:** the `lnd-sdk` `Rails` adapter in production. A `litd` account
caps the payout worker's spend. `fund_spark` pays a Spark invoice from LND.
Affiliate (#11727) and Pylon shares are drained by the same worker.

**Replaces:** Lightning-address payouts from the LDK/MDK receiver, and the
receiver-side refill of the payout Spark wallet. The owner's manual
affiliate sends. Spark addresses are still paid by the payout Spark wallet.

**Continuity and rollback,** following the
Nexus payout continuity analysis (workspace root, `docs/2026-04-09-nexus-payout-continuity-failure-analysis.md`):

1. **Shadow (one week).** The worker plans each payout on both rails and
   sends on the old one. LND checks the route and fee only (`EstimateRouteFee`).
   We compare the decisions.
2. **Canary.** OpenAgents' own test payees, then one payee cohort, are
   switched to LND. Each payout row already records its rail and wallet
   reference, so a payout is always resolved on the rail that sent it, and
   never on both.
3. **Rollback is one setting.** The rail for new payouts is a config value,
   kept in persisted policy. Payouts in flight stay with their rail until
   resolved. No deploy may change it implicitly: the Nexus incident where a
   deploy reset the budget cap is the failure to design against.
4. **Payout truth order during the dual run:** LND's `TrackPaymentV2`
   result, then the ledger's payout row, then reconciliation, then public
   stats. A frozen counter is a stats problem until the first two disagree.
5. **A deploy sticks only after a post-restart payout succeeds.**

**Acceptance:** two weeks of payouts on LND with zero double sends and zero
stuck `unknown` rows older than an hour; reconciliation clean every day;
one rollback rehearsed and reversed on mainnet with a test payee.

When P2 passes, the plugin and hosted-resource receive moves to LND as
well. The LDK receiver is then kept warm for one month and retired.
Changing the `payTo` node needs no client change: x402 challenges carry it
per request. The new node does get its own replay store, as the receiver
rules require.

### P3: liquidity management and Taproot Assets (3 to 6 weeks)

- **Loop:** scheduled Loop Out to a cold address when inbound falls below a
  floor or the hot balance passes its cap. Loop In when outbound for payouts
  runs low.
- **LSP:** a second LSPS1 provider, so no single LSP is a dependency.
- **Channel policy:** a typed policy in `lnd-sdk` (target channel count,
  minimum size, fee schedule, when to close idle channels), reviewed like
  the split rules: a TOML file, versioned.
- **`tapd` stablecoins:** `tapd` (or `litd` integrated mode) beside `lnd`;
  receive a Taproot Assets stablecoin over an asset channel through an RFQ
  edge node; credit it to the ledger at the quoted rate. Start with regtest,
  then a small mainnet asset receive, then pay one asset payout.
- **Acceptance:** 30 days with no manual rebalancing; one asset receive and
  one asset payout on mainnet, reconciled.

### P4: what stays on LDK

- `openagents x402 node` on each machine, agent wallets, and mobile. A Go
  daemon can't be embedded, and these nodes belong to the person.
- `tap-ldk` continues as embedded-wallet research. Its `litd` interop is now
  the property that matters most.
- `crates/wallet` keeps its `LightningWallet` trait, now with two
  implementations: LDK for embedded use, LND for the server.
- The MDK LSP stays available for embedded nodes that need JIT inbound.

## 5. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| **Hot-wallet size.** One node holds all receipts | A cap on the hot balance (start at the size of two weeks of payouts plus channel reserves). Above it, Loop Out (or an on-chain send) to a cold address the node can't spend (multisig or Coldcard). The payout worker spends only through a capped `litd` account |
| **Seed compromise** | Phase in **LND's remote signer**: a watch-only `lnd` holds channels and talks to peers, and a separate signer `lnd` holds the seed and exposes only signing. Put the signer in a separate project with no public address, reachable only from the watch-only node. Wrap its wallet password with Cloud KMS. Cloud KMS can sign secp256k1 ECDSA, but it can't do BIP32 derivation or Schnorr/MuSig2, so it can't be the Lightning signer itself |
| **Macaroon leak** | Per-role macaroons, `ipaddr` caveats, tmpfs only, never logged (a newtype); rotation by root key ID; `admin.macaroon` stays on the host, read by nobody |
| **Force-closes and fee spikes** | Anchor channels only; a reserve of confirmed UTXOs for fee bumping (LND keeps 10,000 sats per anchor channel, up to 100,000); sweeper budgets set; few, large channels to stable peers; alerts on any pending force close; the watchtowers |
| **Restoring stale state** | No database point-in-time restore on a live node, ever. Recovery is seed plus SCB, drilled on regtest every release |
| **Upgrade breakage** (a dependency bug like btcd's in 2022) | Follow the security list; patch releases within days; rehearse on regtest with pinned protos; snapshot first |
| **LSP or peer dependence** | Two LSPs, plus self-funded channels; MDK's JIT stays as an embedded-node fallback |
| **Regulation and custody** | Unchanged in kind: we already hold agent balances and owed earnings. LND concentrates more value in one hot node, and stablecoin holdings for others add their own questions. Get a custody and money-transmission review before P2 and before any P3 asset payout. That is an owner decision |
| **Payout continuity** | The P2 shadow, canary and one-switch rollback; idempotent sends; a post-restart payout before a deploy sticks; persisted policy that deploys can't overwrite |
| **The treasury mnemonic** | No phase reads, prints, or runs a node on it. LND gets a fresh seed generated on its signer host |

## 6. Cost and time

### Infrastructure per month (us-central1, on-demand list prices, approximate)

| Item | Size | Per month |
| --- | --- | --- |
| `oa-lnd-1` | `e2-standard-4` (4 vCPU, 16 GB) | about $98 |
| Disk | 50 GB `pd-ssd` | about $9 |
| Cloud SQL Postgres | `db-custom-2-7680`, 50 GB SSD; double for HA | about $100 (about $200 HA) |
| Remote signer (P1 late or P2) | `e2-small`, separate project | about $13 |
| Watchtower | `e2-micro`/`e2-small` | $7 to $13 |
| Static IP, backups, Secret Manager, logs | | under $10 |
| `oa-bitcoind` | already running (`n2-standard-8`, 2 TB `pd-ssd`) | $0 extra; it costs about $600 a month whether we use it or not |
| **Total new** | | **about $240 to $350** |

A smaller start is possible: bbolt on the VM's disk instead of Cloud SQL
brings it near $130. We would trade backup convenience for that.

### Liquidity capital

| Need | Start with | Notes |
| --- | --- | --- |
| Inbound (receiving) | 5 to 10 million sats, bought (LSPS1) | Fees vary by LSP and term; typically 1% to 3% for six to twelve months |
| Outbound (payouts) | Funded by receipts; seed 2 to 5 million sats | Payouts can only spend what's in channels |
| On-chain reserve | 100,000 to 500,000 sats | Anchor fee bumping and sweeps |
| Opens and closes | a few thousand sats each at normal fee rates | Few large channels keep this small |

Compare with today: MDK's about 2% per received payment. At 10 million sats a
month received, that's about 200,000 sats a month. The fee alone doesn't pay
for the node until volume is several times that. **The case rests on
liquidity control, Taproot Assets and removing a single-LSP dependency, not
on fee savings at today's volume.**

### Staffing and time

| Phase | Work | Calendar |
| --- | --- | --- |
| P0 | one Rust engineer (agent-assisted) | 1 to 2 weeks |
| P1 | the same engineer plus about two days of ops; owner: funding and secrets | 2 to 3 weeks |
| P2 | the engineer; owner: rail switch sign-off | 2 weeks build plus 2 to 4 weeks dual run |
| P3 | the engineer; owner: liquidity budget, asset choice | 3 to 6 weeks |
| Ongoing | about half a day a week for upgrades, liquidity and alerts | |

## 7. Decisions needed from the owner

1. **Go:** start `crates/lnd-sdk` now (P0), rather than waiting for a
   concrete payout need, as the LayerTree plan suggested.
2. **P1 host and budget:** one GCE node on Cloud SQL Postgres (about $240
   to $350 a month), and an inbound liquidity budget of 5 to 10 million sats.
3. **Rail change:** permission for P2 to move outbound payouts off the
   MDK-backed receiver, which changes the workspace rule that payouts stay on
   the MDK/Nexus bridge.
4. **Hot-wallet cap and cold address:** the cap's size and the cold
   destination for Loop Out sweeps.
5. **Remote signer:** from P1, or at P2 when outbound value grows.
6. **Taproot Assets:** server-side assets move to `tapd`, and `tap-ldk`
   becomes embedded-wallet research.
7. **Custody review:** a legal review of custodial balances and third-party
   payouts before P2, and of stablecoin holdings before P3 asset payouts.

## Sources

- [LayerTree integration plan: LDK in Rust, or a Rust `lnd-sdk` driving LND](../research/layertree/integration-plan.md#lightning-node-path-ldk-in-rust-or-a-rust-lnd-sdk-driving-lnd)
- [Central receive, splits, payouts](2026-10-02-central-receive-and-splits.md),
  [pay host](../deployment/pay-host.md),
  [agent payments](agent-payments.md),
  [agent self-registration](../agents/self-registration.md),
  [Pro in Bitcoin](../pro/bitcoin.md),
  [Pylon paid jobs](../compute/pylon.md#paid-jobs-p3-test-sats)
- [Bitcoin and Lightning node history](../bitcoin/2026-09-28-bitcoin-node-history.md)
- Workspace root `docs/2026-04-09-nexus-payout-continuity-failure-analysis.md`
- Code: [`crates/wallet`](../../crates/wallet),
  [`crates/pay-ledger/src/payout.rs`](../../crates/pay-ledger/src/payout.rs),
  [`crates/gateway/src/earnings.rs`](../../crates/gateway/src/earnings.rs),
  [`crates/openagents-cli/src/x402_node.rs`](../../crates/openagents-cli/src/x402_node.rs),
  [`crates/spark-wallet`](../../crates/spark-wallet)
- [tap-ldk README](https://github.com/OpenAgentsInc/tap-ldk),
  [ldk-node fork](https://github.com/OpenAgentsInc/ldk-node),
  [immortal AGENTS.md](https://github.com/OpenAgentsInc/immortal/blob/main/AGENTS.md)
  and `crates/immortal-provider/src/lnd.rs`
- Lightning Labs reference clones in `~/work/projects/lightninglabs/repos/`
  (`lnd`, `taproot-assets`, `loop`, `lightning-terminal`, `lnc-core`,
  `lightning-node-connect`, `docs.lightning.engineering`)
- [LND API reference](https://lightning.engineering/api-docs/),
  [LND docs: macaroons, remote signing, Postgres, leader election, SCB, watchtowers](https://docs.lightning.engineering/),
  [Taproot Assets](https://docs.lightning.engineering/the-lightning-network/taproot-assets),
  [Loop](https://docs.lightning.engineering/lightning-network-tools/loop),
  [Lightning Terminal accounts](https://docs.lightning.engineering/lightning-network-tools/lightning-terminal/accounts)
- Issues: #9829, #9832, #10185 to #10200, #11630, #11698, #11727

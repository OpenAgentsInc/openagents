# LDK Nexus Treasury Transition Audit and Roadmap

Date: 2026-05-15

This is the separate LDK audit requested after the Nexus/Spark timeout
incident analysis. It does not evaluate LND. The working decision is to move
Nexus treasury and Pylon settlement toward LDK immediately, using Spark only as
a legacy bridge while the LDK path is proven and cut over.

## Sources Reviewed

- `competition/ldk/README.md`
- `competition/ldk/ldk-server-presentation-transcript.md`
- `competition/ldk/repos/ldk-server/README.md`
- `competition/ldk/repos/ldk-server/docs/getting-started.md`
- `competition/ldk/repos/ldk-server/docs/configuration.md`
- `competition/ldk/repos/ldk-server/docs/api-guide.md`
- `competition/ldk/repos/ldk-server/docs/operations.md`
- `competition/ldk/repos/ldk-server/ldk-server-grpc/src/proto/api.proto`
- `competition/ldk/repos/ldk-server/ldk-server-grpc/src/proto/events.proto`
- `competition/ldk/repos/ldk-node/README.md`
- `competition/ldk/repos/ldk-node/CHANGELOG.md`
- `competition/ldk/repos/rust-lightning/README.md`
- `competition/ldk/repos/lightningdevkit.org/docs/index.md`
- `competition/ldk/repos/lightningdevkit.org/docs/introduction/architecture.md`
- `competition/ldk/repos/lightningdevkit.org/docs/key_management.md`
- `competition/ldk/repos/lightningdevkit.org/docs/_blog/bolt12-has-arrived.md`
- `competition/ldk/repos/lightningdevkit.org/docs/_blog/announcing-vss.md`
- `competition/ldk/repos/lightningdevkit.org/docs/_blog/announcing-rapid-gossip-sync.md`
- `competition/ldk/repos/vss-client/README.md`
- `competition/ldk/repos/vss-server/README.md`
- `competition/ldk/repos/rapid-gossip-sync-server/README.md`
- `openagents/docs/nexus-treasury.md`
- `openagents/docs/reports/nexus/2026-04-20-treasury-wallet-recovery-runbook.md`
- `openagents/docs/reports/nexus/20260503-provider-presence-heartbeat-hotfix.md`

## Executive Decision

Move the hosted Nexus treasury path to an LDK-backed provider now.

The fastest responsible path is:

1. Put an LDK provider boundary into Nexus so Spark and LDK are swappable.
2. Stand up `ldk-server` as the first LDK daemon target because it already
   exposes gRPC, events, metrics, TLS, HMAC auth, systemd hooks, and node
   operations.
3. Keep the implementation prepared to drop from `ldk-server` to `ldk-node`
   directly if `ldk-server` preview status blocks production use.
4. Use BOLT11 invoices for immediate operator funding workflows.
5. Move durable Pylon payout targets toward BOLT12 offers, with BOLT11 as a
   per-payment compatibility path.
6. Treat Spark as a legacy bridge until no active Pylon or treasury workflow
   still depends on Spark addresses, Spark invoices, or spendable Spark leaves.

This is not just a payment-provider preference. Spark has repeatedly put slow
wallet sync, stale history, and leaf spendability on the operational critical
path. LDK gives us a standard Lightning node model with explicit event streams,
channel state, durable node identity, and standard receive/payment contracts.

## Current Spark Failure Pattern

The existing Nexus treasury path is Spark-first:

- `treasury funding-target` returns Spark receive material and optional
  Spark/Bolt11 invoices.
- Hosted Nexus pays Pylons to Spark addresses.
- Bolt11 compatibility currently does not prove that Spark leaves are spendable
  for Spark-address payouts.
- Pylon workers historically created local Spark payout destinations before
  advertising paid work.

That model has failed in ways that are not acceptable for an operator payment
rail:

- Funding target calls timed out at 10 seconds, 20 seconds, 180 seconds, and
  even 600 seconds in different reports.
- Recovery reports showed wallet inspections timing out after scanning or
  rebuilding Spark wallet storage.
- Spark wallet history could return empty payments even when balances changed.
- The wallet scanned leaves but ignored many as `SplitLocked` or
  `TransferLocked`.
- Relay logs showed `Failed to select leaves:
  TreeServiceError(InsufficientFunds)` even when nominal balance existed.
- Nexus had to add backpressure around payout dispatch because one slow or
  blocked payout target could pile up live sends.
- Runtime mitigations had to distinguish nominal balance from actually
  selectable spendable leaves.

The practical conclusion is stronger than "increase timeouts." Nexus should no
longer put Spark sync, Spark invoice creation, Spark history hydration, or
Spark leaf selection on an interactive funding or payout path.

## Why LDK

LDK is a Rust Lightning stack, not a monolithic daemon. The relevant properties
for OpenAgents are:

- Runtime pieces are intentionally pluggable: persistence, networking, chain
  source, signing, and gossip source can be chosen by the integrator.
- The core protocol implementation is used in production and exposes
  lower-level control when we need custom custody or policy.
- `ldk-node` packages the LDK core with BDK wallet support, SQLite/filesystem or
  custom persistence, bitcoind/Electrum/Esplora chain sources, Rapid Gossip
  Sync, BOLT11, BOLT12, splicing, async payments, LSPS client support, and
  experimental LSPS2 service support.
- `ldk-server` packages `ldk-node` as a daemon with gRPC, CLI, TLS, HMAC auth,
  events, Prometheus metrics, systemd hooks, Tor configuration, and an MCP
  crate.

The target OpenAgents deployment should use this layered approach:

- `ldk-server` first for fast daemonization and integration testing.
- `ldk-node` directly if daemon maturity or API churn becomes the blocker.
- Core `rust-lightning` only where Nexus needs lower-level custom signer,
  persistence, channel, pathfinding, or receipt behavior that `ldk-node` cannot
  expose cleanly.

## Important LDK Server Caveat

The presentation describes LDK Server as an enterprise-ready direction, with
Postgres, Docker, Prometheus, systemd, Tor, LSP support, and a clean API. The
current checked-out `ldk-server` repo documentation is more conservative:

- The README says it is work in progress.
- It says APIs are under development.
- It says it has not been tested for production use.
- The current operations docs describe disk-backed `keys_seed`,
  `ldk_node_data.sqlite`, and `ldk_server_data.sqlite` as the primary backup
  artifacts.

Therefore, do not couple Nexus business logic directly to `ldk-server` RPC
shapes. Wrap it behind an internal provider boundary, pin the tested commit,
and be ready to swap the provider implementation to direct `ldk-node`.

## Target Architecture

### Service Boundary

Add a `TreasuryLightningProvider` boundary under Nexus control:

```text
Nexus API / admin tools
  -> treasury operation store
  -> TreasuryLightningProvider
       -> Spark provider, legacy only
       -> LDK provider, new default
            -> LDK Server gRPC, phase 1
            -> ldk-node direct daemon/library, fallback or phase 2
```

The provider boundary must own:

- idempotency keys
- receive target creation
- outbound payment dispatch
- payment status lookup
- event ingestion
- balance and channel health snapshots
- error normalization
- receipt projection

Nexus-facing code should not know whether a receive target came from Spark,
BOLT11, BOLT12, or an LSP/JIT receive path. It should know only the durable
operation id, rail, amount, beneficiary, current state, and receipt facts.

### LDK Daemon Placement

Run the first LDK service as a sidecar or sibling service on the Nexus host:

```text
nexus-mainnet-1
  nexus-relay / nexus-control
  ldk-server
  bitcoind-backed chain source
  local encrypted or restricted storage volume
  Prometheus scrape / logs / backup job
```

Use loopback gRPC first:

- `127.0.0.1:3536`
- pinned self-signed TLS certificate
- `x-auth: HMAC <unix_timestamp>:<hmac_hex>`
- 60 second clock skew budget
- local API key read from a root-owned secret path

Do not expose the LDK gRPC port publicly during the first production phase.

### Chain Backend

Production should use bitcoind, not public Electrum or public Esplora.

The LDK Server docs explicitly warn that Electrum/Esplora are not recommended
for publicly reachable nodes because LDK cannot verify gossip against the
blockchain in the same way and malicious peers can flood the node with fake
channel announcements. Electrum/Esplora remain acceptable for local, signet,
mobile-style, or private prototype lanes.

### Storage and Backup

Back up at least:

- `<storage_dir>/keys_seed`: critical; node identity and master secret.
- `<network_dir>/ldk_node_data.sqlite`: critical; channel state and on-chain
  wallet data.
- `<network_dir>/ldk_server_data.sqlite`: useful; payment and forwarding
  history.
- TLS certificate and API key for operational continuity, though they are
  reconstructable with client reconfiguration.

Never restore the same LDK node backup into two running instances. That can
create channel-state conflicts and risk funds.

This single-writer rule must be reflected in every Nexus restore and failover
runbook.

### Event Projection

Subscribe to `SubscribeEvents` and project events into Nexus treasury operation
records:

- `PaymentReceived`
- `PaymentSuccessful`
- `PaymentFailed`
- `PaymentClaimable`
- `PaymentForwarded`

The LDK Server event stream uses a bounded broadcast channel. Slow subscribers
can miss events. Therefore, event projection must be backed by periodic
`ListPayments` reconciliation, not treated as the only source of truth.

### Payment Contracts

Immediate receive path:

- `Bolt11Receive` for operator funding invoices.
- `Bolt11ReceiveForHash` only when Nexus needs a hold/claim/fail workflow.
- `Bolt11ReceiveViaJitChannel` only after LSPS2 liquidity has been tested.

Durable Pylon payout target:

- Prefer `Bolt12Receive` offers because BOLT12 is reusable and avoids the
  per-payment BOLT11 invoice problem.
- Use `UnifiedSend` for payer-side support of BIP21, BIP353 HRNs, BOLT11, and
  BOLT12 once the target type is known.
- Keep BOLT11 payout as a compatibility path only when the beneficiary can
  supply a fresh invoice per payout.

The legacy Spark address model must not be copied into the LDK model. Standard
Lightning does not have "pay this reusable BOLT11 invoice forever." Durable
receive identity should be BOLT12, BIP353, LNURL-pay, or a provider-mediated
invoice request flow. The LDK-first target should be BOLT12.

## Mapping Current Nexus Behavior to LDK

| Current Nexus/Spark behavior | LDK replacement |
| --- | --- |
| `treasury funding-target` creates Spark receive material | `Bolt11Receive` for immediate operator invoice, later `Bolt12Receive` for reusable offers |
| Spark address as Pylon payout destination | BOLT12 offer as durable payout destination; BOLT11 only per-payment |
| Spark wallet balance/status | `GetBalances`, `GetNodeInfo`, `ListChannels`, Prometheus metrics |
| Spark payment history scan | `ListPayments` plus event projection |
| Spark send | `Bolt11Send`, `Bolt12Send`, or `UnifiedSend` |
| Spark spendability/leaf checks | Channel/liquidity health, outbound balance, route failures, no-route/precondition errors |
| Spark data sync timeout | LDK wallet sync timestamps, chain backend health, gossip/RGS freshness |
| Spark payout receipt | `PaymentSuccessful` / `PaymentReceived` event plus `GetPaymentDetails` |
| Spark leaf selection block | Insufficient outbound liquidity, failed route, failed precondition, or channel reserve constraint |
| Recovery wallet rebuild | `keys_seed` + `ldk_node_data.sqlite` restore drill with single-writer guard |

## Pylon Changes Required

Pylon cannot remain Spark-address-only.

Required changes:

1. Add payout target variants:
   - `bolt12_offer`
   - `bolt11_invoice`
   - `bip353_name`
   - `lnurl_pay`, optional if we choose to support an HTTPS invoice provider
   - `spark_address`, legacy only
2. Prefer BOLT12 offers for durable worker payout registration.
3. Add per-payment invoice request support for workers that can supply fresh
   BOLT11 invoices.
4. Keep the current Spark target only for old worker builds during migration.
5. Add a capability bit or version marker so Nexus knows whether a Pylon can
   receive over LDK-standard Lightning.
6. Update accepted-work payout records to store the target rail and the exact
   payment artifact used.
7. Update operator/admin APIs and chat tools to surface the payment rail
   clearly.

## Immediate Roadmap

### Phase 0: Provider Boundary and Local Harness

Ship this before any mainnet LDK funds move.

- Add a `TreasuryLightningProvider` trait or equivalent internal interface.
- Move Spark-specific code behind `SparkTreasuryProvider`.
- Add `LdkTreasuryProvider` with a fake/local implementation first.
- Add config:
  - `NEXUS_TREASURY_PROVIDER=spark|ldk|dual`
  - `NEXUS_LDK_SERVER_URL`
  - `NEXUS_LDK_API_KEY_PATH`
  - `NEXUS_LDK_TLS_CERT_PATH`
  - `NEXUS_LDK_STORAGE_DIR`
  - `NEXUS_LDK_NETWORK=regtest|signet|bitcoin`
  - `NEXUS_LDK_CHAIN_BACKEND=bitcoind|electrum|esplora`
- Add treasury operation rows that are not tied to Spark fields.
- Add integration tests for:
  - receive target idempotency
  - send idempotency
  - event projection
  - missed event recovery through payment listing
  - provider error normalization
- Build a local regtest or signet harness with two LDK nodes and a bitcoind
  backend.
- Prove:
  - LDK starts and reports node info.
  - A BOLT11 invoice is generated quickly.
  - A second node pays it.
  - `PaymentReceived` and `PaymentSuccessful` events project into Nexus.
  - Restart does not lose payment/channel state.

### Phase 1: Operator Funding Invoice Cutover

This replaces the slow Spark funding-target path first.

- Deploy LDK Server beside Nexus on a non-public interface.
- Use bitcoind in production.
- Add Prometheus metrics scrape and alerting.
- Add logrotate.
- Add backup job for `keys_seed` and SQLite state.
- Add restore drill runbook.
- Change admin funding invoice creation to use `Bolt11Receive` when
  `NEXUS_TREASURY_PROVIDER=ldk`.
- Keep Spark funding target available as explicit legacy fallback.
- Record phase timing for:
  - API request receipt
  - LDK RPC start/end
  - invoice returned
  - payment observed by event
  - payment reconciled by `ListPayments`

Success gate:

- Funding invoice creation p95 under 2 seconds in normal operation.
- Funding invoice creation never blocks on full wallet history hydration.
- Direct LDK receive event appears in Nexus treasury operation history.

### Phase 2: Pylon Receive Target Migration

This moves worker payout identity off Spark addresses.

- Add Pylon payout target schema variants.
- Update Pylon registration to advertise BOLT12 support when available.
- Add Nexus capability negotiation for worker payout rail.
- Update payout dispatch to prefer BOLT12 offers for durable workers.
- Add BOLT11 per-payment fallback for old or simple workers.
- Keep Spark only for old workers that cannot upgrade yet.

Success gate:

- A new Pylon can register a BOLT12 offer target.
- Nexus can pay that target through LDK.
- Accepted-work payout receipt stores the LDK payment id, rail, target, and
  event-derived terminal state.

### Phase 3: Liquidity and Channel Operations

LDK does not remove the need to manage liquidity. It makes the state explicit.

- Define inbound and outbound liquidity thresholds for Nexus.
- Add admin chat/API tools for:
  - node info
  - balances
  - channels
  - peers
  - open channel
  - close channel
  - splice in
  - splice out
  - payment status
- Evaluate LSPS2/JIT channels after basic BOLT11/BOLT12 receive works.
- Add alerts for:
  - low outbound liquidity
  - low inbound liquidity
  - stale wallet sync timestamp
  - stale RGS timestamp
  - rising failed payment count
  - missing event subscriber

Success gate:

- Admins can diagnose why a payout cannot route without reading raw service
  logs.
- A liquidity issue produces a typed Nexus degraded state, not a generic
  funding-target timeout.

### Phase 4: Spark Decommission

Start only after BOLT12/BOLT11 Pylon settlement works in production.

- Stop creating new Spark payout destinations.
- Stop returning Spark invoice material from standard funding target APIs.
- Keep read-only Spark reconciliation for historical payments.
- Add final migration report of active workers still advertising Spark targets.
- Remove Spark leaf-selection backpressure from the primary payout path.
- Archive old Spark runbooks as legacy.

Success gate:

- No active worker, runbook, admin chat tool, or Autopilot API path requires
  Spark for a new treasury or payout operation.

## Security and Custody Requirements

LDK changes the custody model from Spark service state to node/channel state.
This is better for explicit control, but it raises the cost of sloppy
operations.

Hard requirements:

- Restrict `keys_seed`, `api_key`, TLS key, and SQLite state to the service
  user.
- Never print the API key or seed in logs, docs, issues, or chat responses.
- Pin the TLS certificate for the local Nexus client.
- Keep gRPC on loopback during initial production.
- Back up state after every channel-changing operation.
- Test restore before relying on mainnet funds.
- Do not run two active instances from the same node identity or backup.
- Treat slow or failed persistence as a funds-risk event.

VSS is not a phase-one requirement for the hosted Nexus daemon. It matters
later for mobile, multi-device, or user-wallet recovery. VSS can store
encrypted Lightning state and is integrated with LDK Node, but LDK Node's own
changelog still marks remote persistence as a risk area because unrecoverable
persistence failures can panic after retries. Use local durable storage first
for Nexus.

## Rapid Gossip Sync

RGS is useful, but it is not the first production dependency for the hosted
Nexus node.

Use RGS for:

- lightweight clients
- fast route graph initialization
- mobile-style Pylon wallets
- maybe a future OpenAgents-operated RGS service

Do not use RGS as a substitute for production bitcoind-backed chain truth on
the hosted public Nexus node. The RGS docs describe a semi-trusted server
model. That is fine for performance, but Nexus treasury should keep direct
chain backend authority for the hosted settlement node.

## Operational Runbook Additions

Add or update runbooks for:

- LDK Server install and pinning.
- LDK Server config for mainnet with bitcoind backend.
- API key and TLS certificate provisioning.
- Systemd unit and restart policy.
- Logrotate.
- Prometheus scrape and alerts.
- Backup and restore.
- Mainnet funding invoice smoke.
- BOLT12 Pylon payout target smoke.
- Payment event subscriber restart.
- Missed event reconciliation.
- Spark legacy fallback usage.
- Spark decommission.

Each runbook should include exact commands, expected output fields, failure
states, and the rollback condition.

## Testing Plan

### Unit Tests

- Provider config parsing.
- HMAC metadata construction for LDK Server API.
- TLS certificate pin loading.
- Treasury operation idempotency.
- Rail-specific target validation.
- Error normalization from gRPC codes.

### Integration Tests

- Local regtest LDK invoice creation.
- Local regtest LDK invoice payment.
- Event projection.
- Payment lookup reconciliation after intentionally disconnecting the event
  stream.
- Restart during pending invoice.
- Restart after received payment.
- Insufficient balance/no-route error mapping.
- Pylon BOLT12 target registration.
- BOLT12 payout dispatch.

### Production Smoke

- Read-only `GetNodeInfo`.
- Read-only `GetBalances`.
- Generate a tiny operator invoice.
- Pay from a controlled external wallet.
- Verify Nexus sees `PaymentReceived`.
- Verify `ListPayments` agrees.
- Dispatch a bounded tiny payout to a controlled BOLT12/BOLT11 test target.
- Confirm receipt projection.

Do not run repeated live funding-target calls as a debug loop. Local or signet
reproduction should do the heavy testing.

## API and Admin Chat Requirements

Autopilot/Nexus admin tools should not expose raw LDK internals, but admins
need enough control to operate the node:

- `treasury.status`
- `treasury.createFundingInvoice`
- `treasury.listPayments`
- `treasury.getPayment`
- `treasury.listChannels`
- `treasury.openChannel`
- `treasury.closeChannel`
- `treasury.spliceIn`
- `treasury.spliceOut`
- `treasury.listPeers`
- `treasury.connectPeer`
- `treasury.payInvoice`
- `treasury.payOffer`
- `treasury.decodePaymentTarget`
- `treasury.reconcilePayments`

Every write-side command must require admin authorization, an idempotency key,
and a durable operation row.

## Open Questions

- Which exact `ldk-server` commit should Nexus pin for the first harness?
- Does the current `ldk-server-client` crate expose everything we need, or do
  we generate our own gRPC client from the proto?
- Do we want `ldk-server` long-term, or only as a bridge while we build a
  direct `ldk-node` daemon?
- What is the initial channel capital and inbound liquidity policy?
- Which peer or LSP is the first production liquidity partner?
- How soon can Pylon advertise BOLT12 offers by default?
- Do we need LNURL-pay in addition to BOLT12 for practical compatibility?
- Should Nexus operate its own RGS server later for Pylon/mobile clients?
- When is VSS mature enough for user-facing wallet recovery in OpenAgents?

## Recommended Next Work

1. Create the internal provider boundary and LDK config shape in Nexus.
2. Add a local regtest LDK harness and keep it in CI or a reproducible smoke
   script.
3. Wire the first `ldk-server` client for `GetNodeInfo`, `GetBalances`,
   `Bolt11Receive`, `ListPayments`, and `SubscribeEvents`.
4. Deploy LDK Server on signet or a private mainnet dry-run host with no public
   gRPC exposure.
5. Cut operator funding invoice creation over to LDK.
6. Add BOLT12 payout target support to Pylon.
7. Move accepted-work payouts to LDK for upgraded workers.
8. Decommission Spark from new treasury and payout operations.

## Bottom Line

LDK is the right immediate direction because it gives OpenAgents a standard
Lightning substrate with explicit node state, payment events, channel
operations, BOLT11 compatibility, and BOLT12 reusable receive targets. Spark
has already forced too many slow sync, stale history, and leaf spendability
workarounds into Nexus. The transition should begin with an internal provider
boundary and a local LDK harness, then move operator funding invoices, then
move Pylon payout targets, and only then remove Spark from new operations.

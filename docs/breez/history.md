# Breez and Spark in this repository

Date: 2026-09-28

Scope: a historical survey of every time this repository used the Breez SDK or
the Spark protocol, what worked, and why each attempt ended. It is an audit,
not a runbook. Commit hashes are recoverable with `git show`. This document
names secrets only by purpose; it prints no key.

The companion survey for self-run nodes is
[Bitcoin and Lightning node history](../bitcoin/2026-09-28-bitcoin-node-history.md).
It dates the Breez era as "2024-12 to 2026-05"; the earliest Breez code in
this repository is March 2025, and that survey does not count the June to July
2026 revival described below.

## Summary

Breez or Spark was added and removed five times between March 2025 and
September 2026. It moved real mainnet money twice: as the Nexus treasury that
paid Pylon providers (spring 2026) and as the TypeScript Pylon and treasury
rail (June to July 2026). Neither removal was a verdict on Spark's
cryptography. The reasons were:

1. **Whole-repository resets** (`acc72ba4ca`, `d6b8ce1bec`, `17aa21b544`,
   `dabc08102f`) that deleted everything, Breez included.
2. **Operational failure on a payout critical path** (May 2026): slow sync,
   stale history, and leaves that the wallet could not spend.
3. **Scope retirement** (July 2026): payments left the product entirely.
4. **x402 receiver incompatibility** (September 2026): a Spark wallet cannot
   issue the BOLT11 invoices that the x402 receiver role requires, so the
   current `openagents wallet` uses `ldk-node` instead.

## Timeline

### March to June 2025: Onyx and web wallets (Breez SDK Liquid)

- `6ab37ec59a` (2025-03-22, #757) added the Onyx mobile wallet on
  `@breeztech/react-native-breez-sdk-liquid`, on mainnet, keyed by an Expo
  `BREEZ_API_KEY`. The Onyx guide called it a "built-in Bitcoin wallet powered
  by the Breez SDK (Liquid Implementation)… allowing users to pay for agent
  usage."
- `b11f1f317a` (2025-04-23, #892) added a Vite test web wallet on
  `@breeztech/breez-sdk-liquid` 0.8.
- `b8a3aa40df` (2025-05-12, #898) replaced it with Lightspark's own
  `@buildonspark/spark-sdk` 0.1.14. That is Spark without Breez.
- `acc72ba4ca` (2025-06-04, "Zero base") reset the repository. No
  Breez-specific reason was recorded.

This era used Breez **Liquid**, a different product from Breez **Spark**.

### November 2025: Breez Spark chosen on paper

- `8d1f5f0fdd` and `2d93f8943f` (2025-11-07) wrote ADR-0008, "Breez Spark SDK
  for Marketplace Payments." It quotes the owner: "We're going to basically
  use Spark via Breez for all of our Bitcoin and Lightning stuff." It named
  the tradeoffs "Breez API key required" and "Spark Operator dependency."
- `d6b8ce1bec` (2025-12-01, "Nuke") reset the repository.

### December 2025 to February 2026: `crates/spark` in Rust

- `34251671aa` (2025-12-22, #641) implemented `SparkWallet` over
  `breez_sdk_spark::connect()`. Sending (BOLT11 and Spark addresses, #642) and
  receiving (Spark address and invoices, #643) followed.
- The dependency was `breez-sdk-spark = { git = "https://github.com/breez/spark-sdk", tag = "0.6.6" }`
  (`6f8a1fb401`). The API key was optional in configuration.
- `7077285fd4` turned off real-time sync on regtest because the sync server
  rejects requests without an API key.
- `6a03ee64d0` (2026-01-06) replaced Pylon's Cashu wallet with Spark on
  regtest, with a faucet command against Lightspark's regtest faucet. The
  Pylon v0.1 release notes say "Regtest only (mainnet support planned)."
- `7c37f594c9` added a Breez WASM browser wallet, and `378a199122` a `/wallet`
  page in `apps/web`.
- `17aa21b544` (2026-02-25) pruned the repository to `wgpui` and `vim`. The
  crate survives in
  `backroom/openagents-prune-20260225-205724-wgpui-mvp/crates/spark/`.

**What worked:** `connect`, balance, BOLT11 and Spark-address send, Spark
receive, and regtest funding from the faucet, all from a Rust crate.
**Friction:** the SDK needs a C compiler (`7cf7da4c64` stubbed it out in one
build environment), and sync without an API key failed.

### February to May 2026: desktop, Pylon, and the Nexus treasury on mainnet

- `e215fdc8fa` (2026-02-25) re-added a Spark wallet crate and a desktop wallet
  pane, keyed by `OPENAGENTS_SPARK_API_KEY`. `811d0537dc` (2026-03-11) made
  mainnet the default.
- Pylon gained a standalone Spark runtime (`1a4ea89607`). `783f33d5ff`
  embedded a default Breez API key in source. **That key is still readable in
  history**; treat it as exposed if it is ever reused.
- The Nexus treasury on `nexus-mainnet-1` held a Spark wallet and paid Pylon
  providers to Spark addresses. Incidents:
  - #4189: an on-chain deposit to a Spark address never credited.
  - #4198: the 0.6.6 pin read 0 sats where Breez 0.12.2 read 138,877 sats.
  - #4321: background sync poisoning.
  - #4193: the payout-resilience epic.
- `7e5cd81817` moved to upstream Breez and `070d8c223a` bumped it to 0.13.6.
- `7679951e05` (2026-05-15), the LDK treasury transition audit, is the
  removal decision. It says: "Spark has repeatedly put slow wallet sync, stale
  history, and leaf spendability on the operational critical path." It
  records funding-target timeouts of 10, 20, 180, and 600 seconds; empty
  history while balances changed; leaves stuck as `SplitLocked` or
  `TransferLocked`; and `TreeServiceError(InsufficientFunds)` despite a nominal
  balance.
- `c977306eb9` (#4497), `347a46ec02` (#4500), and `9bfd9fb1e1` (#4505,
  2026-05-18) removed Spark. The closeout says: "Reintroducing Spark… would
  recreate the latency/failure mode that prompted the migration." Remaining
  Spark funds were classified "not recoverable through the active OpenAgents
  production runtime."

**What worked:** real mainnet payouts to providers, Spark addresses as payout
targets, and on-chain funding. **What failed:** everything that required the
wallet to answer quickly and consistently on an interactive path. The treasury
was a single long-lived server wallet under load, which is the case where
leaf fragmentation and sync matter most. The SDK was also pinned far behind
upstream for part of this period.

### June to July 2026: Spark as Pylon's primary rail (TypeScript)

- `4a7aa31c7e` (2026-06-16, #5078) reintroduced Spark as a receive-only backup
  rail, because an MDK recipient cannot receive while offline and a Spark
  recipient can.
- `be688c6bfe` (2026-06-17) added a Spark treasury payout rail in
  `services/mdk-treasury/src/spark-treasury.mjs`.
- `2ff639a3e2` (2026-06-23, #6049) made it an invariant: Spark "is the primary
  rail and must back ALL agent payments + Machine Payments"; MDK handles
  checkouts only.
- `9d64310375` records real Tassadar training settlements of 1,005 sats over
  native Spark. Forum tips also used it.
- `21e82ce829` (2026-07-14, "retire money sites and wallet authority", #8795
  under #8777) removed it: "Payments, markets, settlement, and public proof
  are outside the Codex Workroom MVP." The runbook
  `docs/ops/2026-07-14-vp1-treasury-wallet-recovery-runbook.md` (in history)
  records about 652 sats left in Spark awaiting an owner-approved sweep.
- `d613b8ea22` (2026-08-28) deleted the TypeScript product roots, and
  `dabc08102f` (2026-09-18) removed the last non-document reference.

**What worked:** offline receive, which was the reason for the revival, and
small real settlements. **Why it ended:** product scope, not the wallet.

### September 2026: `ldk-node` instead of Spark

Issue #9777, which built the current `openagents wallet` on `ldk-node`, gives
the present rationale:

> Spark via the Breez SDK cannot satisfy the receiver role [for x402]:
> invoices are signed by the SSP node, and `BreezSdk::receive_payment` only
> takes a memo… One embedded ldk-node covers both x402 roles from one
> balance, so Spark is not carried alongside it.

The phone wallet followed on Mutinynet (`833528ed81`). The
[SDK review](sdk-review.md) rechecks the receive claim against the current
SDK (0.26); it still holds.

## Lessons for a re-add

1. **Keep Spark off any interactive critical path that must answer in
   seconds.** Treat balance, history, and spendable leaves as eventually
   consistent, reserve before sending, and show pending states in the UI.
2. **Stay current with upstream.** Several incidents came from an old pin.
3. **Separate a person's wallet from a treasury.** The failures were a
   single hot server wallet paying many recipients. A phone wallet has a
   different load profile, but agent spending under grants concentrates load
   again.
4. **Never embed the API key in source.** Earlier code did it three times.
5. **Record the retirement path before launch.** Two removals left funds
   stranded awaiting a sweep.
6. **Decide x402 receive separately.** Spark cannot be the x402 receiver.

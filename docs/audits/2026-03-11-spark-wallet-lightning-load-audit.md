# Spark Wallet Lightning Load Audit

Date: 2026-03-11
Branch audited: `main`
Audit type: static repo audit

## Question Audited

What is the current state of being able to load Bitcoin over Lightning into the built-in Spark wallet, especially now that `v0.1` is Mission Control-first and the production app only loads Mission Control by default?

Related question:

- what exists today in the older Spark wallet panes,
- what exists today in Mission Control,
- and what functionality now needs to be rebuilt into Mission Control if it is the only production shell.

## Scope

Primary docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/plans/mission-control-pane.md`
- `docs/v01.md`
- `docs/PANES.md`
- `docs/autopilot-earn/AUTOPILOT_EARN_MVP.md`

Primary code reviewed:

- `crates/spark/src/wallet.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/src/panes/wallet.rs`
- `apps/autopilot-desktop/src/spark_pane.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs`

## Executive Summary

The repo already has real Spark wallet receive and send primitives, including explicit BOLT11 invoice support in the reusable wallet layer and CLI. The older wallet panes also expose materially more wallet functionality than Mission Control does.

But for the product surface that actually matters now, the answer is blunt:

- Mission Control does not currently give the user a real manual Lightning load path into the Spark wallet.
- The production shell exposes withdraw, but not receive.
- The old wallet UI exposes receive-related controls, but the dedicated "Create Lightning Invoice" UI is still wired to the Spark invoice primitive, not the BOLT11 invoice primitive.
- The provider paid-job settlement path is not fully wired for real Lightning payout either: the provider publish path includes an amount but does not attach a BOLT11 invoice, while the buyer auto-payment path expects a `payment-required` event with `bolt11`.

So the state today is:

- underlying capability exists,
- hidden wallet-pane capability is partial,
- Mission Control release-path capability is missing,
- and the end-to-end Lightning payout loop is still not fully implemented.

## What Exists Today

### 1. The reusable Spark layer already supports the right primitives

`crates/spark/src/wallet.rs` currently exposes:

- `get_spark_address()`
- `get_bitcoin_address()`
- `create_invoice(...)` using `ReceivePaymentMethod::SparkInvoice`
- `create_bolt11_invoice(...)` using `ReceivePaymentMethod::Bolt11Invoice`
- `send_payment_simple(...)`
- `list_payments(...)`

This matters because the missing product behavior is not blocked on the crate layer. The crate already knows how to generate:

- a Spark receive target,
- an on-chain Bitcoin address,
- and a BOLT11 Lightning invoice.

### 2. The app-owned Spark worker exposes more than Mission Control uses

`apps/autopilot-desktop/src/spark_wallet.rs` wraps those primitives in app-owned commands:

- `Refresh`
- `GenerateSparkAddress`
- `GenerateBitcoinAddress`
- `CreateInvoice`
- `CreateBolt11Invoice`
- `SendPayment`

Important detail:

- `CreateBolt11Invoice` exists in the worker,
- but the desktop UI does not currently call it.

### 3. The old wallet panes still have more funding functionality than Mission Control

The older wallet surfaces in `apps/autopilot-desktop/src/panes/wallet.rs` and `apps/autopilot-desktop/src/spark_pane.rs` expose:

- refresh wallet,
- generate Spark receive address,
- generate Bitcoin receive address,
- copy Spark address,
- create invoice,
- pay invoice,
- recent payment history,
- dedicated create-invoice pane,
- dedicated pay-invoice pane.

That is materially more wallet functionality than Mission Control exposes today.

### 4. Mission Control only ships a narrow wallet slice

Mission Control currently renders:

- wallet status,
- wallet balance,
- a masked address if one is already present in state,
- and a withdraw input/button.

Mission Control does not currently expose:

- generate Lightning invoice,
- generate Spark receive target,
- generate Bitcoin address,
- copy receive target,
- QR display,
- recent payment history,
- or any explicit "load funds" action.

### 5. Startup only refreshes wallet state; it does not prepare a funding target

On startup, the Mission Control path in `apps/autopilot-desktop/src/render.rs` enqueues `SparkWalletCommand::Refresh`.

It does not also enqueue:

- `GenerateSparkAddress`
- `GenerateBitcoinAddress`
- `CreateBolt11Invoice`

So a fresh Mission Control session can show wallet connectivity and maybe balance, but still have no receive target available to actually fund the wallet.

## Mission Control Vs. Wallet Pane

| Capability | Old Spark wallet panes | Mission Control | Production usefulness now |
| --- | --- | --- | --- |
| Refresh wallet | Yes | Indirect startup refresh only | Partial |
| Show balance | Yes | Yes | Yes |
| Show connectivity | Yes | Yes | Yes |
| Show recent payments | Yes | No | Missing in release shell |
| Generate Spark receive target | Yes | No | Missing in release shell |
| Generate Bitcoin on-chain address | Yes | No | Missing in release shell |
| Copy receive target | Yes | No | Missing in release shell |
| Create invoice from UI | Yes | No | Missing in release shell |
| Create BOLT11 invoice from UI | No | No | Missing everywhere in desktop UI |
| Pay invoice / withdraw | Yes | Yes | Present |
| One-screen manual funding flow | No | No | Missing |

## Key Findings

### 1. Real Lightning settlement for paid jobs is not wired end to end

This is the most important finding.

In `apps/autopilot-desktop/src/input/reducers/jobs.rs`, both the provider result publish path and the feedback publish path call:

- `with_amount(..., None)`

That means the provider publishes an amount, but no BOLT11 invoice.

Separately, the buyer-side auto-payment path in `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs` only pays when it receives:

- `status = payment-required`
- plus a `bolt11` invoice

But the provider side does not currently publish that shape.

Practical consequence:

- the codebase models wallet-confirmed payout as the truth source,
- but the real NIP-90 Lightning payout leg that should create that wallet receive evidence is still incomplete.

This is larger than a Mission Control UX gap. It is a core earn-loop settlement gap.

### 2. Mission Control has no manual Lightning load path

For the actual `v0.1` production shell, the user cannot manually load funds into the built-in wallet over Lightning from Mission Control.

Mission Control gives the user:

- a balance readout,
- a status readout,
- and a withdraw action.

It does not give the user:

- a receive invoice action,
- a receive address generation action,
- a copy action,
- or a QR.

Given `docs/v01.md` and the current shell gating, this is now the critical product gap because the user no longer lives in the old wallet pane during the default flow.

### 3. The desktop UI label "Create Lightning Invoice" is currently misleading

The dedicated `Create Lightning Invoice` pane is wired through `build_create_invoice_command(...)` in `apps/autopilot-desktop/src/input/actions.rs`.

That path currently builds:

- `SparkWalletCommand::CreateInvoice`

not:

- `SparkWalletCommand::CreateBolt11Invoice`

And `CreateInvoice` maps to:

- `ReceivePaymentMethod::SparkInvoice`

while the separate BOLT11 path exists but is unused by the desktop UI.

So the repo currently has:

- real BOLT11 support in the reusable wallet layer and CLI,
- but a desktop UI that still routes its invoice-creation surface to the Spark invoice primitive instead.

This is the clearest existing mismatch between naming and implementation.

### 4. Mission Control wallet address display is passive and often empty

Mission Control displays:

- `spark_address` if present,
- otherwise `bitcoin_address` if present,
- otherwise `NOT GENERATED`

But Mission Control itself has no action to populate either value.

Because startup only runs `Refresh`, a fresh session will often show:

- connected wallet,
- maybe balance,
- but no usable funding target.

That makes the current wallet panel in Mission Control observational, not actionable.

### 5. The old wallet pane is effectively unreachable in the production story

The production shell is Mission Control-only.

In current code and docs:

- `PaneKind::GoOnline` is the startup pane,
- it defaults to fullscreen presentation,
- hotbar and command palette are dev-only,
- separate wallet panes remain available mainly for dev/internal use.

That means the repo still contains wallet functionality outside Mission Control, but the production UX does not actually surface it.

So this is not just "feature exists elsewhere." It is "feature exists elsewhere but is effectively absent from the shipping shell."

### 6. Wallet balance truth is only partial for rail-specific funding diagnosis

`crates/spark/src/wallet.rs` currently maps `get_balance()` to:

- `spark_sats = info.balance_sats`
- `lightning_sats = 0`
- `onchain_sats = 0`

So the desktop can show:

- a total-like wallet number,
- and payment history,

but it cannot truthfully tell the user how much of the balance is Lightning-funded versus on-chain-funded.

For the specific question "did Lightning funds load into my wallet?", this is a diagnostic gap even if total balance changes.

### 7. The default network is still regtest

`apps/autopilot-desktop/src/spark_wallet.rs` defaults `OPENAGENTS_SPARK_NETWORK` to `regtest` when unset.

Also, the reusable Spark crate rejects:

- `testnet`
- `signet`

and only meaningfully supports:

- `mainnet`
- `regtest`

Mission Control does not surface network selection or network truth in the release shell.

Practical consequence:

- even if receive UX were present, the default setup is still not an obvious mainnet user funding path unless environment and credentials are already correct.

## Can You Load Lightning Into Spark Today?

### From Mission Control

No, not in a real manual user-facing way.

What you can do:

- observe wallet status/balance,
- paste an external Lightning invoice and withdraw out.

What you cannot do:

- generate a Lightning invoice to fund the wallet,
- copy a Lightning receive target,
- or complete a one-screen load-funds flow.

### From the older desktop wallet panes

Partially, but not cleanly.

What exists:

- receive target generation for Spark and Bitcoin,
- invoice creation UI,
- payment sending UI,
- payment history.

What is still wrong:

- the desktop UI's invoice creation path is not wired to the BOLT11 primitive even though that primitive exists.

So even outside Mission Control, the desktop receive story is not yet a clean "load over Lightning" path.

### From the underlying app/CLI layer

Yes, the primitive exists.

The strongest evidence is:

- `SparkWallet::create_bolt11_invoice(...)` exists,
- `SparkWalletCommand::CreateBolt11Invoice` exists,
- and `spark-wallet-cli` exposes `bolt11-invoice`.

So the missing work is product wiring, not core wallet capability.

## What Needs To Be Rebuilt Into Mission Control

If Mission Control is the only production shell, then the following wallet functionality must move there or be re-exposed there:

1. A real `LOAD FUNDS` block
   - `GENERATE LIGHTNING INVOICE`
   - `GENERATE BITCOIN ADDRESS`
   - copy actions
   - QR rendering

2. Correct Lightning invoice wiring
   - Mission Control should use `CreateBolt11Invoice` for Lightning funding
   - if Spark invoice remains useful, it should be a separate explicitly named action

3. A visible receive target state
   - generated value,
   - copy status,
   - expiry/status if invoice-based,
   - and recent receive confirmations

4. Wallet history in the same shell
   - at least recent receives and sends
   - enough to prove that funding landed

5. Wallet network truth
   - mainnet vs regtest must be explicit in the release shell
   - not hidden in environment assumptions

6. Provider settlement wiring
   - create a per-job BOLT11 invoice,
   - publish the invoice in the NIP-90 flow,
   - align buyer auto-payment expectations with the provider publish shape,
   - keep paid state gated on wallet-confirmed evidence

## Recommended Priority Order

### P0

- Fix provider payment flow so provider-side NIP-90 settlement actually carries a BOLT11 invoice that the buyer side can pay.
- Add a Mission Control receive/load-funds block.

### P1

- Rewire the existing "Create Lightning Invoice" desktop UI to `CreateBolt11Invoice`.
- Add copy/QR/recent-receive affordances to Mission Control.

### P2

- Make wallet network truth explicit in Mission Control.
- Either populate truthful per-rail balances or stop presenting `lightning_sats` and `onchain_sats` as if they are already authoritative.

## Bottom Line

The repo is closer than "no wallet support," but farther than "Mission Control can load Lightning into Spark."

The honest state today is:

- Spark itself can do it.
- The app worker can do it.
- The CLI can do it.
- The old wallet panes partially expose it.
- Mission Control does not expose it.
- And the real paid-job Lightning settlement loop is still missing the BOLT11 wiring that should make sats actually land in the provider wallet.

If `v0.1` is truly Mission Control-only, then wallet receive/loading is now a Mission Control feature, not a side-pane feature. Right now that feature is not finished.

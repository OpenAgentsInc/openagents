# Production Bundle NIP-90 Transaction Failure Audit

Date: 2026-03-11
Branch audited: `main`
Audit type: packaged production-binary runtime audit plus focused code review

## Audit Question

Why did the signed production app still fail to complete a full real buy/sell NIP-90 compute transaction, even though:

- the app launched,
- the desktop control plane worked,
- Apple FM worked,
- the wallet connected,
- Mission Control updated,
- and session/file logging worked?

This audit explicitly excludes signing and notarization problems. It covers only runtime/product issues that still block a truthful end-to-end paid compute loop in the packaged app.

## Scope

Docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/v01.md`
- prior NIP-90 and Mission Control audits under `docs/audits/`

Code reviewed:

- `apps/autopilot-desktop/src/state/operations.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/nip90_compute_semantics.rs`
- `apps/autopilot-desktop/src/nip90_compute_flow.rs`
- `apps/autopilot-desktop/src/app_state.rs`

Runtime evidence reviewed:

- signed bundle: `target/release/bundle/osx/Autopilot.app`
- production session log: `~/.openagents/logs/autopilot/sessions/20260312T033645Z-pid68351.jsonl`
- Mission Control/runtime mirror: `~/.openagents/logs/autopilot/latest.jsonl`
- desktop control manifest: `~/.openagents/logs/autopilot/desktop-control.json`

## Executive Summary

The packaged app passed the infrastructure check and failed the product check.

What worked:

- the signed bundle launched,
- the local desktop control server came up,
- `autopilotctl` could drive the packaged app,
- Apple FM was healthy,
- Spark wallet connected,
- session logs and desktop-control logs were written correctly.

What did not work:

- the buyer path could get stuck in `requesting-payment` even while real provider events were arriving,
- the provider path could finish compute, publish a result, mint an invoice, and still die unpaid,
- Mission Control and desktop-control projection could show misleading or impossible authority/phase states,
- and the buyer path still does not enforce its approved budget before queueing payment.

Bottom line: the packaged binary is operational, but the NIP-90 transaction loop is still not trustworthy enough to claim "full production buy/sell works."

## What Was Actually Tested

The packaged app was opened and then controlled through the app-owned desktop control runtime instead of direct UI clicking.

The validation included:

- wallet refresh
- Apple FM refresh
- provider online/offline transitions
- live buyer buy-mode dispatch with approved budget `2` sats
- live relay-backed seller acceptance/execution
- review of desktop-control snapshots and persistent JSONL session logs

This is important because the failures below are not speculative static-code concerns. They were observed in the signed production bundle.

## Finding 1: Buyer can stall forever in `requesting-payment`

### What happened

The packaged buy-mode request `d7fc51d5f5c9aaa3bddfdbdf635d06ea5de23fd8d62646d2952412d4110322fa` never advanced to a payable state.

The persistent buyer snapshot showed:

- `phase=requesting-payment`
- `status=result-received`
- `next_expected_event=valid provider invoice`
- `payable_provider_pubkey=null`
- `pending_bolt11=null`
- `payment_pointer=null`
- `payment_notice=provider returned payment-required without bolt11 invoice; waiting for a valid invoice event`

At the same time, the session log proved that multiple providers had responded:

- provider `a018ba05af40...` sent a `result` with `status=success` but no invoice
- provider `dc52438efbf9...` sent `payment-required` feedback with `amount_msats=21000` and `bolt11_present=true`
- provider `101fce8bea02...` sent `payment-required` feedback with `amount_msats=25000` and `bolt11_present=true`
- provider `5c22920b9761...` later errored with `Payment timeout — invoice expired`

### What that means

The packaged buyer was not stuck because the network was silent. It was stuck because no single provider satisfied the buyer's current "payable winner" rule:

- a usable invoice and
- a non-error result
- from the same provider

That is consistent with the current shared selection semantics in `nip90_compute_semantics.rs`:

- `provider_has_payable_result(...)` requires both a valid invoice and a non-error result
- `select_payable_winner(...)` only selects a provider once that condition is true

That part is conceptually correct.

### What is still wrong

The projection and operator truth are still bad:

- `build_buyer_request_flow_snapshot(...)` in `nip90_compute_flow.rs` sets `selected_provider_pubkey` from `last_provider_pubkey.or(winning_provider_pubkey)`, so the UI can look like it has chosen a provider even when it has not chosen a payable winner.
- The buyer kept showing the generic notice `provider returned payment-required without bolt11 invoice` even after invoice-bearing feedback had been observed from other providers.
- The Mission Control line reduced the real failure to `losers ignored: no invoice`, which was too coarse. The actual market state was "invoice-bearing providers existed, but none also had a matching payable result."

### Root cause

This is not a raw parser failure. `provider_ingress.rs` logs `bolt11_present=true` for some providers, which means invoice extraction worked.

The real product bug is that the buyer state model does not distinguish clearly enough between:

- last noisy provider,
- provider with latest non-error result,
- provider with latest valid invoice,
- and true payable winner.

## Finding 2: Approved budget is not enforced before queueing payment

### What happened

The live buyer budget was `2` sats, but the session log showed invoice-bearing provider feedback at:

- `21000` msats
- `25000` msats
- `100000` msats

That means the market was actively offering invoices above the approved budget.

### Code reality

The buyer path currently records invoice amount and selects payable winners, but `prepare_auto_payment_attempt_internal(...)` in `state/operations.rs` never checks whether the invoice amount exceeds `request.budget_sats` before it sets:

- `pending_bolt11`
- `status = PaymentRequired`
- `pending_auto_payment_request_id`

`provider_ingress.rs` then queues `SparkWalletCommand::SendPayment` using that amount.

### Why this matters

In the observed live run, the buyer did not pay because no provider satisfied the same-provider result+invoice rule. That avoided a bad spend by accident.

If a single provider does supply both:

- a valid invoice and
- a valid result

the current code path can auto-pay an invoice above the approved buy-mode budget.

That violates the CLI/UI contract and `docs/MVP.md`'s requirement that wallet/payout truth be explicit and trustworthy.

## Finding 3: Seller can complete compute and still fail unpaid

### What happened

The provider accepted and ran request `80fe308884777b4d6bdc3f82e938938234750d38f65a223df268759aaa7267ae`.

The session log showed the full sell-side flow up to settlement wait:

- request accepted
- Apple FM execution started
- result signed
- result publish queued
- result publish confirmed on relays
- job reached delivered state
- provider entered `awaiting wallet-authoritative payment evidence`
- Lightning invoice was created

The job then failed with:

- `job settlement timed out after 195s while awaiting payment flow`

### What that means

The provider-side app logic got much farther than earlier bugs:

- compute worked
- result delivery worked
- invoice creation worked

The failure was specifically that no buyer payment became wallet-authoritative before the settlement continuity window expired.

### What is product-wrong here

The current system still collapses two very different realities into a generic failed terminal state:

1. local/provider bug
2. external market nonpayment after delivery

For an earner-facing product, those are not the same thing. The second case means:

- the provider did the work,
- the provider delivered the result,
- the provider requested payment,
- but the market did not settle.

That is economically important and should be projected distinctly.

## Finding 4: Active Job projection can show impossible paid state

### What happened

During the packaged seller run, desktop-control emitted an `active_job.lifecycle.changed` event for request `80fe3088...` with:

- `stage=running`
- `phase=paid`
- `next_expected_event=none`
- `payment_pointer=null`

Later the same job failed unpaid.

### Why this is wrong

That state is impossible. A job cannot truthfully be:

- still `running`,
- have no payment pointer,
- and already be in `phase=paid`.

### Likely code seam

`build_active_job_flow_snapshot(...)` in `nip90_compute_flow.rs` treats the job as paid when either:

- `authoritative_payment_pointer(job.payment_id)` is true, or
- `job.ac_settlement_event_id.is_some()`

That second condition is too broad for phase authority. An AC settlement feedback event is not the same thing as wallet-authoritative settlement. Even if it is set later in the normal happy path, it is not a valid substitute for real wallet payment proof when determining `phase=paid`.

Even if this exact log came from stale or reused state, the product invariant is still missing: the projection layer allowed an impossible paid view to be emitted.

## Finding 5: Production verification is still too market-dependent

The packaged-binary verification run used live relays and real counterparties. That is useful, but it is not enough to certify the production bundle.

The current proof surface tells us:

- the app binary launches,
- control-plane sync works,
- logging works,
- and live market events can be observed.

It does not tell us deterministically that the bundled app can complete:

- buyer request publish,
- provider accept,
- local execution,
- result publish,
- invoice publish,
- buyer payment,
- provider settlement

inside the actual `.app` bundle on demand.

Right now, the app has deterministic headless harnesses for much of this logic, but not a packaged-app deterministic e2e that proves the exact shipping surface.

## Fix Plan

### 1. Split buyer provider roles into explicit fields

In `SubmittedNetworkRequest`, `BuyerRequestFlowSnapshot`, and Mission Control projection:

- add `result_provider_pubkey`
- add `invoice_provider_pubkey`
- keep `payable_provider_pubkey`
- stop using `last_provider_pubkey` as the primary "selected" provider field

Then update `build_buyer_request_flow_snapshot(...)` so the pane tells the truth:

- "best result arrived from X"
- "best invoice arrived from Y"
- "no payable winner yet"

### 2. Replace sticky generic payment notices with derived blocker reasons

Instead of one reused string like `provider returned payment-required without bolt11 invoice`, derive an explicit blocker code from the current observation set:

- `result_without_invoice`
- `invoice_without_result`
- `invoice_over_budget`
- `invoice_missing_bolt11`
- `invoice_expired_before_selection`
- `loser_provider_noise_only`

This should be surfaced in:

- Mission Control buy-mode status
- `desktop_control` snapshot
- buy-mode payment history rows
- JSONL runtime logs

### 3. Enforce budget at payment selection time and again at dispatch time

In `state/operations.rs`:

- reject any invoice whose sats amount exceeds `request.budget_sats`
- do not set `pending_bolt11` for over-budget invoices
- record an explicit nonterminal blocker reason instead

In `provider_ingress.rs`:

- refuse to enqueue `SparkWalletCommand::SendPayment` when the invoice exceeds the approved budget
- log the refusal with budget, invoice amount, provider pubkey, and request id

Add regression tests for:

- invoice under budget and payable
- invoice over budget with matching result
- invoice over budget from nonwinning provider
- invoice with no amount metadata but decodeable over-budget BOLT11

### 4. Make seller timeout states economically truthful

In `jobs.rs`, `nip90_compute_flow.rs`, and Mission Control projection:

- separate `delivery_timeout` from `settlement_timeout`
- add a distinct post-delivery unpaid terminal projection such as `delivered_unpaid_timeout`
- keep result event id, invoice id, and continuity window visible in that state

That makes it clear whether:

- the provider failed to do work, or
- the provider did the work and the buyer never paid

### 5. Remove paid-state inference from non-wallet signals

In `build_active_job_flow_snapshot(...)`:

- only treat a job as `phase=paid` when there is real wallet-authoritative evidence
- do not let `ac_settlement_event_id` alone imply payment authority

Add invariant tests that fail if any active-job snapshot ever emits:

- `phase=paid` without wallet payment evidence
- `stage=running` with `phase=paid`
- `stage=failed` with `phase=paid` and no payment pointer

### 6. Add packaged-app deterministic e2e verification

Build a production-bundle smoke harness that:

- launches `Autopilot.app`
- waits for `desktop-control.json`
- drives it through `autopilotctl`
- uses a local relay and controlled buyer/provider peers
- asserts on `latest.jsonl` and session log output

This test must prove the full bundle loop:

- provider online
- buyer request published
- provider accepts
- local execution completes
- result publish confirms
- invoice publish confirms
- buyer wallet pays
- provider wallet records settlement

### 7. Improve log truth for agent and operator use

The file-backed logging is good enough to support agent control now, but it still needs better semantic rows.

Add explicit runtime log events for:

- buyer result candidate observed
- buyer invoice candidate observed
- buyer invoice rejected over budget
- buyer winner unresolved because result and invoice came from different providers
- provider delivered awaiting buyer settlement
- provider delivered unpaid timeout

That makes terminal agents and humans read the same truth.

## Recommended Implementation Order

1. Fix budget enforcement before any further live-sats buyer testing.
2. Fix buyer snapshot truth so Mission Control stops implying a winner when none exists.
3. Fix active-job paid-phase inference.
4. Split unpaid-post-delivery seller timeout from generic failed state.
5. Add packaged-app deterministic e2e.

## Final Assessment

The packaged production binary is closer than it looks.

The hard parts that are already working:

- app launch
- control-plane sync
- Apple FM execution
- relay connectivity
- Spark connectivity
- file-backed logs
- result publish continuity

The remaining blockers are not "desktop app is broken in general." They are specific transaction-truth bugs:

- buyer winner selection/projection is still too ambiguous,
- buyer budget protection is incomplete,
- seller unpaid-delivery is not projected honestly enough,
- and the app still lacks a deterministic proof that the shipping `.app` can complete a full paid loop under controlled conditions.

Until those are fixed, the product cannot honestly claim that the packaged production binary reliably completes the full buy/sell compute transaction.

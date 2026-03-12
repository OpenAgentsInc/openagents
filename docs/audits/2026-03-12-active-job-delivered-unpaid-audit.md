# Active Job Delivered-Unpaid Audit

Date: 2026-03-12
Issue: `#3414` `Active Job hanging on delivered`
Audit type: runtime failure audit plus focused code review

## Audit Question

Why are seller-side jobs repeatedly reaching `delivered`, sitting in `awaiting-payment`, and then timing out unpaid, even though:

- local execution succeeds,
- result publish succeeds,
- provider invoice creation succeeds,
- `payment-required` feedback publish succeeds,
- and the wallet keeps refreshing?

This audit focuses on the actual runtime/product failure. It is not a signing/notarization audit.

## Scope

Docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/v01.md`
- `docs/audits/2026-03-11-production-bundle-nip90-transaction-failure-audit.md`

Code reviewed:

- `apps/autopilot-desktop/src/input/reducers/jobs.rs`
- `apps/autopilot-desktop/src/input/actions.rs`
- `apps/autopilot-desktop/src/input/reducers/wallet.rs`
- `apps/autopilot-desktop/src/nip90_compute_flow.rs`
- `apps/autopilot-desktop/src/nip90_compute_domain_events.rs`
- `apps/autopilot-desktop/src/app_state.rs`

Issue evidence reviewed:

- `gh api repos/OpenAgentsInc/openagents/issues/3414`
- `gh api repos/OpenAgentsInc/openagents/issues/3414/comments`

## Executive Summary

The seller is not stuck in a fake `delivered` state because local execution or invoice generation failed. The seller is reaching the truthful end of the provider-controlled portion of the flow:

- request accepted
- local compute executed
- result published to relays
- Spark Lightning invoice created
- `payment-required` feedback published
- seller begins waiting for buyer settlement

The repeated failure is that no buyer settlement becomes wallet-authoritative before the settlement continuity window expires.

That means the core problem behind `#3414` is not primarily "the Active Job pane hangs." The deeper problem is:

- the app still auto-accepts arbitrary valid open-network jobs by default,
- those jobs are not payment-assured,
- and in a seller-first `v0.1` release, that exposes providers to repeated unpaid execution.

The current UI is not fully wrong. It is showing a real state: `delivered`, then `awaiting-payment`, then `delivered-unpaid` timeout. The product problem is that we are still treating speculative open-market demand as acceptable default seller work in a release whose stated purpose is to bootstrap a reliable seller market.

## What The Issue Evidence Shows

## First successful paid job exists

The issue body includes a prior successful settlement for request `2883fa2dd40259d137f680d0f6297bfeabd11bc061192cc9d033720b78546ca3`:

- `provider.result_published`
- `provider.settlement_confirmed`
- wallet balance updated to `2`

That matters because it proves the basic seller path can work on this machine:

- Apple FM execution works
- Spark receive-side works
- provider success feedback publish works
- the UI can reach `paid`

So `#3414` is not "seller can never get paid." It is "seller is repeatedly taking jobs that do not settle."

## Repeated failing jobs all follow the same pattern

For failing requests like:

- `727a8d5f93d71555ffbf642e399e9bfa4238a21c9cdcda3eb21290d8fd68fbf6`
- `7cb053a2dd481f2c6c32bdba7d84402076b73e878654f6027ee46e689f5f7990`
- `005447edd0aa0d1185082ec205569b581c3bba95740005424a787398a59456d8`
- `e8fca4729780a6099b16edf23af204ded05dbfefa67c44e6299d72ea800edb04`
- `59f7523f655aacf628dec8606461e16c225046730d32f01ed14639aa422d22f4`
- `99ef267509bdb87ed45e42821942b56aa0264a80d1f06a688692c49943c7711b`

the logs show the same sequence:

1. accepted by provider policy
2. Apple FM generation starts and completes
3. NIP-90 result publish is confirmed
4. active job moves to `delivered`
5. provider queues Spark invoice creation
6. provider records `provider.payment_requested`
7. active job moves to `phase=awaiting-payment`
8. repeated wallet refreshes occur
9. no receive payment appears
10. job times out as `provider.delivered_unpaid_timeout`

That is not a local execution failure. It is a post-delivery nonpayment failure.

## Wallet evidence says the seller truly was not paid

The issue logs show repeated wallet refreshes while waiting:

- `Provider queued wallet refresh while awaiting payment evidence`
- Spark sync completes
- balance remains `2`
- no new settled receive payment is found

The timeout then fires with:

- `job delivered but unpaid timed out after 195s while awaiting buyer settlement`

This strongly indicates no buyer settlement actually arrived. There is no evidence in `#3414` that a payment landed and the seller app simply failed to notice it.

## The provider code path is doing what it says it is doing

The core sell-side code confirms the behavior seen in the logs.

In `jobs.rs`, `queue_active_job_payment_required_feedback(...)` does this for delivered open-network jobs:

- if there is already a `pending_bolt11`, it publishes canonical NIP-90 `payment-required`
- otherwise it queues `SparkWalletCommand::CreateBolt11Invoice`

In `wallet.rs`, the provider-side Spark invoice update reconciler moves that invoice into the active job state, which then allows `payment-required` publish.

In `actions.rs`, `run_open_network_paid_transition_reconciliation(...)` does not mark the job paid unless it can resolve a wallet settlement pointer for the delivered job. If it cannot, it:

- queues periodic wallet refreshes
- records `awaiting wallet-authoritative payment evidence`
- keeps waiting until the deadline

Then in `jobs.rs`, timeout handling emits `provider.delivered_unpaid_timeout`.

This all matches the issue logs exactly.

## Finding 1: The app is not stuck before payment; it is timing out after honest delivery

Severity: high

The issue title says "hanging on delivered." The runtime evidence says something more specific:

- the provider delivered work,
- requested Lightning settlement,
- and then the buyer never paid.

That distinction matters. The existing state machine is broadly telling the truth:

- `Stage: delivered`
- `Flow authority: wallet`
- `Flow phase: awaiting-payment`
- `Next event: wallet settlement`

The seller is not blocked on relay publish or local execution at that point. It is blocked on money.

### Why this matters

If we misdiagnose this as a generic seller-state-machine bug, we will miss the real release problem:

- the seller is spending compute on unpaid-risk jobs that the product currently treats as normal auto-acceptable work.

## Finding 2: Default provider auto-accept policy still admits speculative open-network demand

Severity: critical

`next_auto_accept_request_id_for(...)` in `jobs.rs` currently auto-accepts the first pending valid request when:

- provider mode is online
- provider blockers are clear
- inflight count is below limit

`request_accept_block_reason(...)` only blocks on:

- offline/preview mode
- validation not complete / invalid request
- minimum price
- minimum TTL
- inference backend incompatibility
- generic provider blockers
- inflight limit

What it does not check:

- whether the demand source is `StarterDemand` versus `OpenNetwork`
- whether the buyer is trusted
- whether the buyer has any settlement history
- whether the demand class is explicitly safe for seller-first bootstrap
- whether the job is coming from an app-owned targeted peer or from general public relay noise

That means the current default policy for a seller on the public network is effectively:

- if a request is syntactically valid and pays at least `1` sat and has at least `30s` TTL, run it

That is too weak for a release whose stated purpose is to bootstrap a reliable seller network.

## Finding 3: `v0.1` product intent and current default accept policy are misaligned

Severity: critical

The current `v0.1` messaging is seller-first:

- get people online
- assign jobs
- pay sellers
- make payout truth visible

But the observed runtime behavior still defaults to auto-accepting speculative open-network jobs that may never settle.

That is not just an engineering gap. It is a product-policy contradiction:

- seller-first bootstrap implies reliable or at least bounded-risk demand
- the current provider default still behaves like an open public market node

So the issue is not only "payment did not arrive." It is that the shipped product still invites sellers to execute untrusted market demand by default.

## Finding 4: Settlement reconciliation is still heuristic, even though this issue looks like real nonpayment

Severity: medium

`resolve_wallet_settlement_pointer_for_open_network_job(...)` in `actions.rs` resolves a payment pointer by scanning recent settled receive payments and matching on:

- `direction == receive`
- settled status
- non-empty pointer
- not previously used
- not synthetic
- amount matching `quoted_price_sats`
- timestamp at or after invoice creation time

This is adequate for a simple MVP, but it is not strong settlement binding.

Risks:

- if multiple same-amount invoices exist close together, the wrong receive payment could be selected
- if Spark metadata or timestamp behavior changes, a valid receive payment could be missed
- settlement is not being bound to an explicit invoice/payment hash at the app level

Important nuance: this does not appear to be the primary cause of `#3414`, because the balance stayed flat and no receive payment was visible at all. But the heuristic remains a correctness risk for future settlement edge cases.

## Finding 5: The UI makes a real economic failure feel like a stuck pane

Severity: medium

The Active Job pane is technically telling the truth, but it is still easy for an operator to read the state as "the app is hung" instead of "the buyer has not paid."

Current presentation:

- `Stage: delivered`
- `Flow phase: awaiting-payment`
- `Next event: wallet settlement`

What is missing:

- a stronger distinction between `delivered` and `delivered but unpaid`
- a clear statement that work already left the seller and the remaining blocker is buyer settlement
- a higher-signal warning that open-network jobs are speculative demand

This matters because the operator sees completed work and a static pane, and reasonably concludes the app is frozen.

## Finding 6: The issue comment about older timestamps is a secondary log/projection problem

Severity: low

One issue comment notes that after going offline, the log stream showed older timestamps about prior accept/run/deliver states.

That looks like a separate projection/log replay problem:

- historical lifecycle events from the active job or inbox are being surfaced later than expected
- the pane/log stream does not clearly separate "new runtime event" from "historical event being replayed/rendered"

This is confusing, but it is not the root cause of unpaid jobs. It is a secondary observability problem.

## What This Issue Is Not

The evidence does not support these interpretations:

- not an Apple FM execution failure
- not a result-publish failure
- not a Spark invoice-creation failure
- not a `payment-required` publish failure
- not a seller wallet-receive path that is universally broken
- not a "restart fixes local state" issue

The evidence does support:

- seller completed work
- seller requested payment
- seller never observed incoming settlement

## Root Cause

The root cause is product-policy, not just pane rendering:

- the provider is auto-accepting arbitrary valid open-network jobs
- those jobs are not settlement-assured
- when buyers do not settle, the provider correctly sits in `awaiting-payment` and then times out unpaid

The UI symptom is a consequence of that economic reality.

In short:

`#3414` is primarily "seller default policy still trusts speculative open-network demand too much," not "seller runtime forgot how to get from delivered to paid."

## What Needs To Change

## 1. Stop default auto-accept of speculative open-network jobs in seller-first mode

Priority: immediate

For `v0.1`, the default provider auto-accept policy should not treat generic open-network jobs as equivalent to bootstrap-safe demand.

Practical options:

- default auto-accept only `StarterDemand`
- or default auto-accept only targeted / app-owned / trusted demand classes
- or require an explicit operator risk toggle before accepting public open-network jobs

Minimum acceptable change:

- do not auto-accept raw `OpenNetwork` demand by default in seller-first release mode

## 2. Add an explicit demand-risk model to provider acceptance

Priority: immediate

Acceptance policy should consider more than syntax and minimum sats.

Needed inputs:

- demand source class: `starter-demand`, `targeted`, `open-network`
- buyer trust class or provenance
- settlement-capable versus unknown demand
- explicit operator risk mode

The provider should be able to say:

- `safe to auto-execute`
- `visible but manual accept only`
- `reject by default`

## 3. Make the seller pane say “awaiting buyer payment,” not just “delivered”

Priority: short-term

The pane should prominently distinguish:

- `delivered, awaiting buyer payment`
- `delivered, unpaid timeout`

Suggested changes:

- stronger headline for post-delivery unpaid states
- explicit buyer-side blocker text
- visual difference between `delivered` and `paid`
- economic status copy, not just lifecycle copy

## 4. Bind settlement to invoice/payment identity more strongly

Priority: short-term

`resolve_wallet_settlement_pointer_for_open_network_job(...)` should evolve from amount+time heuristics to stronger invoice-linked reconciliation.

Better correlation inputs:

- Lightning invoice hash / payment hash
- explicit invoice id recorded when creating the Spark invoice
- wallet metadata tied back to active job request id

This is not the main cause of `#3414`, but it is the right direction for payout correctness.

## 5. Separate replayed historical lifecycle events from new runtime events

Priority: medium

Mission Control log rendering should make clear whether an entry is:

- a new live event
- or a projection/replay of prior lifecycle history

That will reduce confusion like the issue comment about older timestamps appearing after going offline.

## 6. Add a deterministic unpaid-open-network regression

Priority: immediate

We already have paid-path harnesses. We also need a regression that proves the seller behavior for nonpaying demand is intentional and operator-truthful.

Needed test:

- provider accepts an open-network job
- provider executes and publishes result
- provider creates invoice and publishes `payment-required`
- buyer never settles
- provider transitions to `delivered-unpaid`
- UI/projection says "buyer did not pay" clearly

That regression should exist even if we later tighten policy to prevent default auto-accept, because the failure mode will still matter for explicit-risk/open-market operation.

## Recommended Order Of Fixes

1. Change default auto-accept policy so seller-first mode does not auto-execute speculative open-network demand.
2. Improve Active Job / Mission Control wording so unpaid delivery is unmistakable.
3. Add unpaid-open-network regression coverage.
4. Strengthen settlement correlation beyond amount+timestamp heuristics.
5. Clean up replay-versus-live log stream semantics.

## Bottom Line

`#3414` is not mainly a local provider-runtime malfunction.

The seller runtime is mostly doing what it should:

- it executes,
- delivers,
- invoices,
- and waits for payment.

The real failure is that the current default seller policy still treats open-network demand as safe enough to auto-execute in a seller-first bootstrap release. That is why the operator experiences "stuck on delivered" over and over: the app is repeatedly doing unpaid work for the public market.

The correct fix is not only to polish the pane. It is to stop defaulting sellers into speculative unpaid-risk work and make the remaining risk unmistakable when operators choose to take it.

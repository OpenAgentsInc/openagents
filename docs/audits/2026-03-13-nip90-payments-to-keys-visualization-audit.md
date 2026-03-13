# NIP-90 Payments-To-Keys Visualization Audit

Date: 2026-03-13
Branch audited: `main`
Audit type: codebase and product-surface audit focused on NIP-90 payment identity, payout truth, and visualization readiness

## Audit Question

How does the current Autopilot Desktop codebase represent NIP-90 payments between users today, especially at the pubkey level, and what visualizations should we build so operators can see:

- which pubkeys paid which pubkeys
- when those payments happened
- how much moved
- which provider won a request race
- and how Nostr relays related to the request, result, and payment path

Secondarily:

- what data already exists to support this
- what is still missing
- and what app-owned fact model would make these visualizations truthful instead of decorative

## Scope

Primary docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/v01.md`
- `docs/headless-compute.md`
- `docs/autopilot-earn/MISSION_CONTROL_BUY_MODE_PAYMENTS.md`
- prior related audits under `docs/audits/`

Primary code reviewed:

- `apps/autopilot-desktop/src/provider_nip90_lane.rs`
- `apps/autopilot-desktop/src/input/reducers/provider_ingress.rs`
- `apps/autopilot-desktop/src/input/reducers/wallet.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `apps/autopilot-desktop/src/nip90_compute_flow.rs`
- `apps/autopilot-desktop/src/state/job_inbox.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `apps/autopilot-desktop/src/runtime_log.rs`
- `apps/autopilot-desktop/src/panes/earnings_jobs.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `crates/spark/src/wallet.rs`
- `crates/nostr/core/src/nip90/*`
- `crates/nostr/core/tests/nip90_integration.rs`

Protocol references cross-checked:

- official NIP-90: `https://raw.githubusercontent.com/nostr-protocol/nips/master/90.md`
- official NIP-01: `https://raw.githubusercontent.com/nostr-protocol/nips/master/01.md`

## Executive Summary

The current app does have real payment-to-pubkey ingredients, but they are split across five different truth surfaces:

1. buyer request state
2. provider ingress / active-job state
3. Spark wallet payment state
4. seller history / earn receipts
5. JSONL session logs and activity feed events

That means the app can already answer narrow questions like:

- which provider pubkey last produced a payable result for this request
- what Spark payment pointer was assigned
- what Lightning destination pubkey the buyer wallet paid
- what wallet payment pointer or receive entry settled the seller
- when the current request moved through requesting-payment, awaiting-payment, seller-settled-pending-wallet, or paid

But it still cannot produce one clean, first-class answer to the more human question:

`show me the money graph between Nostr participants over time`

The main reason is simple:

- there is no app-owned, visualization-ready `payment edge` model
- Nostr pubkeys and Lightning destination pubkeys are mixed but not normalized into one identity graph
- relay provenance is only tracked as transport health and aggregate publish counts, not as per-event route evidence
- historical per-provider race detail is lossy because live state keeps only the latest observation per provider

So the current system is good enough for current status panes and debugging logs, but not yet good enough for a true payments-to-keys visualization pane.

The right next step is not "draw a graph directly from everything." The right next step is:

- create a small app-owned `Nip90PaymentFact` ledger
- persist it alongside the existing projection and receipt streams
- make pubkey identity explicit by namespace
- then build visualization panes from that ledger

## Protocol Truth Versus Product Truth

At the protocol layer, the repo correctly models the NIP-90 basics:

- job requests and results
- feedback kind `7000`
- feedback status `payment-required`
- `amount` tags with optional `bolt11`
- relay hints on requests and result references

The local `nostr` crate tests also explicitly cover `payment-required`, `amount`, `bolt11`, request relay tags, and the request/result kind mapping.

At the product layer, though, the app needs more than protocol correctness. It needs to make these identities legible:

- requester Nostr pubkey
- winning provider Nostr pubkey
- invoice-emitting provider Nostr pubkey
- result-emitting provider Nostr pubkey
- buyer-side Spark send pointer
- seller-side Spark receive pointer
- Lightning destination pubkey
- relay set that actually carried request and response traffic

Today those facts exist, but in different places and with different retention quality.

## What The Current Code Already Knows

### 1. Buyer-side request state is already rich

`SubmittedNetworkRequest` in `state/operations.rs` stores:

- `request_id`
- `target_provider_pubkeys`
- `last_provider_pubkey`
- `result_provider_pubkey`
- `invoice_provider_pubkey`
- `winning_provider_pubkey`
- `winning_result_event_id`
- `last_payment_pointer`
- `payment_required_at_epoch_seconds`
- `payment_sent_at_epoch_seconds`
- `payment_failed_at_epoch_seconds`
- `pending_bolt11`
- `payment_notice`
- `payment_error`
- per-provider observations
- duplicate outcome summaries

This is already enough to drive a live buyer-side "provider race" visualization for the current request.

### 2. Buyer flow snapshots already expose a near-visual model

`BuyerRequestFlowSnapshot` in `nip90_compute_flow.rs` already normalizes current buyer truth into fields such as:

- selected, result, invoice, and payable provider pubkeys
- invoice amount, fees, total debit, and net wallet delta
- payment pointer and payment hash
- Lightning destination pubkey
- loser-provider counts and loser reasons
- authority and phase
- timing fields such as payment-required, sent, or failed timestamps

That is a strong current-state model.

It is not yet a good historical model because it is reconstructed from mutable request state plus wallet state, not from a persistent fact stream.

### 3. Wallet state already carries the Lightning-side payment identity

`PaymentSummary` in `crates/spark/src/wallet.rs` stores:

- `id`
- `direction`
- `status`
- `amount_sats`
- `fees_sats`
- `timestamp`
- `method`
- `description`
- `invoice`
- `destination_pubkey`
- `payment_hash`
- `htlc_status`
- `htlc_expiry_epoch_seconds`
- `status_detail`

This is crucial because it gives the app real money movement facts.

But `destination_pubkey` is Lightning transport identity, not guaranteed to be the same thing as the winner's Nostr pubkey. Any visualization that collapses those into one unlabelled "pubkey" would be misleading.

### 4. Seller ingress keeps the buyer Nostr pubkey on arrival

`JobInboxNetworkRequest` in `state/job_inbox.rs` stores:

- `request_id`
- `requester`
- `target_provider_pubkeys`
- request shape and raw event JSON
- price and TTL metadata

So on the provider side, the app really does know who asked for the work.

That means a seller-side graph can show `buyer nostr pubkey -> local provider` for live work.

### 5. Earn receipts know more than the UI currently shows

`EarnKernelReceiptState` records ingress and lifecycle receipts with fields including:

- `request_id`
- `requester`
- `payment_pointer`
- wallet-authoritative settlement hints
- linked evidence refs such as `oa://wallet/payments/<pointer>`

This is valuable because it means the persistence layer is already closer to a canonical money graph than the current panes are.

### 6. Session logs already form a replayable event tape

`runtime_log.rs` writes JSONL session entries with:

- `timestamp_ms`
- compute-domain events like
  - `buyer.result_candidate_observed`
  - `buyer.invoice_candidate_observed`
  - `buyer.selected_payable_provider`
  - `buyer.payment_blocked`
  - `buyer.payment_settled`
  - `buyer.seller_settled_pending_wallet_confirmation`
  - `provider.result_published`
  - `provider.payment_requested`
  - `provider.settlement_confirmed`

This is enough to build an offline replay prototype right now from logs alone.

It is not a strong long-term pane substrate because logs are append-only diagnostics, not a product-owned query model.

## The Main Structural Gaps

## Finding 1: There is no first-class payment edge model

Today the "same payment" is spread across:

- buyer request state
- Spark send history
- seller job history
- seller receive history
- earn receipts
- JSONL log lines

That means there is no single app-owned row that says:

- payer actor
- payee actor
- request id
- amount
- fee
- source relays
- buyer send pointer
- seller receive pointer
- current settlement phase
- timestamps for request, result, invoice, send, seller-settled, wallet-confirmed

Without this, every future visualization would have to reconstruct the world by joining mutable state, wallet snapshots, and logs on the fly.

That is fragile and replay-hostile.

## Finding 2: The app mixes identity namespaces without making them explicit

The current surfaces expose at least three identity classes:

- Nostr requester pubkey
- Nostr provider pubkey
- Lightning destination pubkey

Current Buy Mode history rows can show both:

- `provider_pubkey`
- `destination_pubkey`

but they do not make the namespace distinction into a first-class product concept.

That matters because a graph labelled only with "pubkey" would be lying about what the node means.

The visualization layer needs an explicit actor model such as:

- `nostr_pubkey_hex`
- `nostr_npub`
- `lightning_destination_pubkey`
- `actor_role`
- `is_local_actor`

## Finding 3: Relay provenance is aggregate, not event-specific

The current relay layer is strong on health and counts:

- configured relays
- connected/disconnected/error state
- latency
- last seen
- publish `accepted_relays` and `rejected_relays`

But it does not retain per-event relay path data.

Examples:

- `ProviderNip90BuyerResponseEvent` does not store which relay carried the event
- `ProviderNip90PublishOutcome` stores only counts, not the exact relay URLs that accepted or rejected
- `JobInboxNetworkRequest` stores raw event shape and raw JSON, but not the relay URL it arrived on

This means a relay visualization can show:

- which relays are healthy right now
- how many relays accepted a publish

but not:

- which relay delivered the winning result
- which relay delivered the invoice-bearing feedback
- which relay(s) carried the buyer request that later produced a payout

That is the biggest blocker for a truthful "payments through relays" pane.

## Finding 4: Historical race detail is lossy

`NetworkRequestProviderObservation` keeps only the latest observation per provider.

That is good for live state. It is bad for historical animation.

What gets lost or compressed:

- interim processing feedback bursts
- repeated payment-required retries
- the order in which providers emitted results
- the exact moment one provider overtook another

`duplicate_outcomes` helps, but it is still summary data:

- it is not a full event stream
- it does not store all timing detail
- it does not store relay path detail

So a true request-race replay needs either:

- a product-owned persisted event strip
- or a parser over session logs and activity feed entries

## Finding 5: Seller history drops the requester pubkey from the main UI projection

`JobHistoryReceiptRow` stores:

- job id
- payout sats
- payment pointer
- delivery and provenance metadata

But it does not store:

- requester pubkey
- local provider pubkey

That means the current earnings/history surfaces can tell the seller:

- how much was earned
- when the wallet received it
- which job got paid

but not:

- who paid me

The underlying receipt stream knows more than this. The projection dropped it.

For a payments-to-keys view, that is a serious projection gap.

## Finding 6: Wallet backfill is still heuristic

Buy Mode history can backfill older sends from Spark history, but the candidate logic still relies on wallet description heuristics such as a `DVM textgen` prefix.

That is useful for debugging.

It is not a strong canonical basis for a graph surface that claims to show historical NIP-90 money movement by actor.

Any history graph should prefer:

- request-id-linked fact rows
- receipt-linked settlement facts

and use wallet-description inference only as a degraded fallback, visibly labelled as such.

## What We Can Visualize Right Now Without New Data Capture

These are truthful if scoped carefully.

### A. Buyer Race Matrix

Use current `BuyerRequestFlowSnapshot` plus `SubmittedNetworkRequest.provider_observations`.

Show:

- current request in the center
- one lane per provider pubkey
- result seen
- invoice seen
- payable winner chosen
- blocked over budget
- payment queued
- wallet pending

Visual treatment:

- vertical provider columns
- colored chips for result and invoice
- a bright winner rail when `payable_provider_pubkey` exists
- dimmed loser rails with reason badges such as `no invoice`, `late result`, or `error-only`

Truth status:

- good for current request
- not a full historical replay

### B. Buy Mode Payment Ledger

Use `buy_mode_payment_ledger_entries(...)`.

Show:

- provider Nostr pubkey
- Lightning destination pubkey
- request id
- amount
- fees
- total debit
- wallet status
- payment pointer
- time

Visual treatment:

- compact timeline ledger
- amounts as bars
- fee overlays
- hover state that expands request ids and hashes

Truth status:

- good for current and recent buyer sends
- must label Lightning destination separately from Nostr provider identity

### C. Seller Earnings Timeline

Use wallet-reconciled seller payout rows plus persisted seller history.

Show:

- wallet-authoritative receives over time
- payout size
- job id
- confirmation latency

Visual treatment:

- horizontal pulse chart
- each payout a glowing capsule
- confirmation latency encoded as trail length

Truth status:

- truthful for payouts
- cannot yet say who paid each payout without requester data being carried into the UI projection

## Visualizations That Need One New Fact Model

These are the interesting ones.

## 1. Settlement Atlas

Concept:

- a constellations-style graph of pubkeys
- buyers on the left hemisphere
- providers on the right hemisphere
- payment edges arcing between them
- edge thickness = sats volume
- edge glow = recency
- pulse speed = settlement latency
- node halo = current role intensity and lifetime volume

What it answers:

- which providers are actually getting paid
- which buyers are active
- who is sending many small payments versus fewer large ones
- which provider clusters are repeatedly winning work

Truth requirements:

- canonical payer actor
- canonical payee actor
- amount
- timestamp
- request id
- settlement status

Best pane name:

- `Settlement Atlas`

## 2. Spark Replay

Concept:

- a timeline scrubber across the bottom
- each request becomes a spark
- spark leaves buyer node when request is published
- splits toward multiple providers during result race
- winner path thickens when invoice + result align
- spark turns gold when payment pointer exists
- spark lands at seller only when settlement is wallet-authoritative

What it answers:

- how request races evolved
- whether the buyer paid late or early
- where the flow stalled
- whether "seller-settled-pending-wallet" was local lag or real nonpayment

Truth requirements:

- ordered event stream
- provider race history
- buyer send event
- seller settlement event

Best pane name:

- `Spark Replay`

## 3. Relay Choreography

Concept:

- three-column or five-column braid view
- buyer pubkeys on the left
- relays in the middle
- providers on the right
- request, result, invoice, and settlement flows drawn as animated threads

Visual language:

- thin cold lines for request fanout
- bright cyan line for winning result
- amber line for invoice-bearing `payment-required`
- green line for wallet-confirmed settlement
- red broken strands for rejected or degraded relays

What it answers:

- which relays carried demand
- whether a winning provider consistently arrives through one relay
- whether publish failures cluster on certain relays
- whether relay health is correlated with payout completion

Truth requirements:

- per-event relay url or relay set
- publish accept/reject by relay, not only aggregate counts
- ingress-side relay attribution

Best pane name:

- `Relay Choreography`

## 4. Key Ledger

Concept:

- a sortable operator table with one row per Nostr pubkey
- received sats
- sent sats
- jobs won
- invoices emitted
- average settlement latency
- failure counts
- last-seen relay activity

Visual treatment:

- more Bloomberg terminal than toy graph
- tiny in-row sparklines for payment history
- edge-click opens graph or replay filtered to that pubkey

What it answers:

- who are the top paid providers
- which pubkeys repeatedly request but never settle
- which provider pubkeys generate invoices but rarely receive wallet-authoritative settlement

Truth requirements:

- actor ledger
- request and settlement joins

Best pane name:

- `Key Ledger`

## 5. Settlement Ladder

Concept:

- one request at a time
- six rungs:
  - request observed
  - result observed
  - invoice observed
  - payment pointer assigned
  - seller settled
  - buyer wallet confirmed

Each rung illuminates only when its proof exists.

This is not as visually ambitious, but it is extremely truthful and would immediately reduce operator confusion.

Best pane name:

- `Settlement Ladder`

## Proposed Relay Visualization Specifically

The relay-specific pane should not try to fake more than we know.

Phase 1, truthful with current data:

- use live relay health from `ProviderNip90LaneSnapshot`
- show connected/disconnected/error relays
- show recent accepted/rejected publish counts as aggregate bubbles
- show current request lanes only as "broadcast to N relays" and "published on N relays"

Phase 2, after capture changes:

- record per-event ingress relay URL
- record per-publish accepted relay URLs and rejected relay URLs
- attach that data to the payment fact stream
- then draw real threads through relay columns

Until Phase 2 exists, any per-relay historical payment graph would be guesswork.

## Recommended Visualization-Ready Data Model

Add one app-owned ledger in `apps/autopilot-desktop`, not a reusable crate:

### `Nip90PaymentFact`

Suggested fields:

- `fact_id`
- `request_id`
- `request_type`
- `buyer_nostr_pubkey`
- `provider_nostr_pubkey`
- `invoice_provider_pubkey`
- `result_provider_pubkey`
- `lightning_destination_pubkey`
- `buyer_payment_pointer`
- `seller_payment_pointer`
- `buyer_payment_hash`
- `amount_sats`
- `fees_sats`
- `total_debit_sats`
- `wallet_method`
- `status`
- `settlement_authority`
- `request_published_at`
- `result_observed_at`
- `invoice_observed_at`
- `buyer_payment_pointer_at`
- `seller_settlement_feedback_at`
- `buyer_wallet_confirmed_at`
- `seller_wallet_confirmed_at`
- `selected_relays`
- `publish_accepted_relays`
- `publish_rejected_relays`
- `source_quality`

### `Nip90Actor`

Suggested fields:

- `actor_id`
- `namespace`
- `pubkey`
- `display_label`
- `is_local`
- `role_mask`

Namespaces should at least be:

- `nostr`
- `lightning_destination`

### `Nip90RelayHop`

Suggested fields:

- `request_id`
- `event_id`
- `hop_kind`
- `relay_url`
- `direction`
- `accepted`
- `observed_at`

This can be sparse at first.

## What To Persist Versus What To Derive

Persist:

- request-to-provider winner facts
- payment settlement facts
- actor identities
- relay hops when known

Derive at render time:

- node size by total sats
- edge thickness by rolling volume
- pulse speed by latency bucket
- color by phase or authority

Do not make the pane reconstruct core economic truth from raw logs each time.

## Recommended Shipping Sequence

### Step 1

Add the fact ledger and actor ledger.

This is the real enabling step.

### Step 2

Ship `Settlement Ladder` and `Key Ledger`.

Why first:

- truthful with minimal new capture
- high operator value
- easy to validate against wallet and history panes

### Step 3

Ship `Settlement Atlas`.

Why next:

- best "wow" surface once facts are normalized
- directly answers "who got paid by whom"

### Step 4

Ship `Spark Replay`.

Why after the atlas:

- needs ordered event data
- strongest payoff once request-race history is persisted

### Step 5

Ship `Relay Choreography`.

Why last:

- needs new relay-hop capture
- otherwise it risks visual theatre without proof

## Concrete Product Recommendations

1. Introduce an app-owned `Nip90PaymentFact` stream in `apps/autopilot-desktop` and persist it beside the current lifecycle projection and earn-kernel receipts.

2. Carry requester pubkey through seller-facing history projections so the earnings side can answer `who paid me`.

3. Make identity namespace explicit everywhere a pane shows both provider pubkeys and Lightning destination pubkeys.

4. Extend `ProviderNip90BuyerResponseEvent`, request ingress, and publish outcomes to preserve relay url attribution when known.

5. Preserve more than just the latest provider observation if we want historical race playback to be trustworthy.

6. Treat JSONL session logs as a backfill and audit source, not as the primary product read model for payment graphs.

7. Build the first payment visualization pane from the new fact stream, not from direct joins across request state and wallet snapshots.

## Bottom Line

The current repo already contains enough payment truth to justify a first-class NIP-90 payments visualization effort.

But the implementation is one level too implicit.

Right now we have:

- live buyer winner selection
- wallet-authoritative payment truth
- seller payout truth
- request and result telemetry
- relay health
- JSONL replay events

What we do not yet have is the one thing a great payment graph pane needs:

- a canonical, app-owned settlement edge per request that links actors, money, timing, and relay provenance in one place

Build that first, then the visualizations become both creative and defensible.

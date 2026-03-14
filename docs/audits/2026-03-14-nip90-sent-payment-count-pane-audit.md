# NIP-90 Sent Payment Count Pane Audit

Date: 2026-03-14
Branch audited: `main`
Audit type: product and implementation audit for a new pane that shows a definitive count of NIP-90 payments sent over a given time period

## Audit Question

What in the current desktop codebase is relevant to a new pane that can answer:

- how many NIP-90 payments were sent
- over a user-selected time period
- with a definitive count rather than an inferred approximation

And specifically:

- what current state is authoritative
- what current state is only projected or degraded
- what pane, control-plane, and persistence work would actually be required

Clarified product requirement from follow-up:

- the real target is one daily report, not just a pane
- the report needs both `payment_count` and `total_sats_sent`
- it should be pullable from a CLI command
- it must account for NIP-90 activity observed across every connected relay
- relay fan-in must be deduplicated so the report counts payments, not relay sightings

## Scope

Primary docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/PANES.md`
- `docs/headless-compute.md`
- `docs/v01.md`
- `docs/autopilot-earn/README.md`
- `docs/autopilot-earn/MISSION_CONTROL_BUY_MODE_PAYMENTS.md`
- prior payment audits under `docs/audits/`

Primary code reviewed:

- `apps/autopilot-desktop/src/state/nip90_payment_facts.rs`
- `apps/autopilot-desktop/src/nip90_compute_flow.rs`
- `apps/autopilot-desktop/src/state/operations.rs`
- `apps/autopilot-desktop/src/input/reducers/wallet.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `crates/spark/src/wallet.rs`
- `apps/autopilot-desktop/src/panes/buy_mode.rs`
- `apps/autopilot-desktop/src/panes/key_ledger.rs`
- `apps/autopilot-desktop/src/panes/seller_earnings_timeline.rs`
- `apps/autopilot-desktop/src/panes/settlement_atlas.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/nip90_compute_domain_events.rs`

## Executive Summary

The desktop already has a strong app-owned NIP-90 payment visualization substrate, but it does not yet have a definitive reporting substrate for buyer-side sent payments over daily or arbitrary time windows.

The main gap is structural:

- the current persisted ledger is keyed by `request_id`
- a definitive sent-payment count needs to be keyed by payment attempt or payment pointer
- a definitive daily report also needs a stable aggregation contract exposed through the control plane or CLI

Today the app can truthfully answer:

- what the latest known payment state is for a request
- which provider won
- which wallet payment pointer is currently associated with that request
- whether a seller payout is wallet-confirmed

Today the app cannot yet truthfully answer, for an arbitrary period:

- how many distinct NIP-90 buyer payments were sent

That is because the current model collapses multiple buyer payment attempts for the same request into one row, caps retained history, and mixes authoritative and degraded timestamps in the same request-scoped fact record.

There is also an important relay point:

- payment truth should not be counted per relay
- relay traffic from every connected relay should be used only to discover and bind request/result/invoice evidence
- the final daily report should count deduplicated wallet-backed payment attempts

So the correct implementation path is not “add a pane on top of current facts as-is.”

The correct path is:

1. add an app-owned buyer payment-attempt ledger keyed by payment pointer
2. make time-window counting use wallet-authoritative send timestamps from that ledger
3. bind those payment-attempt rows to request evidence observed across the full connected relay set
4. expose a daily report through `autopilotctl` or equivalent desktop-control action
5. then optionally add a pane that reads the same report substrate

## Implementation Status

Implemented on 2026-03-14 through the follow-on issue set created from this audit:

- `#3613` wallet-authoritative buyer payment-attempt ledger and aggregation substrate
  - landed in commit `373e9656a`
- `#3614` desktop-control and `autopilotctl` daily/window report contract
  - landed in commit `410da2423`
- `#3615` `NIP-90 Sent Payments` pane on top of the shared report substrate
  - landed in commit `57ddaa339`

As implemented, the product now has:

- an app-owned buyer payment-attempt ledger keyed by payment pointer
- a wallet-authoritative daily/window report with `payment_count`, `total_sats_sent`, `total_fee_sats`, and `total_wallet_debit_sats`
- relay-aware request/result/invoice evidence binding across the connected relay set
- a CLI/control-plane report contract and a dedicated pane that read the same substrate

## Current Truth Surfaces

### 1. Spark wallet is the only buyer-send authority

For buyer-side outgoing payments, the authoritative source is Spark wallet history:

- `PaymentSummary.direction`
- `PaymentSummary.status`
- `PaymentSummary.timestamp`
- `PaymentSummary.id`
- `PaymentSummary.amount_sats`
- `PaymentSummary.fees_sats`
- `PaymentSummary.payment_hash`
- `PaymentSummary.destination_pubkey`

For a definitive count of payments sent, the only safe primary event is:

- a Spark payment row with `direction=send`
- tied to a NIP-90 request
- in a terminal success state

Everything else is supporting evidence.

For the clarified daily report, this means:

- `payment_count` should be derived from distinct wallet payment pointers
- `total_sats_sent` should be derived from wallet debit totals for those pointers
- relay data must not independently increment either number

### 2. Buyer request state is request-scoped, not payment-attempt-scoped

`SubmittedNetworkRequest` stores:

- `last_payment_pointer`
- `payment_required_at_epoch_seconds`
- `payment_sent_at_epoch_seconds`
- `payment_failed_at_epoch_seconds`
- `pending_bolt11`
- winner/result/invoice provider identities

This is good live UX state, but it only stores the latest pointer, not a full history of send attempts.

### 3. The persisted NIP-90 payment fact ledger is request-scoped

`Nip90PaymentFact` stores one row per request:

- `fact_id = payment_fact_id(request_id)`
- one `buyer_payment_pointer`
- one `buyer_payment_hash`
- one buyer wallet confirmation timestamp

That model is appropriate for:

- settlement ladder views
- actor/edge visualizations
- latest-known request status

It is not sufficient for a definitive sent-payment count.

### 4. Relay fan-in already exists, but only as evidence, not as report truth

The current buyer/provider path already spans the connected relay set:

- request publish selected/accepted/rejected relays
- result relay URLs
- invoice relay URLs
- provider observation history relay URLs
- relay-hop derivation in the fact ledger

That is good enough to say:

- this request was seen and reconciled across the connected relay set

It is not yet a reporting contract that can say:

- here is the definitive daily total from every connected relay

because relay evidence is still request-scoped and deduplicated only inside the latest-known request fact model.

### 5. Existing payment panes are visualization-first, not count-first

Current panes built from `Nip90PaymentFactLedgerState` are:

- `Buy Mode`
- `Seller Earnings Timeline`
- `Settlement Ladder`
- `Key Ledger`
- `Settlement Atlas`
- `Spark Replay`
- `Relay Choreography`

These panes are useful operator surfaces, but none of them currently provides:

- a generic buyer-side sent-payment count
- a custom absolute time window
- a payment-attempt-level history model

## Findings

## Finding 1: The current persisted fact model cannot count multiple sends for one request

Severity: high

The strongest blocker is that the current ledger collapses payment state by `request_id`.

That means:

- one request can only retain one `buyer_payment_pointer`
- later retries overwrite earlier send attempts in request state
- the persisted fact ledger also retains only one buyer payment pointer for that request

If the same request ever produces:

- a failed payment attempt and a later successful retry
- a wallet pointer replacement
- any future multi-attempt payment behavior

the current request-scoped ledger cannot produce a definitive count of payments sent. It can only show the latest request-associated payment state.

For the requested pane, this is the core architectural gap.

Recommended fix:

- introduce a buyer payment-attempt record keyed by `payment_pointer`
- include `request_id`, `attempt_status`, `wallet_timestamp`, `amount_sats`, `fees_sats`, and binding quality
- count payment attempts from that ledger, not from request-scoped facts

## Finding 2: The current ledger is retention-capped, so long-window counts can silently undercount

Severity: high

`Nip90PaymentFactLedgerState` is globally capped:

- facts: `4096`
- actors: `4096`
- relay hops: `8192`

Facts are sorted by latest event time and truncated.

That means older request facts are eventually dropped from the persisted product read model. A new pane built directly on top of the current fact vector would not be definitive for large or historical time periods.

This is not just a UI issue. It is a storage-contract issue.

Recommended fix:

- keep a count substrate in a ledger that is not globally truncated by recentness
- or maintain a secondary index/summary store specifically for payment-attempt history

## Finding 3: Session-log backfill is explicitly degraded and budget-limited

Severity: high

The current ledger can import older facts from session JSONL logs, but that path is intentionally partial:

- startup delay before background backfill
- hot log deferral
- max file size cap
- max total byte budget cap
- imported facts marked `log-backfill`

That is correct for UI responsiveness, but it means log backfill is not a definitive counting substrate.

A pane that promises a definitive sent-payment count cannot rely on:

- session-log backfill completeness
- log-derived timestamps
- log-derived request recovery

except as degraded recovery when authoritative rows are already persisted elsewhere.

## Finding 4: Timestamp merging currently prefers the earliest seen timestamp, not the most authoritative one

Severity: medium

When facts merge, timestamps use `merge_timestamp`, which keeps the minimum non-null value.

That is risky for time-window counting because:

- a degraded log-backfill timestamp can win over a later wallet-derived timestamp
- a request projection timestamp can survive even after stronger evidence appears
- boundary-sensitive counts can move across the requested time window

For a definitive count over a given period, timestamp selection needs explicit authority rules, not “earliest wins.”

Recommended fix:

- define a canonical buyer-send timestamp field sourced from wallet-confirmed send time
- only fall back to degraded timestamps when the row is explicitly marked non-authoritative and excluded from the definitive count

## Finding 5: Counting “from every connected relay” must mean deduped relay fan-in, not per-relay tallying

Severity: high

The clarified requirement is easy to misimplement.

If the report literally tallies raw relay-observed NIP-90 payment evidence, it will overcount because:

- the same request or invoice can appear on multiple relays
- publish fanout intentionally writes to multiple relays
- result and feedback ingress can be duplicated across relay paths

So the correct interpretation is:

- observe request/result/invoice evidence from every connected relay
- dedupe that evidence down to one request/payment binding
- then count the wallet-backed payment once

This should be stated explicitly in both implementation docs and CLI help text.

## Finding 6: Current panes do not expose a generic buyer-side sent-payment count, and some existing summaries are not safe to reuse

Severity: medium

Relevant current behavior:

- `Buy Mode` summary only counts the dedicated buy-mode request type
- wallet-backfill in buy-mode depends on a description heuristic (`dvm textgen`)
- there is no equivalent generic wallet-only binding path for arbitrary NIP-90 buyer sends
- `Key Ledger` aggregates actor activity, not a time-windowed sent-payment count
- the `Key Ledger` inactive preview computes “sent” by summing all facts, including fallback amounts where no buyer debit exists
- `Seller Earnings Timeline` is seller-receive-focused, not buyer-send-focused

So there is no existing pane that can be extended trivially into “definitive count of all NIP-90 payments sent.”

Recommended fix:

- keep the new pane buyer-send-specific
- define its metric contract independently instead of borrowing the current preview totals

## Finding 7: There is no daily-report or CLI contract yet for this metric

Severity: medium

The current control plane exposes:

- buy-mode status
- pane snapshots
- wallet status
- active-job status

It does not expose a stable report contract like:

- `date`
- `payment_count`
- `total_sats_sent`
- `connected_relay_count`
- `relay_urls_considered`
- `deduped_request_count`
- `degraded_binding_count`

So even with a correct underlying ledger, the requested CLI report still needs:

- a desktop-control snapshot field or action
- an `autopilotctl` command that prints or emits the report

## Finding 8: There is no pane state or input contract yet for an arbitrary time period

Severity: medium

Current time-range handling patterns in the app are limited:

- `JobHistory` uses cyclical presets (`all-time`, `24h`, `7d`)
- advanced payment panes are read-only visualization panes
- tool bridge and pane actions for advanced payment panes do not support filter inputs

If the new pane must support a real “given time period,” the app needs new state for:

- start timestamp
- end timestamp
- possibly presets plus custom absolute bounds

Recommended fix:

- add explicit pane-owned time-window state
- prefer `[start, end)` absolute epoch seconds internally
- optionally layer presets (`24h`, `7d`, `30d`, `custom`) on top

For the CLI path, the same contract should support at least:

- `--date YYYY-MM-DD`
- optional `--start` / `--end`

## Finding 9: The new pane will also need desktop-control and test-surface work

Severity: medium

The desktop-control runtime and tool bridge are now part of the real product contract.

Today:

- advanced payment panes are openable
- some are snapshot-able only through generic pane metadata
- they do not expose pane-specific data through `desktop_control_pane_snapshot_details`
- they do not expose tool-writable inputs for time filtering

If this pane is implemented only visually, it will be hard to verify through:

- `autopilotctl pane status`
- packaged smoke tests
- future automation

Recommended fix:

- add pane snapshot details for the new pane
- add pane input handling for time-window fields
- update `docs/headless-compute.md` if the control-plane contract changes

## Finding 10: Ownership boundaries are favorable; this should stay app-owned

Severity: low

This feature belongs in `apps/autopilot-desktop`.

It touches:

- product truth composition
- pane registration and rendering
- app-owned automation/control surfaces

It does not require moving product logic into:

- `crates/wgpui`
- `crates/spark`
- `crates/nostr`

Reusable crates may remain unchanged unless a narrowly reusable primitive is genuinely missing.

## Recommended Product Contract

For the clarified requirement, the definitive report should be:

- one daily or arbitrary-window buyer send report
- sourced from wallet-authoritative buyer sends
- bound to NIP-90 requests whose evidence may have arrived from any connected relay
- deduplicated across relay fan-in

Suggested report fields:

- `report_date` or `window_start` / `window_end`
- `payment_count`
- `total_sats_sent`
- `total_fee_sats`
- `total_wallet_debit_sats`
- `connected_relay_count`
- `relay_urls_considered`
- `deduped_request_count`
- `degraded_binding_count`

The top-line definitive metrics should be:

- `settled_sent_count`
- count of buyer-side Spark `send` payments
- bound to NIP-90 requests
- with wallet-terminal success
- whose canonical wallet-confirmed timestamp falls within `[start_epoch_seconds, end_epoch_seconds)`

The pane should also expose non-definitive neighboring metrics so operators can understand gaps:

- `pending_send_count`
- `failed_send_count`
- `unbound_wallet_send_count`
- `degraded_recovered_count`

That keeps the top-line number definitive while still surfacing incomplete or degraded evidence.

## Recommended Implementation Shape

### 1. Add a buyer payment-attempt ledger

Add an app-owned persisted model, for example:

- `Nip90BuyerPaymentAttempt`

Suggested fields:

- `payment_pointer`
- `request_id`
- `request_type`
- `wallet_direction`
- `wallet_status`
- `wallet_confirmed_at`
- `wallet_first_seen_at`
- `amount_sats`
- `fees_sats`
- `total_debit_sats`
- `payment_hash`
- `destination_pubkey`
- `buyer_nostr_pubkey`
- `provider_nostr_pubkey`
- `binding_quality`
- `source_quality`

The key point is:

- one row per payment attempt, not one row per request

### 2. Keep the current request-scoped fact ledger for visualization panes

The current `Nip90PaymentFact` model is still useful for:

- settlement ladder
- atlas
- actor ledger
- replay

Do not overload that request-scoped model to answer a payment-attempt counting question.

### 3. Add a pane-owned time-range state

Suggested pane state:

- preset selector
- optional absolute start/end inputs
- cached summary numbers

Use absolute epoch seconds internally so:

- desktop control
- tool bridge
- tests

all speak the same range contract.

### 4. Expose a CLI-first report

Likely contract shape:

- desktop control action or snapshot field for `nip90_sent_payment_report`
- `autopilotctl nip90-payments daily --date YYYY-MM-DD`

That command should read the same app-owned report substrate the UI uses.

### 5. Add a new pane under app ownership

Likely touch points:

- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/pane_registry.rs`
- `apps/autopilot-desktop/src/pane_system.rs`
- `apps/autopilot-desktop/src/panes/mod.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/panes/<new pane>.rs`

### 6. Extend control-plane visibility

Likely touch points:

- `apps/autopilot-desktop/src/input/tool_bridge.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `docs/headless-compute.md`

If the pane is meant to be product-real, not just local eye candy, this should be done in the same implementation slice.

## Minimum Acceptance Criteria For The Future Implementation

The pane should not ship as “definitive” unless all of the following are true:

- counts are based on payment-attempt rows, not request rows
- the counted rows are wallet-authoritative buyer sends
- the chosen time field is explicitly defined and source-authoritative
- relay fan-in from the full connected relay set is deduped before counting
- the retained history is not silently truncated for the supported window
- retries do not collapse into one counted row
- the pane clearly separates definitive counts from degraded recovery counts
- pane snapshot/control-plane support exists for automation

## Suggested Tests

Minimum new coverage should include:

- one request, one successful send, counted once
- one request, failed send then successful retry, counted as one settled send and two attempts
- two successful sends for two requests in one window, counted twice
- one request observed on multiple relays but counted once
- boundary test at exact `start` and `end`
- restart/reload with persisted ledger still producing the same count
- history large enough to exceed the current 4096-row request-fact limit without undercounting the payment-attempt ledger
- desktop-control pane snapshot reflects the same count as the visible pane
- CLI report output matches the same underlying report fields

## Bottom Line

The current codebase is close to supporting this feature visually, but not yet semantically.

If implemented directly on top of the existing request-scoped `Nip90PaymentFact` rows, the pane would look convincing while still failing the user’s actual requirement.

The requirement is a definitive count.

That requires:

- payment-attempt-level persistence
- wallet-authoritative time semantics
- non-truncated history for the supported query window
- and control-plane visibility so the feature is testable

Without those changes, the correct label would be “approximate request-linked payment count,” not “definitive count of all NIP-90 payments sent.”

## Created Issues

- `#3612` Epic: definitive NIP-90 sent-payments daily report across connected relays
- `#3613` Add wallet-authoritative NIP-90 buyer payment-attempt ledger and daily aggregation
- `#3614` Expose daily NIP-90 sent-payment report via desktop control and `autopilotctl`
- `#3615` Add NIP-90 sent-payments report pane using the daily report substrate

# False-Paid Seller Settlement After-Action Audit

Date: March 12, 2026

## Summary

We shipped and exercised seller payment flows that could report `paid` even when the seller's Spark wallet balance did not increase for the current job.

This was a real launch-blocking correctness failure.

The immediate trigger was a live seller report: the Active Job pane said `paid`, Mission Control showed successful settlement, but the seller's wallet balance was `2 sats` before and after the run. That proved the app was capable of asserting seller payment off stale or mismatched wallet evidence.

The failure was not one isolated bug. It was a stack failure:

- seller settlement correlation was too permissive
- release and packaged-app verification asserted UI/domain success but not wallet delta truth
- earlier headless and autopilotctl harnesses validated the happy path but did not defend against false-positive settlement on repeated tiny payouts

This audit explains how we discovered the issue, why our prior harnesses missed it, what we changed, what tests now exist, and what release gates must remain in place.

## User-Visible Failure

The concrete failure report was:

- seller went online
- a targeted open-network request was accepted and completed
- Active Job advanced to `paid`
- compute-domain logs emitted `provider.settlement_confirmed`
- seller wallet balance did not increase

That meant the product was violating the core truth requirement from MVP:

> if we say a seller got paid, their wallet balance must actually have increased for that job.

The old behavior was especially dangerous because it made the most important success state in the app untrustworthy.

## How We Discovered It

We discovered the problem from a live operator cross-check, not from our automated verification:

1. Seller observed an apparently successful run.
2. Logs showed:
   - result published
   - settlement confirmed
   - job projected as `paid`
3. Human follow-up asked the only question that mattered:
   - "Was your balance higher after that than before?"
4. Seller confirmed balance was `2 sats` before and `2 sats` after.

That immediately invalidated the earlier interpretation that the payout was real.

The critical lesson is simple:

- UI state is not enough.
- domain events are not enough.
- even a `settlement_confirmed` log row is not enough if correlation is wrong.
- for seller payout truth, the release gate must include wallet delta.

## Root Cause

The primary bug was in open-network seller settlement reconciliation inside:

- [actions.rs](/Users/christopherdavid/code/openagents/apps/autopilot-desktop/src/input/actions.rs)

The reconciliation logic was willing to fall back to heuristic matching when it could not prove exact settlement identity.

Specifically, for open-network seller jobs it could match a wallet receive by:

- same amount
- nearby timestamp window

That is unsafe for repeated tiny payouts like `2 sats`, because an older unrelated receive of the same amount can be reused as if it belonged to the current job.

In practice, the app did this:

1. Seller completed a job.
2. Exact invoice/payment-hash identity was not available or not required.
3. Reconciliation found an older `2 sat` receive in the wallet.
4. The job advanced from delivered/unpaid into `paid`.
5. UI and logs reflected success even though the current job had not increased the seller balance.

That is the direct root cause of the false-positive payment state.

## Why Our Existing Harnesses Missed It

The pre-existing harnesses were validating the wrong success criteria.

### 1. Headless and desktop tests over-trusted semantic events

We already had good coverage for:

- buyer/provider handshake
- result publication
- invoice flow
- settlement-style domain events
- post-payment UI state

But those checks mostly asserted things like:

- buyer reached `paid`
- provider emitted `provider.settlement_confirmed`
- Active Job transitioned into `paid`

Those are necessary checks, but they are not sufficient checks if settlement correlation itself is buggy.

### 2. Packaged `autopilotctl` roundtrip proved the wrong thing

The packaged release harness in:

- [check-v01-packaged-autopilotctl-roundtrip.sh](/Users/christopherdavid/code/openagents/scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh)

was previously asserting:

- chat worked
- buy mode roundtrip completed
- seller logs showed payment requested
- seller logs showed settlement confirmed

It did **not** assert:

- seller wallet balance before the cycle
- seller wallet balance after the cycle
- seller wallet delta for that specific cycle

That is the exact reason the harness could bless a false-positive seller payout.

### 3. Repeated tiny payouts are adversarial for heuristic correlation

Our common smoke-test amount was `2 sats`.

That amount is useful operationally, but it is also the worst possible amount for a heuristic bug:

- tiny
- repeated
- common
- easy to collide with previous successful receives

So the harness design accidentally created the perfect condition for a false-positive while also failing to measure the thing that would expose it.

## Fixes Implemented

### A. Require exact seller settlement identity

Committed in:

- `5238707c4` `Require exact seller settlement identity`

Change:

- open-network seller payout reconciliation now requires exact settlement identity
- valid matches are:
  - exact Bolt11 invoice match
  - exact payment-hash match
- the old amount/timestamp heuristic fallback was removed

Effect:

- a previous `2 sat` receive can no longer be reused to mark a new job `paid`
- a seller job stays in delivered/unpaid if the current job cannot be proven against current wallet evidence

This directly fixes the false-positive `paid` transition.

### B. Harden packaged release verification with real seller wallet delta

Committed in:

- `e9f08bc59` `Require real seller wallet delta in release roundtrip`

Change in:

- [check-v01-packaged-autopilotctl-roundtrip.sh](/Users/christopherdavid/code/openagents/scripts/release/check-v01-packaged-autopilotctl-roundtrip.sh)

The release harness now:

1. captures seller balance before each buy cycle
2. waits for the expected settlement events
3. refreshes seller wallet state
4. requires seller balance to increase
5. records before/after/delta in the summary artifact

Effect:

The packaged app is no longer allowed to pass release verification by only emitting `paid`-shaped events. It must show real seller wallet growth on that cycle.

### C. Delivered/unpaid truth path remained explicit

The earlier `#3414` work remains part of the fix chain:

- unpaid seller states stay explicit in Active Job and Mission Control
- replay rows are marked as replay instead of looking like fresh runtime success
- deterministic unpaid-open-network regressions exist

Those changes matter because once exact identity is enforced, more jobs will correctly remain in delivered/unpaid instead of being incorrectly promoted to `paid`.

## Tests and Gates Now Covering This

### Rust regressions

The key regression added for the root bug is:

- `cargo test -p autopilot-desktop --lib input::actions::tests::open_network_wallet_pointer_requires_exact_invoice_or_payment_hash_identity -- --exact --nocapture`

This ensures seller settlement correlation does not fall back to fuzzy amount/time matching.

The existing truth regressions that continue to matter:

- `cargo test -p autopilot-desktop --lib app_state::tests::unpaid_open_network_seller_lifecycle_stays_truthful_through_timeout -- --exact --nocapture`
- `cargo test -p autopilot-desktop --lib pane_renderer::tests::delivered_unpaid_active_job_shows_nonpayment_state -- --exact --nocapture`
- `cargo test -p autopilot-desktop --lib nip90_compute_flow::tests::active_job_snapshot_distinguishes_delivered_unpaid_timeout -- --exact --nocapture`

These ensure the seller UI remains truthful when payout does not occur.

### Packaged release gate

The packaged release/autopilotctl roundtrip now additionally proves:

- seller wallet balance before cycle
- seller wallet balance after cycle
- seller wallet delta > 0

That closes the exact gap that let us miss this before.

### Sanity checks used during the fix

We also verified:

- `cargo check -p autopilot-desktop --lib`
- `git diff --check`
- shell syntax for the release harness

## What We Should Have Been Measuring From The Start

For seller payout truth, the canonical success tuple is:

1. provider invoice/payment identity exists
2. provider wallet receives a new payment tied to that identity
3. provider wallet balance increases accordingly
4. only then may the job advance to `paid`

We previously treated step 2 as a soft inference and skipped step 3 in the release harness.

That was the mistake.

## Operational Guidance Going Forward

These rules should remain non-negotiable:

### 1. Never ship a seller payout path that is only event-truthful

If the app says `paid`, we must be able to point to:

- exact invoice or payment-hash identity
- new wallet record
- resulting balance delta

### 2. Packaged release verification must always include wallet-delta assertions

The real packaged binary is what matters at launch. Any release harness that does not measure seller balance before/after is incomplete.

### 3. Tiny repeated payout amounts require identity-level matching

If we continue using tiny smoke-test prices, exact settlement identity is mandatory. Heuristics are not acceptable.

### 4. Delivered/unpaid is a success of truthfulness, not a failure of UI

It is better for the UI to say:

- delivered
- awaiting buyer payment
- unpaid timeout

than to claim `paid` incorrectly.

## Remaining Risks

The false-positive `paid` bug is fixed, but these risks still remain:

- open-network buyers may still genuinely fail to pay
- payment identity could still be absent in edge cases if upstream wallet/provider metadata is malformed
- release verification now catches missing wallet delta, but live operators still need to look at seller balance when diagnosing weird settlement reports

In other words:

- the correctness hole is fixed
- the nonpayment market reality is not

## Final Assessment

This was a serious correctness miss.

The app told a seller they got paid when they did not. The code bug enabled that, and our launch verification failed to challenge it because it trusted semantic success instead of wallet truth.

The system is materially better now because:

- seller settlement no longer uses fuzzy correlation
- packaged release verification now requires real seller balance increase
- unpaid seller flows remain explicit and test-covered

That is the minimum bar going forward: seller payment claims must be wallet-authoritative, not narrative-authoritative.

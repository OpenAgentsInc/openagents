# Issue 3430 Audit: Earnings History Reset and Wallet Refresh Drift

## Scope

This audit explains the behavior reported in GitHub issue `#3430`:

- earnings history showing `0` after app restart even though the wallet still held sats
- `Today` / `This Month` / `All Time` not behaving truthfully
- wallet balance appearing stale after reopen and only catching up later
- operator perception that `Go Offline` updates the wallet more reliably than normal wallet refresh

This is an audit of the current codebase behavior on `main`, not a fix note.

## User-Visible Symptom

The report is internally consistent:

1. User had already earned sats earlier in the day.
2. Wallet still showed those sats.
3. Earnings history restarted from `0` after reopen.
4. New jobs incremented the history from that fresh zero baseline instead of from the already-earned amount.
5. After reopen, wallet balance itself also appeared behind the real total.
6. At some later point, the wallet finally caught up to the true balance.

That means there are at least two failures:

- the earnings scoreboard is not reconstructing its authoritative earning history on restart
- the wallet pane is not reliably converging to the post-sync Spark truth on its own

## Executive Summary

There are three separate bugs behind `#3430`.

### 1. Earnings history is effectively session-local

The Mission Control earnings panel is computed from `JobHistoryState` plus reconciled Spark receive payments.

The problem is that `JobHistoryState` is initialized empty on startup in `apps/autopilot-desktop/src/render.rs`, and I do not see a production hydration path that rebuilds it from the persisted authoritative receipt bundle on restart.

What *is* persisted:

- `EarnKernelReceiptState` in `apps/autopilot-desktop/src/state/earn_kernel_receipts.rs`
- `EarnJobLifecycleProjectionState` in `apps/autopilot-desktop/src/app_state.rs`

What the earnings scoreboard actually reads:

- `state.job_history`
- `state.spark_wallet`

So on restart, the app remembers replay/projection activity, but not the scoreboard’s own authoritative history input.

That explains the exact user report:

- wallet still had earlier sats
- earnings panel restarted from `0`
- new earnings accumulated from there

### 2. The earnings time windows are anchored to a stale hardcoded reference time

`JobHistoryState` defaults `reference_epoch_seconds` to `1_761_920_000`, which is:

- `2025-10-31 14:13:20 UTC`

I do not see production code updating that reference timestamp.

That means:

- `Today` is effectively calculated relative to late October 2025
- `This Month` is compared against October 2025, not the current month

So even if the restart-persistence problem did not exist, the current scoreboard time windows are already wrong.

This is why the earnings panel can look partly plausible while still being false:

- `All Time` depends on the rows present in memory
- `Today` uses wallet receive timestamps against a stale threshold that is so old it will often include almost everything
- `This Month` can silently be wrong because it is pegged to the wrong calendar month

### 3. Wallet refresh is snapshot-based, not sync-driven

The Spark wallet pane does not appear to subscribe to a “wallet sync completed, now refresh visible state” signal.

At startup:

- `open_startup_panes()` in `apps/autopilot-desktop/src/render.rs` immediately queues `SparkWalletCommand::Refresh`

That refresh path in `apps/autopilot-desktop/src/spark_wallet.rs`:

- ensures the wallet exists
- fetches network status
- fetches balance
- fetches recent payments
- updates UI state once

But it does **not**:

- wait for Spark’s background sync to finish before trusting that snapshot
- schedule a follow-up refresh when Spark later catches up
- hydrate from an independently persisted wallet snapshot

So if startup refresh runs before Spark has fully synchronized the wallet state, the UI can show an old balance and then stay stale until some later refresh command happens.

That matches the issue report:

- reopen showed a lower wallet balance than expected
- later, the wallet balance caught up

## Detailed Failure Path

## A. Why replay/open rows survive restart but earnings totals do not

The operator saw replay activity after reopen.

That makes sense because the app does persist lifecycle projection rows:

- `EarnJobLifecycleProjectionState::default()` loads its projection file on startup
- those rows are then mirrored into Mission Control as `[REPLAY/OPEN]` style entries

But the earnings scoreboard does **not** derive from that projection stream.

Instead, `EarningsScoreboardState::refresh_from_sources()` in `apps/autopilot-desktop/src/app_state.rs` uses:

- `job_history.wallet_reconciled_payout_rows(spark_wallet)`

If `job_history.rows` is empty after restart, then:

- `jobs_today = 0`
- `sats_today = 0`
- `sats_this_month = 0`
- `lifetime_sats = 0`

even though the replay stream still shows old completed and paid jobs.

This is the core truth mismatch in `#3430`:

- the UI clearly remembers jobs in one subsystem
- but the earnings counters are built from a different subsystem that is not rebuilt on restart

## B. Why the wallet can be lower after reopen even if the real Spark balance is higher

The wallet worker keeps a live `SparkWallet` object inside `SparkPaneState`, but the visible pane state is only updated when commands are processed and their resulting snapshot is sent back to the render state.

The refresh path does not look event-driven after Spark background sync completes.

So the sequence can be:

1. app starts
2. startup refresh is queued immediately
3. Spark background sync is still catching up
4. the refresh returns a partially stale balance/payment view
5. the UI shows that stale number
6. Spark later finishes syncing
7. no automatic UI refresh occurs from that completion alone
8. a later explicit refresh happens and the number jumps

This is especially plausible because the issue body explicitly reports that the wallet finally updated later, not that it stayed permanently wrong.

## C. Why “Go Offline fixed it” is probably a timing artifact, not a special offline wallet path

The current `SetOnline { online: false }` flow in `apps/autopilot-desktop/src/input.rs` does **not** queue a wallet refresh in the same way that the online path does.

So the user observation:

- “something with going offline seems to trigger an update”

should not be read as “there is a deliberate go-offline wallet reconciliation step.”

The more likely explanation is:

- some other refresh happened later
- or the user observed the balance only after enough time had passed for Spark to catch up and another command-driven snapshot to apply

Operationally, the user complaint is still valid:

- the UI became truthful too late
- the path by which it became truthful was opaque

## D. The scoreboard time math is already invalid even when history exists

`EarningsScoreboardState::refresh_from_sources()` does:

- `threshold = job_history.reference_epoch_seconds - 86_400`
- today counts rows whose `wallet_received_at_epoch_seconds >= threshold`
- month counts rows whose receipt month matches `reference_epoch_seconds`

But `JobHistoryState::default()` hardcodes:

- `reference_epoch_seconds: 1_761_920_000`

and I do not see production code advancing it to current wall-clock time.

That means:

- the “today” cutoff is frozen near late October 2025
- the “month” comparison is frozen to October 2025

So:

- `Today` is not really today
- `This Month` is not really this month

This is independent of the restart bug.

## Why Our Existing Surfaces Made This Harder to See

The app has several parallel representations of seller state:

- wallet pane
- earnings scoreboard
- replay/projection stream
- active job pane
- earn-kernel receipts

Those do not currently share one restart-authoritative source of truth.

So the operator can see:

- replay rows proving jobs happened
- a wallet balance that may or may not be caught up
- an earnings scoreboard that has forgotten earlier jobs

Each subsystem is internally explainable, but the combined operator experience is false and confusing.

## Why Prior Tests Did Not Catch This

The current test mix heavily exercises:

- payment correctness
- seller `paid` vs `delivered-unpaid`
- buyer payment reconciliation
- packaged roundtrip balance deltas

But it does not appear to enforce the exact restart truth that `#3430` exposed:

1. persist receipts and projection rows
2. restart app state
3. rebuild visible earnings counters
4. assert that wallet, history, and replay all tell the same story

It also does not appear to enforce:

- that earnings time windows use a live current reference time
- that a startup Spark sync eventually updates the visible wallet balance without a human guessing which action to click

## Root Cause Breakdown

## Root Cause 1: No startup rehydration path from persisted seller receipts into `JobHistoryState`

Persisted seller evidence exists.

But the scoreboard still depends on an in-memory structure that is reset to empty at startup.

This is the direct cause of:

- history `0` after reopen
- wallet nonzero while earnings scoreboard resets

## Root Cause 2: `reference_epoch_seconds` is stale and not maintained

The earnings time-window math is using a static old reference timestamp.

This is the direct cause of:

- wrong `This Month`
- overbroad `Today`
- unstable operator trust in the panel

## Root Cause 3: Wallet pane is command-refresh-driven instead of sync-completion-driven

The visible Spark pane state is updated when refresh commands run, not when the underlying Spark sync lifecycle finishes and produces newer truth.

This is the direct cause of:

- stale balance after reopen
- later “catch up” behavior that feels random

## What Needs To Change

## 1. Make earnings restart-authoritative

One of these must become true:

- `JobHistoryState` is rebuilt from persisted authoritative earn-kernel receipts on startup
- or the earnings scoreboard stops depending on ephemeral `job_history` and computes directly from persisted authoritative receipt data plus wallet payments

The second option is cleaner because it removes a fragile replay-only middle layer from the scoring path.

## 2. Remove the hardcoded reference clock from earnings calculations

`reference_epoch_seconds` must be replaced or updated with real current time.

Minimum acceptable fix:

- set it from `current_epoch_seconds()` every refresh cycle

Better fix:

- stop storing a synthetic reference inside `JobHistoryState` and pass current time into the relevant calculations directly

## 3. Make the wallet pane converge automatically after Spark sync catches up

The wallet pane needs a deterministic convergence rule such as:

- after wallet initialization, keep refreshing until Spark reports sync-complete and the visible balance/payments are current
- or subscribe to/bridge the Spark sync-complete event into a UI-state refresh
- or both

The user should not need a lucky second action to see the real balance.

## 4. Make restart truth consistent across replay, wallet, and earnings

On restart, the app should converge to a state where:

- replay rows show historical jobs
- wallet shows the actual current balance
- earnings scoreboard reflects authoritative persisted earnings

If those three disagree, the app should surface a reconciliation state explicitly instead of pretending the scoreboard is `ready`.

## Test Gaps That Must Be Filled

The following regression tests should exist before trusting this path:

1. `restart_preserves_earnings_scoreboard_from_persisted_receipts`
   - seed persisted seller receipts and wallet payments
   - restart `RenderState`
   - assert `Today` / `This Month` / `All Time` survive restart truthfully

2. `earnings_scoreboard_uses_live_reference_time`
   - assert month/day windows are derived from current time, not a fixed epoch constant

3. `startup_wallet_balance_catches_up_after_spark_sync`
   - simulate a stale initial wallet snapshot followed by a later synced snapshot
   - assert the visible balance converges without requiring a mode toggle guess

4. `projection_wallet_earnings_are_consistent_after_restart`
   - persisted replay rows + persisted receipts + wallet payments
   - assert all visible seller surfaces agree

5. `wallet_refresh_and_go_offline_do_not_have_different_truth_paths`
   - if balance changes after one, it must also change after the other when the underlying data is the same

## Release Impact

`#3430` is release-significant.

Even if sats are truly in the wallet, the current app can:

- forget prior earnings in the earnings panel on restart
- show a stale wallet balance after reopen
- present replay history that contradicts the scoreboard

That is not an operator-nit. It directly damages the core promise of the product:

- “I did work”
- “I got paid”
- “the app tells me the truth about both”

## Bottom Line

The app is currently persisting the wrong seller story.

It preserves:

- replayable lifecycle artifacts
- authoritative receipt bundles

but it fails to reconstruct the one operator-facing earnings surface that is supposed to summarize them.

At the same time, wallet refresh is not wired to converge automatically after Spark background sync completes, so reopen can show stale balances until some later refresh happens.

Issue `#3430` is therefore a multi-part truth bug:

- restart persistence bug
- stale reference-time bug
- wallet convergence bug

All three need to be fixed together if the earnings panel and wallet pane are going to be trustworthy on launch and after restart.

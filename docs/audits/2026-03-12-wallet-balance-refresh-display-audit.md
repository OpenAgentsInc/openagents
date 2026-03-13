# Wallet Balance Refresh And Display Audit

Date: 2026-03-12

## Scope

This audit explains how `autopilot-desktop` currently updates and displays the Spark wallet balance, why the visible balance can appear to jump around during buy mode and sell mode, and which code paths are responsible.

This is an audit of the current implementation, not a fix patch.

## Executive Summary

The visible wallet balance in Autopilot is driven from a single app-owned state object, `SparkPaneState`, but that state is refreshed by many different triggers and each refresh is assembled from multiple separate Spark calls rather than one atomic wallet snapshot.

The important consequence is:

- the app does not have true concurrent wallet writers inside the app process
- but it does have many queued refresh/reload commands
- and each refresh can observe a different point-in-time backend state

So the balance can look inconsistent even though the worker is single-threaded.

The main causes of visible balance instability are:

1. `SparkWalletWorker` accepts many uncoalesced `Refresh` / `Reload` requests.
2. A single refresh is not atomic. It runs:
   - `network_status()` which calls `sync_wallet()`
   - `get_balance()`
   - `list_all_payments()`
3. The app stores only the latest composite result, with no snapshot revision or monotonicity guard.
4. Pending outbound payment logic can intentionally hold back `balance` updates until the matching payment reaches a terminal wallet state.
5. Different surfaces render missing/stale balance differently:
   - Mission Control wallet panel shows `LOADING`
   - some control/CLI surfaces collapse missing balance to `0`
   - buy-mode start blockers describe it as `balance unavailable`

## Canonical Owner Of Wallet Balance

The in-app balance source of truth is `RenderState.spark_wallet.balance`, which is an `Option<openagents_spark::Balance>`.

Relevant code:

- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`

`Balance` comes from `crates/spark/src/wallet.rs` and currently consists of:

- `spark_sats`
- `lightning_sats`
- `onchain_sats`

The value shown in the app is `Balance::total_sats()`.

Right now, `get_balance()` in the Spark crate populates only `spark_sats`; `lightning_sats` and `onchain_sats` are `0` in the current wrapper.

## How Wallet Updates Actually Flow

### 1. Worker model

`SparkWalletWorker` is spawned on its own thread and runs commands serially.

That means:

- there is no true in-process parallel mutation of `SparkPaneState`
- the worker handles wallet commands one at a time
- the UI thread only receives cloned snapshots

Relevant code:

- `SparkWalletWorker::spawn`
- `SparkWalletWorker::drain_updates`
- `SparkPaneState::apply_command`

`drain_updates()` drains every queued worker result and leaves the UI with only the last received snapshot for that frame.

So the instability is not caused by races inside the worker. It is caused by repeated serial refreshes sampling changing backend state.

### 2. Refresh vs reload

There are two important wallet commands:

- `Refresh`
- `Reload`

`Refresh`:

- reuses the existing `SparkWallet`
- runs the sync/balance/payment refresh path

`Reload`:

- drops the current `SparkWallet`
- clears `network_status`
- then calls the same refresh path

Important nuance:

- `Reload` does not clear `balance`
- so the app can continue showing an old balance while a new wallet object is being rebuilt and refreshed

Relevant code:

- `SparkPaneState::refresh`
- `SparkPaneState::reload`

### 3. What a refresh actually does

Every refresh currently does this, in order:

1. `ensure_wallet()`
2. `wallet.network_status()`
3. `wallet.get_balance()`
4. `wallet.list_all_payments()`

The first problem is that `network_status()` itself performs a wallet sync:

- in `crates/spark/src/wallet.rs`, `network_status()` calls `sdk.sync_wallet(...)`

The second problem is that `get_balance()` and `list_all_payments()` are separate calls:

- `get_balance()` calls `sdk.get_info(ensure_synced: true)`
- `list_all_payments()` pages through `sdk.list_payments(...)`

So the app is building one visible state from multiple backend reads that are not guaranteed to describe the same instant.

There is no snapshot token, revision number, or read timestamp carried through the refresh path.

## Every Known Trigger That Queues Wallet Updates

### Startup

On startup, opening the `GoOnline` or `SparkWallet` startup panes does two things:

- starts startup convergence mode
- queues an immediate `Reload`

Then a periodic startup convergence tick queues up to 3 more `Reload`s at 2-second intervals.

Relevant code:

- `render.rs` `open_startup_panes(...)`
- `input.rs` `run_startup_spark_wallet_convergence_tick(...)`
- `spark_wallet.rs`
  - `begin_startup_convergence`
  - `startup_convergence_refresh_due`
  - `note_startup_convergence_refresh_queued`

This is explicitly designed to let the app settle after startup sync, but it also means the wallet can visibly move through multiple sampled states shortly after launch.

### Go Online

When provider mode is turned online, the app queues a `Reload`.

Relevant code:

- `input.rs` provider online/offline action path

So a `Go Online` click always adds another full wallet reload, even if startup convergence is still underway.

### Mission Control manual refresh

Mission Control `Refresh Wallet` queues a `Reload`.

Relevant code:

- `input/actions.rs` `MissionControlPaneAction::RefreshWallet`

### Opening the wallet pane

Opening the wallet pane from the hotbar can queue a `Reload` if it was not already open.

Relevant code:

- `hotbar.rs` `activate_hotbar_slot(...)`

### Desktop control / autopilotctl

The desktop control plane and `autopilotctl wallet refresh` route to a wallet refresh action which queues a `Reload`.

Relevant code:

- `desktop_control.rs`
- `input/tool_bridge.rs`

### Buy mode pending-payment watchdog

While buyer auto-payment is waiting for local wallet confirmation, the watchdog queues a `Reload` every 5 seconds.

Relevant code:

- `state/operations.rs` `BUYER_AUTO_PAYMENT_REFRESH_INTERVAL`
- `input/actions.rs` `run_pending_buyer_payment_watchdog_tick(...)`

### Seller-side payment-evidence refresh

While a delivered provider job is waiting for buyer payment evidence, the seller path queues a `Refresh` every 5 seconds.

Relevant code:

- `input/actions.rs`
  - `ACTIVE_JOB_PAYMENT_EVIDENCE_REFRESH_INTERVAL`
  - `active_job_payment_evidence_refresh_due(...)`

### Invoice creation / send payment actions

Wallet mutation actions also trigger immediate refresh logic:

- `CreateInvoice`
- `CreateBolt11Invoice`
- `SendPayment`

Relevant code:

- `SparkPaneState::create_invoice`
- `SparkPaneState::create_bolt11_invoice`
- `SparkPaneState::send_payment`

So a send or invoice action can update visible wallet state immediately, before later watchdog-driven refreshes run.

## Special Pending-Payment Balance Gate

There is an app-specific balance-hold behavior for outbound payments.

When the app sends a payment:

- it stores `pending_balance_confirmation_payment_id`
- then refreshes balance and payments

Later, `apply_balance_refresh_with_payment_confirmation(...)` decides whether to accept the fetched balance.

Behavior today:

- if there is no pending payment pointer, the fetched balance is applied immediately
- if there is a pending pointer but the payment is not yet found in `recent_payments`, the visible `balance` is not updated
- if the payment is found but still nonterminal, the visible `balance` is not updated
- only once the payment is terminal does the app apply the fetched balance and clear the pending pointer

This means the app can intentionally freeze the visible balance during outbound payment confirmation, even if the backend balance has changed for some other reason.

Relevant code:

- `SparkPaneState::send_payment`
- `SparkPaneState::apply_balance_refresh_with_payment_confirmation`

## Where The Balance Is Rendered

### Mission Control wallet panel

Mission Control uses:

- `spark_wallet.balance.total_sats()` if present
- `LOADING` if `balance` is `None`

Relevant code:

- `pane_renderer.rs`

This is the cleanest display path.

### Spark Wallet pane

The dedicated wallet pane is considered `Loading` until both:

- `network_status` is present
- `balance` is present

Then it renders:

- Spark sats
- Lightning sats
- Onchain sats
- Total sats

Relevant code:

- `panes/wallet.rs`

### Buy Mode gating

Buy Mode start readiness uses `spark_wallet.balance` directly:

- if present and above budget, buy mode can start
- if present but low, it says balance is too low
- if absent, it says `balance unavailable`

Relevant code:

- `app_state.rs` `mission_control_buy_mode_available_balance_sats(...)`

### Desktop control / autopilotctl snapshot

Desktop control exports:

- `balance_sats = state.spark_wallet.balance.map_or(0, ...)`

So unlike Mission Control, the control snapshot collapses missing balance to `0`.

Relevant code:

- `desktop_control.rs`

This is an important inconsistency. During loading/reconciling, UI and CLI can disagree:

- UI says `LOADING`
- control snapshot says `0`

## Why The Balance Can “Jump Around”

### 1. The app has many refresh sources and does not coalesce them

There is no central dedupe or “latest desired refresh wins” mechanism.

If startup convergence, Go Online, buy-mode watchdog, seller payment-evidence refresh, and a manual refresh all queue near each other, the worker will run them serially.

That means the UI can walk through several snapshots in short succession.

### 2. Each refresh is multi-step and non-atomic

A single refresh mixes:

- sync status
- balance fetch
- payment list

Because these are separate reads, one refresh can observe:

- a newer balance but older payment list
- an older balance but newer payment list
- a transient backend state that is corrected by the next refresh

### 3. Reload preserves old balance while rebuilding wallet internals

`Reload` clears the wallet object and `network_status`, but not `balance`.

That means the display can temporarily show:

- stale prior balance
- new status label like `unknown` or `reconciling`

Then later jump to a newly fetched balance.

### 4. Pending outbound payment logic intentionally blocks balance replacement

During buyer auto-payment, the visible balance may stay frozen until the matching payment becomes terminal.

So if there are other wallet changes at the same time, the visible number can lag and then jump once the gate clears.

### 5. Startup convergence intentionally retries multiple times

This is working as designed, but it amplifies visible balance movement after launch.

The app intentionally rechecks the wallet several times because first-run/startup Spark state is not assumed to be converged immediately.

### 6. Control surfaces disagree on missing balance

Mission Control and wallet pane treat missing balance as loading.

Desktop control snapshots collapse missing balance to `0`.

So logs, CLI, and UI can appear inconsistent even when all are reading from the same app state.

## Most Likely Real-World Scenarios Behind The Reported Symptom

### Startup / reopen

Likely sequence:

1. app opens with `balance = None`
2. startup reload runs
3. Spark backend returns a not-yet-fully-converged balance
4. startup convergence reloads run again 2 seconds later
5. balance moves to a later, corrected value

### Go Online

Likely sequence:

1. startup convergence is still active or recently finished
2. user clicks `Go Online`
3. provider path queues another `Reload`
4. relay/provider state changes happen while Spark sync is still settling
5. visible balance updates again

### Buy mode

Likely sequence:

1. buyer sends payment
2. app stores a pending payment pointer
3. wallet refreshes continue every 5 seconds
4. displayed balance is held or sampled across multiple backend states
5. once the payment becomes terminal, the displayed balance snaps to a later number

### Sell mode

Likely sequence:

1. seller delivers result and starts waiting for payout evidence
2. seller-side payment-evidence refresh runs every 5 seconds
3. receive payment lands at the backend
4. one refresh sees partial/stale state
5. a later refresh sees the settled receive and the visible balance jumps

## What The Current Design Gets Right

- Wallet operations are off the main thread.
- The app has one app-owned wallet state object instead of many independent balance stores.
- Startup convergence and payment watchdogs exist because backend settlement is not instant.
- Mission Control wallet panel now prefers `LOADING` over fake zero values.

## What Is Missing For Stable Balance Truth

The current design is missing:

1. a coalesced wallet refresh scheduler
2. a single atomic balance-plus-payments snapshot from the wallet layer
3. snapshot revisioning or timestamps on wallet state
4. a monotonic or reasoned merge policy for balance updates
5. consistent fallback semantics across UI and control surfaces
6. explicit operator-facing explanation of why the balance is currently provisional

## Recommended Fix Directions

1. Replace ad hoc `Refresh` / `Reload` queueing with one wallet refresh coordinator.
2. Add a refresh reason enum and expose it in UI/logs so operators can tell whether a change came from:
   - startup convergence
   - manual refresh
   - buy-mode watchdog
   - seller payment-evidence polling
   - send/invoice action
3. Make the Spark layer return one composite snapshot struct:
   - sync status
   - balance
   - payment page(s)
   - fetched-at timestamp / revision
4. Stop collapsing missing balance to `0` in desktop control.
5. Consider rendering provisional wallet states explicitly, for example:
   - `RECONCILING`
   - `PENDING PAYMENT CONFIRMATION`
   - `STALE / LAST KNOWN`
6. Add regression coverage for:
   - startup convergence changing the balance across retries
   - buyer pending-payment hold behavior
   - seller receive settlement convergence
   - control-plane snapshot parity with Mission Control during loading/reconciling

## Bottom Line

The wallet balance is not being mutated by random concurrent threads inside the app.

The instability comes from a different problem:

- many refresh triggers
- no coalescing
- non-atomic wallet reads
- conditional balance suppression during pending payments
- inconsistent rendering of missing/provisional state

That combination is enough to make the number look jumpy and unreliable even though the wallet worker itself is serial.

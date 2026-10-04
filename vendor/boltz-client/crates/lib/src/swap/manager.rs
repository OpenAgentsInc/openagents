use std::collections::HashSet;
use std::sync::Arc;

use platform_utils::tokio;
use tokio::sync::{Mutex, mpsc, watch};
use tokio::task::JoinSet;

use crate::api::ws::{SwapStatusSubscriber, SwapStatusUpdate};
use crate::config::CCTP_MESSAGE_TRANSMITTER_V2;
use crate::error::BoltzError;
use crate::events::{BoltzSwapEvent, EventEmitter};
use crate::evm::contracts::{
    DeliveredAmount, DeliveredAmountSource, decode_delivered_from_logs, parse_address,
};
use crate::evm::lockup::{
    SpentClassification, classify_spent_lockup, is_swap_still_locked_by_swap,
};
use crate::evm::provider::TxReceipt;
use crate::models::{BoltzSwap, BoltzSwapStatus, Bridge, BridgeKind};
use crate::store::BoltzStorage;
use crate::swap::locks::SwapLocks;
use crate::swap::reverse::{ReverseSwapExecutor, current_unix_timestamp};

/// Shared WS-subscription set: the event loop inserts on track, and the spawned
/// per-swap handlers remove on terminal cleanup. Guarded by an async mutex held
/// only for the brief set operation, never across an `.await`.
type TrackedIds = Arc<Mutex<HashSet<String>>>;

/// Maximum number of receipt-poll attempts for a `Claiming` swap (5s * 60 = 5min).
/// If the receipt is still not found after this, the loop iteration exits and
/// relies on the WS `transaction.claimed` message. On process restart,
/// `resume_all` re-triggers the poll, so this is self-healing across restarts.
const RECEIPT_POLL_MAX_ATTEMPTS: u32 = 60;
/// Interval between receipt-poll attempts.
const RECEIPT_POLL_INTERVAL_SECS: u64 = 5;

/// Background swap manager.
///
/// Owns a single event loop that acts as a **dispatcher**: it receives
/// WebSocket status updates and the delivery-poll tick, and spawns the actual
/// per-swap work (claim, receipt poll, on-chain checks, delivery confirmation)
/// into a `JoinSet` so different swaps progress in parallel and one swap's slow
/// operation never blocks another's update.
///
/// Concurrency model: **serialize per swap, parallelize across swaps.** Every
/// spawned handler — and the caller-facing `accept_degraded_quote` /
/// `update_swap_slippage` / `refresh_pending_deliveries` paths — holds the
/// swap's [`SwapLocks`] entry across its `get → mutate → persist` sequence, so
/// concurrent work on one swap is serialized (the load-bearing guard against
/// the whole-record `upsert_swap` clobbering a field, and against two competing
/// claim txs) while distinct swaps never block each other. See the 2026-06-09
/// decision-log entry.
pub(crate) struct SwapManager {
    store: Arc<dyn BoltzStorage>,
    /// Channel for sending swap IDs to track.
    cmd_tx: mpsc::Sender<String>,
    /// Shutdown signal — dropping the sender stops the event loop.
    shutdown_tx: watch::Sender<()>,
    task_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    /// Sync-safe handle used by `Drop` to abort the task if `shutdown()` was
    /// never called.
    abort_handle: tokio::task::AbortHandle,
}

impl SwapManager {
    /// Create the manager and spawn its central event loop.
    ///
    /// `ws_rx` is the global receiver for all WebSocket status updates.
    pub fn start(
        executor: Arc<ReverseSwapExecutor>,
        store: Arc<dyn BoltzStorage>,
        event_emitter: Arc<EventEmitter>,
        ws_subscriber: Arc<SwapStatusSubscriber>,
        swap_locks: Arc<SwapLocks>,
        ws_rx: mpsc::Receiver<SwapStatusUpdate>,
        delivery_poll_interval_secs: Option<u64>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(64);
        let (shutdown_tx, shutdown_rx) = watch::channel(());

        let handle = tokio::spawn(Self::run_loop(
            executor,
            store.clone(),
            event_emitter,
            ws_subscriber,
            swap_locks,
            ws_rx,
            cmd_rx,
            shutdown_rx,
            delivery_poll_interval_secs,
        ));

        let abort_handle = handle.abort_handle();

        Self {
            store,
            cmd_tx,
            shutdown_tx,
            task_handle: Mutex::new(Some(handle)),
            abort_handle,
        }
    }

    /// Begin tracking a swap. The manager will subscribe to WS updates for it
    /// and progress it through the state machine.
    pub async fn track_swap(&self, swap_id: &str) {
        let _ = self.cmd_tx.send(swap_id.to_string()).await;
    }

    /// Resume all non-terminal swaps from the store, returning their ids.
    ///
    /// With background polling enabled this is an **optional accelerator**: the
    /// periodic [`Self::reconcile_tracking`] pass re-tracks any non-terminal
    /// swap anyway, so calling this only makes resumption immediate (and yields
    /// the id list) rather than waiting for the first tick. With polling
    /// disabled (`delivery_poll_interval_secs == None`) there is no reconcile
    /// pass, so this is the **only** way previous-run swaps get tracked.
    pub async fn resume_all(&self) -> Result<Vec<String>, BoltzError> {
        let active = self.store.list_active_swaps().await?;
        let mut ids = Vec::with_capacity(active.len());
        for swap in &active {
            tracing::info!(swap_id = swap.id, status = ?swap.status, "Resuming swap");
            self.track_swap(&swap.id).await;
            ids.push(swap.id.clone());
        }
        Ok(ids)
    }

    /// Signal the event loop to shut down and wait for it to exit.
    pub async fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
        if let Some(handle) = self.task_handle.lock().await.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for SwapManager {
    fn drop(&mut self) {
        self.abort_handle.abort();
    }
}

impl SwapManager {
    // ─── Central event loop ─────────────────────────────────────────

    #[expect(clippy::too_many_arguments)]
    async fn run_loop(
        executor: Arc<ReverseSwapExecutor>,
        store: Arc<dyn BoltzStorage>,
        event_emitter: Arc<EventEmitter>,
        ws_subscriber: Arc<SwapStatusSubscriber>,
        swap_locks: Arc<SwapLocks>,
        mut ws_rx: mpsc::Receiver<SwapStatusUpdate>,
        mut cmd_rx: mpsc::Receiver<String>,
        mut shutdown_rx: watch::Receiver<()>,
        delivery_poll_interval_secs: Option<u64>,
    ) {
        // One unit of dispatched work, produced by the `select!` and acted on
        // afterwards so no spawned task borrows `tasks` across the select.
        enum Step {
            Stop,
            DeliveryTick,
            Ws(SwapStatusUpdate),
            Track(String),
        }

        // Swap IDs currently tracked (for WS dispatch filtering). Shared with
        // the spawned handlers, which remove their swap on terminal cleanup.
        let tracked_ids: TrackedIds = Arc::new(Mutex::new(HashSet::new()));

        // In-flight per-swap work. The loop only dispatches; each WS handler and
        // delivery poll runs here under its swap's lock, so one swap's slow
        // operation (e.g. a multi-minute receipt poll) never blocks another.
        let mut tasks: JoinSet<()> = JoinSet::new();

        // Background ticker driving both delivery confirmation (`Settling`) and
        // autonomous recovery of stuck `Claiming` swaps. `None` disables it
        // (callers drive delivery confirmation via `refresh_pending_deliveries`;
        // `Claiming` recovery then only runs on WS events / `resume_all`). Missed
        // ticks (if a branch handler ran long) just coalesce into idempotent
        // catch-up polls, so the default missed-tick behavior is fine — and
        // `set_missed_tick_behavior` isn't available on the WASM tokio shim.
        let mut delivery_ticker = delivery_poll_interval_secs.map(|secs| {
            tokio::time::interval(platform_utils::time::Duration::from_secs(secs.max(1)))
        });

        // Native `interval` fires its first tick immediately, so a swap resumed
        // as `Settling`/`Claiming` is serviced right away. The WASM tokio shim
        // fires the first tick only after one full period, so poll once up front
        // there — otherwise a resumed swap's first check is delayed by up to
        // `delivery_poll_interval_secs` after every page reload.
        #[cfg(all(target_family = "wasm", target_os = "unknown"))]
        if delivery_poll_interval_secs.is_some() {
            Self::spawn_delivery_tick(
                &mut tasks,
                &executor,
                &store,
                &event_emitter,
                &ws_subscriber,
                &swap_locks,
                &tracked_ids,
            );
        }

        loop {
            // Reap finished handlers so the JoinSet stays bounded by in-flight
            // work rather than total work ever dispatched.
            while tasks.try_join_next().is_some() {}

            let step = tokio::select! {
                _ = shutdown_rx.changed() => Step::Stop,
                () = async {
                    match delivery_ticker.as_mut() {
                        Some(t) => { t.tick().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => Step::DeliveryTick,
                update = ws_rx.recv() => match update {
                    Some(u) => Step::Ws(u),
                    None => Step::Stop,
                },
                cmd = cmd_rx.recv() => match cmd {
                    Some(id) => Step::Track(id),
                    None => Step::Stop,
                },
            };

            match step {
                Step::Stop => break,
                Step::DeliveryTick => Self::spawn_delivery_tick(
                    &mut tasks,
                    &executor,
                    &store,
                    &event_emitter,
                    &ws_subscriber,
                    &swap_locks,
                    &tracked_ids,
                ),
                Step::Ws(update) => {
                    if !tracked_ids.lock().await.contains(&update.swap_id) {
                        tracing::warn!(boltz_id = update.swap_id, "WS update for untracked swap");
                        continue;
                    }
                    let executor = executor.clone();
                    let store = store.clone();
                    let event_emitter = event_emitter.clone();
                    let ws_subscriber = ws_subscriber.clone();
                    let swap_locks = swap_locks.clone();
                    let tracked_ids = tracked_ids.clone();
                    tasks.spawn(async move {
                        // Hold the swap's lock across the whole handler so a
                        // second event for the same swap — or a caller-driven
                        // claim — runs strictly after this one, on re-read state.
                        let _guard = swap_locks.lock(&update.swap_id).await;
                        Self::handle_ws_update(
                            &executor,
                            &store,
                            &event_emitter,
                            &ws_subscriber,
                            &tracked_ids,
                            &update,
                        )
                        .await;
                    });
                }
                Step::Track(swap_id) => {
                    if let Err(e) =
                        Self::start_tracking(&ws_subscriber, &tracked_ids, &swap_id).await
                    {
                        tracing::error!(swap_id, error = %e, "Failed to start tracking swap");
                    }
                }
            }
        }

        // Graceful shutdown: let in-flight per-swap work finish. This matches
        // the old inline loop, where a running handler could not be interrupted
        // by the shutdown signal anyway. `Drop` aborts as the hard backstop.
        while tasks.join_next().await.is_some() {}

        tracing::info!("SwapManager event loop exiting");
    }

    /// Spawn the per-tick background work: re-track any resurrected/unresumed
    /// non-terminal swap ([`Self::reconcile_tracking`]) and advance in-flight
    /// swaps one step ([`poll_pending_swaps`]). Shared by the periodic tick and
    /// the WASM up-front kick (the WASM ticker doesn't fire immediately).
    fn spawn_delivery_tick(
        tasks: &mut JoinSet<()>,
        executor: &Arc<ReverseSwapExecutor>,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &Arc<EventEmitter>,
        ws_subscriber: &Arc<SwapStatusSubscriber>,
        swap_locks: &Arc<SwapLocks>,
        tracked_ids: &TrackedIds,
    ) {
        {
            let store = store.clone();
            let ws_subscriber = ws_subscriber.clone();
            let tracked_ids = tracked_ids.clone();
            tasks.spawn(async move {
                Self::reconcile_tracking(&store, &ws_subscriber, &tracked_ids).await;
            });
        }
        {
            let executor = executor.clone();
            let store = store.clone();
            let event_emitter = event_emitter.clone();
            let swap_locks = swap_locks.clone();
            tasks.spawn(async move {
                poll_pending_swaps(&executor, &store, &event_emitter, &swap_locks).await;
            });
        }
    }

    /// Begin tracking a specific swap: subscribe to WS and wait for the
    /// backend to send the current status. The WS update will drive any
    /// needed action via `handle_ws_update` — we don't act on local state
    /// here because another instance may have progressed the swap.
    async fn start_tracking(
        ws_subscriber: &Arc<SwapStatusSubscriber>,
        tracked_ids: &TrackedIds,
        swap_id: &str,
    ) -> Result<(), BoltzError> {
        tracked_ids.lock().await.insert(swap_id.to_string());
        // Undo the optimistic insert if the subscribe fails, so `reconcile_tracking`
        // doesn't treat the swap as tracked forever and skip re-engaging it.
        if let Err(e) = ws_subscriber.subscribe(swap_id).await {
            tracked_ids.lock().await.remove(swap_id);
            return Err(e);
        }
        Ok(())
    }

    /// Re-track every non-terminal swap in the store that isn't already tracked.
    ///
    /// Runs on the delivery-poll cadence. Its job is convergence under
    /// optimistic, eventually-consistent replication: when an out-of-order sync
    /// write from another instance *resurrects* a swap (overwrites a fresher
    /// local state with a staler non-terminal one), or a swap was simply never
    /// `resume`d, this re-engages it — re-subscribe to WS (Boltz re-pushes the
    /// current status) and let the state machine + on-chain checks drive it back
    /// to the chain-derived state. So the local store is treated as a possibly
    /// stale cache that the manager continuously reconciles against Boltz/chain,
    /// which makes convergence robust to *any* sync merge policy with nothing
    /// required of the embedder.
    ///
    /// Idempotent: already-tracked swaps are skipped, and a swap that was
    /// finalized between the `list` and the re-subscribe is harmless — the
    /// subscribe-triggered status push drives `handle_ws_update` to clean it up.
    ///
    /// `Settling` is excluded: its progression is delivery-poll-driven
    /// (`poll_pending_swaps` -> `confirm_delivery`, which also re-completes a
    /// resurrected `Settling` swap), and `handle_ws_update` does nothing for a
    /// `Settling` swap but immediately untrack it. Re-tracking it would just
    /// subscribe/unsubscribe every tick for the whole delivery window.
    async fn reconcile_tracking(
        store: &Arc<dyn BoltzStorage>,
        ws_subscriber: &Arc<SwapStatusSubscriber>,
        tracked_ids: &TrackedIds,
    ) {
        let active = match store.list_active_swaps().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to list active swaps for tracking reconcile");
                return;
            }
        };
        for swap in active {
            if swap.status == BoltzSwapStatus::Settling {
                continue;
            }
            if tracked_ids.lock().await.contains(&swap.id) {
                continue;
            }
            tracing::info!(swap_id = swap.id, status = ?swap.status, "Re-tracking active swap");
            if let Err(e) = Self::start_tracking(ws_subscriber, tracked_ids, &swap.id).await {
                tracing::error!(swap_id = swap.id, error = %e, "Failed to re-track active swap");
            }
        }
    }

    /// Process a WS status update for a tracked swap.
    #[expect(clippy::too_many_lines)]
    async fn handle_ws_update(
        executor: &Arc<ReverseSwapExecutor>,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &Arc<EventEmitter>,
        ws_subscriber: &Arc<SwapStatusSubscriber>,
        tracked_ids: &TrackedIds,
        update: &SwapStatusUpdate,
    ) {
        let swap_id = &update.swap_id;
        let mut swap = match store.get_swap(swap_id).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                tracing::warn!(swap_id, "WS update for unknown swap");
                return;
            }
            Err(e) => {
                tracing::error!(swap_id, error = %e, "Failed to load swap for WS update");
                return;
            }
        };

        if swap.status.is_terminal() {
            tracing::debug!(swap_id, status = ?swap.status, "Swap already terminal, cleaning up");
            Self::cleanup_terminal(ws_subscriber, tracked_ids, swap_id).await;
            return;
        }

        // Settling = past the claim: delivery is confirmed by the background
        // poll, not the WS. This MUST run before the status match below — else a
        // late `swap.expired` / `invoice.expired` (re-delivered after
        // `resume_all` re-subscribes) would wrongly drive an already-claimed
        // swap to `Expired`. A Settling swap only leaves Settling via confirmed
        // delivery, by design.
        if swap.status == BoltzSwapStatus::Settling {
            tracing::debug!(
                swap_id,
                "Swap settling; delivery confirmation is poll-driven"
            );
            Self::cleanup_terminal(ws_subscriber, tracked_ids, swap_id).await;
            return;
        }

        // Monotonicity guard: ignore a forward-progress WS event that maps to an
        // earlier lifecycle stage than the swap has already reached. This
        // generalizes the terminal/Settling short-circuits above to the
        // intermediate states, so a late or replayed `invoice.paid` can't demote
        // an already-advanced (e.g. `Claiming`) swap back to `InvoicePaid` —
        // which would strip the `handle_terminal_ws_event` `Claiming` re-check
        // from a subsequent terminal event and let it finalize (and drop) a
        // successful, preimage-revealed claim. `transaction.confirmed` is
        // intentionally NOT covered (it is status-aware: it resumes a `Claiming`
        // swap rather than regressing it) — see `ws_progress_stage`.
        if let Some(stage) = ws_progress_stage(update.status.as_str())
            && stage < swap.status
        {
            tracing::debug!(
                swap_id,
                ws_status = update.status,
                event_stage = ?stage,
                current = ?swap.status,
                "Ignoring stale forward-progress WS event that would regress status"
            );
            return;
        }

        tracing::info!(
            swap_id,
            local_status = ?swap.status,
            ws_status = update.status,
            "Processing WS update"
        );

        match update.status.as_str() {
            "swap.created" | "invoice.set" | "invoice.pending" => {}
            "invoice.paid" => {
                update_swap_status(
                    &**store,
                    event_emitter,
                    &mut swap,
                    BoltzSwapStatus::InvoicePaid,
                )
                .await;
            }
            "transaction.mempool" => {
                if let Some(tx) = &update.transaction {
                    let mut s = swap;
                    s.lockup_tx_id = Some(tx.id.clone());
                    s.updated_at = current_unix_timestamp();
                    if let Err(e) = store.upsert_swap(&s).await {
                        tracing::error!(swap_id, error = %e, "Failed to persist lockup_tx_id");
                    }
                    event_emitter
                        .emit(&BoltzSwapEvent::SwapUpdated { swap: s })
                        .await;
                }
            }
            "transaction.confirmed" => {
                if matches!(swap.status, BoltzSwapStatus::Claiming) {
                    Self::handle_claiming_resume(executor, store, event_emitter, &swap).await;
                } else {
                    // tBTC locked on-chain. Update local status, then claim.
                    let mut s = swap.clone();
                    if let Some(tx) = &update.transaction {
                        s.lockup_tx_id = Some(tx.id.clone());
                    }
                    s.status = BoltzSwapStatus::TbtcLocked;
                    s.updated_at = current_unix_timestamp();
                    if let Err(e) = store.upsert_swap(&s).await {
                        tracing::error!(swap_id, error = %e, "Failed to persist TbtcLocked status");
                    }
                    event_emitter
                        .emit(&BoltzSwapEvent::SwapUpdated { swap: s.clone() })
                        .await;
                    Self::do_claim(executor, store, event_emitter, &mut s, false).await;
                }
            }
            // `invoice.settled`: reverse swap success. Boltz can only settle the
            //   hold invoice once it holds the preimage, and the preimage only
            //   reaches the chain inside our atomic claim tx (claim + DEX +
            //   bridge-send, all-or-nothing). So a *genuine* settled event is
            //   itself proof the source-side claim + send succeeded.
            // `transaction.claimed`: submarine/chain swap success (included
            //   for completeness, not expected for reverse swaps).
            //
            // We don't act on the WS event alone — a spoofed/buggy one mustn't
            // finalize a swap. With a tx hash we verify the receipt; with a
            // call_id we recover then verify it; with neither we fall back to the
            // ERC20Swap lock state as a claim-retry guard (see below).
            "invoice.settled" | "transaction.claimed" => {
                if let Some(ref tx_hash) = swap.claim_tx_hash {
                    let reached_terminal =
                        Self::poll_receipt(executor, store, event_emitter, swap_id, tx_hash).await;
                    if reached_terminal {
                        Self::cleanup_terminal(ws_subscriber, tracked_ids, swap_id).await;
                    }
                } else if let Some(call_id) = swap.pending_call_id.clone() {
                    // No tx hash recorded, but we have the gas-sponsor call_id:
                    // recover the tx hash and verify the receipt on-chain
                    // rather than trusting the WS event blindly.
                    let reached_terminal =
                        Self::resume_pending_call(executor, store, event_emitter, &swap, &call_id)
                            .await;
                    if reached_terminal {
                        Self::cleanup_terminal(ws_subscriber, tracked_ids, swap_id).await;
                    }
                } else {
                    // No tx hash and no call_id, so we can't fetch the receipt.
                    // The lock state here is a CLAIM-RETRY GUARD, not a delivery
                    // (or claim-vs-refund) check: it only tells us whether the
                    // swap is still claimable, so a spoofed/premature settled
                    // event can't make us abandon recoverable tBTC. Delivery is
                    // gated separately (`post_claim_status` -> `Settling` poll).
                    match is_swap_still_locked_by_swap(
                        &executor.evm_provider,
                        &swap,
                        executor.secrets.as_ref(),
                    )
                    .await
                    {
                        Ok(true) => {
                            // Still claimable: the claim hasn't happened (the
                            // settled event was premature or spoofed). Retry it
                            // rather than finalizing on a WS message alone — else
                            // the tBTC would sit until it refunds back to Boltz.
                            tracing::warn!(
                                swap_id,
                                "WS reports settled but swap still locked on-chain; retrying claim"
                            );
                            Self::check_on_chain_and_retry(executor, store, event_emitter, &swap)
                                .await;
                        }
                        Ok(false) => {
                            // Spent. Combined with the settled event — which
                            // requires the preimage from our atomic claim — this
                            // means the source-side claim + send succeeded (a
                            // refund couldn't have produced a preimage to settle).
                            // Gate delivery: `post_claim_status` completes `Direct`
                            // (atomically delivered in that same tx) and holds
                            // `Oft`/`Cctp` in `Settling` for the delivery poll.
                            let next = post_claim_status(&swap);
                            update_swap_status(&**store, event_emitter, &mut swap, next).await;
                            Self::cleanup_terminal(ws_subscriber, tracked_ids, swap_id).await;
                        }
                        Err(e) => {
                            // Couldn't read the lock state — leave the swap
                            // tracked; the next WS update or `resume_all` retries.
                            tracing::warn!(
                                swap_id,
                                error = %e,
                                "On-chain lock check failed; leaving swap for retry"
                            );
                        }
                    }
                }
            }
            "invoice.expired" | "swap.expired" => {
                Self::handle_terminal_ws_event(
                    executor,
                    store,
                    event_emitter,
                    ws_subscriber,
                    tracked_ids,
                    &mut swap,
                    BoltzSwapStatus::Expired,
                )
                .await;
            }
            "invoice.failedToPay"
            | "transaction.lockupFailed"
            | "transaction.refunded"
            | "swap.refunded" => {
                let reason = update
                    .failure_reason
                    .clone()
                    .unwrap_or_else(|| update.status.clone());
                Self::handle_terminal_ws_event(
                    executor,
                    store,
                    event_emitter,
                    ws_subscriber,
                    tracked_ids,
                    &mut swap,
                    BoltzSwapStatus::Failed { reason },
                )
                .await;
            }
            _ => {
                tracing::debug!(
                    swap_id,
                    ws_status = update.status,
                    "Unknown WS status, ignoring"
                );
            }
        }
    }

    /// Execute the claim flow for a swap, handling all outcomes inline.
    async fn do_claim(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap: &mut BoltzSwap,
        skip_drift_check: bool,
    ) {
        let swap_id = swap.id.clone();

        update_swap_status(&**store, event_emitter, swap, BoltzSwapStatus::Claiming).await;

        match executor.claim_and_swap(swap, skip_drift_check).await {
            Ok(tx_hash) => {
                swap.claim_tx_hash = Some(tx_hash);
                swap.updated_at = current_unix_timestamp();
                if let Err(e) = store.upsert_swap(swap).await {
                    tracing::error!(swap_id, error = %e, "Failed to persist claim tx hash");
                }
            }
            Err(BoltzError::QuoteDegradedBeyondSlippage {
                expected_usd,
                quoted_usd,
            }) => {
                tracing::warn!(
                    swap_id,
                    expected_usd,
                    quoted_usd,
                    "Claim-time quote degraded beyond slippage tolerance"
                );
                // The claim was NOT attempted on-chain — the drift check
                // short-circuits before any signing/submit, so the tBTC is
                // still locked and the preimage was never revealed. Revert the
                // status set above back to `TbtcLocked` so the persisted (and
                // emitted) state matches the documented degraded-quote contract:
                // the consumer accepts the new rate via `accept_degraded_quote`
                // (which also tolerates `Claiming`) and the next claim retries.
                swap.status = BoltzSwapStatus::TbtcLocked;
                swap.updated_at = current_unix_timestamp();
                if let Err(e) = store.upsert_swap(swap).await {
                    tracing::error!(swap_id, error = %e, "Failed to persist TbtcLocked after degraded quote");
                }
                event_emitter
                    .emit(&BoltzSwapEvent::QuoteDegraded {
                        swap: swap.clone(),
                        expected_usd,
                        quoted_usd,
                    })
                    .await;
            }
            Err(e) => {
                tracing::error!(swap_id, error = %e, "Claim failed, staying in Claiming for retry");
            }
        }
    }

    /// Handle a terminal Boltz WS event (`*.expired` / refund / `failedToPay`)
    /// for a tracked swap.
    ///
    /// The naive action — mark the swap `Expired`/`Failed` and stop — is unsafe
    /// when the swap is mid-claim: once `do_claim` records progress the swap
    /// sits in `Claiming` until the receipt poll promotes it, and in that window
    /// the atomic claim may have ALREADY revealed the preimage and committed the
    /// bridge-send on-chain. Finalizing such a swap on a WS event alone would
    /// strand an already-successful bridged swap (delivery never confirmed,
    /// `delivered_amount` never recorded, dropped from tracking).
    ///
    /// So when `Claiming`, re-check the on-chain lock first: if still locked the
    /// claim never happened and the terminal event is legitimate. If spent, the
    /// event alone can't say why — `swaps()` reads `false` for a claim *and* a
    /// refund — so classify via the `Claim`/`Refund` events: a *claim* advances
    /// through the post-claim path (a refund event for an already-claimed swap is
    /// spurious), a *refund* makes the terminal event legitimate, and an
    /// unresolved/failed read leaves the swap for retry rather than finalizing on
    /// the event alone. For all pre-claim states
    /// (`Created`/`InvoicePaid`/`TbtcLocked`) the event is
    /// authoritative and finalizes directly.
    async fn handle_terminal_ws_event(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        ws_subscriber: &SwapStatusSubscriber,
        tracked_ids: &TrackedIds,
        swap: &mut BoltzSwap,
        terminal_status: BoltzSwapStatus,
    ) {
        let swap_id = swap.id.clone();

        if swap.status == BoltzSwapStatus::Claiming {
            match is_swap_still_locked_by_swap(
                &executor.evm_provider,
                swap,
                executor.secrets.as_ref(),
            )
            .await
            {
                Ok(true) => {
                    // Claim never happened — the tBTC is still locked and will
                    // refund to Boltz. The terminal event is legitimate.
                }
                Ok(false) => {
                    // Spent — but a claim and a refund both clear the lock.
                    // Classify: only a *claim* means this terminal event is
                    // spurious (advance the already-successful swap); a *refund*
                    // makes it legitimate (fall through to finalize).
                    match classify_spent_lockup(
                        &executor.evm_provider,
                        swap,
                        executor.secrets.as_ref(),
                    )
                    .await
                    {
                        Ok(SpentClassification::Claimed { claim_tx_hash }) => {
                            tracing::warn!(
                                swap_id,
                                ws_terminal = ?terminal_status,
                                winning_tx = claim_tx_hash,
                                "Terminal WS event for an already-claimed swap; advancing post-claim instead of finalizing"
                            );
                            let resolved = Self::advance_via_winning_claim(
                                executor,
                                store,
                                event_emitter,
                                &swap_id,
                                &claim_tx_hash,
                            )
                            .await;
                            if resolved {
                                Self::cleanup_terminal(ws_subscriber, tracked_ids, &swap_id).await;
                            }
                            return;
                        }
                        Ok(SpentClassification::Refunded) => {
                            // Genuinely refunded — the terminal event is correct.
                            // Fall through to finalize `terminal_status`.
                        }
                        Ok(SpentClassification::Unknown) => {
                            tracing::warn!(
                                swap_id,
                                "Terminal WS event; lockup spent but claim/refund unresolved; leaving for retry"
                            );
                            return;
                        }
                        Err(e) => {
                            tracing::warn!(
                                swap_id,
                                error = %e,
                                "Terminal WS event; spent-lockup classification failed; leaving for retry"
                            );
                            return;
                        }
                    }
                }
                Err(e) => {
                    // Couldn't verify the lock — don't finalize blindly. Leave
                    // the swap tracked; the next WS update or `resume_all` retries.
                    tracing::warn!(
                        swap_id,
                        error = %e,
                        "Could not verify lock state on terminal WS event; leaving swap for retry"
                    );
                    return;
                }
            }
        }

        update_swap_status(&**store, event_emitter, swap, terminal_status).await;
        Self::cleanup_terminal(ws_subscriber, tracked_ids, &swap_id).await;
    }

    /// Handle resuming a swap stuck in `Claiming` status. Either the tx hash
    /// is known (poll chain for receipt) or unknown (check on-chain if preimage
    /// was revealed).
    async fn handle_claiming_resume(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap: &BoltzSwap,
    ) {
        if let Some(ref tx_hash) = swap.claim_tx_hash {
            let _ = Self::poll_receipt(executor, store, event_emitter, &swap.id, tx_hash).await;
        } else if let Some(ref call_id) = swap.pending_call_id {
            // Crash between gas-sponsor submission and confirmation: the claim
            // was handed to the sponsor (and likely mined) but we never
            // recorded a tx hash. Recover it directly from the call_id.
            Self::resume_pending_call(executor, store, event_emitter, swap, call_id).await;
        } else {
            // Crash during Alchemy call: we set Claiming but never got a tx
            // hash back. Check on-chain if the claim went through anyway.
            Self::check_on_chain_and_retry(executor, store, event_emitter, swap).await;
        }
    }

    /// Recover a mid-claim swap from its persisted gas-sponsor `call_id`:
    /// re-poll for the tx hash, persist it (clearing `pending_call_id`), then
    /// poll the receipt to reach a terminal state. Falls back to the on-chain
    /// rescan if the `call_id` can't be resolved (e.g. the sponsor no longer
    /// knows it). Returns `true` if a terminal state was reached.
    async fn resume_pending_call(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap: &BoltzSwap,
        call_id: &str,
    ) -> bool {
        let swap_id = &swap.id;
        match executor.poll_pending_call(call_id).await {
            Ok(tx_hash) => {
                tracing::info!(
                    swap_id,
                    tx_hash,
                    "Recovered claim tx hash from pending call_id"
                );
                if let Ok(Some(mut s)) = store.get_swap(swap_id).await {
                    s.claim_tx_hash = Some(tx_hash.clone());
                    s.pending_call_id = None;
                    s.updated_at = current_unix_timestamp();
                    if let Err(e) = store.upsert_swap(&s).await {
                        tracing::error!(swap_id, error = %e, "Failed to persist recovered tx hash");
                    }
                }
                Self::poll_receipt(executor, store, event_emitter, swap_id, &tx_hash).await
            }
            Err(e) => {
                tracing::warn!(
                    swap_id,
                    error = %e,
                    "Could not recover tx hash from pending call_id, falling back to on-chain check"
                );
                Self::check_on_chain_and_retry(executor, store, event_emitter, swap).await;
                false
            }
        }
    }

    /// Poll `eth_get_transaction_receipt` for a known tx hash until it resolves
    /// to a success or revert, or the attempt budget is exhausted. A
    /// mined-but-ambiguous (absent/unknown status) or transient-error result
    /// keeps polling — only an explicit success/revert is acted on. Returns
    /// `true` if a terminal state was reached. See `apply_successful_receipt`
    /// and `recover_from_reverted_receipt` for the per-outcome handling.
    async fn poll_receipt(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap_id: &str,
        tx_hash: &str,
    ) -> bool {
        for attempt in 0..RECEIPT_POLL_MAX_ATTEMPTS {
            match Self::check_receipt_once(executor, store, event_emitter, swap_id, tx_hash).await {
                ReceiptOutcome::Advanced(reached_terminal) => return reached_terminal,
                // A reverted-but-recoverable claim is re-claimed here, on the
                // WS/resume-driven poll (NOT on the background tick — see
                // `recover_claiming_swap`).
                ReceiptOutcome::Reverted => {
                    return Self::recover_from_reverted_receipt(
                        executor,
                        store,
                        event_emitter,
                        swap_id,
                        tx_hash,
                    )
                    .await;
                }
                ReceiptOutcome::NotMined => {}
            }

            if attempt < RECEIPT_POLL_MAX_ATTEMPTS.saturating_sub(1) {
                platform_utils::tokio::time::sleep(platform_utils::time::Duration::from_secs(
                    RECEIPT_POLL_INTERVAL_SECS,
                ))
                .await;
            }
        }

        // Timed out — rely on WS `transaction.claimed` to complete.
        // On process restart, `resume_all` re-triggers the poll.
        tracing::warn!(swap_id, tx_hash, "Receipt poll timed out, waiting for WS");
        false
    }

    /// One receipt check for a known claim tx hash. A success is applied here;
    /// a revert is returned as [`ReceiptOutcome::Reverted`] for the caller to
    /// handle, since the right reaction differs (`poll_receipt` re-claims, the
    /// tick must not). An ambiguous or transiently-failed receipt is `NotMined`.
    async fn check_receipt_once(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap_id: &str,
        tx_hash: &str,
    ) -> ReceiptOutcome {
        match executor
            .evm_provider
            .eth_get_transaction_receipt(tx_hash)
            .await
        {
            Ok(Some(receipt)) if receipt.is_success() => ReceiptOutcome::Advanced(
                Self::apply_successful_receipt(
                    executor,
                    store,
                    event_emitter,
                    swap_id,
                    tx_hash,
                    &receipt,
                )
                .await,
            ),
            Ok(Some(receipt)) if receipt.is_reverted() => ReceiptOutcome::Reverted,
            Ok(_) => ReceiptOutcome::NotMined,
            Err(e) => {
                tracing::warn!(swap_id, error = %e, "Receipt poll failed");
                ReceiptOutcome::NotMined
            }
        }
    }

    /// Handle a confirmed-success claim receipt: record the delivered amount and
    /// advance to the post-claim status. Returns `true` once the swap reached
    /// its post-claim state; `false` if a `Direct` receipt lacked delivery
    /// evidence, leaving the swap in `Claiming` (see below).
    async fn apply_successful_receipt(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap_id: &str,
        tx_hash: &str,
        receipt: &TxReceipt,
    ) -> bool {
        let Ok(Some(mut swap)) = store.get_swap(swap_id).await else {
            return true;
        };
        apply_delivered_amount(executor, &mut swap, receipt, tx_hash);
        // A Direct swap completes only with positive in-receipt evidence that the
        // output token was delivered to the destination (the decoded ERC20
        // transfer, surfaced as `delivered_amount`). `claim_tx_hash` comes from
        // the gas sponsor; a successful-but-unrelated tx (compromised sponsor)
        // lacks that log and must not drive a false `Completed`. Without evidence
        // we don't finalize — the swap stays in `Claiming`; if the real claim
        // never ran the preimage was never revealed and the LN HTLC refunds.
        // Bridged swaps gate on confirmed delivery downstream
        // (`post_claim_status` -> `confirm_delivery`).
        if !direct_completion_has_evidence(&swap) {
            tracing::error!(
                swap_id,
                tx_hash,
                "Direct claim receipt missing delivery evidence; not completing"
            );
            return false;
        }
        tracing::info!(swap_id, tx_hash, "Claim receipt confirmed");
        let next = post_claim_status(&swap);
        update_swap_status(&**store, event_emitter, &mut swap, next).await;
        true
    }

    /// Handle a reverted claim receipt without stranding recoverable funds.
    ///
    /// A revert does NOT by itself mean the swap failed: the lockup may still be
    /// claimable (this tx never revealed the preimage — a slippage revert, or a
    /// stale/wrong persisted hash), in which case marking `Failed` would drop the
    /// swap and strand recoverable tBTC until it refunds to Boltz. So re-check
    /// the on-chain lock first, mirroring `handle_terminal_ws_event` and the
    /// no-hash `invoice.settled` path:
    /// - **still locked** → drop the dead hash and retry the claim (runs under
    ///   this swap's lock, so it cannot race a competing claim);
    /// - **spent** → it was spent elsewhere (timeout refund to Boltz), so no
    ///   recoverable funds remain — finalize `Failed`;
    /// - **lock unreadable** → don't finalize blindly; keep the swap tracked for
    ///   the next WS event / `resume_all`.
    ///
    /// Returns `true` if a terminal state was reached.
    async fn recover_from_reverted_receipt(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap_id: &str,
        tx_hash: &str,
    ) -> bool {
        tracing::error!(swap_id, tx_hash, "Claim tx reverted");
        let Ok(Some(mut swap)) = store.get_swap(swap_id).await else {
            return false;
        };
        match is_swap_still_locked_by_swap(&executor.evm_provider, &swap, executor.secrets.as_ref())
            .await
        {
            Ok(true) => {
                tracing::warn!(
                    swap_id,
                    tx_hash,
                    "Claim tx reverted but lockup still claimable; retrying claim"
                );
                swap.claim_tx_hash = None;
                Self::do_claim(executor, store, event_emitter, &mut swap, false).await;
                false
            }
            Ok(false) => {
                // Spent — but a lockup spent *by a claim* is a SUCCESS, not a
                // failure: our tx merely lost the race (possibly to another
                // instance sharing these keys). Only a refund is a real failure,
                // and `swaps()` can't tell the two apart — classify via the
                // on-chain Claim/Refund events before finalizing anything.
                match classify_spent_lockup(
                    &executor.evm_provider,
                    &swap,
                    executor.secrets.as_ref(),
                )
                .await
                {
                    Ok(SpentClassification::Refunded) => {
                        update_swap_status(
                            &**store,
                            event_emitter,
                            &mut swap,
                            BoltzSwapStatus::Failed {
                                reason: "Lockup refunded".to_string(),
                            },
                        )
                        .await;
                        true
                    }
                    Ok(SpentClassification::Claimed { claim_tx_hash }) => {
                        tracing::warn!(
                            swap_id,
                            reverted_tx = tx_hash,
                            winning_tx = claim_tx_hash,
                            "Our claim reverted but the lockup was claimed; advancing via the winning claim"
                        );
                        Self::advance_via_winning_claim(
                            executor,
                            store,
                            event_emitter,
                            swap_id,
                            &claim_tx_hash,
                        )
                        .await
                    }
                    Ok(SpentClassification::Unknown) => {
                        tracing::warn!(
                            swap_id,
                            tx_hash,
                            "Claim reverted and lockup spent, but claim/refund unresolved; leaving swap for retry"
                        );
                        false
                    }
                    Err(e) => {
                        tracing::warn!(
                            swap_id,
                            error = %e,
                            "Claim reverted; spent-lockup classification failed; leaving swap for retry"
                        );
                        false
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    swap_id,
                    error = %e,
                    "Claim reverted and lock check failed; leaving swap for retry"
                );
                false
            }
        }
    }

    /// Adopt the winning claim tx (located via the on-chain `Claim` event) onto
    /// the swap and advance through its receipt. Used when our own claim tx
    /// reverted or never mined but the lockup was provably *claimed* — possibly
    /// by another instance. Applying the winning tx's receipt records the
    /// delivered amount / `bridge_ref` from the tx that actually delivered, not
    /// our dead one. Returns `true` if a post-claim state was reached.
    ///
    /// A `Claim` log only exists for a *mined, successful* tx (reverted-tx logs
    /// aren't returned by `eth_getLogs`), so one receipt read suffices — no poll
    /// loop. That keeps this cheap on the sequential background tick AND off any
    /// `poll_receipt → recover → poll` cycle, since `check_receipt_once` performs
    /// no recovery. If the receipt is briefly unavailable, we've already
    /// persisted `claim_tx_hash`, so the normal `Claiming` tick retries it.
    async fn advance_via_winning_claim(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap_id: &str,
        claim_tx_hash: &str,
    ) -> bool {
        if let Ok(Some(mut s)) = store.get_swap(swap_id).await {
            // Another path/instance already advanced it — don't clobber the
            // record or re-read a now-irrelevant receipt.
            if s.status.is_terminal() || s.status == BoltzSwapStatus::Settling {
                return true;
            }
            s.claim_tx_hash = Some(claim_tx_hash.to_string());
            s.pending_call_id = None;
            s.updated_at = current_unix_timestamp();
            if let Err(e) = store.upsert_swap(&s).await {
                tracing::error!(swap_id, error = %e, "Failed to persist winning claim tx hash");
            }
        }
        match Self::check_receipt_once(executor, store, event_emitter, swap_id, claim_tx_hash).await
        {
            ReceiptOutcome::Advanced(reached_terminal) => reached_terminal,
            ReceiptOutcome::Reverted => {
                // Impossible by construction (a `Claim` log implies success).
                // Don't recover — just leave it for retry rather than looping.
                tracing::error!(
                    swap_id,
                    claim_tx_hash,
                    "Winning Claim tx unexpectedly reverted; leaving for retry"
                );
                false
            }
            ReceiptOutcome::NotMined => {
                // Receipt not yet available; `claim_tx_hash` is persisted, so the
                // next `Claiming` tick / WS event / resume retries it.
                tracing::debug!(
                    swap_id,
                    claim_tx_hash,
                    "Winning claim receipt not yet available; will retry"
                );
                false
            }
        }
    }

    /// One-shot recovery of a `Claiming` swap with a known `claim_tx_hash` (the
    /// background tick re-runs it). Advances a mined claim; once the lockup is
    /// past its timeout, finalizes `Failed` if it refunded. Deliberately never
    /// re-claims here — re-submitting a dropped or reverted claim each tick would
    /// compete with a slow tx or loop on a persistent revert, draining sponsor gas
    /// (reverted-claim re-claim lives in `poll_receipt`). The lock probe is gated
    /// behind the timeout (`current_l1`, shared per pass; `None` = read failed,
    /// defer) so a stuck swap costs one receipt read per tick.
    async fn recover_claiming_swap(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap: &BoltzSwap,
        tx_hash: &str,
        current_l1: Option<u64>,
    ) {
        let swap_id = &swap.id;

        match Self::check_receipt_once(executor, store, event_emitter, swap_id, tx_hash).await {
            // Mined successfully → advanced (or stays Claiming pending Direct
            // evidence). Either way nothing more to do on this tick.
            ReceiptOutcome::Advanced(_) => return,
            // Reverted or not-yet-mined: deliberately do NOT re-claim here; let the
            // timeout gate below finalize a genuinely stuck swap.
            ReceiptOutcome::Reverted | ReceiptOutcome::NotMined => {}
        }

        // Gate the (2-call) lock probe on the timeout: before it there is nothing
        // actionable, so a stuck swap costs only the receipt read above per tick.
        let Some(current_l1) = current_l1 else {
            return;
        };
        if !claiming_eligible_for_refund_check(current_l1, swap.timeout_block_height) {
            tracing::debug!(
                swap_id,
                "Claim tx not yet mined and lockup not past timeout; waiting"
            );
            return;
        }

        match is_swap_still_locked_by_swap(&executor.evm_provider, swap, executor.secrets.as_ref())
            .await
        {
            Ok(true) => {
                tracing::debug!(swap_id, "Past timeout but lockup not yet refunded; waiting");
            }
            Ok(false) => {
                // Past timeout and spent — but "spent" is claimed OR refunded.
                // Only finalize Failed on a *refund*; a claim (even one whose tx
                // we never saw mine, e.g. another instance's) advances instead.
                match classify_spent_lockup(&executor.evm_provider, swap, executor.secrets.as_ref())
                    .await
                {
                    Ok(SpentClassification::Refunded) => {
                        tracing::warn!(
                            swap_id,
                            claim_tx_hash = ?swap.claim_tx_hash,
                            "Claim tx never mined and lockup refunded past timeout; finalizing Failed"
                        );
                        let mut s = swap.clone();
                        update_swap_status(
                            &**store,
                            event_emitter,
                            &mut s,
                            BoltzSwapStatus::Failed {
                                reason: "Claim transaction never mined; lockup refunded"
                                    .to_string(),
                            },
                        )
                        .await;
                    }
                    Ok(SpentClassification::Claimed { claim_tx_hash }) => {
                        tracing::warn!(
                            swap_id,
                            winning_tx = claim_tx_hash,
                            "Lockup was claimed though our claim tx didn't mine; advancing via the winning claim"
                        );
                        Self::advance_via_winning_claim(
                            executor,
                            store,
                            event_emitter,
                            swap_id,
                            &claim_tx_hash,
                        )
                        .await;
                    }
                    Ok(SpentClassification::Unknown) => {
                        tracing::debug!(
                            swap_id,
                            "Lockup spent past timeout but claim/refund unresolved; leaving for retry"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(swap_id, error = %e, "Spent-lockup classification failed; leaving for retry");
                    }
                }
            }
            Err(e) => {
                tracing::warn!(swap_id, error = %e, "Claiming-poll lock check failed; leaving for retry");
            }
        }
    }

    /// Check on-chain whether the preimage was already revealed. If still
    /// locked, retry the claim. If spent, classify (claim vs refund) and either
    /// advance the claimed swap or finalize the refunded one.
    async fn check_on_chain_and_retry(
        executor: &ReverseSwapExecutor,
        store: &Arc<dyn BoltzStorage>,
        event_emitter: &EventEmitter,
        swap: &BoltzSwap,
    ) {
        let swap_id = &swap.id;

        match is_swap_still_locked_by_swap(&executor.evm_provider, swap, executor.secrets.as_ref())
            .await
        {
            Ok(true) => {
                // Still locked — safe to retry claim.
                tracing::info!(swap_id, "Swap still locked on-chain, retrying claim");
                let mut s = swap.clone();
                s.status = BoltzSwapStatus::TbtcLocked;
                s.updated_at = current_unix_timestamp();
                if let Err(e) = store.upsert_swap(&s).await {
                    tracing::error!(swap_id, error = %e, "Failed to persist TbtcLocked reset");
                }
                Self::do_claim(executor, store, event_emitter, &mut s, false).await;
            }
            Ok(false) => {
                // Spent — but claimed or refunded? Only a *claim* should advance
                // the swap to its post-claim status; a *refund* is a failure.
                // `swaps()` can't distinguish them, so classify via events.
                match classify_spent_lockup(&executor.evm_provider, swap, executor.secrets.as_ref())
                    .await
                {
                    Ok(SpentClassification::Claimed { claim_tx_hash }) => {
                        tracing::info!(
                            swap_id,
                            winning_tx = claim_tx_hash,
                            "Swap already claimed on-chain; advancing through post-claim status"
                        );
                        Self::advance_via_winning_claim(
                            executor,
                            store,
                            event_emitter,
                            swap_id,
                            &claim_tx_hash,
                        )
                        .await;
                    }
                    Ok(SpentClassification::Refunded) => {
                        let mut s = swap.clone();
                        update_swap_status(
                            &**store,
                            event_emitter,
                            &mut s,
                            BoltzSwapStatus::Failed {
                                reason: "Lockup refunded".to_string(),
                            },
                        )
                        .await;
                    }
                    Ok(SpentClassification::Unknown) => {
                        tracing::warn!(
                            swap_id,
                            "Lockup spent but claim/refund unresolved; leaving for retry"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(swap_id, error = %e, "Spent-lockup classification failed; leaving for retry");
                    }
                }
            }
            Err(e) => {
                tracing::error!(swap_id, error = %e, "On-chain check failed");
            }
        }
    }

    /// Unsubscribe from WS and remove from tracking set after a swap
    /// reaches a terminal state.
    async fn cleanup_terminal(
        ws_subscriber: &SwapStatusSubscriber,
        tracked_ids: &TrackedIds,
        swap_id: &str,
    ) {
        ws_subscriber.unsubscribe(swap_id).await;
        tracked_ids.lock().await.remove(swap_id);
    }
}

pub(crate) async fn update_swap_status(
    store: &dyn BoltzStorage,
    emitter: &EventEmitter,
    swap: &mut BoltzSwap,
    new_status: BoltzSwapStatus,
) {
    swap.status = new_status;
    swap.updated_at = current_unix_timestamp();
    if let Err(e) = store.upsert_swap(swap).await {
        tracing::error!(swap_id = swap.id, error = %e, "Failed to update swap status");
    }
    emitter
        .emit(&BoltzSwapEvent::SwapUpdated { swap: swap.clone() })
        .await;
}

/// Decode the claim receipt's logs and write the bridge tracking handle (and,
/// where it's already authoritative, the delivered amount) onto `swap` in
/// memory. The caller persists `swap` afterwards.
///
/// Per bridge:
/// - **OFT/Direct**: the decoded amount (`amountReceivedLD` / the ERC20
///   transfer) is final, so `delivered_amount` is set now. For OFT, `bridge_ref`
///   is the `LayerZero` GUID; Direct has no bridge.
/// - **CCTP**: the on-chain figure is the *burn* amount (source `feeExecuted` is
///   0), which overstates delivery by the fast-transfer fee, so `delivered_amount`
///   is left unset until Circle attests the real `feeExecuted`. `bridge_ref` is
///   synthesized as `"<source_domain>:<burn_tx_hash>"` (the tx hash isn't
///   available to the log decoder) — the key Circle Iris indexes by.
fn apply_delivered_amount(
    executor: &ReverseSwapExecutor,
    swap: &mut BoltzSwap,
    receipt: &TxReceipt,
    tx_hash: &str,
) {
    let Some(source) = delivered_source_for(executor, swap) else {
        tracing::warn!(
            swap_id = swap.id,
            tx_hash,
            dest = %swap.destination_chain,
            "No delivered-amount source resolvable for destination"
        );
        return;
    };

    match decode_delivered_from_logs(&receipt.logs, &source) {
        Some(decoded) => {
            let (delivered, bridge_ref) = delivered_and_ref(decoded, tx_hash);
            swap.delivered_amount = delivered;
            swap.bridge_ref = bridge_ref;
        }
        None => {
            tracing::warn!(
                swap_id = swap.id,
                tx_hash,
                dest = %swap.destination_chain,
                "No matching log in claim receipt; delivered_amount left unset"
            );
        }
    }
}

/// Map a decoded claim receipt to `(delivered_amount, bridge_ref)`.
///
/// CCTP defers the amount — the source `feeExecuted` is 0, so the decoded burn
/// amount overstates delivery by the fast-transfer fee — and keys Circle Iris
/// by `"<source_domain>:<burn_tx_hash>"`. OFT/Direct amounts are final at claim
/// (`amountReceivedLD` / the ERC20 transfer); OFT carries the `LayerZero` GUID
/// as its `bridge_ref`, Direct has none.
fn delivered_and_ref(decoded: DeliveredAmount, tx_hash: &str) -> (Option<u64>, Option<String>) {
    match decoded.cctp_source_domain {
        Some(domain) => (None, Some(format!("{domain}:{tx_hash}"))),
        None => (Some(decoded.amount), decoded.lz_guid),
    }
}

/// Choose the post-claim status for a swap whose claim receipt just confirmed
/// successfully. `Direct` delivery is complete on Arbitrum; `Oft`/`Cctp` enter
/// `Settling` so the background poll can confirm cross-chain delivery before
/// `Completed`.
///
/// A bridged swap is *never* marked `Completed` here: completion only happens
/// once delivery is confirmed (`confirm_delivery`). If a `bridge_ref` couldn't
/// be recovered from the receipt (anomalous — a successful bridged claim should
/// emit one), the swap still holds in `Settling` rather than falsely completing;
/// `confirm_delivery` will log that it can't track it. This is the safe failure
/// mode: a stuck-but-honest `Settling` beats a "Completed" we never verified.
/// Lifecycle stage a forward-progress WS status corresponds to, for the
/// monotonicity guard in `handle_ws_update`. Returns `None` for events that are
/// not pure forward-progress and must reach the status match on their own:
///
/// - `transaction.confirmed` — status-aware (resumes a `Claiming` swap; would be
///   wrongly suppressed if gated by its `TbtcLocked` stage).
/// - `transaction.mempool` — records `lockup_tx_id` only, never changes status.
/// - `invoice.settled` / `transaction.claimed` and the terminal events — gated
///   by their own on-chain re-checks / short-circuits, not by stage ordering.
/// - unknown statuses — handled by the catch-all arm.
///
/// Only the events that unconditionally *set* a forward status are mapped, so a
/// stale/replayed one cannot regress an already-advanced swap.
fn ws_progress_stage(ws_status: &str) -> Option<BoltzSwapStatus> {
    match ws_status {
        "swap.created" | "invoice.set" | "invoice.pending" => Some(BoltzSwapStatus::Created),
        "invoice.paid" => Some(BoltzSwapStatus::InvoicePaid),
        _ => None,
    }
}

/// Whether a confirmed-success claim receipt is sufficient to mark this swap
/// `Completed`. A `Direct` swap requires positive in-receipt delivery evidence
/// (a decoded transfer of the output token to the destination, surfaced as
/// `delivered_amount`) because `claim_tx_hash` originates from the gas sponsor
/// and a successful-but-unrelated tx must not drive a false `Completed`. Bridged
/// swaps don't complete in `poll_receipt` at all (`post_claim_status` holds them
/// in `Settling`), so they are unaffected here.
fn direct_completion_has_evidence(swap: &BoltzSwap) -> bool {
    swap.bridge_kind != BridgeKind::Direct || swap.delivered_amount.is_some()
}

/// Outcome of one claim-receipt check ([`SwapManager::check_receipt_once`]).
enum ReceiptOutcome {
    /// Receipt confirmed success and was applied; the bool is whether the swap
    /// reached its post-claim state (`false` = a `Direct` receipt lacked delivery
    /// evidence, so it stays `Claiming`).
    Advanced(bool),
    /// Receipt is on-chain but reverted. The caller decides how to react.
    Reverted,
    /// Not yet mined, or a transient RPC error — the caller keeps waiting.
    NotMined,
}

/// Whether a stuck `Claiming` swap is past its lockup timeout — the point after
/// which a refund to Boltz (hence finalizing `Failed`) becomes possible. Both are
/// **L1** heights. Before it, the caller skips the on-chain lock probe.
fn claiming_eligible_for_refund_check(current_l1: u64, timeout_block_height: u64) -> bool {
    current_l1 > timeout_block_height
}

fn post_claim_status(swap: &BoltzSwap) -> BoltzSwapStatus {
    match swap.bridge_kind {
        BridgeKind::Direct => BoltzSwapStatus::Completed,
        BridgeKind::Oft | BridgeKind::Cctp => {
            if swap.bridge_ref.is_none() {
                tracing::warn!(
                    swap_id = swap.id,
                    "Bridged swap missing bridge_ref after claim; holding in Settling (delivery unconfirmable)"
                );
            }
            BoltzSwapStatus::Settling
        }
    }
}

/// Advance every in-flight swap one step in a single store-driven pass (not tied
/// to WS tracking, so it covers swaps resumed after a restart). Used by both the
/// background tick and the on-demand [`crate::BoltzService::refresh_pending_deliveries`].
///
/// Per swap, under its lock:
/// - **`Settling`** → confirm cross-chain delivery and finalize if delivered
///   ([`confirm_delivery`]).
/// - **`Claiming`** (with a persisted `claim_tx_hash`) → autonomously recover so
///   progress doesn't depend solely on a Boltz WS event
///   ([`SwapManager::recover_claiming_swap`]). No-hash crash-recovery cases stay
///   with the WS / `resume_all` paths.
pub(crate) async fn poll_pending_swaps(
    executor: &ReverseSwapExecutor,
    store: &Arc<dyn BoltzStorage>,
    event_emitter: &EventEmitter,
    swap_locks: &SwapLocks,
) {
    let swaps = match store.list_active_swaps().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to list active swaps for pending-swap poll");
            return;
        }
    };

    // One shared L1 height read per pass, used only to gate the (2-call) lock
    // probe in `recover_claiming_swap` behind the lockup timeout — so a stuck
    // `Claiming` swap costs just one cheap receipt read per tick until then.
    // Fetched only when a recoverable `Claiming` swap exists; a read failure
    // leaves it `None` (receipt re-check still runs; refund decision deferred).
    let current_l1 = if swaps
        .iter()
        .any(|s| s.status == BoltzSwapStatus::Claiming && s.claim_tx_hash.is_some())
    {
        match executor.evm_provider.eth_l1_block_number().await {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(error = %e, "L1 height read failed; deferring claiming refund check");
                None
            }
        }
    } else {
        None
    };

    for swap in swaps {
        match swap.status {
            BoltzSwapStatus::Settling => {
                // Serialize against any other work on this swap (a concurrent
                // delivery poll from `refresh_pending_deliveries`, or a claim
                // handler). The snapshot's bridge_ref/bridge_kind are fixed
                // post-claim, so querying delivery off it is safe; the eventual
                // status write re-reads under the lock in `finalize_completed`.
                let _guard = swap_locks.lock(&swap.id).await;
                confirm_delivery(executor, store, event_emitter, &swap).await;
            }
            BoltzSwapStatus::Claiming if swap.claim_tx_hash.is_some() => {
                let _guard = swap_locks.lock(&swap.id).await;
                // Re-read under the lock: the snapshot predates the lock, so a
                // concurrent handler may already have advanced the swap. Only act
                // if it is still `Claiming` with a hash.
                let swap = match store.get_swap(&swap.id).await {
                    Ok(Some(s)) => s,
                    Ok(None) => continue,
                    Err(e) => {
                        tracing::warn!(swap_id = swap.id, error = %e, "Failed to re-read claiming swap");
                        continue;
                    }
                };
                if swap.status != BoltzSwapStatus::Claiming {
                    continue;
                }
                let Some(tx_hash) = swap.claim_tx_hash.clone() else {
                    continue;
                };
                SwapManager::recover_claiming_swap(
                    executor,
                    store,
                    event_emitter,
                    &swap,
                    &tx_hash,
                    current_l1,
                )
                .await;
            }
            _ => {}
        }
    }
}

/// Confirm cross-chain delivery for a single `Settling` swap and finalize it if
/// delivered. CCTP queries Circle Iris (and persists the authoritative
/// `feeExecuted`-adjusted amount); OFT queries `LayerZero` Scan (amount already
/// recorded at claim). Leaves the swap `Settling` on any not-yet-delivered or
/// transient-error outcome — the next poll retries.
async fn confirm_delivery(
    executor: &ReverseSwapExecutor,
    store: &Arc<dyn BoltzStorage>,
    event_emitter: &EventEmitter,
    swap: &BoltzSwap,
) {
    let Some(bridge_ref) = swap.bridge_ref.clone() else {
        tracing::warn!(
            swap_id = swap.id,
            "Settling swap has no bridge_ref; cannot confirm delivery"
        );
        return;
    };

    match swap.bridge_kind {
        BridgeKind::Cctp => match executor.cctp_delivery_status(&bridge_ref).await {
            Ok(status) => {
                // Fast path (every chain): Circle's forwarder minted.
                if let Some(delivered) = cctp_completion_amount(&status) {
                    finalize_completed(store, event_emitter, &swap.id, Some(delivered)).await;
                    return;
                }
                // The destination probe is a *fallback*, gated on Circle
                // reporting the forward FAILED — a forward still in progress
                // needs no probe, keeping the (rate-limited) Solana RPC off the
                // happy path. Funds are attested and recoverable, so a FAILED
                // forward is surfaced and confirmed from the destination chain,
                // never treated as a swap failure.
                if status.forward_failed() {
                    tracing::warn!(
                        swap_id = swap.id,
                        error_code = status.forward_error_code.as_deref().unwrap_or("unknown"),
                        "CCTP forward FAILED; funds attested and recoverable, confirming \
                         from destination chain"
                    );
                    match executor.cctp_destination_delivered(swap, &status).await {
                        Ok(Some(delivered)) => {
                            finalize_completed(store, event_emitter, &swap.id, Some(delivered))
                                .await;
                        }
                        Ok(None) => {}
                        // Transient (e.g. rate-limited) — the next poll retries.
                        Err(e) => {
                            tracing::debug!(swap_id = swap.id, error = %e, "CCTP destination delivery probe failed");
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(swap_id = swap.id, error = %e, "CCTP delivery status query failed");
            }
        },
        BridgeKind::Oft => match executor.oft_delivery_status(&bridge_ref).await {
            Ok(status) => {
                if status.is_delivered() {
                    finalize_completed(store, event_emitter, &swap.id, None).await;
                }
            }
            Err(e) => {
                tracing::warn!(swap_id = swap.id, error = %e, "OFT delivery status query failed");
            }
        },
        // Direct swaps never enter Settling; defensively finalize.
        BridgeKind::Direct => {
            finalize_completed(store, event_emitter, &swap.id, None).await;
        }
    }
}

/// Mark a `Settling` swap `Completed`, idempotently.
///
/// `poll_pending_swaps` runs on two tasks — the background ticker (event loop)
/// and the on-demand [`crate::BoltzService::refresh_pending_deliveries`] (caller
/// task) — each operating on its own snapshot. Re-reading here and
/// bailing unless the swap is still `Settling` prevents a double `Completed`
/// emission (and a redundant store write) when both observe the same swap as
/// delivered. The delivered amount is identical on either path, so the only
/// thing being deduplicated is the event/write, not the value.
async fn finalize_completed(
    store: &Arc<dyn BoltzStorage>,
    event_emitter: &EventEmitter,
    swap_id: &str,
    delivered_amount: Option<u64>,
) {
    match store.get_swap(swap_id).await {
        Ok(Some(mut fresh)) => {
            if fresh.status != BoltzSwapStatus::Settling {
                // Already finalized by a concurrent poll — don't double-emit.
                return;
            }
            if let Some(amount) = delivered_amount {
                fresh.delivered_amount = Some(amount);
            }
            update_swap_status(
                &**store,
                event_emitter,
                &mut fresh,
                BoltzSwapStatus::Completed,
            )
            .await;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::warn!(swap_id, error = %e, "Failed to re-read swap before completion");
        }
    }
}

/// The authoritative CCTP delivered amount, but only once delivery is real:
/// the message must be *forwarded* (minted on the destination) and *attested*
/// (so the finalized `feeExecuted` — hence the amount — is known). Returns
/// `None` while either is still pending, keeping the swap `Settling`.
fn cctp_completion_amount(status: &crate::evm::cctp::CctpMessageStatus) -> Option<u64> {
    if status.is_forwarded() {
        status.delivered_amount
    } else {
        None
    }
}

fn delivered_source_for(
    executor: &ReverseSwapExecutor,
    swap: &BoltzSwap,
) -> Option<DeliveredAmountSource> {
    let registry = &executor.chain_registry;
    let dest = registry.find(&swap.destination_chain, swap.asset)?;
    match dest.bridge {
        // CCTP swaps emit a MessageSent log from the MessageTransmitter rather
        // than an OFTSent / Transfer log.
        Bridge::Cctp { .. } => {
            let message_transmitter = parse_address(CCTP_MESSAGE_TRANSMITTER_V2).ok()?;
            Some(DeliveredAmountSource::Cctp {
                message_transmitter,
            })
        }
        // Direct delivery: read the final ERC20 Transfer of the output token
        // (USDT or USDC) to the user on Arbitrum.
        Bridge::Direct => {
            let token = parse_address(dest.dex_output_token).ok()?;
            let user = parse_address(&swap.destination_address).ok()?;
            Some(DeliveredAmountSource::ArbitrumTransfer { token, user })
        }
        Bridge::Oft { mesh, .. } => {
            let oft_addr = registry.oft_for(mesh)?;
            let oft_contract = parse_address(oft_addr).ok()?;
            Some(DeliveredAmountSource::OftSent { oft_contract })
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::evm::cctp::CctpMessageStatus;
    use crate::models::Asset;

    fn swap_with(bridge_kind: BridgeKind, bridge_ref: Option<&str>) -> BoltzSwap {
        BoltzSwap {
            id: "s1".to_string(),
            status: BoltzSwapStatus::Claiming,
            bridge_kind,
            key_source: crate::models::SwapKeySource::Derived { claim_key_index: 0 },
            chain_id: 42161,
            claim_address: "0xabc".to_string(),
            destination_address: "0xdef".to_string(),
            destination_chain: "Arbitrum One".to_string(),
            asset: Asset::Usdt,
            refund_address: "0x123".to_string(),
            erc20swap_address: "0xswap".to_string(),
            router_address: "0xrouter".to_string(),
            invoice: "lnbc".to_string(),
            invoice_amount_sats: 100_000,
            onchain_amount: 99_500,
            expected_output_amount: 71_000_000,
            slippage_bps: 100,
            timeout_block_height: 123_456,
            lockup_tx_id: None,
            claim_tx_hash: None,
            pending_call_id: None,
            delivered_amount: None,
            bridge_ref: bridge_ref.map(str::to_string),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        }
    }

    #[macros::test_all]
    fn direct_completes_immediately() {
        let swap = swap_with(BridgeKind::Direct, None);
        assert_eq!(post_claim_status(&swap), BoltzSwapStatus::Completed);
    }

    #[macros::test_all]
    fn direct_completion_requires_delivery_evidence() {
        // Direct + no decoded transfer log => not enough to complete.
        let mut direct = swap_with(BridgeKind::Direct, None);
        direct.delivered_amount = None;
        assert!(!direct_completion_has_evidence(&direct));

        // Direct + decoded delivered amount => evidence present, may complete.
        direct.delivered_amount = Some(71_000_000);
        assert!(direct_completion_has_evidence(&direct));

        // Bridged swaps aren't gated by this predicate (they hold in Settling
        // and complete only via confirmed delivery), so it's vacuously true.
        let oft = swap_with(BridgeKind::Oft, None);
        assert!(direct_completion_has_evidence(&oft));
    }

    #[macros::test_all]
    fn ws_progress_stage_maps_only_pure_progress_events() {
        use BoltzSwapStatus::{Created, InvoicePaid};
        assert_eq!(ws_progress_stage("swap.created"), Some(Created));
        assert_eq!(ws_progress_stage("invoice.set"), Some(Created));
        assert_eq!(ws_progress_stage("invoice.pending"), Some(Created));
        assert_eq!(ws_progress_stage("invoice.paid"), Some(InvoicePaid));

        // Status-aware / self-gated / metadata-only events are NOT gated by
        // stage ordering — they must reach the match.
        assert_eq!(ws_progress_stage("transaction.confirmed"), None);
        assert_eq!(ws_progress_stage("transaction.mempool"), None);
        assert_eq!(ws_progress_stage("invoice.settled"), None);
        assert_eq!(ws_progress_stage("swap.expired"), None);
        assert_eq!(ws_progress_stage("transaction.refunded"), None);
        assert_eq!(ws_progress_stage("garbage"), None);
    }

    #[macros::test_all]
    fn monotonicity_guard_drops_stale_invoice_paid_but_keeps_forward() {
        // The guard ignores the event iff its mapped stage is strictly below the
        // swap's current status, so a replayed `invoice.paid`
        // must not regress a `TbtcLocked`/`Claiming` swap.
        // The guard ignores the event iff `stage < swap.status`.
        let stage = ws_progress_stage("invoice.paid").unwrap();
        assert!(stage < BoltzSwapStatus::TbtcLocked); // -> ignored
        assert!(stage < BoltzSwapStatus::Claiming); //   -> ignored
        // Legitimate forward transition Created -> InvoicePaid is NOT ignored.
        assert!(stage >= BoltzSwapStatus::Created);
        // A swap already at InvoicePaid: equal, not below -> falls through (no-op).
        assert!(stage >= BoltzSwapStatus::InvoicePaid);
    }

    #[macros::test_all]
    fn bridged_with_ref_settles() {
        let oft = swap_with(BridgeKind::Oft, Some("0xguid"));
        assert_eq!(post_claim_status(&oft), BoltzSwapStatus::Settling);
        let cctp = swap_with(BridgeKind::Cctp, Some("3:0xhash"));
        assert_eq!(post_claim_status(&cctp), BoltzSwapStatus::Settling);
    }

    #[macros::test_all]
    fn bridged_without_ref_holds_in_settling_not_completed() {
        // Anomalous: a successful bridged claim should yield a bridge_ref.
        // Without one we still must NOT mark the swap Completed unverified —
        // it holds in Settling (delivery unconfirmable) rather than lying.
        let oft = swap_with(BridgeKind::Oft, None);
        assert_eq!(post_claim_status(&oft), BoltzSwapStatus::Settling);
        let cctp = swap_with(BridgeKind::Cctp, None);
        assert_eq!(post_claim_status(&cctp), BoltzSwapStatus::Settling);
    }

    // ─── claiming_eligible_for_refund_check: timeout-gated lock probe ───

    #[macros::test_all]
    fn refund_check_only_past_timeout() {
        // Strictly past the timeout: a refund is possible, so probe the lock.
        assert!(claiming_eligible_for_refund_check(1_001, 1_000));
        // At or before the timeout: no refund possible yet → skip the lock probe
        // (the gate that keeps a stuck swap at one cheap receipt read per tick).
        assert!(!claiming_eligible_for_refund_check(1_000, 1_000));
        assert!(!claiming_eligible_for_refund_check(999, 1_000));
    }

    // ─── delivered_and_ref: per-bridge field mapping at claim time ──────

    #[macros::test_all]
    fn cctp_defers_amount_and_keys_iris_by_domain_and_tx() {
        let decoded = DeliveredAmount {
            amount: 1_000_000,
            lz_guid: None,
            cctp_source_domain: Some(3),
        };
        let (delivered, bridge_ref) = delivered_and_ref(decoded, "0xburn");
        // CCTP amount is not authoritative at claim — deferred to attestation.
        assert_eq!(delivered, None);
        assert_eq!(bridge_ref.as_deref(), Some("3:0xburn"));
    }

    #[macros::test_all]
    fn oft_sets_final_amount_and_carries_lz_guid() {
        let decoded = DeliveredAmount {
            amount: 990_000,
            lz_guid: Some("0xguid".to_string()),
            cctp_source_domain: None,
        };
        let (delivered, bridge_ref) = delivered_and_ref(decoded, "0xclaim");
        assert_eq!(delivered, Some(990_000));
        assert_eq!(bridge_ref.as_deref(), Some("0xguid"));
    }

    #[macros::test_all]
    fn direct_sets_final_amount_and_no_bridge_ref() {
        let decoded = DeliveredAmount {
            amount: 500_000,
            lz_guid: None,
            cctp_source_domain: None,
        };
        let (delivered, bridge_ref) = delivered_and_ref(decoded, "0xclaim");
        assert_eq!(delivered, Some(500_000));
        assert_eq!(bridge_ref, None);
    }

    // ─── cctp_completion_amount: only complete once forwarded + attested ──

    fn cctp_status(forward_tx: Option<&str>, delivered: Option<u64>) -> CctpMessageStatus {
        CctpMessageStatus {
            found: true,
            forward_tx_hash: forward_tx.map(str::to_string),
            delivered_amount: delivered,
            ..Default::default()
        }
    }

    #[macros::test_all]
    fn cctp_completes_only_when_forwarded_and_attested() {
        // Forwarded (minted) + attested (amount known) → complete with amount.
        assert_eq!(
            cctp_completion_amount(&cctp_status(Some("0xfwd"), Some(980_000))),
            Some(980_000)
        );
        // Attested but not yet forwarded → still settling.
        assert_eq!(
            cctp_completion_amount(&cctp_status(None, Some(980_000))),
            None
        );
        // Forwarded but no attested amount yet → still settling.
        assert_eq!(
            cctp_completion_amount(&cctp_status(Some("0xfwd"), None)),
            None
        );
        // Neither → still settling.
        assert_eq!(cctp_completion_amount(&cctp_status(None, None)), None);
    }
}

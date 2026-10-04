use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use platform_utils::tokio::sync::RwLock;

use crate::models::BoltzSwap;

/// Event emitted when a swap's state changes.
#[derive(Debug, Clone)]
pub enum BoltzSwapEvent {
    /// A swap's persisted state was updated.
    SwapUpdated { swap: BoltzSwap },
    /// The claim-time DEX quote has degraded beyond the slippage tolerance
    /// compared to the creation-time quote. Auto-claim is paused.
    /// Call `accept_degraded_quote` to proceed at the current rate.
    QuoteDegraded {
        swap: BoltzSwap,
        expected_usd: u64,
        quoted_usd: u64,
    },
}

/// Callback trait for receiving swap events.
#[macros::async_trait]
pub trait BoltzEventListener: Send + Sync {
    async fn on_event(&self, event: BoltzSwapEvent);
}

/// Manages event listeners and broadcasts events.
pub struct EventEmitter {
    listener_index: AtomicU64,
    /// `Arc` so `emit` can snapshot the listeners and drop the read lock before
    /// dispatching (see [`Self::emit`]).
    listeners: RwLock<BTreeMap<String, Arc<dyn BoltzEventListener>>>,
}

impl EventEmitter {
    pub fn new() -> Self {
        Self {
            listener_index: AtomicU64::new(0),
            listeners: RwLock::new(BTreeMap::new()),
        }
    }

    pub async fn add_listener(&self, listener: Box<dyn BoltzEventListener>) -> String {
        let index = self.listener_index.fetch_add(1, Ordering::Relaxed);
        let id = format!("boltz_listener_{index}");
        self.listeners
            .write()
            .await
            .insert(id.clone(), Arc::from(listener));
        id
    }

    pub async fn remove_listener(&self, id: &str) -> bool {
        self.listeners.write().await.remove(id).is_some()
    }

    /// Broadcast an event to every registered listener.
    ///
    /// Snapshots the listeners and releases the read lock before any `on_event`
    /// runs, so a callback may (de)register a listener without deadlocking.
    /// Dispatch is sequential — keep `on_event` fast.
    pub async fn emit(&self, event: &BoltzSwapEvent) {
        let listeners: Vec<Arc<dyn BoltzEventListener>> = {
            let guard = self.listeners.read().await;
            guard.values().cloned().collect()
        };
        for listener in listeners {
            listener.on_event(event.clone()).await;
        }
    }
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::models::{Asset, BoltzSwapStatus, BridgeKind};

    fn sample_event() -> BoltzSwapEvent {
        BoltzSwapEvent::SwapUpdated {
            swap: BoltzSwap {
                id: "s1".to_string(),
                status: BoltzSwapStatus::Created,
                bridge_kind: BridgeKind::Oft,
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
                bridge_ref: None,
                created_at: 1_700_000_000,
                updated_at: 1_700_000_000,
            },
        }
    }

    struct NoopListener;
    #[macros::async_trait]
    impl BoltzEventListener for NoopListener {
        async fn on_event(&self, _event: BoltzSwapEvent) {}
    }

    /// Re-enters the emitter from its callback (needs the write lock). Pre-fix
    /// this deadlocked against the read lock `emit` held across dispatch.
    struct ReentrantListener {
        emitter: Arc<EventEmitter>,
    }
    #[macros::async_trait]
    impl BoltzEventListener for ReentrantListener {
        async fn on_event(&self, _event: BoltzSwapEvent) {
            let id = self.emitter.add_listener(Box::new(NoopListener)).await;
            assert!(self.emitter.remove_listener(&id).await);
        }
    }

    #[macros::async_test_all]
    async fn reentrant_listener_does_not_deadlock() {
        let emitter = Arc::new(EventEmitter::new());
        emitter
            .add_listener(Box::new(ReentrantListener {
                emitter: emitter.clone(),
            }))
            .await;

        // Guarded so a regression fails as a timeout rather than hanging CI.
        let event = sample_event();
        platform_utils::tokio::time::timeout(
            platform_utils::time::Duration::from_secs(5),
            emitter.emit(&event),
        )
        .await
        .expect("emit deadlocked: read lock held across on_event re-entry");
    }
}

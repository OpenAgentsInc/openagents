#[cfg(test)]
use platform_utils::tokio;

use crate::error::BoltzError;
use crate::models::BoltzSwap;

/// Persistence interface for Boltz swap state.
///
/// The boltz crate defines the trait; the caller provides the implementation.
/// (A volatile `MemoryBoltzStorage` exists for the crate's own unit tests only,
/// gated behind `#[cfg(test)]`, so it is never reachable by an embedder.)
///
/// This is all a seedless service needs — random per-swap secrets are persisted
/// inline on the swap row, so there is no key-index counter to maintain. Seeded
/// services additionally require [`DerivedKeyStore`].
#[macros::async_trait]
pub trait BoltzStorage: Send + Sync {
    /// Insert a new swap or overwrite an existing one, keyed by `swap.id`.
    ///
    /// Idempotent by contract: swap progression replays the same row through
    /// this method repeatedly. A store must accept both a first write and any
    /// later overwrite, and must never reject one because the row does (or does
    /// not) already exist.
    ///
    /// In seedless mode the row carries the swap's only copy of its preimage and
    /// claim key, so this write must be durable before it returns — a swap whose
    /// secrets aren't persisted before its invoice is paid is unclaimable.
    async fn upsert_swap(&self, swap: &BoltzSwap) -> Result<(), BoltzError>;
    async fn get_swap(&self, id: &str) -> Result<Option<BoltzSwap>, BoltzError>;
    /// Return all swaps with non-terminal status.
    async fn list_active_swaps(&self) -> Result<Vec<BoltzSwap>, BoltzError>;
}

/// Extra persistence required only when preimages are seed-derived.
///
/// `increment_key_index` must be durable: the new index must be persisted
/// before the method returns. This is the sole defense against preimage
/// reuse — if the counter regresses after a crash, a previously-used
/// preimage hash could be sent to Boltz, enabling fund theft.
///
/// In-process concurrency is handled for you — the library serializes issuance,
/// so a single-process embedder needs only durability, not an atomic counter.
/// Atomicity across *separate processes* sharing one store is still the impl's
/// responsibility. Seedless services do not implement this trait.
#[macros::async_trait]
pub trait DerivedKeyStore: BoltzStorage {
    /// Atomically reserve the next key index and return it.
    async fn increment_key_index(&self) -> Result<u32, BoltzError>;
}

/// In-memory store for the crate's own unit tests, gated behind `#[cfg(test)]`
/// so it is compiled only when testing this crate — never in a build where the
/// crate is a dependency, so an embedder can't reach it.
///
/// Its `key_index` lives in a volatile `Mutex<u32>` that resets to 0 on every
/// process restart, which would violate the [`BoltzStorage`] durability
/// invariant above (index regression re-derives used preimages — a fund-theft
/// vector). That is exactly why it must stay test-only.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryBoltzStorage {
    swaps: tokio::sync::Mutex<std::collections::HashMap<String, BoltzSwap>>,
    key_index: tokio::sync::Mutex<u32>,
}

#[cfg(test)]
impl MemoryBoltzStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
#[macros::async_trait]
impl BoltzStorage for MemoryBoltzStorage {
    async fn upsert_swap(&self, swap: &BoltzSwap) -> Result<(), BoltzError> {
        self.swaps
            .lock()
            .await
            .insert(swap.id.clone(), swap.clone());
        Ok(())
    }

    async fn get_swap(&self, id: &str) -> Result<Option<BoltzSwap>, BoltzError> {
        Ok(self.swaps.lock().await.get(id).cloned())
    }

    async fn list_active_swaps(&self) -> Result<Vec<BoltzSwap>, BoltzError> {
        Ok(self
            .swaps
            .lock()
            .await
            .values()
            .filter(|s| !s.status.is_terminal())
            .cloned()
            .collect())
    }
}

#[cfg(test)]
#[macros::async_trait]
impl DerivedKeyStore for MemoryBoltzStorage {
    async fn increment_key_index(&self) -> Result<u32, BoltzError> {
        let mut idx = self.key_index.lock().await;
        let current = *idx;
        *idx = current
            .checked_add(1)
            .ok_or_else(|| BoltzError::Store("Key index overflow".to_string()))?;
        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use super::*;
    use crate::models::{Asset, BoltzSwapStatus, BridgeKind, SwapKeySource};

    fn test_swap(id: &str, status: BoltzSwapStatus) -> BoltzSwap {
        BoltzSwap {
            id: id.to_string(),
            status,
            bridge_kind: BridgeKind::Oft,
            key_source: SwapKeySource::Derived { claim_key_index: 0 },
            chain_id: 42161,
            claim_address: "0xabc".to_string(),
            destination_address: "0xdef".to_string(),
            destination_chain: "Arbitrum One".to_string(),
            asset: Asset::Usdt,
            refund_address: "0x123".to_string(),
            erc20swap_address: "0xswap".to_string(),
            router_address: "0xrouter".to_string(),
            invoice: "lnbc...".to_string(),
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
        }
    }

    #[macros::async_test_all]
    async fn test_insert_and_get() {
        let store = MemoryBoltzStorage::new();
        let swap = test_swap("1", BoltzSwapStatus::Created);
        store.upsert_swap(&swap).await.unwrap();

        let retrieved = store.get_swap("1").await.unwrap().unwrap();
        assert_eq!(retrieved.id, "1");
    }

    #[macros::async_test_all]
    async fn test_upsert_swap() {
        let store = MemoryBoltzStorage::new();
        let mut swap = test_swap("1", BoltzSwapStatus::Created);
        store.upsert_swap(&swap).await.unwrap();

        swap.status = BoltzSwapStatus::TbtcLocked;
        store.upsert_swap(&swap).await.unwrap();

        let retrieved = store.get_swap("1").await.unwrap().unwrap();
        assert_eq!(retrieved.status, BoltzSwapStatus::TbtcLocked);
    }

    #[macros::async_test_all]
    async fn test_upsert_creates_missing_row() {
        // upsert is idempotent: a write for an id never seen before creates it
        // rather than erroring, so crash/retry paths can replay the same row.
        let store = MemoryBoltzStorage::new();
        let swap = test_swap("1", BoltzSwapStatus::Created);
        store.upsert_swap(&swap).await.unwrap();
        assert_eq!(store.get_swap("1").await.unwrap().unwrap().id, "1");
    }

    #[macros::async_test_all]
    async fn test_list_active_swaps() {
        let store = MemoryBoltzStorage::new();
        store
            .upsert_swap(&test_swap("1", BoltzSwapStatus::Created))
            .await
            .unwrap();
        store
            .upsert_swap(&test_swap("2", BoltzSwapStatus::Completed))
            .await
            .unwrap();
        store
            .upsert_swap(&test_swap("3", BoltzSwapStatus::TbtcLocked))
            .await
            .unwrap();

        let active = store.list_active_swaps().await.unwrap();
        assert_eq!(active.len(), 2);
    }

    #[macros::async_test_all]
    async fn test_key_index_management() {
        let store = MemoryBoltzStorage::new();

        let idx0 = store.increment_key_index().await.unwrap();
        assert_eq!(idx0, 0);

        let idx1 = store.increment_key_index().await.unwrap();
        assert_eq!(idx1, 1);
    }

    #[macros::async_test_all]
    async fn test_get_nonexistent_returns_none() {
        let store = MemoryBoltzStorage::new();
        assert!(store.get_swap("nonexistent").await.unwrap().is_none());
    }

    #[macros::async_test_all]
    async fn test_pending_call_id_round_trips() {
        let store = MemoryBoltzStorage::new();
        let mut swap = test_swap("1", BoltzSwapStatus::Claiming);
        assert!(swap.pending_call_id.is_none());
        store.upsert_swap(&swap).await.unwrap();

        // Mid-claim: record the in-flight call_id.
        swap.pending_call_id = Some("call_abc".to_string());
        store.upsert_swap(&swap).await.unwrap();
        assert_eq!(
            store.get_swap("1").await.unwrap().unwrap().pending_call_id,
            Some("call_abc".to_string())
        );

        // Confirmed: tx hash recorded, pending marker cleared.
        swap.claim_tx_hash = Some("0xdeadbeef".to_string());
        swap.pending_call_id = None;
        store.upsert_swap(&swap).await.unwrap();
        let final_swap = store.get_swap("1").await.unwrap().unwrap();
        assert_eq!(final_swap.claim_tx_hash, Some("0xdeadbeef".to_string()));
        assert!(final_swap.pending_call_id.is_none());
    }
}

//! Per-swap serialization primitive.
//!
//! The swap manager runs on a "serialize per swap, parallelize across swaps"
//! model: any task that mutates a swap (the event loop's spawned WS handlers,
//! the delivery poll, and the caller-facing `accept_degraded_quote` /
//! `update_swap_slippage`) holds that swap's lock across its whole
//! `get → mutate → persist` sequence. This is what keeps the whole-record,
//! last-write-wins `BoltzStorage::upsert_swap` safe under concurrency: without
//! it, two writers racing on one swap clobber each other's fields, and two claim
//! paths could submit competing gas-sponsored claim txs for the same swap.
//!
//! Different swaps take different locks and never block each other, so a slow
//! per-swap operation (e.g. a multi-minute receipt poll) only serializes further
//! work on *that* swap.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use platform_utils::tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

/// A keyed async mutex, one logical lock per swap id.
///
/// Entries are created on first use and removed once the last holder releases,
/// so the map stays bounded by the number of swaps with in-flight work rather
/// than the number of swaps ever seen — important for a long-running server.
#[derive(Default)]
pub(crate) struct SwapLocks {
    inner: Arc<Inner>,
}

#[derive(Default)]
struct Inner {
    // std Mutex: only ever held for the brief map lookup/insert/remove below,
    // never across an `.await`, so it cannot block the async runtime.
    map: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl SwapLocks {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Acquire the lock for `swap_id`, waiting if another task holds it. The
    /// returned guard releases on drop and prunes the map entry when it was the
    /// last holder.
    pub(crate) async fn lock(&self, swap_id: &str) -> SwapLockGuard {
        let mutex = {
            let mut map = self.inner.map.lock().expect("swap-lock map poisoned");
            map.entry(swap_id.to_owned())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };
        // `lock_owned` keeps the `Arc<AsyncMutex>` alive inside the guard for as
        // long as the lock is held, so a concurrent `lock()` can still upgrade
        // the same map entry rather than racing a half-removed one.
        let guard = mutex.lock_owned().await;
        SwapLockGuard {
            _guard: guard,
            inner: self.inner.clone(),
            swap_id: swap_id.to_owned(),
        }
    }
}

/// RAII guard holding one swap's lock until dropped.
pub(crate) struct SwapLockGuard {
    _guard: OwnedMutexGuard<()>,
    inner: Arc<Inner>,
    swap_id: String,
}

impl Drop for SwapLockGuard {
    fn drop(&mut self) {
        let mut map = self.inner.map.lock().expect("swap-lock map poisoned");
        if let Some(mutex) = map.get(&self.swap_id) {
            // Field drop runs *after* this `Drop::drop`, so `self._guard` still
            // owns its `Arc<AsyncMutex>` clone here. The only strong refs to a
            // truly idle lock are therefore: the map's own clone + this guard's
            // = 2. A waiting `lock()` call holds a third (its pending
            // `lock_owned` future owns a clone), so a count > 2 means "someone
            // else still needs this entry" and we leave it. Pruning only at
            // count <= 2 keeps the map bounded without ever dropping an entry a
            // waiter is about to lock.
            if Arc::strong_count(mutex) <= 2 {
                map.remove(&self.swap_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "browser-tests")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    /// Two tasks contending on the *same* swap id observe mutual exclusion: the
    /// critical sections never overlap (peak concurrency stays at 1).
    #[macros::async_test_all]
    async fn same_id_is_mutually_exclusive() {
        let locks = Arc::new(SwapLocks::new());
        let active = Arc::new(AtomicU32::new(0));
        let peak = Arc::new(AtomicU32::new(0));

        let run = |locks: Arc<SwapLocks>, active: Arc<AtomicU32>, peak: Arc<AtomicU32>| async move {
            for _ in 0..50 {
                let _guard = locks.lock("swap-a").await;
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                // Yield while holding the lock to give a racing task the chance
                // to interleave if exclusion were broken.
                platform_utils::tokio::task::yield_now().await;
                active.fetch_sub(1, Ordering::SeqCst);
            }
        };

        let a = run(locks.clone(), active.clone(), peak.clone());
        let b = run(locks.clone(), active.clone(), peak.clone());
        platform_utils::tokio::join!(a, b);

        assert_eq!(
            peak.load(Ordering::SeqCst),
            1,
            "critical sections overlapped"
        );
    }

    /// A released, uncontended lock prunes its map entry; a held lock does not.
    #[macros::async_test_all]
    async fn entries_are_pruned_after_release() {
        let locks = SwapLocks::new();

        {
            let _guard = locks.lock("swap-x").await;
            assert_eq!(locks.inner.map.lock().unwrap().len(), 1);
        }
        // Guard dropped → entry pruned.
        assert_eq!(
            locks.inner.map.lock().unwrap().len(),
            0,
            "idle lock entry was not pruned"
        );

        // Distinct ids don't serialize and each cleans up independently.
        {
            let _x = locks.lock("swap-x").await;
            let _y = locks.lock("swap-y").await;
            assert_eq!(locks.inner.map.lock().unwrap().len(), 2);
        }
        assert_eq!(locks.inner.map.lock().unwrap().len(), 0);
    }
}

//! Memo - cached derived values.
//!
//! Memos compute derived values from signals and other memos,
//! caching the result and recomputing only when dependencies change.

use crate::runtime::{SubscriberId, with_runtime};
use parking_lot::RwLock;
use slotmap::new_key_type;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::sync::Arc;

new_key_type! {
    /// Unique identifier for a memo.
    pub struct MemoId;
}

trait DirtyMemo: Send + Sync {
    fn mark_dirty(&self);
}

struct MemoRegistry {
    memos: parking_lot::Mutex<HashMap<SubscriberId, Arc<dyn DirtyMemo>>>,
}

impl MemoRegistry {
    fn new() -> Self {
        Self {
            memos: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    fn register(&self, id: SubscriberId, memo: Arc<dyn DirtyMemo>) {
        self.memos.lock().insert(id, memo);
    }

    fn unregister(&self, id: &SubscriberId) {
        self.memos.lock().remove(id);
    }

    /// Mark the memo dirty and notify its subscribers. Returns true if handled.
    fn mark_dirty(&self, id: SubscriberId) -> bool {
        if let Some(memo) = self.memos.lock().get(&id) {
            memo.mark_dirty();
            return true;
        }
        false
    }
}

thread_local! {
    static MEMO_REGISTRY: MemoRegistry = MemoRegistry::new();
}

/// Mark a memo dirty for the given subscriber id. Returns true if a memo was found.
pub(crate) fn mark_memo_dirty(subscriber: SubscriberId) -> bool {
    MEMO_REGISTRY.with(|reg| reg.mark_dirty(subscriber))
}

/// A memoized computation that caches its result.
pub struct Memo<T: Send + Sync + 'static> {
    inner: Arc<MemoInner<T>>,
}

struct MemoInner<T: Send + Sync + 'static> {
    /// The compute function.
    compute: Box<dyn Fn() -> T + Send + Sync>,
    /// Cached value.
    value: RwLock<Option<T>>,
    /// Whether the memo is dirty (needs recomputation).
    dirty: RwLock<bool>,
    /// Subscribers to notify when value changes.
    subscribers: RwLock<SmallVec<[SubscriberId; 4]>>,
    /// The subscriber ID for this memo (for tracking dependencies).
    subscriber_id: SubscriberId,
}

impl<T: Send + Sync + 'static> DirtyMemo for MemoInner<T> {
    fn mark_dirty(&self) {
        *self.dirty.write() = true;
        // Notify subscribers that depend on this memo.
        let subs = self.subscribers.read();
        with_runtime(|rt| {
            for &sub in subs.iter() {
                rt.queue_effect(sub);
            }
        });
    }
}

impl<T: Send + Sync + 'static> Memo<T> {
    /// Create a new memo with a compute function.
    pub fn new<F>(compute: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        // Get a subscriber ID for this memo
        let subscriber_id = with_runtime(|rt| {
            // We need to create a subscriber ID - for now, use a simple approach
            // In a full implementation, we'd have a proper subscriber registry
            let id = rt.scopes.insert(crate::scope::Scope::new());
            // Convert scope to subscriber ID (this is a simplification)
            unsafe { std::mem::transmute::<_, SubscriberId>(id) }
        });

        let inner = Arc::new(MemoInner {
            compute: Box::new(compute),
            value: RwLock::new(None),
            dirty: RwLock::new(true),
            subscribers: RwLock::new(SmallVec::new()),
            subscriber_id,
        });

        MEMO_REGISTRY.with(|reg| {
            reg.register(subscriber_id, inner.clone());
        });

        Self { inner }
    }

    /// Mark the memo as dirty (needs recomputation).
    pub fn mark_dirty(&self) {
        *self.inner.dirty.write() = true;
        // Notify subscribers
        let subs = self.inner.subscribers.read();
        with_runtime(|rt| {
            for &sub in subs.iter() {
                rt.queue_effect(sub);
            }
        });
    }

    /// Subscribe to this memo.
    pub(crate) fn subscribe(&self, subscriber: SubscriberId) {
        let mut subs = self.inner.subscribers.write();
        if !subs.contains(&subscriber) {
            subs.push(subscriber);
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Memo<T> {
    /// Get the memoized value, recomputing if necessary.
    pub fn get(&self) -> T {
        // Track dependency
        if let Some(subscriber) = with_runtime(|rt| rt.current_subscriber()) {
            self.subscribe(subscriber);
        }

        // Check if we need to recompute
        let is_dirty = *self.inner.dirty.read();
        if is_dirty || self.inner.value.read().is_none() {
            // Recompute with dependency tracking
            let new_value = with_runtime(|rt| {
                let prev = rt.current_subscriber();
                rt.set_current_subscriber(Some(self.inner.subscriber_id));
                let result = (self.inner.compute)();
                rt.set_current_subscriber(prev);
                result
            });

            *self.inner.value.write() = Some(new_value.clone());
            *self.inner.dirty.write() = false;

            new_value
        } else {
            self.inner.value.read().clone().unwrap()
        }
    }

    /// Get the cached value without recomputing or tracking.
    pub fn get_untracked(&self) -> Option<T> {
        self.inner.value.read().clone()
    }
}

impl<T: Send + Sync + 'static> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Create a new memo with a compute function.
pub fn create_memo<T, F>(compute: F) -> Memo<T>
where
    T: Clone + Send + Sync + 'static,
    F: Fn() -> T + Send + Sync + 'static,
{
    Memo::new(compute)
}

impl<T: Send + Sync + 'static> Drop for Memo<T> {
    fn drop(&mut self) {
        MEMO_REGISTRY.with(|reg| {
            reg.unregister(&self.inner.subscriber_id);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::create_signal;

    #[test]
    fn test_memo_basic() {
        let memo = create_memo(|| 42);
        assert_eq!(memo.get(), 42);
    }

    #[test]
    fn test_memo_caching() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let memo = create_memo(move || {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            100
        });

        // First call computes
        assert_eq!(memo.get(), 100);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Second call uses cache
        assert_eq!(memo.get(), 100);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // After marking dirty, recomputes
        memo.mark_dirty();
        assert_eq!(memo.get(), 100);
        assert_eq!(call_count.load(Ordering::SeqCst), 2);
    }
}

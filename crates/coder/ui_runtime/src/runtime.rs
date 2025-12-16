//! Runtime - the global reactive context.
//!
//! The Runtime manages the reactive graph, tracking dependencies
//! between signals, effects, and memos.

use crate::scope::{Scope, ScopeId};
use parking_lot::Mutex;
use slotmap::{SlotMap, new_key_type};
use std::cell::RefCell;
use std::sync::Arc;

new_key_type! {
    /// Unique identifier for a subscriber (effect or memo).
    pub struct SubscriberId;
}

/// The global reactive runtime.
pub struct Runtime {
    /// All active scopes.
    pub(crate) scopes: SlotMap<ScopeId, Scope>,
    /// Currently executing subscriber (for dependency tracking).
    pub(crate) current_subscriber: Option<SubscriberId>,
    /// Pending effects to run.
    pub(crate) pending_effects: Vec<SubscriberId>,
    /// Whether we're currently batching updates.
    pub(crate) batching: bool,
}

impl Runtime {
    /// Create a new runtime.
    pub fn new() -> Self {
        Self {
            scopes: SlotMap::with_key(),
            current_subscriber: None,
            pending_effects: Vec::new(),
            batching: false,
        }
    }

    /// Create a new scope.
    pub fn create_scope(&mut self) -> ScopeId {
        self.scopes.insert(Scope::new())
    }

    /// Dispose of a scope and all its children.
    pub fn dispose_scope(&mut self, scope_id: ScopeId) {
        if let Some(scope) = self.scopes.remove(scope_id) {
            // Dispose children recursively
            for child in scope.children {
                self.dispose_scope(child);
            }
        }
    }

    /// Get the current subscriber (for dependency tracking).
    pub fn current_subscriber(&self) -> Option<SubscriberId> {
        self.current_subscriber
    }

    /// Set the current subscriber.
    pub fn set_current_subscriber(&mut self, subscriber: Option<SubscriberId>) {
        self.current_subscriber = subscriber;
    }

    /// Queue an effect to run.
    pub fn queue_effect(&mut self, effect_id: SubscriberId) {
        if !self.pending_effects.contains(&effect_id) {
            self.pending_effects.push(effect_id);
        }
    }

    /// Flush pending effects.
    pub fn flush_effects(&mut self) -> Vec<SubscriberId> {
        std::mem::take(&mut self.pending_effects)
    }

    /// Start batching updates.
    pub fn batch<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R,
    {
        let was_batching = self.batching;
        self.batching = true;
        let result = f(self);
        self.batching = was_batching;

        // Flush effects after batch completes
        if !was_batching {
            let effects = self.flush_effects();
            // Effects are run by the scheduler
            for effect_id in effects {
                self.queue_effect(effect_id);
            }
        }

        result
    }

    /// Check if we're batching.
    pub fn is_batching(&self) -> bool {
        self.batching
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

// Thread-local runtime for the reactive context
thread_local! {
    static RUNTIME: RefCell<Option<Arc<Mutex<Runtime>>>> = const { RefCell::new(None) };
}

/// Get or create the thread-local runtime.
pub fn with_runtime<F, R>(f: F) -> R
where
    F: FnOnce(&mut Runtime) -> R,
{
    RUNTIME.with(|rt| {
        let mut borrow = rt.borrow_mut();
        if borrow.is_none() {
            *borrow = Some(Arc::new(Mutex::new(Runtime::new())));
        }
        let runtime = borrow.as_ref().unwrap().clone();
        drop(borrow);
        let mut guard = runtime.lock();
        f(&mut guard)
    })
}

/// Initialize the runtime (optional, for explicit setup).
pub fn init_runtime() {
    RUNTIME.with(|rt| {
        let mut borrow = rt.borrow_mut();
        if borrow.is_none() {
            *borrow = Some(Arc::new(Mutex::new(Runtime::new())));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_creation() {
        let runtime = Runtime::new();
        assert!(runtime.current_subscriber.is_none());
        assert!(runtime.pending_effects.is_empty());
    }

    #[test]
    fn test_scope_lifecycle() {
        let mut runtime = Runtime::new();
        let scope_id = runtime.create_scope();
        assert!(runtime.scopes.contains_key(scope_id));

        runtime.dispose_scope(scope_id);
        assert!(!runtime.scopes.contains_key(scope_id));
    }

    #[test]
    fn test_with_runtime() {
        with_runtime(|rt| {
            assert!(rt.current_subscriber.is_none());
        });
    }
}

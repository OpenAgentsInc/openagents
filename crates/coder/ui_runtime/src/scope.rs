//! Scope - manages reactive computation lifecycle.
//!
//! Scopes track effects and cleanups, ensuring proper disposal
//! when a component or computation is destroyed.

use crate::runtime::{with_runtime, SubscriberId};
use slotmap::new_key_type;
use smallvec::SmallVec;

new_key_type! {
    /// Unique identifier for a scope.
    pub struct ScopeId;
}

/// A scope that manages reactive computations.
pub struct Scope {
    /// Parent scope (if nested).
    pub(crate) parent: Option<ScopeId>,
    /// Child scopes.
    pub(crate) children: SmallVec<[ScopeId; 4]>,
    /// Effects owned by this scope.
    pub(crate) effects: SmallVec<[SubscriberId; 8]>,
    /// Cleanup functions to run on dispose.
    pub(crate) cleanups: Vec<Box<dyn FnOnce() + Send>>,
}

impl Scope {
    /// Create a new scope.
    pub fn new() -> Self {
        Self {
            parent: None,
            children: SmallVec::new(),
            effects: SmallVec::new(),
            cleanups: Vec::new(),
        }
    }

    /// Create a child scope.
    pub fn new_child(parent: ScopeId) -> Self {
        Self {
            parent: Some(parent),
            children: SmallVec::new(),
            effects: SmallVec::new(),
            cleanups: Vec::new(),
        }
    }

    /// Add a cleanup function.
    pub fn on_cleanup<F>(&mut self, cleanup: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.cleanups.push(Box::new(cleanup));
    }

    /// Run all cleanup functions.
    pub fn cleanup(&mut self) {
        for cleanup in self.cleanups.drain(..) {
            cleanup();
        }
    }

    /// Add an effect to this scope.
    pub fn add_effect(&mut self, effect_id: SubscriberId) {
        self.effects.push(effect_id);
    }

    /// Add a child scope.
    pub fn add_child(&mut self, child_id: ScopeId) {
        self.children.push(child_id);
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a new root scope.
pub fn create_scope() -> ScopeId {
    with_runtime(|rt| rt.create_scope())
}

/// Create a child scope.
pub fn create_child_scope(parent: ScopeId) -> ScopeId {
    with_runtime(|rt| {
        let child_id = rt.scopes.insert(Scope::new_child(parent));
        if let Some(parent_scope) = rt.scopes.get_mut(parent) {
            parent_scope.add_child(child_id);
        }
        child_id
    })
}

/// Dispose of a scope.
pub fn dispose_scope(scope_id: ScopeId) {
    with_runtime(|rt| {
        // Run cleanup functions
        if let Some(scope) = rt.scopes.get_mut(scope_id) {
            scope.cleanup();
        }
        rt.dispose_scope(scope_id);
    });
}

/// Register a cleanup function for the current scope.
pub fn on_cleanup<F>(scope_id: ScopeId, cleanup: F)
where
    F: FnOnce() + Send + 'static,
{
    with_runtime(|rt| {
        if let Some(scope) = rt.scopes.get_mut(scope_id) {
            scope.on_cleanup(cleanup);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_scope_creation() {
        let scope = Scope::new();
        assert!(scope.parent.is_none());
        assert!(scope.children.is_empty());
    }

    #[test]
    fn test_scope_cleanup() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let mut scope = Scope::new();
        scope.on_cleanup(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

        assert!(!called.load(Ordering::SeqCst));
        scope.cleanup();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_create_scope() {
        let scope_id = create_scope();
        with_runtime(|rt| {
            assert!(rt.scopes.contains_key(scope_id));
        });

        dispose_scope(scope_id);
        with_runtime(|rt| {
            assert!(!rt.scopes.contains_key(scope_id));
        });
    }
}

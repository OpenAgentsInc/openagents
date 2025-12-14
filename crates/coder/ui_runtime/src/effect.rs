//! Effect - reactive side effects.
//!
//! Effects run automatically when their dependencies change,
//! enabling side effects like logging, network requests, or DOM updates.

use crate::runtime::{with_runtime, SubscriberId};
use crate::scope::ScopeId;
use parking_lot::Mutex;
use std::sync::Arc;

/// A handle to a running effect that can be used to stop it.
pub struct EffectHandle {
    pub(crate) id: SubscriberId,
    stopped: Arc<Mutex<bool>>,
}

impl EffectHandle {
    /// Stop this effect from running.
    pub fn stop(&self) {
        *self.stopped.lock() = true;
    }

    /// Check if this effect is stopped.
    pub fn is_stopped(&self) -> bool {
        *self.stopped.lock()
    }
}

/// An effect that re-runs when its dependencies change.
pub struct Effect {
    /// The effect function.
    func: Box<dyn FnMut() + Send>,
    /// Whether the effect is stopped.
    stopped: Arc<Mutex<bool>>,
    /// The subscriber ID for dependency tracking.
    pub(crate) subscriber_id: SubscriberId,
    /// The scope this effect belongs to.
    pub(crate) scope_id: Option<ScopeId>,
}

impl Effect {
    /// Create a new effect.
    pub fn new<F>(func: F) -> (Self, EffectHandle)
    where
        F: FnMut() + Send + 'static,
    {
        let stopped = Arc::new(Mutex::new(false));

        // Get a subscriber ID for this effect
        let subscriber_id = with_runtime(|rt| {
            let id = rt.scopes.insert(crate::scope::Scope::new());
            unsafe { std::mem::transmute::<_, SubscriberId>(id) }
        });

        let effect = Self {
            func: Box::new(func),
            stopped: stopped.clone(),
            subscriber_id,
            scope_id: None,
        };

        let handle = EffectHandle {
            id: subscriber_id,
            stopped,
        };

        (effect, handle)
    }

    /// Run the effect.
    pub fn run(&mut self) {
        if *self.stopped.lock() {
            return;
        }

        // Set current subscriber for dependency tracking
        with_runtime(|rt| {
            rt.set_current_subscriber(Some(self.subscriber_id));
        });

        // Run the effect
        (self.func)();

        // Clear current subscriber
        with_runtime(|rt| {
            rt.set_current_subscriber(None);
        });
    }

    /// Check if this effect is stopped.
    pub fn is_stopped(&self) -> bool {
        *self.stopped.lock()
    }
}

/// Storage for active effects.
pub struct EffectRegistry {
    effects: Mutex<Vec<Effect>>,
}

impl EffectRegistry {
    /// Create a new effect registry.
    pub fn new() -> Self {
        Self {
            effects: Mutex::new(Vec::new()),
        }
    }

    /// Register an effect.
    pub fn register(&self, effect: Effect) {
        self.effects.lock().push(effect);
    }

    /// Run all pending effects for the given subscriber IDs.
    pub fn run_effects(&self, ids: &[SubscriberId]) {
        let mut effects = self.effects.lock();
        for effect in effects.iter_mut() {
            if ids.contains(&effect.subscriber_id) && !effect.is_stopped() {
                effect.run();
            }
        }
    }

    /// Remove stopped effects.
    pub fn cleanup(&self) {
        let mut effects = self.effects.lock();
        effects.retain(|e| !e.is_stopped());
    }
}

impl Default for EffectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Global effect registry
thread_local! {
    static EFFECT_REGISTRY: EffectRegistry = const { EffectRegistry { effects: Mutex::new(Vec::new()) } };
}

/// Create and run an effect.
pub fn create_effect<F>(mut func: F) -> EffectHandle
where
    F: FnMut() + Send + 'static,
{
    let (mut effect, handle) = Effect::new(move || func());

    // Run the effect immediately to establish dependencies
    effect.run();

    // Register for future runs
    EFFECT_REGISTRY.with(|registry| {
        registry.register(effect);
    });

    handle
}

/// Run all pending effects.
pub fn flush_effects() {
    let pending = with_runtime(|rt| rt.flush_effects());
    if !pending.is_empty() {
        EFFECT_REGISTRY.with(|registry| {
            registry.run_effects(&pending);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::create_signal;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_effect_runs_immediately() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let _handle = create_effect(move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_effect_handle_stop() {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();

        let (mut effect, handle) = Effect::new(move || {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        effect.run();
        assert_eq!(count.load(Ordering::SeqCst), 1);

        handle.stop();

        effect.run();
        assert_eq!(count.load(Ordering::SeqCst), 1); // Still 1, effect didn't run
    }
}

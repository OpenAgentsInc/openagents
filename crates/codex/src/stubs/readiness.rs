//! Stub readiness flag implementation
//!
//! This is a simplified stub for readiness tracking.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A readiness flag that can be checked
#[derive(Clone, Default)]
pub struct Readiness {
    ready: Arc<AtomicBool>,
}

impl Readiness {
    /// Create a new readiness flag
    pub fn new() -> Self {
        Self {
            ready: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if ready
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Mark as ready
    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    /// Create a flag for this readiness
    pub fn flag(&self) -> ReadinessFlag {
        ReadinessFlag {
            ready: self.ready.clone(),
        }
    }

    /// Create a token that marks ready when dropped
    pub fn token(&self) -> Token {
        Token {
            ready: Some(self.ready.clone()),
        }
    }
}

/// A flag that can be used to mark readiness
#[derive(Clone)]
pub struct ReadinessFlag {
    ready: Arc<AtomicBool>,
}

impl ReadinessFlag {
    /// Check if ready
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    /// Mark as ready
    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }
}

/// A token that marks the readiness as ready when dropped
pub struct Token {
    ready: Option<Arc<AtomicBool>>,
}

impl Token {
    /// Defuse the token so it doesn't mark ready on drop
    pub fn defuse(&mut self) {
        self.ready = None;
    }
}

impl Drop for Token {
    fn drop(&mut self) {
        if let Some(ready) = &self.ready {
            ready.store(true, Ordering::SeqCst);
        }
    }
}

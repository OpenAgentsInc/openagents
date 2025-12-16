//! Stub readiness flag implementation
//!
//! This is a simplified stub for readiness tracking.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
#[derive(Clone, Debug)]
pub struct ReadinessFlag {
    ready: Arc<AtomicBool>,
}

impl ReadinessFlag {
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

    /// Subscribe to readiness changes - returns a future that resolves to a Token when ready
    pub async fn subscribe(&self) -> Result<Token, std::io::Error> {
        let ready = self.ready.clone();
        // Poll until ready
        loop {
            if ready.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        Ok(Token {
            ready: Some(self.ready.clone()),
        })
    }

    /// Wait until ready (alias for subscribe)
    pub async fn wait_ready(&self) {
        loop {
            if self.ready.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    /// Mark as ready (alias) - async version that takes a token and returns Result
    pub async fn mark_ready(&self, _token: Token) -> Result<bool, std::io::Error> {
        let was_ready = self.ready.load(Ordering::SeqCst);
        self.ready.store(true, Ordering::SeqCst);
        Ok(!was_ready)
    }
}

impl Default for ReadinessFlag {
    fn default() -> Self {
        Self::new()
    }
}

/// A token that marks the readiness as ready when dropped
#[derive(Clone)]
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

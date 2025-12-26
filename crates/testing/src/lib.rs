//! Shared Testing Utilities
//!
//! This crate provides common testing infrastructure used across OpenAgents test suites.
//! It eliminates duplication and provides consistent patterns for:
//!
//! - TestApp for integration testing
//! - Mock implementations of common traits
//! - Test fixtures and factories
//! - Assertion helpers
//! - Mock Nostr relay for protocol testing

pub mod fixtures;
pub mod faucet;
pub mod mocks;
pub mod mock_relay;
pub mod test_app;

// Re-export commonly used testing utilities
pub use fixtures::*;
pub use faucet::*;
pub use mocks::*;
pub use mock_relay::*;
pub use test_app::*;

#[cfg(test)]
mod tests {
    use super::fixtures;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::sync::Barrier;

    #[test]
    fn test_unit_test_smoke() {
        let event_id = fixtures::test_event_id();
        assert!(!event_id.is_empty());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_parallel_tasks_in_unit_tests() {
        let counter = Arc::new(AtomicUsize::new(0));
        let barrier = Arc::new(Barrier::new(3));

        let mut handles = Vec::new();
        for _ in 0..2 {
            let counter = Arc::clone(&counter);
            let barrier = Arc::clone(&barrier);
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                counter.fetch_add(1, Ordering::SeqCst);
            }));
        }

        barrier.wait().await;
        for handle in handles {
            handle.await.expect("join task");
        }

        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}

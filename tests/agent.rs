// Register test modules
mod agent {
    mod manager;
    mod nostr;
}

// Re-export tests
pub use agent::*;
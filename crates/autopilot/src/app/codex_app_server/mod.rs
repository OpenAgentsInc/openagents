//! Codex app-server client.
//!
//! Re-exports from the codex-client crate.

// Re-export everything from codex-client
#[allow(unused_imports)]
pub(crate) use codex_client::*;

// Re-export types module for compatibility
pub(crate) mod types {
    #[allow(unused_imports)]
    pub use codex_client::*;
}

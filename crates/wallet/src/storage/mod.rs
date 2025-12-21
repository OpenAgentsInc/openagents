//! Wallet storage layer
//!
//! Handles secure key storage and persistent data

pub mod keychain;
pub mod config;

pub use config::WalletConfig;
pub use keychain::SecureKeychain;

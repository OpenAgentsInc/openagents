//! Spark Bitcoin payment integration for OpenAgents
//!
//! This crate provides Bitcoin payment capabilities through the Spark SDK,
//! with BIP39 mnemonic-based key derivation that shares the same seed phrase
//! with Nostr (NIP-06) for unified identity management.
//!
//! # Architecture
//!
//! ```text
//!                     BIP39 Mnemonic (12/24 words)
//!                               |
//!         +---------------------+---------------------+
//!         |                                           |
//!    m/44'/1237'/0'/0/0                        m/44'/0'/0'/0/0
//!    (NIP-06 Nostr)                            (BIP44 Bitcoin)
//!         |                                           |
//!    Nostr Keypair                             Spark Signer
//!    (crates/nostr/core)                       (crates/spark)
//!         |                                           |
//!         +---------------------+---------------------+
//!                               |
//!                         UnifiedIdentity
//!                      (crates/compute/domain)
//! ```
//!
//! # Status
//!
//! **Phase 1: Core Integration (COMPLETED)**
//! - ✅ SparkSigner with BIP44 key derivation
//! - ✅ Integration into UnifiedIdentity
//! - ✅ Basic wallet types
//! - ✅ Breez SDK initialization with BreezSdk::connect()
//!
//! **Phase 2: Wallet Operations (IN PROGRESS)**
//! - ⏸️ Balance queries (pending)
//! - ⏸️ Wallet info (pending)
//! - ⏸️ Sync operations (pending)
//!
//! **Phase 3+: Payment Methods, Tokens, Multi-Network (PLANNED)**
//!
//! # Example
//!
//! ```rust
//! use openagents_spark::SparkSigner;
//!
//! // Create a signer from a BIP39 mnemonic
//! let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
//! let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("valid mnemonic");
//!
//! // Get the public key
//! let pubkey = signer.public_key_hex();
//! println!("Public key: {}", pubkey);
//! ```
//!
//! ```rust,ignore
//! use spark::{SparkSigner, SparkWallet, WalletConfig, Network};
//!
//! // Create a wallet (async)
//! let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("valid mnemonic");
//! let config = WalletConfig {
//!     network: Network::Testnet,
//!     ..Default::default()
//! };
//! let wallet = SparkWallet::new(signer, config).await?;
//!
//! // Get balance (currently stub - returns zero)
//! let balance = wallet.get_balance().await?;
//! println!("Total: {} sats", balance.total_sats());
//! ```

pub mod error;
pub mod signer;
pub mod wallet;

pub use error::SparkError;
pub use signer::SparkSigner;
pub use wallet::{Balance, Network, SparkWallet, WalletConfig, WalletInfo};

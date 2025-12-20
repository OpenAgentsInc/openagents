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
//! ```
//!
//! # Example
//!
//! ```rust
//! use spark::SparkSigner;
//!
//! // Create a signer from a BIP39 mnemonic
//! let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
//! let signer = SparkSigner::from_mnemonic(mnemonic, "").expect("valid mnemonic");
//!
//! // Get the public key
//! let pubkey = signer.public_key_hex();
//! println!("Public key: {}", pubkey);
//! ```

pub mod error;
pub mod signer;

pub use error::SparkError;
pub use signer::SparkSigner;

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
//! use openagents_spark::{SparkSigner, SparkWallet, WalletConfig, Network};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a wallet (async)
//! let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
//! let signer = SparkSigner::from_mnemonic(mnemonic, "")?;
//! let config = WalletConfig {
//!     network: Network::Testnet,
//!     ..Default::default()
//! };
//! let wallet = SparkWallet::new(signer, config).await?;
//!
//! // Send a payment
//! let response = wallet.send_payment_simple("lnbc1...", None).await?;
//! println!("Payment sent: {}", response.payment.id);
//!
//! // Create an invoice to receive payment
//! let invoice = wallet.create_invoice(1000, Some("Coffee".to_string()), None).await?;
//! println!("Invoice: {}", invoice.payment_request);
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod signer;
pub mod wallet;

pub use error::SparkError;
pub use signer::SparkSigner;
pub use wallet::{
    parse_input,
    Balance, Config, ExternalInputParser, InputType, KeySetType, Network,
    NetworkStatus, NetworkStatusReport, Payment, PaymentDetails, PaymentMethod,
    PaymentStatus, PaymentType, SendPaymentOptions, SparkHtlcDetails, SparkHtlcOptions,
    SparkHtlcStatus, SparkWallet, SparkWalletBuilder, WalletConfig, WalletInfo,
};
pub use breez_sdk_spark::{EventListener, SdkEvent};

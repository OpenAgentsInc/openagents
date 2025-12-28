//! Neobank - Multi-currency treasury for autonomous agents
//!
//! This crate provides Cashu eCash wallet functionality for agents,
//! supporting both BTC and USD denominated tokens from different mints.
//!
//! # Overview
//!
//! Neobank enables agents to:
//! - Hold BTC and USD eCash proofs
//! - Deposit via Lightning (mint quotes)
//! - Pay Lightning invoices (melt operations)
//! - Track balances per currency/mint
//!
//! # Example
//!
//! ```ignore
//! use neobank::{CashuWallet, Currency, MintConfig};
//! use std::path::Path;
//!
//! // Create a wallet connected to a BTC mint
//! let mint = MintConfig::default_btc_mint();
//! let seed = [0u8; 32]; // Use proper seed in production
//! let db_path = Path::new("wallet.db");
//!
//! let wallet = CashuWallet::new(
//!     mint.url,
//!     Currency::Btc,
//!     &seed,
//!     db_path,
//! ).await?;
//!
//! // Check balance
//! let balance = wallet.balance().await?;
//! println!("Balance: {}", balance);
//!
//! // Create deposit invoice
//! let quote = wallet.create_mint_quote(10000).await?;
//! println!("Pay this invoice: {}", quote.bolt11);
//! ```

pub mod error;
pub mod exchange;
pub mod mint_config;
pub mod types;
pub mod wallet;

// Re-exports for convenient access
pub use error::{Error, Result};
pub use exchange::{
    ExchangeClient, Order, OrderParams, OrderSide, OrderStatus, SettlementMethod,
    SettlementMode, SettlementReceipt, Trade, TradeAttestation, TradeOutcome, TradeStatus,
};
pub use mint_config::{KnownMints, MintConfig};
pub use types::{Amount, AssetId, Currency};
pub use wallet::{CashuWallet, MeltQuote, MeltResult, MintQuote, QuoteState};

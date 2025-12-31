//! Wallet CLI error types with user-friendly messages and recovery hints.
//!
//! This module provides structured error types for wallet operations that include:
//! - Clear, actionable error messages
//! - Recovery suggestions to help users resolve issues
//! - Categorization of error types for better handling

#![allow(dead_code)]

use thiserror::Error;

/// Wallet CLI error types with user-friendly messages.
#[derive(Debug, Error)]
pub enum WalletError {
    /// Wallet has not been initialized yet.
    #[error("No wallet found")]
    WalletNotInitialized,

    /// Wallet already exists and would be overwritten.
    #[error("Wallet already exists")]
    WalletAlreadyExists,

    /// Insufficient balance for the requested operation.
    #[error("Insufficient balance: need {required} sats but only have {available} sats")]
    InsufficientBalance { required: u64, available: u64 },

    /// Payment amount is zero or negative.
    #[error("Invalid amount: {0}")]
    InvalidAmount(String),

    /// Payment amount exceeds configured limit.
    #[error("Amount {amount} sats exceeds configured limit of {limit} sats")]
    AmountExceedsLimit { amount: u64, limit: u64 },

    /// Invalid Lightning invoice format.
    #[error("Invalid Lightning invoice: {0}")]
    InvalidLightningInvoice(String),

    /// Invalid Spark address format.
    #[error("Invalid Spark address: {0}")]
    InvalidSparkAddress(String),

    /// Invalid Bitcoin address format.
    #[error("Invalid Bitcoin address: {0}")]
    InvalidBitcoinAddress(String),

    /// Invalid LNURL format.
    #[error("Invalid LNURL: {0}")]
    InvalidLnurl(String),

    /// Invalid payment destination (could not determine type).
    #[error("Invalid payment destination: {0}")]
    InvalidDestination(String),

    /// Network connection failed or timed out.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Spark network is not connected.
    #[error("Not connected to Spark network")]
    SparkNotConnected,

    /// Payment failed on the network.
    #[error("Payment failed: {0}")]
    PaymentFailed(String),

    /// Invoice has expired.
    #[error("Invoice expired")]
    InvoiceExpired,

    /// No route found for Lightning payment.
    #[error("No route found to destination")]
    NoRouteFound,

    /// Payee not found in address book.
    #[error("Payee '{0}' not found")]
    PayeeNotFound(String),

    /// Payee already exists in address book.
    #[error("Payee '{0}' already exists")]
    PayeeAlreadyExists(String),

    /// Identity not found.
    #[error("Identity '{0}' not found")]
    IdentityNotFound(String),

    /// Identity already exists.
    #[error("Identity '{0}' already exists")]
    IdentityAlreadyExists(String),

    /// Invalid identity name format.
    #[error("Invalid identity name: {0}")]
    InvalidIdentityName(String),

    /// Invalid mnemonic phrase.
    #[error("Invalid mnemonic phrase: {0}")]
    InvalidMnemonic(String),

    /// Wallet is password protected but no password provided.
    #[error("Wallet is password protected")]
    PasswordRequired,

    /// Wrong password provided.
    #[error("Wrong password")]
    WrongPassword,

    /// Non-interactive operation requires confirmation.
    #[error("Non-interactive mode requires --yes flag to confirm")]
    ConfirmationRequired,

    /// No relays configured.
    #[error("No Nostr relays configured")]
    NoRelaysConfigured,

    /// Invalid npub or nprofile format.
    #[error("Invalid Nostr identifier: {0}")]
    InvalidNostrId(String),

    /// QR code decoding failed.
    #[error("Failed to decode QR code: {0}")]
    QrDecodeFailed(String),

    /// File operation failed.
    #[error("File error: {0}")]
    FileError(String),

    /// Generic error with message.
    #[error("{0}")]
    Other(String),
}

impl WalletError {
    /// Returns a user-friendly recovery suggestion for this error.
    pub fn recovery_hint(&self) -> Option<&'static str> {
        match self {
            Self::WalletNotInitialized => Some(
                "Run 'openagents wallet init' to create a new wallet, or 'openagents wallet import' to restore from mnemonic.",
            ),
            Self::WalletAlreadyExists => Some(
                "Use 'openagents wallet export' to backup your current wallet, then 'openagents wallet import' to replace it.",
            ),
            Self::InsufficientBalance { .. } => Some(
                "Run 'openagents wallet balance' to check your funds. Use 'openagents wallet receive' to get a payment address.",
            ),
            Self::InvalidAmount(_) => {
                Some("Amount must be a positive number of satoshis (e.g., 1000 for 1000 sats).")
            }
            Self::AmountExceedsLimit { .. } => Some(
                "Update your limit with 'openagents wallet settings set security.max_send_sats <amount>'.",
            ),
            Self::InvalidLightningInvoice(_) => Some(
                "Lightning invoices start with 'lnbc' (mainnet), 'lntb' (testnet), or 'lnbcrt' (regtest).",
            ),
            Self::InvalidSparkAddress(_) => Some(
                "Spark addresses start with 'sp1' or 'sprt1'. Check that you copied the full address.",
            ),
            Self::InvalidBitcoinAddress(_) => Some(
                "Bitcoin addresses start with 'bc1' (mainnet), 'tb1' (testnet), or 'bcrt1' (regtest).",
            ),
            Self::InvalidLnurl(_) => Some(
                "LNURL should be a bech32-encoded URL starting with 'lnurl' or a Lightning address (user@domain.com).",
            ),
            Self::InvalidDestination(_) => Some(
                "Supported formats: Lightning invoice (lnbc...), Spark address (sp1...), Bitcoin address (bc1...), or LNURL.",
            ),
            Self::NetworkError(_) => Some(
                "Check your internet connection. Run 'openagents wallet status' to verify Spark network connectivity.",
            ),
            Self::SparkNotConnected => Some(
                "Run 'openagents wallet status' to check connection. Ensure BREEZ_API_KEY is set for mainnet.",
            ),
            Self::PaymentFailed(_) => Some(
                "Use 'openagents wallet retry --last' to retry the most recent failed payment.",
            ),
            Self::InvoiceExpired => Some(
                "Request a new invoice from the recipient. Invoices typically expire after 1 hour.",
            ),
            Self::NoRouteFound => Some(
                "The payment network couldn't find a path. Try a smaller amount or contact the recipient.",
            ),
            Self::PayeeNotFound(_) => Some(
                "Use 'openagents wallet payee list' to see saved payees, or 'openagents wallet payee add <name> <address>' to add one.",
            ),
            Self::PayeeAlreadyExists(_) => {
                Some("Use 'openagents wallet payee remove <name>' first, then add the new address.")
            }
            Self::IdentityNotFound(_) => {
                Some("Use 'openagents wallet identity list' to see available identities.")
            }
            Self::IdentityAlreadyExists(_) => Some(
                "Choose a different name or remove the existing identity with 'openagents wallet identity remove <name>'.",
            ),
            Self::InvalidIdentityName(_) => Some(
                "Identity names can only contain letters, numbers, hyphens (-), and underscores (_).",
            ),
            Self::InvalidMnemonic(_) => Some(
                "Recovery phrases are 12 or 24 words separated by spaces. Check spelling and word order.",
            ),
            Self::PasswordRequired => Some(
                "Set OPENAGENTS_WALLET_PASSWORD environment variable or run in a terminal to enter password interactively.",
            ),
            Self::WrongPassword => Some(
                "Try again with the correct password. If forgotten, you'll need your recovery phrase to restore the wallet.",
            ),
            Self::ConfirmationRequired => {
                Some("Add --yes flag to confirm the operation in non-interactive mode.")
            }
            Self::NoRelaysConfigured => Some(
                "Add relays with 'openagents wallet settings set nostr.relays [\"wss://relay.damus.io\"]'.",
            ),
            Self::InvalidNostrId(_) => {
                Some("Expected an npub (npub1...) or nprofile (nprofile1...) identifier.")
            }
            Self::QrDecodeFailed(_) => Some(
                "Ensure the image file exists and contains a valid QR code. Supported formats: PNG, JPEG, GIF.",
            ),
            Self::FileError(_) => {
                Some("Check that the file exists and you have read/write permissions.")
            }
            Self::Other(_) => None,
        }
    }

    /// Returns true if the user's balance was not affected by this error.
    ///
    /// This is useful for payment errors where users want to know if they lost money.
    pub fn balance_unaffected(&self) -> bool {
        matches!(
            self,
            Self::InsufficientBalance { .. }
                | Self::InvalidAmount(_)
                | Self::AmountExceedsLimit { .. }
                | Self::InvalidLightningInvoice(_)
                | Self::InvalidSparkAddress(_)
                | Self::InvalidBitcoinAddress(_)
                | Self::InvalidDestination(_)
                | Self::NetworkError(_)
                | Self::SparkNotConnected
                | Self::InvoiceExpired
                | Self::NoRouteFound
                | Self::PayeeNotFound(_)
                | Self::ConfirmationRequired
        )
    }
}

/// Extension trait for adding recovery hints to Results.
pub trait ResultExt<T> {
    /// Adds context and a recovery hint to the error.
    fn with_recovery_hint(self, hint: &'static str) -> anyhow::Result<T>;
}

impl<T, E: std::error::Error + Send + Sync + 'static> ResultExt<T> for Result<T, E> {
    fn with_recovery_hint(self, hint: &'static str) -> anyhow::Result<T> {
        self.map_err(|e| anyhow::anyhow!("{}\n\nHint: {}", e, hint))
    }
}

/// Format an error with its recovery hint for display.
pub fn format_error_with_hint(error: &WalletError) -> String {
    let mut output = format!("Error: {}", error);
    if let Some(hint) = error.recovery_hint() {
        output.push_str("\n\nHint: ");
        output.push_str(hint);
    }
    output
}

/// Print an error with its recovery hint to stderr.
pub fn print_error_with_hint(error: &WalletError) {
    eprintln!("{}", format_error_with_hint(error));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_not_initialized_has_hint() {
        let error = WalletError::WalletNotInitialized;
        assert!(error.recovery_hint().is_some());
        assert!(error.recovery_hint().unwrap().contains("wallet init"));
    }

    #[test]
    fn test_insufficient_balance_has_hint() {
        let error = WalletError::InsufficientBalance {
            required: 1000,
            available: 500,
        };
        assert!(error.to_string().contains("1000"));
        assert!(error.to_string().contains("500"));
        assert!(error.recovery_hint().unwrap().contains("balance"));
    }

    #[test]
    fn test_amount_exceeds_limit_has_hint() {
        let error = WalletError::AmountExceedsLimit {
            amount: 10000,
            limit: 5000,
        };
        assert!(error.to_string().contains("10000"));
        assert!(error.to_string().contains("5000"));
        assert!(error.recovery_hint().unwrap().contains("max_send_sats"));
    }

    #[test]
    fn test_balance_unaffected_for_validation_errors() {
        assert!(
            WalletError::InsufficientBalance {
                required: 100,
                available: 50
            }
            .balance_unaffected()
        );
        assert!(WalletError::InvalidAmount("negative".to_string()).balance_unaffected());
        assert!(WalletError::InvalidDestination("bad".to_string()).balance_unaffected());
        assert!(WalletError::InvoiceExpired.balance_unaffected());
    }

    #[test]
    fn test_format_error_with_hint() {
        let error = WalletError::PayeeNotFound("alice".to_string());
        let formatted = format_error_with_hint(&error);
        assert!(formatted.contains("alice"));
        assert!(formatted.contains("Hint:"));
        assert!(formatted.contains("payee list"));
    }

    #[test]
    fn test_invalid_identity_name_hint() {
        let error = WalletError::InvalidIdentityName("has spaces".to_string());
        assert!(error.recovery_hint().unwrap().contains("letters"));
    }

    #[test]
    fn test_network_error_hint() {
        let error = WalletError::NetworkError("timeout".to_string());
        assert!(error.recovery_hint().unwrap().contains("wallet status"));
    }
}

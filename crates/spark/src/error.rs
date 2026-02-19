//! Error types for Spark integration

use thiserror::Error;

/// Errors that can occur during Spark operations
#[derive(Debug, Error)]
pub enum SparkError {
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),

    #[error("key derivation failed: {0}")]
    KeyDerivation(String),

    #[error("initialization failed: {0}")]
    InitializationFailed(String),

    #[error("wallet error: {0}")]
    Wallet(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("invalid address: {0}")]
    InvalidAddress(String),

    #[error("insufficient funds: you need {required} sats but only have {available} sats")]
    InsufficientFunds { required: u64, available: u64 },

    #[error("payment failed: {0}")]
    PaymentFailed(String),

    #[error("no route found to recipient - they may be offline or unreachable")]
    PaymentRouteNotFound,

    #[error("payment timed out - the recipient did not respond in time")]
    PaymentTimeout,

    #[error("invalid invoice: {0}")]
    InvalidInvoice(String),

    #[error("invoice has expired - request a new one from the recipient")]
    InvoiceExpired,

    #[error("balance query failed: {0}")]
    BalanceQueryFailed(String),

    #[error("failed to get address: {0}")]
    GetAddressFailed(String),
}

impl SparkError {
    /// Get a user-friendly error message explaining what went wrong
    /// and what the user can do about it.
    pub fn user_friendly_message(&self) -> String {
        match self {
            SparkError::InvalidMnemonic(_) => {
                "Your recovery phrase is invalid. Please check that you entered all 12 or 24 words correctly.".to_string()
            }
            SparkError::KeyDerivation(_) => {
                "Failed to derive keys from your recovery phrase. This may indicate a corrupted wallet.".to_string()
            }
            SparkError::InitializationFailed(e) => {
                format!("Failed to connect to the payment network. Please check your internet connection.\n\nTechnical details: {}", e)
            }
            SparkError::Wallet(e) => {
                format!("A wallet error occurred: {}", e)
            }
            SparkError::Network(_) => {
                "Network error. Please check your internet connection and try again.".to_string()
            }
            SparkError::InvalidAddress(_) => {
                "The payment address or invoice is invalid. Please check it and try again.".to_string()
            }
            SparkError::InsufficientFunds { required, available } => {
                format!(
                    "You don't have enough funds for this payment.\n\
                     Required: {} sats\n\
                     Available: {} sats\n\
                     Shortfall: {} sats\n\n\
                     Your balance was NOT deducted. Add more funds and try again.",
                    required, available, required.saturating_sub(*available)
                )
            }
            SparkError::PaymentFailed(e) => {
                format!(
                    "Payment failed. Your balance was NOT deducted.\n\n\
                     Reason: {}\n\n\
                     You can safely retry this payment.",
                    e
                )
            }
            SparkError::PaymentRouteNotFound => {
                "Could not find a route to the recipient. This can happen if:\n\
                 • The recipient is offline\n\
                 • Their node has insufficient inbound capacity\n\
                 • Network congestion is too high\n\n\
                 Your balance was NOT deducted. Try again later or ask the recipient for a new invoice.".to_string()
            }
            SparkError::PaymentTimeout => {
                "The payment timed out before completion.\n\n\
                 Your balance was NOT deducted. The recipient may be offline. Try again later.".to_string()
            }
            SparkError::InvalidInvoice(e) => {
                format!(
                    "The invoice is invalid or malformed.\n\n\
                     Details: {}\n\n\
                     Please request a new invoice from the recipient.",
                    e
                )
            }
            SparkError::InvoiceExpired => {
                "This invoice has expired. Request a new invoice from the recipient.".to_string()
            }
            SparkError::BalanceQueryFailed(_) => {
                "Failed to query your balance. Please check your internet connection and try again.".to_string()
            }
            SparkError::GetAddressFailed(_) => {
                "Failed to generate a receive address. Please try again.".to_string()
            }
        }
    }

    /// Returns true if this error indicates the user's balance was NOT affected
    pub fn balance_unaffected(&self) -> bool {
        matches!(
            self,
            SparkError::PaymentFailed(_)
                | SparkError::PaymentRouteNotFound
                | SparkError::PaymentTimeout
                | SparkError::InsufficientFunds { .. }
                | SparkError::InvalidInvoice(_)
                | SparkError::InvoiceExpired
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_friendly_messages_exist() {
        // Ensure all error variants have non-empty user-friendly messages
        let errors = vec![
            SparkError::InvalidMnemonic("test".to_string()),
            SparkError::KeyDerivation("test".to_string()),
            SparkError::InitializationFailed("test".to_string()),
            SparkError::Wallet("test".to_string()),
            SparkError::Network("test".to_string()),
            SparkError::InvalidAddress("test".to_string()),
            SparkError::InsufficientFunds {
                required: 1000,
                available: 500,
            },
            SparkError::PaymentFailed("test".to_string()),
            SparkError::PaymentRouteNotFound,
            SparkError::PaymentTimeout,
            SparkError::InvalidInvoice("test".to_string()),
            SparkError::InvoiceExpired,
            SparkError::BalanceQueryFailed("test".to_string()),
            SparkError::GetAddressFailed("test".to_string()),
        ];

        for error in errors {
            let msg = error.user_friendly_message();
            assert!(
                !msg.is_empty(),
                "Error {:?} should have a user-friendly message",
                error
            );
        }
    }

    #[test]
    fn test_balance_unaffected_for_payment_errors() {
        // Payment failures should not affect balance
        assert!(SparkError::PaymentFailed("test".to_string()).balance_unaffected());
        assert!(SparkError::PaymentRouteNotFound.balance_unaffected());
        assert!(SparkError::PaymentTimeout.balance_unaffected());
        assert!(
            SparkError::InsufficientFunds {
                required: 1000,
                available: 500
            }
            .balance_unaffected()
        );
        assert!(SparkError::InvalidInvoice("test".to_string()).balance_unaffected());
        assert!(SparkError::InvoiceExpired.balance_unaffected());
    }

    #[test]
    fn test_balance_affected_for_other_errors() {
        // These errors don't affect balance (they occur before payment)
        assert!(!SparkError::InvalidMnemonic("test".to_string()).balance_unaffected());
        assert!(!SparkError::Network("test".to_string()).balance_unaffected());
        assert!(!SparkError::InitializationFailed("test".to_string()).balance_unaffected());
    }

    #[test]
    fn test_insufficient_funds_message_shows_amounts() {
        let error = SparkError::InsufficientFunds {
            required: 1000,
            available: 500,
        };
        let msg = error.user_friendly_message();
        assert!(msg.contains("1000"), "Should show required amount");
        assert!(msg.contains("500"), "Should show available amount");
        assert!(
            msg.contains("NOT deducted"),
            "Should clarify balance is safe"
        );
    }
}

//! NIP-47: Nostr Wallet Connect (NWC)
//!
//! Standardized protocol for Nostr clients to interact with remote Lightning wallets
//! through encrypted direct messages over Nostr relays.
//!
//! Core types and connection URL helpers for Nostr Wallet Connect.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Event kind for wallet capabilities advertisement
pub const INFO_EVENT_KIND: u16 = 13194;

/// Event kind for command request (client → service)
pub const REQUEST_KIND: u16 = 23194;

/// Event kind for command response (service → client)
pub const RESPONSE_KIND: u16 = 23195;

/// Event kind for notifications using NIP-04 (legacy)
pub const NOTIFICATION_KIND_NIP04: u16 = 23196;

/// Event kind for notifications using NIP-44
pub const NOTIFICATION_KIND_NIP44: u16 = 23197;

/// Errors that can occur during NIP-47 operations
#[derive(Debug, Error)]
pub enum Nip47Error {
    #[error("rate limited: {0}")]
    RateLimited(String),

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("insufficient balance: {0}")]
    InsufficientBalance(String),

    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("restricted: {0}")]
    Restricted(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("payment failed: {0}")]
    PaymentFailed(String),

    #[error("unsupported encryption: {0}")]
    UnsupportedEncryption(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("other error: {0}")]
    Other(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Standard NWC error codes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    RateLimited,
    NotImplemented,
    InsufficientBalance,
    QuotaExceeded,
    Restricted,
    Unauthorized,
    Internal,
    PaymentFailed,
    UnsupportedEncryption,
    NotFound,
    Other,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::RateLimited => "RATE_LIMITED",
            ErrorCode::NotImplemented => "NOT_IMPLEMENTED",
            ErrorCode::InsufficientBalance => "INSUFFICIENT_BALANCE",
            ErrorCode::QuotaExceeded => "QUOTA_EXCEEDED",
            ErrorCode::Restricted => "RESTRICTED",
            ErrorCode::Unauthorized => "UNAUTHORIZED",
            ErrorCode::Internal => "INTERNAL",
            ErrorCode::PaymentFailed => "PAYMENT_FAILED",
            ErrorCode::UnsupportedEncryption => "UNSUPPORTED_ENCRYPTION",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::Other => "OTHER",
        }
    }
}

/// Error response structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub code: ErrorCode,
    pub message: String,
}

/// Nostr Wallet Connect URI.
///
/// Format: `nostr+walletconnect://<wallet-pubkey>?relay=<wss://relay>&secret=<secret>&lud16=<addr>`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NostrWalletConnectUrl {
    /// Wallet service public key (hex)
    pub wallet_pubkey: String,
    /// Relay URLs
    pub relays: Vec<String>,
    /// Client secret (hex)
    pub secret: String,
    /// Optional lightning address
    pub lud16: Option<String>,
}

impl NostrWalletConnectUrl {
    /// Create a new Nostr Wallet Connect URL.
    pub fn new(
        wallet_pubkey: impl Into<String>,
        relays: Vec<String>,
        secret: impl Into<String>,
    ) -> Self {
        Self {
            wallet_pubkey: wallet_pubkey.into(),
            relays,
            secret: secret.into(),
            lud16: None,
        }
    }

    /// Set optional lightning address.
    pub fn with_lud16(mut self, lud16: impl Into<String>) -> Self {
        self.lud16 = Some(lud16.into());
        self
    }

    /// Parse a nostr+walletconnect:// URL.
    pub fn parse(url: &str) -> Result<Self, Nip47Error> {
        const PREFIX: &str = "nostr+walletconnect://";
        if !url.starts_with(PREFIX) {
            return Err(Nip47Error::Parse(
                "URL must start with nostr+walletconnect://".to_string(),
            ));
        }

        let url = &url[PREFIX.len()..];
        let parts: Vec<&str> = url.splitn(2, '?').collect();
        let wallet_pubkey = parts[0].to_string();
        if wallet_pubkey.is_empty() {
            return Err(Nip47Error::Parse("Missing wallet pubkey".to_string()));
        }

        let mut relays = Vec::new();
        let mut secret = None;
        let mut lud16 = None;

        if parts.len() > 1 {
            for param in parts[1].split('&') {
                let kv: Vec<&str> = param.splitn(2, '=').collect();
                if kv.len() == 2 {
                    let key = kv[0];
                    let value = urlencoding::decode(kv[1])
                        .map_err(|e| Nip47Error::Parse(e.to_string()))?
                        .to_string();

                    match key {
                        "relay" => relays.push(value),
                        "secret" => secret = Some(value),
                        "lud16" => lud16 = Some(value),
                        _ => {}
                    }
                }
            }
        }

        let secret = secret
            .ok_or_else(|| Nip47Error::Parse("Missing required secret parameter".to_string()))?;

        if relays.is_empty() {
            return Err(Nip47Error::Parse(
                "At least one relay is required".to_string(),
            ));
        }

        Ok(Self {
            wallet_pubkey,
            relays,
            secret,
            lud16,
        })
    }

}

impl std::fmt::Display for NostrWalletConnectUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut url = format!("nostr+walletconnect://{}", self.wallet_pubkey);

        let mut params = Vec::new();
        for relay in &self.relays {
            params.push(format!("relay={}", urlencoding::encode(relay)));
        }
        params.push(format!("secret={}", urlencoding::encode(&self.secret)));
        if let Some(lud16) = &self.lud16 {
            params.push(format!("lud16={}", urlencoding::encode(lud16)));
        }

        url.push('?');
        url.push_str(&params.join("&"));
        write!(f, "{}", url)
    }
}

/// NWC command methods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    PayInvoice,
    MultiPayInvoice,
    PayKeysend,
    MultiPayKeysend,
    MakeInvoice,
    LookupInvoice,
    ListTransactions,
    GetBalance,
    GetInfo,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::PayInvoice => "pay_invoice",
            Method::MultiPayInvoice => "multi_pay_invoice",
            Method::PayKeysend => "pay_keysend",
            Method::MultiPayKeysend => "multi_pay_keysend",
            Method::MakeInvoice => "make_invoice",
            Method::LookupInvoice => "lookup_invoice",
            Method::ListTransactions => "list_transactions",
            Method::GetBalance => "get_balance",
            Method::GetInfo => "get_info",
        }
    }
}

/// Request structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub method: Method,
    pub params: RequestParams,
}

/// Request parameters for different methods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestParams {
    PayInvoice(PayInvoiceParams),
    MultiPayInvoice(MultiPayInvoiceParams),
    PayKeysend(PayKeysendParams),
    MultiPayKeysend(MultiPayKeysendParams),
    MakeInvoice(MakeInvoiceParams),
    LookupInvoice(LookupInvoiceParams),
    ListTransactions(ListTransactionsParams),
    GetBalance(GetBalanceParams),
    GetInfo(GetInfoParams),
}

/// Parameters for pay_invoice method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayInvoiceParams {
    pub invoice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for multi_pay_invoice method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPayInvoiceParams {
    pub invoices: Vec<MultiPayInvoiceItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPayInvoiceItem {
    pub id: String,
    pub invoice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<u64>,
}

/// Parameters for pay_keysend method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayKeysendParams {
    pub amount: u64,
    pub pubkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tlv_records: Option<Vec<TlvRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlvRecord {
    #[serde(rename = "type")]
    pub record_type: u64,
    pub value: String,
}

/// Parameters for multi_pay_keysend method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPayKeysendParams {
    pub keysends: Vec<MultiPayKeysendItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiPayKeysendItem {
    pub id: String,
    pub amount: u64,
    pub pubkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tlv_records: Option<Vec<TlvRecord>>,
}

/// Parameters for make_invoice method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MakeInvoiceParams {
    pub amount: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for lookup_invoice method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupInvoiceParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice: Option<String>,
}

/// Parameters for list_transactions method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListTransactionsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpaid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub transaction_type: Option<TransactionType>,
}

/// Parameters for get_balance method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetBalanceParams {}

/// Parameters for get_info method
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetInfoParams {}

/// Transaction type filter
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Incoming,
    Outgoing,
}

/// Response structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub result_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ResponseResult>,
}

impl Response {
    pub fn success(result_type: impl Into<String>, result: ResponseResult) -> Self {
        Self {
            result_type: result_type.into(),
            error: None,
            result: Some(result),
        }
    }

    pub fn error(result_type: impl Into<String>, error: ErrorResponse) -> Self {
        Self {
            result_type: result_type.into(),
            error: Some(error),
            result: None,
        }
    }
}

/// Response results for different methods
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResponseResult {
    PayInvoice(PayInvoiceResult),
    MakeInvoice(Invoice),
    LookupInvoice(Invoice),
    ListTransactions(ListTransactionsResult),
    GetBalance(BalanceResult),
    GetInfo(InfoResult),
}

/// Result for pay_invoice and pay_keysend
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PayInvoiceResult {
    pub preimage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees_paid: Option<u64>,
}

/// Invoice structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invoice {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub state: InvoiceState,
    pub invoice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage: Option<String>,
    pub payment_hash: String,
    pub amount: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees_paid: Option<u64>,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Invoice/payment state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InvoiceState {
    Pending,
    Settled,
    Expired,
    Failed,
}

/// Result for list_transactions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListTransactionsResult {
    pub transactions: Vec<Transaction>,
}

/// Transaction structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub state: InvoiceState,
    pub invoice: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preimage: Option<String>,
    pub payment_hash: String,
    pub amount: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fees_paid: Option<u64>,
    pub created_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Result for get_balance
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceResult {
    pub balance: u64,
}

/// Result for get_info
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfoResult {
    pub network: Network,
    pub block_height: u64,
    pub block_hash: String,
    pub methods: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
}

/// Bitcoin network
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Mainnet,
    Testnet,
    Signet,
    Regtest,
}

/// Notification type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    PaymentReceived,
    PaymentSent,
}

/// Notification structure
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notification {
    pub notification_type: NotificationType,
    pub notification: Transaction,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_serialization() {
        let error = ErrorResponse {
            code: ErrorCode::Unauthorized,
            message: "No wallet connected".to_string(),
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("UNAUTHORIZED"));
    }

    #[test]
    fn test_method_serialization() {
        let methods = vec![Method::PayInvoice, Method::GetBalance, Method::GetInfo];
        for method in methods {
            let json = serde_json::to_string(&method).unwrap();
            assert_eq!(json, format!("\"{}\"", method.as_str()));
        }
    }

    #[test]
    fn test_pay_invoice_request() {
        let request = Request {
            method: Method::PayInvoice,
            params: RequestParams::PayInvoice(PayInvoiceParams {
                invoice: "lnbc1...".to_string(),
                amount: Some(1000),
                metadata: None,
            }),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("pay_invoice"));
        assert!(json.contains("lnbc1"));
    }

    #[test]
    fn test_pay_invoice_response() {
        let response = Response::success(
            "pay_invoice",
            ResponseResult::PayInvoice(PayInvoiceResult {
                preimage: "abc123".to_string(),
                fees_paid: Some(100),
            }),
        );
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_get_balance_request() {
        let request = Request {
            method: Method::GetBalance,
            params: RequestParams::GetBalance(GetBalanceParams {}),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("get_balance"));
    }

    #[test]
    fn test_get_balance_response() {
        let response = Response::success(
            "get_balance",
            ResponseResult::GetBalance(BalanceResult { balance: 10000 }),
        );
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("10000"));
    }

    #[test]
    fn test_get_info_response() {
        let response = Response::success(
            "get_info",
            ResponseResult::GetInfo(InfoResult {
                network: Network::Mainnet,
                block_height: 800000,
                block_hash: "000000000000000000012345".to_string(),
                methods: vec!["pay_invoice".to_string(), "get_balance".to_string()],
                notifications: Some(vec!["payment_received".to_string()]),
                alias: Some("My Node".to_string()),
                color: None,
                pubkey: Some("03abc...".to_string()),
            }),
        );
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("mainnet"));
        assert!(json.contains("800000"));
    }

    #[test]
    fn test_error_response_serialization() {
        let response = Response::error(
            "get_balance",
            ErrorResponse {
                code: ErrorCode::Unauthorized,
                message: "No wallet connected".to_string(),
            },
        );
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("UNAUTHORIZED"));
        assert!(json.contains("No wallet connected"));
    }

    #[test]
    fn test_nwc_url_roundtrip() {
        let url = NostrWalletConnectUrl::new(
            "pubkey123",
            vec!["wss://relay.example.com".to_string()],
            "secret123",
        )
        .with_lud16("alice@example.com");

        let encoded = url.to_string();
        assert!(encoded.starts_with("nostr+walletconnect://pubkey123?"));
        assert!(encoded.contains("relay=wss%3A%2F%2Frelay.example.com"));
        assert!(encoded.contains("secret=secret123"));
        assert!(encoded.contains("lud16=alice%40example.com"));

        let decoded = NostrWalletConnectUrl::parse(&encoded).unwrap();
        assert_eq!(decoded.wallet_pubkey, "pubkey123");
        assert_eq!(decoded.secret, "secret123");
        assert_eq!(decoded.lud16, Some("alice@example.com".to_string()));
    }

    #[test]
    fn test_nwc_url_requires_relay_and_secret() {
        let missing_secret = "nostr+walletconnect://pubkey?relay=wss%3A%2F%2Frelay.example.com";
        assert!(NostrWalletConnectUrl::parse(missing_secret).is_err());

        let missing_relay = "nostr+walletconnect://pubkey?secret=abc123";
        assert!(NostrWalletConnectUrl::parse(missing_relay).is_err());
    }

    #[test]
    fn test_make_invoice_request() {
        let request = Request {
            method: Method::MakeInvoice,
            params: RequestParams::MakeInvoice(MakeInvoiceParams {
                amount: 5000,
                description: Some("Test invoice".to_string()),
                description_hash: None,
                expiry: Some(3600),
                metadata: None,
            }),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("make_invoice"));
        assert!(json.contains("5000"));
    }

    #[test]
    fn test_invoice_state_serialization() {
        let states = vec![
            InvoiceState::Pending,
            InvoiceState::Settled,
            InvoiceState::Expired,
            InvoiceState::Failed,
        ];
        for state in states {
            let json = serde_json::to_string(&state).unwrap();
            match state {
                InvoiceState::Pending => assert_eq!(json, "\"pending\""),
                InvoiceState::Settled => assert_eq!(json, "\"settled\""),
                InvoiceState::Expired => assert_eq!(json, "\"expired\""),
                InvoiceState::Failed => assert_eq!(json, "\"failed\""),
            }
        }
    }

    #[test]
    fn test_notification_serialization() {
        let notification = Notification {
            notification_type: NotificationType::PaymentReceived,
            notification: Transaction {
                transaction_type: TransactionType::Incoming,
                state: InvoiceState::Settled,
                invoice: "lnbc...".to_string(),
                description: Some("Payment".to_string()),
                description_hash: None,
                preimage: Some("abc".to_string()),
                payment_hash: "def".to_string(),
                amount: 1000,
                fees_paid: Some(10),
                created_at: 1234567890,
                expires_at: None,
                settled_at: Some(1234567900),
                metadata: None,
            },
        };
        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("payment_received"));
        assert!(json.contains("incoming"));
    }

    #[test]
    fn test_list_transactions_request() {
        let request = Request {
            method: Method::ListTransactions,
            params: RequestParams::ListTransactions(ListTransactionsParams {
                from: Some(1693876973),
                until: Some(1703225078),
                limit: Some(10),
                offset: Some(0),
                unpaid: Some(true),
                transaction_type: Some(TransactionType::Incoming),
            }),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("list_transactions"));
        assert!(json.contains("incoming"));
    }

    #[test]
    fn test_pay_keysend_request() {
        let request = Request {
            method: Method::PayKeysend,
            params: RequestParams::PayKeysend(PayKeysendParams {
                amount: 1000,
                pubkey: "03abc...".to_string(),
                preimage: None,
                tlv_records: Some(vec![TlvRecord {
                    record_type: 5482373484,
                    value: "hexdata".to_string(),
                }]),
                metadata: None,
            }),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("pay_keysend"));
        assert!(json.contains("5482373484"));
    }
}

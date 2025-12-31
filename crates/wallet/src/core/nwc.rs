//! Nostr Wallet Connect service helpers.

use anyhow::{Context, Result};
use nostr::{
    ErrorCode, ErrorResponse, Event, EventTemplate, GetBalanceParams, GetInfoParams,
    INFO_EVENT_KIND, InfoResult, Invoice, InvoiceState, ListTransactionsParams,
    ListTransactionsResult, LookupInvoiceParams, MakeInvoiceParams, Method,
    Network as Nip47Network, PayInvoiceParams, PayInvoiceResult, REQUEST_KIND, RESPONSE_KIND,
    Request, RequestParams, Response, ResponseResult, Transaction, TransactionType, decrypt,
    decrypt_v2, encrypt, encrypt_v2, finalize_event,
};
use nostr_client::{PoolConfig, RelayPool};
use serde_json::json;
use spark::wallet::PaymentDetails;
use spark::{Payment, PaymentStatus, PaymentType, SparkError, SparkWallet};
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

use crate::storage::config::WalletConfig;
use crate::storage::nwc::NwcConnection;

const ENCRYPTION_TAG: &str = "encryption";
const ENCRYPTION_NIP44: &str = "nip44_v2";
const ENCRYPTION_NIP04: &str = "nip04";
const ENCRYPTION_SUPPORTED: &str = "nip44_v2 nip04";

const EXPIRATION_TAG: &str = "expiration";
const P_TAG: &str = "p";
const E_TAG: &str = "e";
const D_TAG: &str = "d";

const SUPPORTED_METHODS: &[Method] = &[
    Method::PayInvoice,
    Method::MakeInvoice,
    Method::LookupInvoice,
    Method::ListTransactions,
    Method::GetBalance,
    Method::GetInfo,
];

#[derive(Debug, Clone, Copy)]
enum EncryptionScheme {
    Nip44,
    Nip04,
}

impl EncryptionScheme {
    #[allow(dead_code)]
    fn as_tag_value(self) -> &'static str {
        match self {
            EncryptionScheme::Nip44 => ENCRYPTION_NIP44,
            EncryptionScheme::Nip04 => ENCRYPTION_NIP04,
        }
    }
}

pub struct NwcConnectionOutput {
    pub connection: NwcConnection,
    pub uri: String,
}

pub fn build_connection(name: Option<String>, relays: Vec<String>) -> Result<NwcConnectionOutput> {
    if relays.is_empty() {
        anyhow::bail!("At least one relay is required to create an NWC connection.");
    }

    let wallet_secret = nostr::generate_secret_key();
    let wallet_pubkey =
        nostr::get_public_key_hex(&wallet_secret).context("Failed to derive wallet pubkey")?;
    let client_secret = nostr::generate_secret_key();
    let client_pubkey =
        nostr::get_public_key_hex(&client_secret).context("Failed to derive client pubkey")?;

    let created_at = current_timestamp()?;
    let wallet_secret_hex = hex::encode(wallet_secret);
    let client_secret_hex = hex::encode(client_secret);

    let connection = NwcConnection {
        id: wallet_pubkey.clone(),
        name,
        wallet_pubkey: wallet_pubkey.clone(),
        wallet_secret: wallet_secret_hex,
        client_pubkey,
        relays: relays.clone(),
        created_at,
    };

    let uri =
        nostr::NostrWalletConnectUrl::new(wallet_pubkey, relays, client_secret_hex).to_string();

    Ok(NwcConnectionOutput { connection, uri })
}

pub async fn publish_info_event(connection: &NwcConnection) -> Result<()> {
    let secret_key = secret_hex_to_bytes(&connection.wallet_secret)?;
    let content = supported_methods().join(" ");

    let tags = vec![vec![
        ENCRYPTION_TAG.to_string(),
        ENCRYPTION_SUPPORTED.to_string(),
    ]];

    let event = finalize_event(
        &EventTemplate {
            created_at: current_timestamp()?,
            kind: INFO_EVENT_KIND,
            tags,
            content,
        },
        &secret_key,
    )
    .context("Failed to sign NWC info event")?;

    let client = crate::core::client::NostrClient::new(connection.relays.clone());
    let results = client.publish_event(&event).await?;
    let failures: Vec<_> = results
        .into_iter()
        .filter_map(|result| result.error_message())
        .collect();

    if !failures.is_empty() {
        anyhow::bail!("Failed to publish NWC info event: {}", failures.join(", "));
    }

    Ok(())
}

pub struct NwcService {
    wallet: SparkWallet,
    config: WalletConfig,
    connections: Vec<NwcConnection>,
    pool: RelayPool,
}

impl NwcService {
    pub async fn new(
        wallet: SparkWallet,
        config: WalletConfig,
        connections: Vec<NwcConnection>,
    ) -> Result<Self> {
        if connections.is_empty() {
            anyhow::bail!("No NWC connections configured. Create one with `nwc create`.");
        }

        let mut relays = HashSet::new();
        for connection in &connections {
            for relay in &connection.relays {
                relays.insert(relay.clone());
            }
        }

        let pool = RelayPool::new(PoolConfig::default());
        for relay in relays {
            pool.add_relay(&relay).await?;
        }
        pool.connect_all().await?;

        Ok(Self {
            wallet,
            config,
            connections,
            pool,
        })
    }

    pub async fn run(&self) -> Result<()> {
        let wallet_pubkeys: Vec<String> = self
            .connections
            .iter()
            .map(|connection| connection.wallet_pubkey.clone())
            .collect();

        let filter = json!({
            "kinds": [REQUEST_KIND],
            "#p": wallet_pubkeys,
        });

        let mut rx = self.pool.subscribe("nwc-requests", &[filter]).await?;

        while let Some(event) = rx.recv().await {
            if let Err(err) = self.handle_event(event).await {
                warn!("NWC request handling failed: {}", err);
            }
        }

        Ok(())
    }

    async fn handle_event(&self, event: Event) -> Result<()> {
        let Some(connection) = find_connection_for_event(&event, &self.connections) else {
            debug!("Ignoring NWC request without matching connection");
            return Ok(());
        };

        if is_expired(&event) {
            debug!("Ignoring expired NWC request {}", event.id);
            return Ok(());
        }

        let encryption_tag = find_tag_value(&event.tags, ENCRYPTION_TAG);
        let mut unsupported_encryption = false;
        let schemes = match encryption_tag.as_deref() {
            Some(ENCRYPTION_NIP44) => vec![EncryptionScheme::Nip44],
            Some(ENCRYPTION_NIP04) | None => vec![EncryptionScheme::Nip04],
            Some(_) => {
                unsupported_encryption = true;
                vec![EncryptionScheme::Nip44, EncryptionScheme::Nip04]
            }
        };

        let (scheme, request) = match decrypt_request(&event, connection, &schemes) {
            Ok(value) => value,
            Err(err) => {
                warn!("Failed to decrypt NWC request {}: {}", event.id, err);
                return Ok(());
            }
        };

        let method = request.method.clone();
        let response = if unsupported_encryption {
            Response::error(
                method.as_str(),
                ErrorResponse {
                    code: ErrorCode::UnsupportedEncryption,
                    message: format!(
                        "Unsupported encryption: {}",
                        encryption_tag.unwrap_or_else(|| "unknown".to_string())
                    ),
                },
            )
        } else if connection.client_pubkey != event.pubkey {
            Response::error(
                method.as_str(),
                ErrorResponse {
                    code: ErrorCode::Unauthorized,
                    message: "Client pubkey not authorized for this connection".to_string(),
                },
            )
        } else {
            self.handle_request(request, &connection.wallet_pubkey)
                .await
        };

        let response_event = build_response_event(&response, scheme, connection, &event)?;
        self.pool.publish(&response_event).await?;

        Ok(())
    }

    async fn handle_request(&self, request: Request, pubkey: &str) -> Response {
        match request.method {
            Method::PayInvoice => match request.params {
                RequestParams::PayInvoice(params) => {
                    handle_pay_invoice(&self.wallet, &self.config, params).await
                }
                _ => invalid_request(Method::PayInvoice, "Expected pay_invoice params"),
            },
            Method::MakeInvoice => match request.params {
                RequestParams::MakeInvoice(params) => {
                    handle_make_invoice(&self.wallet, params).await
                }
                _ => invalid_request(Method::MakeInvoice, "Expected make_invoice params"),
            },
            Method::LookupInvoice => match request.params {
                RequestParams::LookupInvoice(params) => {
                    handle_lookup_invoice(&self.wallet, params).await
                }
                _ => invalid_request(Method::LookupInvoice, "Expected lookup_invoice params"),
            },
            Method::ListTransactions => match request.params {
                RequestParams::ListTransactions(params) => {
                    handle_list_transactions(&self.wallet, params).await
                }
                _ => invalid_request(
                    Method::ListTransactions,
                    "Expected list_transactions params",
                ),
            },
            Method::GetBalance => match request.params {
                RequestParams::GetBalance(GetBalanceParams {}) => {
                    handle_get_balance(&self.wallet).await
                }
                _ => invalid_request(Method::GetBalance, "Expected get_balance params"),
            },
            Method::GetInfo => match request.params {
                RequestParams::GetInfo(GetInfoParams {}) => {
                    handle_get_info(&self.config, pubkey).await
                }
                _ => invalid_request(Method::GetInfo, "Expected get_info params"),
            },
            Method::PayKeysend | Method::MultiPayInvoice | Method::MultiPayKeysend => {
                Response::error(
                    request.method.as_str(),
                    ErrorResponse {
                        code: ErrorCode::NotImplemented,
                        message: "Method not implemented".to_string(),
                    },
                )
            }
        }
    }
}

async fn handle_pay_invoice(
    wallet: &SparkWallet,
    config: &WalletConfig,
    params: PayInvoiceParams,
) -> Response {
    let amount_sats = match params.amount {
        Some(msats) => match msats_to_sats(msats) {
            Ok(amount) => Some(amount),
            Err(err) => {
                return Response::error(
                    Method::PayInvoice.as_str(),
                    ErrorResponse {
                        code: ErrorCode::Other,
                        message: err.to_string(),
                    },
                );
            }
        },
        None => None,
    };

    if let Some(amount) = amount_sats {
        if let Err(err) = enforce_payment_limits(amount, config) {
            return Response::error(
                Method::PayInvoice.as_str(),
                ErrorResponse {
                    code: ErrorCode::Restricted,
                    message: err.to_string(),
                },
            );
        }
    }

    match wallet
        .send_payment_simple(&params.invoice, amount_sats)
        .await
    {
        Ok(response) => {
            let preimage = match extract_preimage(&response.payment) {
                Some(value) => value,
                None => {
                    return Response::error(
                        Method::PayInvoice.as_str(),
                        ErrorResponse {
                            code: ErrorCode::Internal,
                            message: "Payment completed without preimage".to_string(),
                        },
                    );
                }
            };
            let fees_paid = match msats_from_sats(response.payment.fees) {
                Ok(fees) if fees > 0 => Some(fees),
                _ => None,
            };
            Response::success(
                Method::PayInvoice.as_str(),
                ResponseResult::PayInvoice(PayInvoiceResult {
                    preimage,
                    fees_paid,
                }),
            )
        }
        Err(err) => Response::error(Method::PayInvoice.as_str(), spark_error(err)),
    }
}

async fn handle_make_invoice(wallet: &SparkWallet, params: MakeInvoiceParams) -> Response {
    let amount_sats = match msats_to_sats(params.amount) {
        Ok(value) => value,
        Err(err) => {
            return Response::error(
                Method::MakeInvoice.as_str(),
                ErrorResponse {
                    code: ErrorCode::Other,
                    message: err.to_string(),
                },
            );
        }
    };

    match wallet
        .create_invoice(amount_sats, params.description.clone(), params.expiry)
        .await
    {
        Ok(response) => {
            let created_at = current_timestamp().unwrap_or(0);
            let invoice_details = parse_invoice_details(&response.payment_request).await;
            let (payment_hash, description, description_hash, expires_at, invoice_amount) =
                invoice_details;

            let amount_msats = invoice_amount.unwrap_or_else(|| params.amount);

            let invoice = Invoice {
                transaction_type: TransactionType::Incoming,
                state: InvoiceState::Pending,
                invoice: response.payment_request,
                description: params.description.or(description),
                description_hash: params.description_hash.or(description_hash),
                preimage: None,
                payment_hash,
                amount: amount_msats,
                fees_paid: None,
                created_at,
                expires_at,
                settled_at: None,
                metadata: params.metadata,
            };

            Response::success(
                Method::MakeInvoice.as_str(),
                ResponseResult::MakeInvoice(invoice),
            )
        }
        Err(err) => Response::error(Method::MakeInvoice.as_str(), spark_error(err)),
    }
}

async fn handle_lookup_invoice(wallet: &SparkWallet, params: LookupInvoiceParams) -> Response {
    if params.payment_hash.is_none() && params.invoice.is_none() {
        return Response::error(
            Method::LookupInvoice.as_str(),
            ErrorResponse {
                code: ErrorCode::Other,
                message: "payment_hash or invoice is required".to_string(),
            },
        );
    }

    match find_payment(wallet, &params).await {
        Ok(Some(payment)) => match payment_to_invoice(&payment) {
            Ok(invoice) => Response::success(
                Method::LookupInvoice.as_str(),
                ResponseResult::LookupInvoice(invoice),
            ),
            Err(err) => Response::error(
                Method::LookupInvoice.as_str(),
                ErrorResponse {
                    code: ErrorCode::Internal,
                    message: err.to_string(),
                },
            ),
        },
        Ok(None) => Response::error(
            Method::LookupInvoice.as_str(),
            ErrorResponse {
                code: ErrorCode::NotFound,
                message: "Invoice not found".to_string(),
            },
        ),
        Err(err) => Response::error(
            Method::LookupInvoice.as_str(),
            ErrorResponse {
                code: ErrorCode::Internal,
                message: err.to_string(),
            },
        ),
    }
}

async fn handle_list_transactions(
    wallet: &SparkWallet,
    params: ListTransactionsParams,
) -> Response {
    match list_transactions(wallet, params).await {
        Ok(result) => Response::success(
            Method::ListTransactions.as_str(),
            ResponseResult::ListTransactions(result),
        ),
        Err(err) => Response::error(
            Method::ListTransactions.as_str(),
            ErrorResponse {
                code: ErrorCode::Internal,
                message: err.to_string(),
            },
        ),
    }
}

async fn handle_get_balance(wallet: &SparkWallet) -> Response {
    match wallet.get_balance().await {
        Ok(balance) => match msats_from_sats(balance.total_sats() as u128) {
            Ok(msats) => Response::success(
                Method::GetBalance.as_str(),
                ResponseResult::GetBalance(nostr::BalanceResult { balance: msats }),
            ),
            Err(err) => Response::error(
                Method::GetBalance.as_str(),
                ErrorResponse {
                    code: ErrorCode::Internal,
                    message: err.to_string(),
                },
            ),
        },
        Err(err) => Response::error(Method::GetBalance.as_str(), spark_error(err)),
    }
}

async fn handle_get_info(config: &WalletConfig, pubkey: &str) -> Response {
    let network = match config.network.bitcoin.to_lowercase().as_str() {
        "mainnet" => Nip47Network::Mainnet,
        "testnet" => Nip47Network::Testnet,
        "signet" => Nip47Network::Signet,
        "regtest" => Nip47Network::Regtest,
        _ => Nip47Network::Mainnet,
    };

    let info = InfoResult {
        network,
        block_height: 0,
        block_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        methods: supported_methods(),
        notifications: None,
        alias: None,
        color: None,
        pubkey: Some(pubkey.to_string()),
    };

    Response::success(Method::GetInfo.as_str(), ResponseResult::GetInfo(info))
}

fn invalid_request(method: Method, message: &str) -> Response {
    Response::error(
        method.as_str(),
        ErrorResponse {
            code: ErrorCode::Other,
            message: message.to_string(),
        },
    )
}

fn spark_error(err: SparkError) -> ErrorResponse {
    match err {
        SparkError::InsufficientFunds { .. } => ErrorResponse {
            code: ErrorCode::InsufficientBalance,
            message: err.user_friendly_message(),
        },
        SparkError::PaymentFailed(_)
        | SparkError::PaymentRouteNotFound
        | SparkError::PaymentTimeout => ErrorResponse {
            code: ErrorCode::PaymentFailed,
            message: err.user_friendly_message(),
        },
        SparkError::InvalidInvoice(_) | SparkError::InvoiceExpired => ErrorResponse {
            code: ErrorCode::Other,
            message: err.user_friendly_message(),
        },
        SparkError::InvalidAddress(_) => ErrorResponse {
            code: ErrorCode::Other,
            message: err.user_friendly_message(),
        },
        _ => ErrorResponse {
            code: ErrorCode::Internal,
            message: err.user_friendly_message(),
        },
    }
}

fn supported_methods() -> Vec<String> {
    SUPPORTED_METHODS
        .iter()
        .map(|method| method.as_str().to_string())
        .collect()
}

fn enforce_payment_limits(amount_sats: u64, config: &WalletConfig) -> Result<()> {
    if let Some(limit) = config.security.max_send_sats {
        if amount_sats > limit {
            anyhow::bail!(
                "Payment amount {} sats exceeds configured limit {} sats.",
                amount_sats,
                limit
            );
        }
    }

    if let Some(threshold) = config.security.confirm_large_sats {
        if amount_sats >= threshold {
            anyhow::bail!(
                "Payment amount {} sats requires confirmation; lower amount or adjust confirm_large_sats.",
                amount_sats
            );
        }
    }

    Ok(())
}

fn current_timestamp() -> Result<u64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System time error")?
        .as_secs())
}

fn is_expired(event: &Event) -> bool {
    let Some(value) = find_tag_value(&event.tags, EXPIRATION_TAG) else {
        return false;
    };

    let Ok(expiration) = value.parse::<u64>() else {
        return false;
    };

    if let Ok(now) = current_timestamp() {
        return expiration < now;
    }

    false
}

fn find_tag_value(tags: &[Vec<String>], key: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.get(0).map(|value| value == key).unwrap_or(false))
        .and_then(|tag| tag.get(1).cloned())
}

fn find_tag_values(tags: &[Vec<String>], key: &str) -> Vec<String> {
    tags.iter()
        .filter(|tag| tag.get(0).map(|value| value == key).unwrap_or(false))
        .filter_map(|tag| tag.get(1).cloned())
        .collect()
}

fn find_connection_for_event<'a>(
    event: &Event,
    connections: &'a [NwcConnection],
) -> Option<&'a NwcConnection> {
    let tags = find_tag_values(&event.tags, P_TAG);
    if tags.is_empty() {
        return None;
    }

    connections
        .iter()
        .find(|connection| tags.iter().any(|tag| tag == &connection.wallet_pubkey))
}

fn decrypt_request(
    event: &Event,
    connection: &NwcConnection,
    schemes: &[EncryptionScheme],
) -> Result<(EncryptionScheme, Request)> {
    let secret_key = secret_hex_to_bytes(&connection.wallet_secret)?;
    let sender_pubkey = pubkey_hex_to_compressed(&event.pubkey)?;

    for scheme in schemes {
        let decrypted = match scheme {
            EncryptionScheme::Nip44 => decrypt_v2(&secret_key, &sender_pubkey, &event.content)
                .context("NIP-44 decrypt failed"),
            EncryptionScheme::Nip04 => decrypt(&secret_key, &sender_pubkey, &event.content)
                .context("NIP-04 decrypt failed"),
        };

        if let Ok(plaintext) = decrypted {
            let request: Request =
                serde_json::from_str(&plaintext).context("Failed to parse NWC request")?;
            return Ok((*scheme, request));
        }
    }

    anyhow::bail!("Unable to decrypt NWC request")
}

fn build_response_event(
    response: &Response,
    scheme: EncryptionScheme,
    connection: &NwcConnection,
    request_event: &Event,
) -> Result<Event> {
    let secret_key = secret_hex_to_bytes(&connection.wallet_secret)?;
    let recipient_pubkey = pubkey_hex_to_compressed(&request_event.pubkey)?;

    let plaintext = serde_json::to_string(response).context("Failed to serialize response")?;
    let encrypted = match scheme {
        EncryptionScheme::Nip44 => encrypt_v2(&secret_key, &recipient_pubkey, &plaintext)
            .context("Failed to encrypt response with NIP-44")?,
        EncryptionScheme::Nip04 => encrypt(&secret_key, &recipient_pubkey, &plaintext)
            .context("Failed to encrypt response with NIP-04")?,
    };

    let mut tags = vec![
        vec![P_TAG.to_string(), request_event.pubkey.clone()],
        vec![E_TAG.to_string(), request_event.id.clone()],
    ];

    if let Some(value) = find_tag_value(&request_event.tags, D_TAG) {
        tags.push(vec![D_TAG.to_string(), value]);
    }

    finalize_event(
        &EventTemplate {
            created_at: current_timestamp()?,
            kind: RESPONSE_KIND,
            tags,
            content: encrypted,
        },
        &secret_key,
    )
    .context("Failed to sign NWC response event")
}

fn secret_hex_to_bytes(secret_hex: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(secret_hex).context("Invalid secret key hex")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid secret key length"))
}

fn pubkey_hex_to_compressed(pubkey_hex: &str) -> Result<[u8; 33]> {
    let bytes = hex::decode(pubkey_hex).context("Invalid pubkey hex")?;
    let pk_bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid pubkey length"))?;

    let mut compressed = [0u8; 33];
    compressed[0] = 0x02;
    compressed[1..].copy_from_slice(&pk_bytes);
    Ok(compressed)
}

fn msats_to_sats(msats: u64) -> Result<u64> {
    if msats % 1000 != 0 {
        anyhow::bail!("Amount must be in whole satoshis (1000 msats)");
    }
    Ok(msats / 1000)
}

fn msats_from_sats(sats: u128) -> Result<u64> {
    let msats = sats
        .checked_mul(1000)
        .ok_or_else(|| anyhow::anyhow!("Amount overflow"))?;
    u64::try_from(msats).context("Amount exceeds supported range")
}

fn extract_preimage(payment: &Payment) -> Option<String> {
    match payment.details.as_ref() {
        Some(PaymentDetails::Lightning { preimage, .. }) => preimage.clone(),
        Some(PaymentDetails::Spark { htlc_details, .. }) => {
            htlc_details.as_ref().and_then(|htlc| htlc.preimage.clone())
        }
        _ => None,
    }
}

fn payment_hash_for(payment: &Payment) -> String {
    if let Some(details) = payment.details.as_ref() {
        match details {
            PaymentDetails::Lightning { payment_hash, .. } => {
                if is_hex_64(payment_hash) {
                    return payment_hash.clone();
                }
            }
            PaymentDetails::Spark { htlc_details, .. } => {
                if let Some(htlc) = htlc_details.as_ref() {
                    if is_hex_64(&htlc.payment_hash) {
                        return htlc.payment_hash.clone();
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(invoice) = extract_invoice(payment) {
        return sha256_hex(&invoice);
    }

    sha256_hex(&payment.id)
}

fn extract_invoice(payment: &Payment) -> Option<String> {
    match payment.details.as_ref() {
        Some(PaymentDetails::Lightning { invoice, .. }) => Some(invoice.clone()),
        Some(PaymentDetails::Spark {
            invoice_details, ..
        }) => invoice_details
            .as_ref()
            .map(|details| details.invoice.clone()),
        Some(PaymentDetails::Token {
            invoice_details, ..
        }) => invoice_details
            .as_ref()
            .map(|details| details.invoice.clone()),
        _ => None,
    }
}

fn extract_description(payment: &Payment) -> Option<String> {
    match payment.details.as_ref() {
        Some(PaymentDetails::Lightning { description, .. }) => description.clone(),
        Some(PaymentDetails::Spark {
            invoice_details, ..
        }) => invoice_details
            .as_ref()
            .and_then(|details| details.description.clone()),
        Some(PaymentDetails::Token {
            invoice_details, ..
        }) => invoice_details
            .as_ref()
            .and_then(|details| details.description.clone()),
        _ => None,
    }
}

fn payment_state(payment: &Payment) -> InvoiceState {
    match payment.status {
        PaymentStatus::Completed => InvoiceState::Settled,
        PaymentStatus::Pending => InvoiceState::Pending,
        PaymentStatus::Failed => {
            if payment.payment_type == PaymentType::Receive {
                InvoiceState::Expired
            } else {
                InvoiceState::Failed
            }
        }
    }
}

fn payment_to_transaction(payment: &Payment) -> Result<Option<Transaction>> {
    let invoice = match extract_invoice(payment) {
        Some(value) => value,
        None => return Ok(None),
    };

    let amount = msats_from_sats(payment.amount)?;
    let fees_paid = match msats_from_sats(payment.fees) {
        Ok(value) if value > 0 => Some(value),
        _ => None,
    };

    let transaction_type = match payment.payment_type {
        PaymentType::Send => TransactionType::Outgoing,
        PaymentType::Receive => TransactionType::Incoming,
    };

    Ok(Some(Transaction {
        transaction_type,
        state: payment_state(payment),
        invoice,
        description: extract_description(payment),
        description_hash: None,
        preimage: extract_preimage(payment),
        payment_hash: payment_hash_for(payment),
        amount,
        fees_paid,
        created_at: payment.timestamp,
        expires_at: None,
        settled_at: if payment.status == PaymentStatus::Completed {
            Some(payment.timestamp)
        } else {
            None
        },
        metadata: None,
    }))
}

fn payment_to_invoice(payment: &Payment) -> Result<Invoice> {
    let invoice =
        extract_invoice(payment).ok_or_else(|| anyhow::anyhow!("Payment has no invoice"))?;
    let amount = msats_from_sats(payment.amount)?;
    let fees_paid = match msats_from_sats(payment.fees) {
        Ok(value) if value > 0 => Some(value),
        _ => None,
    };

    Ok(Invoice {
        transaction_type: match payment.payment_type {
            PaymentType::Send => TransactionType::Outgoing,
            PaymentType::Receive => TransactionType::Incoming,
        },
        state: payment_state(payment),
        invoice,
        description: extract_description(payment),
        description_hash: None,
        preimage: extract_preimage(payment),
        payment_hash: payment_hash_for(payment),
        amount,
        fees_paid,
        created_at: payment.timestamp,
        expires_at: None,
        settled_at: if payment.status == PaymentStatus::Completed {
            Some(payment.timestamp)
        } else {
            None
        },
        metadata: None,
    })
}

async fn parse_invoice_details(
    invoice: &str,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<u64>,
    Option<u64>,
) {
    let hash = sha256_hex(invoice);

    if let Ok(input) = breez_sdk_spark::parse_input(invoice, None).await {
        match input {
            breez_sdk_spark::InputType::Bolt11Invoice(details) => {
                return (
                    details.payment_hash,
                    details.description,
                    details.description_hash,
                    Some(details.timestamp + details.expiry),
                    details.amount_msat,
                );
            }
            breez_sdk_spark::InputType::SparkInvoice(details) => {
                let amount_msats = details
                    .amount
                    .and_then(|amount| amount.checked_mul(1000))
                    .and_then(|amount| u64::try_from(amount).ok());
                return (
                    hash,
                    details.description,
                    None,
                    details.expiry_time,
                    amount_msats,
                );
            }
            _ => {}
        }
    }

    (hash, None, None, None, None)
}

async fn list_transactions(
    wallet: &SparkWallet,
    params: ListTransactionsParams,
) -> Result<ListTransactionsResult> {
    let from = params.from.unwrap_or(0);
    let until = params
        .until
        .unwrap_or_else(|| current_timestamp().unwrap_or(u64::MAX));
    let limit = params.limit.unwrap_or(20) as usize;
    let offset = params.offset.unwrap_or(0) as usize;

    let mut results = Vec::new();
    let mut skipped = 0usize;
    let mut page_offset = 0u32;
    let page_size = 50u32;

    loop {
        let payments = wallet
            .list_payments(Some(page_size), Some(page_offset))
            .await?;
        if payments.is_empty() {
            break;
        }

        for payment in &payments {
            if payment.timestamp < from || payment.timestamp > until {
                continue;
            }

            if let Some(transaction_type) = params.transaction_type.as_ref() {
                if *transaction_type == TransactionType::Incoming
                    && payment.payment_type != PaymentType::Receive
                {
                    continue;
                }
                if *transaction_type == TransactionType::Outgoing
                    && payment.payment_type != PaymentType::Send
                {
                    continue;
                }
            }

            if params.unpaid.unwrap_or(false) == false
                && payment.payment_type == PaymentType::Receive
                && payment.status != PaymentStatus::Completed
            {
                continue;
            }

            let Some(transaction) = payment_to_transaction(payment)? else {
                continue;
            };

            if skipped < offset {
                skipped += 1;
                continue;
            }

            results.push(transaction);
            if results.len() >= limit {
                break;
            }
        }

        if results.len() >= limit || payments.len() < page_size as usize {
            break;
        }

        page_offset = page_offset.saturating_add(page_size);
    }

    Ok(ListTransactionsResult {
        transactions: results,
    })
}

async fn find_payment(
    wallet: &SparkWallet,
    params: &LookupInvoiceParams,
) -> Result<Option<Payment>> {
    let mut page_offset = 0u32;
    let page_size = 50u32;

    loop {
        let payments = wallet
            .list_payments(Some(page_size), Some(page_offset))
            .await?;
        if payments.is_empty() {
            break;
        }

        for payment in &payments {
            if let Some(invoice) = params.invoice.as_ref() {
                if extract_invoice(payment).as_deref() == Some(invoice.as_str()) {
                    return Ok(Some(payment.clone()));
                }
            }

            if let Some(hash) = params.payment_hash.as_ref() {
                if payment_hash_for(payment) == *hash {
                    return Ok(Some(payment.clone()));
                }
            }
        }

        if payments.len() < page_size as usize {
            break;
        }

        page_offset = page_offset.saturating_add(page_size);
    }

    Ok(None)
}

fn sha256_hex(value: &str) -> String {
    use bitcoin::hashes::{Hash, sha256};
    let hash = sha256::Hash::hash(value.as_bytes());
    hex::encode(hash.as_byte_array())
}

fn is_hex_64(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msats_to_sats() {
        assert_eq!(msats_to_sats(2000).unwrap(), 2);
        assert!(msats_to_sats(1500).is_err());
    }

    #[test]
    fn test_payment_hash_fallback() {
        let payment = Payment {
            id: "payment-1".to_string(),
            payment_type: PaymentType::Send,
            status: PaymentStatus::Completed,
            amount: 10,
            fees: 1,
            timestamp: 1_700_000_000,
            method: spark::PaymentMethod::Lightning,
            details: None,
        };
        let hash = payment_hash_for(&payment);
        assert_eq!(hash.len(), 64);
    }
}

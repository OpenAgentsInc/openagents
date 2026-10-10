//! The Spark wallet: Breez's SDK (`breez-sdk-spark`) behind the
//! [`Node`](crate::model::Node) trait that the phone's Wallet tab and
//! `openagents wallet` on computers use. Everything here is blocking; each
//! call runs on the wallet's own Tokio runtime.
//!
//! The caller chooses where the SDK keeps its records: the phone passes
//! Breez's SQLite store, computers the JSON file in [`crate::store`].
//! Real-time sync to Breez's server is off. The SDK logs through `tracing`,
//! and neither the app nor the command installs a subscriber, so nothing it
//! traces is written anywhere.

use crate::model::{
    AgentPayFailure, Ask, ClaimQuote, Contact, DepositProblem, DepositRow, Destination, FeeRates,
    InvoicePayment, LnurlTerms, Node, Paid, PayFailure, PaymentRow, Provider, Quote, QuoteFailure,
    SendRequest, Speed,
};
use breez_sdk_spark::{
    AddContactRequest, BreezSdk, BuyBitcoinRequest, ClaimDepositOutcome, ClaimDepositRequest,
    DepositClaimError, EventListener, Fee, FetchClaimDepositQuoteRequest, GetInfoRequest,
    InputType, ListContactsRequest, ListPaymentsRequest, ListUnclaimedDepositsRequest,
    LnurlPayRequest, MaxFee, Network, OnchainConfirmationSpeed, Payment, PaymentDetails,
    PaymentMethod, PaymentRequest, PaymentStatus, PaymentType, PrepareLnurlPayRequest,
    PrepareLnurlPayResponse, PrepareSendPaymentRequest, PrepareSendPaymentResponse,
    ReceivePaymentMethod, ReceivePaymentRequest, RefundDepositRequest, SdkBuilder, SdkError,
    SdkEvent, Seed, SendPaymentMethod, SendPaymentOptions, SendPaymentRequest, StorageBackend,
    SuccessActionProcessed, SyncWalletRequest, default_config,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// OpenAgents' Breez API key. The owner confirmed with the Breez team that it
/// is a basic validation key, which every shipped app carries and none can
/// hide, so it is committed here plainly (2026-09-28). It is a Base64 X.509
/// certificate that Breez's services check; it is not a spending credential.
pub const BREEZ_API_KEY: &str = "MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0=";

/// How long a Lightning or Spark send waits for the payment to settle
/// before it returns as pending.
const SEND_WAIT_SECS: u32 = 30;

/// The SDK configuration every device uses: the given network, OpenAgents'
/// API key, and no real-time sync server.
pub fn sdk_config(network: Network) -> breez_sdk_spark::Config {
    let mut config = default_config(network);
    config.api_key = Some(BREEZ_API_KEY.to_owned());
    // Encrypted sync to Breez's server is for several devices on one
    // wallet, which the app does not offer.
    config.real_time_sync_server_url = None;
    // Claim on-chain deposits (a MoonPay purchase, or bitcoin sent to the
    // deposit address) at maturity for up to the network's fastest
    // recommended rate plus 1 sat/vB, so ordinary deposits credit without a
    // tap. The default, 1 sat/vB, leaves them waiting whenever fees rise;
    // the screen offers a quoted claim for any deposit still waiting.
    config.max_deposit_claim_fee = Some(MaxFee::NetworkRecommended {
        leeway_sat_per_vbyte: 1,
    });
    config
}

/// The MoonPay page for buying `amount_sats` into `address`, built as Breez's
/// SDK builds it (`breez_sdk_common::buy::moonpay`, tag 0.26.0: the same
/// key, parameters, and order) but without the signature that Breez's server
/// no longer supplies. MoonPay checks a signature only when one is sent.
pub fn moonpay_url(address: &str, amount_sats: u64) -> Result<String, String> {
    let address = address.trim();
    if address.is_empty() || !address.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("no Bitcoin deposit address".into());
    }
    let btc = format!(
        "{}.{:08}",
        amount_sats / 100_000_000,
        amount_sats % 100_000_000
    );
    let mut url = url::Url::parse("https://buy.moonpay.io").map_err(|error| error.to_string())?;
    url.query_pairs_mut().extend_pairs([
        ("apiKey", "pk_live_Mx5g6bpD6Etd7T0bupthv7smoTNn2Vr"),
        ("currencyCode", "btc"),
        ("walletAddress", address),
        ("colorCode", "#055DEB"),
        ("theme", "light"),
        ("quoteCurrencyAmount", btc.as_str()),
        ("lockAmount", "true"),
        (
            "redirectURL",
            "https://buy.moonpay.io/transaction_receipt?addFunds=true",
        ),
    ]);
    Ok(url.into())
}

/// A prepared payment, kept here between its quote and its confirmation.
enum Prepared {
    Send(Box<PrepareSendPaymentResponse>, Option<SendPaymentOptions>),
    Lnurl(Box<PrepareLnurlPayResponse>),
}

/// The running SDK and the runtime that drives it. Dropping it disconnects.
pub struct SparkNode {
    runtime: tokio::runtime::Runtime,
    sdk: BreezSdk,
    prepared: Mutex<HashMap<u64, Prepared>>,
    next_quote: Mutex<u64>,
}

impl SparkNode {
    /// Start the SDK with a mnemonic, keeping its records in `storage`.
    /// Blocking; it reaches the Spark operators.
    pub fn open(
        storage: Arc<dyn StorageBackend>,
        network: Network,
        mnemonic: &str,
    ) -> Result<Self, String> {
        // Two rustls providers are linked, so name one, as the app does. A
        // second install is refused harmlessly.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("openagents-spark")
            .enable_all()
            .build()
            .map_err(|_| "The wallet could not start its worker.".to_string())?;
        let seed = Seed::Mnemonic {
            mnemonic: mnemonic.to_owned(),
            passphrase: None,
        };
        let sdk = runtime
            .block_on(
                SdkBuilder::new(sdk_config(network), seed)
                    .with_storage_backend(storage)
                    .build(),
            )
            .map_err(|error| describe("start", &error.to_string()))?;
        Ok(Self {
            runtime,
            sdk,
            prepared: Mutex::new(HashMap::new()),
            next_quote: Mutex::new(1),
        })
    }

    fn keep(&self, prepared: Prepared) -> u64 {
        let mut next = self
            .next_quote
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let id = *next;
        *next += 1;
        let mut kept = self
            .prepared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        // Only the latest quote can be confirmed.
        kept.clear();
        kept.insert(id, prepared);
        id
    }

    fn quote_input(&self, request: &SendRequest) -> Result<Quote, QuoteFailure> {
        let input = request.input.as_str();
        let parsed = self
            .runtime
            .block_on(self.sdk.parse(input))
            .map_err(|_| QuoteFailure::Refused(unparsed(input)))?;
        let parsed = match parsed {
            // A BIP21 URI pays through the best method it lists.
            InputType::Bip21(details) => {
                let amount = request.amount_sats.or(details.amount_sat);
                let method = details
                    .payment_methods
                    .into_iter()
                    .next()
                    .ok_or_else(|| QuoteFailure::Refused(UNREADABLE.into()))?;
                let request = SendRequest {
                    amount_sats: amount,
                    ..request.clone()
                };
                return self.quote_parsed(&request, method);
            }
            other => other,
        };
        self.quote_parsed(request, parsed)
    }

    fn quote_parsed(
        &self,
        request: &SendRequest,
        parsed: InputType,
    ) -> Result<Quote, QuoteFailure> {
        let (input, amount) = (request.input.as_str(), request.amount_sats);
        match parsed {
            InputType::LightningAddress(details) => {
                let label = details.address.clone();
                self.quote_lnurl(details.pay_request, label, request)
            }
            InputType::LnurlPay(pay_request) => {
                let label = pay_request.domain.clone();
                self.quote_lnurl(pay_request, label, request)
            }
            InputType::Bolt11Invoice(details) => {
                if details.amount_msat.is_none() && amount.is_none() {
                    return Err(QuoteFailure::NeedsAmount(Ask::amount(
                        "This invoice has no amount. Enter one.",
                    )));
                }
                let amount = if details.amount_msat.is_some() {
                    None
                } else {
                    amount
                };
                self.quote_send(input, amount, details.description)
            }
            InputType::SparkAddress(_) | InputType::BitcoinAddress(_) => {
                if amount.is_none() {
                    return Err(QuoteFailure::NeedsAmount(Ask::amount(
                        "Enter the amount to send to this address.",
                    )));
                }
                self.quote_send(input, amount, None)
            }
            InputType::SparkInvoice(details) => {
                let amount = if details.amount.is_some() {
                    None
                } else if amount.is_none() {
                    return Err(QuoteFailure::NeedsAmount(Ask::amount(
                        "This Spark invoice has no amount. Enter one.",
                    )));
                } else {
                    amount
                };
                self.quote_send(input, amount, details.description)
            }
            InputType::CrossChainAddress(_) => Err(QuoteFailure::Refused(
                "Sending to other chains isn't available in this app yet.".into(),
            )),
            InputType::LnurlWithdraw(_) => Err(QuoteFailure::Refused(
                "This code withdraws to a wallet; receiving it isn't available yet.".into(),
            )),
            _ => Err(QuoteFailure::Refused(UNREADABLE.into())),
        }
    }

    fn quote_send(
        &self,
        input: &str,
        amount: Option<u64>,
        note: Option<String>,
    ) -> Result<Quote, QuoteFailure> {
        let prepared = self
            .runtime
            .block_on(self.sdk.prepare_send_payment(PrepareSendPaymentRequest {
                payment_request: PaymentRequest::Input {
                    input: input.to_owned(),
                },
                amount: amount.map(u128::from),
                token_identifier: None,
                conversion_options: None,
                fee_policy: None,
            }))
            .map_err(|error| QuoteFailure::Refused(describe("quote", &error.to_string())))?;
        if prepared.token_identifier.is_some() || prepared.conversion_estimate.is_some() {
            return Err(QuoteFailure::Refused(
                "Token payments aren't available in this app yet.".into(),
            ));
        }
        let amount_sats = sats_of(prepared.amount)?;
        let mut speeds = vec![];
        let (destination, fee_sats, options) = match &prepared.payment_method {
            SendPaymentMethod::BitcoinAddress { address, fee_quote } => {
                // Every speed's fee is in the quote; medium until the person
                // chooses.
                let fee = |speed: &breez_sdk_spark::SendOnchainSpeedFeeQuote| {
                    speed.user_fee_sat + speed.l1_broadcast_fee_sat
                };
                speeds = vec![
                    (Speed::Slow, fee(&fee_quote.speed_slow)),
                    (Speed::Medium, fee(&fee_quote.speed_medium)),
                    (Speed::Fast, fee(&fee_quote.speed_fast)),
                ];
                (
                    Destination::Bitcoin(address.address.clone()),
                    fee(&fee_quote.speed_medium),
                    Some(SendPaymentOptions::BitcoinAddress {
                        confirmation_speed: OnchainConfirmationSpeed::Medium,
                    }),
                )
            }
            SendPaymentMethod::Bolt11Invoice {
                invoice_details,
                spark_transfer_fee_sats,
                lightning_fee_sats,
            } => {
                // A payee on Spark is paid directly, which costs less.
                let (fee, prefer_spark) = match spark_transfer_fee_sats {
                    Some(fee) if fee <= lightning_fee_sats => (*fee, true),
                    _ => (*lightning_fee_sats, false),
                };
                (
                    Destination::Lightning(invoice_details.invoice.bolt11.clone()),
                    fee,
                    Some(SendPaymentOptions::Bolt11Invoice {
                        prefer_spark,
                        completion_timeout_secs: Some(SEND_WAIT_SECS),
                    }),
                )
            }
            SendPaymentMethod::SparkAddress { address, fee, .. } => {
                (Destination::Spark(address.clone()), sats_of(*fee)?, None)
            }
            SendPaymentMethod::SparkInvoice { fee, .. } => {
                (Destination::Spark(input.to_owned()), sats_of(*fee)?, None)
            }
            SendPaymentMethod::CrossChainAddress { .. } => {
                return Err(QuoteFailure::Refused(
                    "Sending to other chains isn't available in this app yet.".into(),
                ));
            }
        };
        let id = self.keep(Prepared::Send(Box::new(prepared), options));
        Ok(Quote {
            id,
            destination,
            amount_sats,
            fee_sats,
            note: note.filter(|note| !note.trim().is_empty()),
            comment: None,
            speed: (!speeds.is_empty()).then_some(Speed::Medium),
            speeds,
        })
    }

    fn quote_lnurl(
        &self,
        pay_request: breez_sdk_spark::LnurlPayRequestDetails,
        label: String,
        request: &SendRequest,
    ) -> Result<Quote, QuoteFailure> {
        let terms = LnurlTerms::of(
            label.clone(),
            pay_request.min_sendable,
            pay_request.max_sendable,
            pay_request.comment_allowed,
            &pay_request.metadata_str,
        );
        let (amount, comment) = terms.check(
            request.amount_sats,
            request.comment.as_deref(),
            request.format,
        )?;
        let prepared = self
            .runtime
            .block_on(self.sdk.prepare_lnurl_pay(PrepareLnurlPayRequest {
                amount: u128::from(amount),
                pay_request,
                comment: comment.clone(),
                validate_success_action_url: None,
                token_identifier: None,
                conversion_options: None,
                fee_policy: None,
            }))
            .map_err(|error| QuoteFailure::Refused(describe("quote", &error.to_string())))?;
        if prepared.conversion_estimate.is_some() {
            return Err(QuoteFailure::Refused(
                "Token payments aren't available in this app yet.".into(),
            ));
        }
        let (amount_sats, fee_sats) = (prepared.amount_sats, prepared.fee_sats);
        let id = self.keep(Prepared::Lnurl(Box::new(prepared)));
        Ok(Quote {
            id,
            destination: Destination::LightningAddress(label),
            amount_sats,
            fee_sats,
            note: terms.description,
            comment,
            speeds: vec![],
            speed: None,
        })
    }
}

impl Drop for SparkNode {
    fn drop(&mut self) {
        let _ = self.runtime.block_on(self.sdk.disconnect());
    }
}

const UNREADABLE: &str = "That isn't a payment request this wallet can pay: paste a Lightning invoice, Lightning address, Spark address, or Bitcoin address.";

/// Why `input` could not be read: for something shaped like a Lightning
/// address (`name@domain`), that its domain did not answer, since reading
/// one asks that domain for an invoice; otherwise [`UNREADABLE`].
fn unparsed(input: &str) -> String {
    match lightning_address_domain(input) {
        Some(domain) => format!(
            "Couldn't reach {domain} to get an invoice for that Lightning address. Check the address, or try again in a minute."
        ),
        None => UNREADABLE.into(),
    }
}

/// The domain of `input` when it is shaped like a Lightning address.
fn lightning_address_domain(input: &str) -> Option<&str> {
    let input = input.trim();
    let input = input
        .strip_prefix("lightning:")
        .or_else(|| input.strip_prefix("LIGHTNING:"))
        .unwrap_or(input);
    let (name, domain) = input.split_once('@')?;
    let plain = |text: &str| {
        !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
    };
    (plain(name) && plain(domain) && domain.contains('.') && !domain.starts_with('.'))
        .then_some(domain)
}

fn sats_of(amount: u128) -> Result<u64, QuoteFailure> {
    u64::try_from(amount).map_err(|_| QuoteFailure::Refused("The amount is too large.".into()))
}

/// An SDK error for the screen. Its text names no key material.
fn describe(step: &str, detail: &str) -> String {
    let detail = detail.trim();
    let lower = detail.to_ascii_lowercase();
    if lower.contains("insufficient") {
        return "The wallet doesn't hold enough to pay this and its fee.".into();
    }
    match step {
        "start" => format!(
            "The wallet could not reach Spark ({detail}). Check the connection and try again."
        ),
        "quote" => format!("This payment could not be prepared ({detail})."),
        "pay" => format!("The payment did not go through ({detail})."),
        "buy" => format!("The purchase could not start ({detail})."),
        "claim" => format!("The deposit could not be claimed ({detail})."),
        "refund" => format!("The deposit could not be refunded ({detail})."),
        "contact" => format!("The contact could not be saved ({detail})."),
        "fees" => format!("The network's fee rates could not be read ({detail})."),
        _ => format!("Spark could not be read ({detail}). Refresh to try again."),
    }
}

/// Calls `notify` when the SDK reports a sync or a payment.
struct Listener(Arc<dyn Fn() + Send + Sync>);

#[async_trait::async_trait]
impl EventListener for Listener {
    async fn on_event(&self, event: SdkEvent) {
        if matches!(
            event,
            SdkEvent::Synced
                | SdkEvent::PaymentSucceeded { .. }
                | SdkEvent::PaymentPending { .. }
                | SdkEvent::PaymentFailed { .. }
                | SdkEvent::ClaimedDeposits { .. }
                | SdkEvent::UnclaimedDeposits { .. }
                | SdkEvent::NewDeposits { .. }
                | SdkEvent::UnilateralExitStateChanged
        ) {
            (self.0)();
        }
    }
}

impl Node for SparkNode {
    fn balance(&self) -> Result<u64, String> {
        self.runtime
            .block_on(self.sdk.get_info(GetInfoRequest {
                ensure_synced: Some(false),
            }))
            .map(|info| info.balance_sats)
            .map_err(|error| describe("read", &error.to_string()))
    }

    fn sync(&self) -> Result<(), String> {
        self.runtime
            .block_on(self.sdk.sync_wallet(SyncWalletRequest {}))
            .map(|_| ())
            .map_err(|error| describe("read", &error.to_string()))
    }

    fn spark_address(&self) -> Result<String, String> {
        self.receive(ReceivePaymentMethod::SparkAddress)
    }

    fn bitcoin_address(&self) -> Result<String, String> {
        self.receive(ReceivePaymentMethod::BitcoinAddress { new_address: None })
    }

    fn invoice(&self, amount_sats: Option<u64>, description: &str) -> Result<String, String> {
        self.receive(ReceivePaymentMethod::Bolt11Invoice {
            description: description.to_owned(),
            amount_sats,
            expiry_secs: None,
            payment_hash: None,
            receiver_identity_public_key: None,
        })
    }

    fn quote(&self, request: &SendRequest) -> Result<Quote, QuoteFailure> {
        self.quote_input(request)
    }

    fn pay(&self, quote: u64, idempotency_key: &str) -> Result<Paid, PayFailure> {
        let prepared = self
            .prepared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&quote)
            .ok_or_else(|| {
                PayFailure::NotSent("That quote has expired. Review the payment again.".into())
            })?;
        let payment = match prepared {
            Prepared::Send(prepare_response, options) => self
                .runtime
                .block_on(self.sdk.send_payment(SendPaymentRequest {
                    prepare_response: *prepare_response,
                    options,
                    idempotency_key: Some(idempotency_key.to_owned()),
                }))
                .map(|response| (response.payment, None)),
            Prepared::Lnurl(prepare_response) => self
                .runtime
                .block_on(self.sdk.lnurl_pay(LnurlPayRequest {
                    prepare_response: *prepare_response,
                    idempotency_key: Some(idempotency_key.to_owned()),
                }))
                .map(|response| (response.payment, response.success_action)),
        };
        payment
            .map(|(payment, success)| Paid {
                row: row(&payment),
                message: success.and_then(|action| success_message(&action)),
            })
            .map_err(|error| pay_failure(&error))
    }

    fn payments(&self, limit: u32) -> Result<Vec<PaymentRow>, String> {
        self.runtime
            .block_on(self.sdk.list_payments(ListPaymentsRequest {
                limit: Some(limit),
                ..ListPaymentsRequest::default()
            }))
            .map(|response| response.payments.iter().map(row).collect())
            .map_err(|error| describe("read", &error.to_string()))
    }

    fn buy(&self, provider: Provider, amount_sats: u64) -> Result<String, String> {
        let request = match provider {
            Provider::Moonpay => BuyBitcoinRequest::Moonpay {
                locked_amount_sat: Some(amount_sats),
                redirect_url: None,
            },
            Provider::CashApp => BuyBitcoinRequest::CashApp { amount_sats },
        };
        match self.runtime.block_on(self.sdk.buy_bitcoin(request)) {
            Ok(response) => Ok(response.url),
            // The SDK has Breez's server sign the MoonPay URL, and Breez's
            // production server answers that call `Unimplemented`
            // (2026-09-28, #9865). MoonPay opens the same URL unsigned for
            // this key, so build it for the same deposit address.
            Err(_) if provider == Provider::Moonpay => self
                .bitcoin_address()
                .and_then(|address| moonpay_url(&address, amount_sats))
                .map_err(|error| describe("buy", &error)),
            Err(error) => Err(describe("buy", &error.to_string())),
        }
    }

    fn deposits(&self) -> Result<Vec<DepositRow>, String> {
        let deposits = self
            .runtime
            .block_on(
                self.sdk
                    .list_unclaimed_deposits(ListUnclaimedDepositsRequest {}),
            )
            .map_err(|error| describe("read", &error.to_string()))?
            .deposits;
        Ok(deposits
            .into_iter()
            .map(|deposit| DepositRow {
                problem: deposit.claim_error.as_ref().map(|error| match error {
                    DepositClaimError::MaxDepositClaimFeeExceeded {
                        required_fee_sats, ..
                    } => DepositProblem::FeeAboveLimit(*required_fee_sats),
                    DepositClaimError::MissingUtxo { .. } => DepositProblem::Missing,
                    DepositClaimError::Generic { message } => {
                        DepositProblem::Failed(message.clone())
                    }
                }),
                txid: deposit.txid,
                vout: deposit.vout,
                amount_sats: deposit.amount_sats,
                mature: deposit.is_mature,
                refund_txid: deposit.refund_tx_id,
            })
            .collect())
    }

    fn claim_quote(&self, txid: &str, vout: u32) -> Result<ClaimQuote, String> {
        let quote = self
            .runtime
            .block_on(
                self.sdk
                    .fetch_claim_deposit_quote(FetchClaimDepositQuoteRequest {
                        txid: txid.to_owned(),
                        vout,
                    }),
            )
            .map_err(|error| describe("claim", &error.to_string()))?;
        let (chosen, early) = match quote.instant {
            Some(instant) => (instant, true),
            None => (quote.mature, false),
        };
        Ok(ClaimQuote {
            fee_sats: chosen.fee_sats,
            credit_sats: chosen.credit_amount_sats,
            early,
            confirmations: quote.confirmations,
            confirmations_required: chosen.confirmations_required,
        })
    }

    fn claim(&self, txid: &str, vout: u32, max_fee_sats: u64) -> Result<String, String> {
        let response = self
            .runtime
            .block_on(self.sdk.claim_deposit(ClaimDepositRequest {
                txid: txid.to_owned(),
                vout,
                max_fee: Some(MaxFee::Fixed {
                    amount: max_fee_sats,
                }),
            }))
            .map_err(|error| describe("claim", &error.to_string()))?;
        Ok(match response.outcome {
            ClaimDepositOutcome::Settled { .. } => "Claimed. It's in your balance.".into(),
            ClaimDepositOutcome::Submitted => {
                "Claim submitted. It reaches your balance shortly.".into()
            }
            ClaimDepositOutcome::Deferred { .. } => {
                "Not claimable yet. The wallet claims it on its own as it confirms.".into()
            }
        })
    }

    fn fee_rates(&self) -> Result<FeeRates, String> {
        self.runtime
            .block_on(self.sdk.recommended_fees())
            .map(|fees| FeeRates {
                fastest: fees.fastest_fee,
                half_hour: fees.half_hour_fee,
                hour: fees.hour_fee,
            })
            .map_err(|error| describe("fees", &error.to_string()))
    }

    fn refund(
        &self,
        txid: &str,
        vout: u32,
        address: &str,
        sat_per_vbyte: u64,
    ) -> Result<String, String> {
        self.runtime
            .block_on(self.sdk.refund_deposit(RefundDepositRequest {
                txid: txid.to_owned(),
                vout,
                destination_address: address.to_owned(),
                fee: Fee::Rate { sat_per_vbyte },
            }))
            .map(|response| response.tx_id)
            .map_err(|error| describe("refund", &error.to_string()))
    }

    fn set_speed(&self, quote: u64, speed: Speed) -> Result<u64, String> {
        let mut kept = self
            .prepared
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let Some(Prepared::Send(prepared, options)) = kept.get_mut(&quote) else {
            return Err("That quote has expired. Review the payment again.".into());
        };
        let SendPaymentMethod::BitcoinAddress { fee_quote, .. } = &prepared.payment_method else {
            return Err("Only a Bitcoin address payment has speeds.".into());
        };
        let (chosen, confirmation_speed) = match speed {
            Speed::Slow => (&fee_quote.speed_slow, OnchainConfirmationSpeed::Slow),
            Speed::Medium => (&fee_quote.speed_medium, OnchainConfirmationSpeed::Medium),
            Speed::Fast => (&fee_quote.speed_fast, OnchainConfirmationSpeed::Fast),
        };
        let fee = chosen.user_fee_sat + chosen.l1_broadcast_fee_sat;
        *options = Some(SendPaymentOptions::BitcoinAddress { confirmation_speed });
        Ok(fee)
    }

    fn contacts(&self) -> Result<Vec<Contact>, String> {
        self.runtime
            .block_on(self.sdk.list_contacts(ListContactsRequest {
                offset: None,
                limit: Some(100),
            }))
            .map(|contacts| {
                contacts
                    .into_iter()
                    .map(|contact| Contact {
                        name: contact.name,
                        address: contact.payment_identifier,
                    })
                    .collect()
            })
            .map_err(|error| describe("read", &error.to_string()))
    }

    fn add_contact(&self, name: &str, address: &str) -> Result<(), String> {
        self.runtime
            .block_on(self.sdk.add_contact(AddContactRequest {
                name: name.to_owned(),
                payment_identifier: address.to_owned(),
            }))
            .map(|_| ())
            .map_err(|error| describe("contact", &error.to_string()))
    }

    fn exit_state(&self) -> Result<String, String> {
        self.runtime
            .block_on(self.sdk.export_unilateral_exit_state())
            .map(|response| response.exit_state)
            .map_err(|error| describe("read", &error.to_string()))
    }

    fn subscribe(&self, notify: Arc<dyn Fn() + Send + Sync>) {
        self.runtime
            .block_on(self.sdk.add_event_listener(Box::new(Listener(notify))));
    }

    fn invoice_fee(&self, invoice: &str) -> Result<u64, String> {
        self.prepare_invoice(invoice)
            .map(|(_, _, fee)| fee)
            .map_err(|failure| match failure {
                AgentPayFailure::InsufficientFunds => {
                    "The wallet doesn't hold enough to pay this and its fee.".into()
                }
                AgentPayFailure::FeeTooHigh(_) => "The fee is above the ceiling.".into(),
                AgentPayFailure::Failed(message) | AgentPayFailure::Unknown(message) => message,
            })
    }

    fn pay_invoice(
        &self,
        invoice: &str,
        max_fee_sats: u64,
        idempotency_key: &str,
    ) -> Result<InvoicePayment, AgentPayFailure> {
        let (prepared, options, fee) = self.prepare_invoice(invoice)?;
        if fee > max_fee_sats {
            return Err(AgentPayFailure::FeeTooHigh(fee));
        }
        let payment = self
            .runtime
            .block_on(self.sdk.send_payment(SendPaymentRequest {
                prepare_response: prepared,
                options: Some(options),
                idempotency_key: Some(idempotency_key.to_owned()),
            }))
            .map(|response| response.payment)
            .map_err(|error| agent_pay_failure(&error))?;
        let preimage = match &payment.details {
            Some(PaymentDetails::Lightning { htlc_details, .. }) => htlc_details.preimage.clone(),
            Some(PaymentDetails::Spark {
                htlc_details: Some(htlc_details),
                ..
            }) => htlc_details.preimage.clone(),
            _ => None,
        };
        Ok(InvoicePayment {
            row: row(&payment),
            preimage: preimage.map(|p| p.to_ascii_lowercase()),
        })
    }
}

/// Whether an SDK error from a send leaves the payment's outcome unknown.
///
/// Only the variants the SDK raises while checking the request, before it
/// hands anything to Spark or Lightning, are definite refusals. A network,
/// storage, Spark, chain, LNURL, signer, or generic error can arrive after
/// the transfer left, so it may have paid. Unrecognized variants count as
/// unknown: a payment wrongly called failed can be paid twice.
fn send_outcome_unknown(error: &SdkError) -> bool {
    !matches!(
        error,
        SdkError::InsufficientFunds { .. }
            | SdkError::InvalidUuid(_)
            | SdkError::InvalidInput(_)
            | SdkError::CrossChainAmountOutOfRange { .. }
            | SdkError::CrossChainRouteUnavailable { .. }
            | SdkError::MaxDepositClaimFeeExceeded { .. }
            | SdkError::MissingUtxo { .. }
            | SdkError::DepositClaimInProgress { .. }
            | SdkError::RefundReplacementFeeTooLow { .. }
            | SdkError::OptimizationAlreadyRunning
            | SdkError::InsufficientCpfpFunds { .. }
    )
}

/// The words for a send whose outcome is unknown. Its text names no key
/// material.
fn unknown_outcome(detail: &str) -> String {
    format!(
        "The wallet lost track of this payment while sending it ({}), so it may have gone through. Check the payment history before paying again.",
        detail.trim()
    )
}

/// An SDK failure while paying a quote.
fn pay_failure(error: &SdkError) -> PayFailure {
    if matches!(error, SdkError::InsufficientFunds { .. }) {
        PayFailure::NotSent("The wallet doesn't hold enough to pay this and its fee.".into())
    } else if send_outcome_unknown(error) {
        PayFailure::Unknown(unknown_outcome(&error.to_string()))
    } else {
        PayFailure::NotSent(format!(
            "The payment did not go through ({}).",
            error.to_string().trim()
        ))
    }
}

/// An SDK failure while sending an agent's invoice.
fn agent_pay_failure(error: &SdkError) -> AgentPayFailure {
    if matches!(error, SdkError::InsufficientFunds { .. }) {
        return AgentPayFailure::InsufficientFunds;
    }
    match pay_failure(error) {
        PayFailure::Unknown(message) => AgentPayFailure::Unknown(message),
        PayFailure::NotSent(message) => AgentPayFailure::Failed(message),
    }
}

/// An SDK failure while preparing an agent's invoice. Preparing sends
/// nothing, so every failure here is definite.
fn prepare_failure(error: &SdkError) -> AgentPayFailure {
    if matches!(error, SdkError::InsufficientFunds { .. }) {
        AgentPayFailure::InsufficientFunds
    } else {
        AgentPayFailure::Failed(describe("pay", &error.to_string()))
    }
}

impl SparkNode {
    /// Prepare an agent's BOLT11 payment: the prepared send, its options,
    /// and its fee in sats. A payee on Spark is paid directly when cheaper.
    fn prepare_invoice(
        &self,
        invoice: &str,
    ) -> Result<(PrepareSendPaymentResponse, SendPaymentOptions, u64), AgentPayFailure> {
        let prepared = self
            .runtime
            .block_on(self.sdk.prepare_send_payment(PrepareSendPaymentRequest {
                payment_request: PaymentRequest::Input {
                    input: invoice.to_owned(),
                },
                amount: None,
                token_identifier: None,
                conversion_options: None,
                fee_policy: None,
            }))
            .map_err(|error| prepare_failure(&error))?;
        let SendPaymentMethod::Bolt11Invoice {
            spark_transfer_fee_sats,
            lightning_fee_sats,
            ..
        } = &prepared.payment_method
        else {
            return Err(AgentPayFailure::Failed(
                "The request isn't a Lightning invoice.".into(),
            ));
        };
        if prepared.token_identifier.is_some() || prepared.conversion_estimate.is_some() {
            return Err(AgentPayFailure::Failed(
                "Token payments aren't available in this app yet.".into(),
            ));
        }
        let (fee, prefer_spark) = match spark_transfer_fee_sats {
            Some(fee) if fee <= lightning_fee_sats => (*fee, true),
            _ => (*lightning_fee_sats, false),
        };
        let options = SendPaymentOptions::Bolt11Invoice {
            prefer_spark,
            completion_timeout_secs: Some(SEND_WAIT_SECS),
        };
        Ok((prepared, options, fee))
    }

    fn receive(&self, payment_method: ReceivePaymentMethod) -> Result<String, String> {
        self.runtime
            .block_on(
                self.sdk
                    .receive_payment(ReceivePaymentRequest { payment_method }),
            )
            .map(|response| response.payment_request)
            .map_err(|error| describe("read", &error.to_string()))
    }
}

/// What an LNURL recipient says after a payment, as plain text. A URL is
/// shown, never opened.
fn success_message(action: &SuccessActionProcessed) -> Option<String> {
    let text = match action {
        SuccessActionProcessed::Message { data } => data.message.clone(),
        SuccessActionProcessed::Url { data } => format!("{} {}", data.description, data.url),
        SuccessActionProcessed::Aes { result } => match result {
            breez_sdk_spark::AesSuccessActionDataResult::Decrypted { data } => {
                format!("{} {}", data.description, data.plaintext)
            }
            breez_sdk_spark::AesSuccessActionDataResult::ErrorStatus { .. } => {
                "The recipient's message could not be decrypted.".to_owned()
            }
        },
    };
    crate::model::plain_text(&text, 300)
}

/// A payment as the history shows it.
fn row(payment: &Payment) -> PaymentRow {
    PaymentRow {
        id: payment.id.clone(),
        received: payment.payment_type == PaymentType::Receive,
        amount_sats: u64::try_from(payment.amount).unwrap_or(u64::MAX),
        fee_sats: u64::try_from(payment.fees).unwrap_or(u64::MAX),
        method: match payment.method {
            PaymentMethod::Lightning => "Lightning",
            PaymentMethod::Spark => "Spark",
            PaymentMethod::Token => "Token",
            PaymentMethod::Deposit => "Bitcoin deposit",
            PaymentMethod::Withdraw => "Bitcoin withdrawal",
            PaymentMethod::Unknown => "Payment",
        }
        .to_owned(),
        status: match payment.status {
            PaymentStatus::Completed => "completed",
            PaymentStatus::Pending => "pending",
            PaymentStatus::Failed => "failed",
        }
        .to_owned(),
        at: payment.timestamp,
    }
}

#[cfg(test)]
mod unparsed_tests {
    use super::*;

    #[test]
    fn an_unreachable_lightning_address_names_its_domain() {
        assert_eq!(
            lightning_address_domain("someone@example.invalid"),
            Some("example.invalid")
        );
        assert_eq!(
            lightning_address_domain("lightning:a.b@pay.example.com"),
            Some("pay.example.com")
        );
        assert_eq!(lightning_address_domain("lnbc10u1pjexample"), None);
        assert_eq!(lightning_address_domain("a@localhost"), None);
        assert_eq!(lightning_address_domain("a b@example.com"), None);
        assert!(unparsed("someone@example.invalid").starts_with("Couldn't reach example.invalid"));
        assert_eq!(unparsed("garbage"), UNREADABLE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_variant() -> Vec<(SdkError, bool)> {
        vec![
            (SdkError::SparkError("transfer rejected".into()), true),
            (
                SdkError::InsufficientFunds {
                    token_identifier: None,
                },
                false,
            ),
            (SdkError::InvalidUuid("x".into()), false),
            (SdkError::InvalidInput("x".into()), false),
            (
                SdkError::CrossChainAmountOutOfRange {
                    reason: "x".into(),
                    too_small: true,
                    bound_amount: None,
                    bound_usd_cents: None,
                },
                false,
            ),
            (
                SdkError::CrossChainRouteUnavailable {
                    reason: "x".into(),
                    temporary: true,
                },
                false,
            ),
            (SdkError::NetworkError("connection reset".into()), true),
            (SdkError::StorageError("disk full".into()), true),
            (SdkError::ChainServiceError("x".into()), true),
            (
                SdkError::MaxDepositClaimFeeExceeded {
                    tx: "t".into(),
                    vout: 0,
                    max_fee: None,
                    required_fee_sats: 1,
                    required_fee_rate_sat_per_vbyte: 1,
                },
                false,
            ),
            (
                SdkError::MissingUtxo {
                    tx: "t".into(),
                    vout: 0,
                },
                false,
            ),
            (
                SdkError::DepositClaimInProgress {
                    tx: "t".into(),
                    vout: 0,
                },
                false,
            ),
            (
                SdkError::RefundReplacementFeeTooLow {
                    pending_fee_sats: 1,
                    required_fee_sats: 2,
                },
                false,
            ),
            (SdkError::LnurlError("x".into()), true),
            (SdkError::Signer("x".into()), true),
            (SdkError::OptimizationAlreadyRunning, false),
            (SdkError::OptimizationCancelled, true),
            (SdkError::InsufficientCpfpFunds { required_sat: 1 }, false),
            (SdkError::Generic("timeout".into()), true),
        ]
    }

    #[test]
    fn a_send_error_is_unknown_unless_the_sdk_refused_before_sending() {
        for (error, unknown) in every_variant() {
            let failure = pay_failure(&error);
            assert_eq!(failure.outcome_unknown(), unknown, "{error:?}");
            if unknown {
                assert!(
                    failure.message().contains("may have gone through"),
                    "{error:?}"
                );
                assert!(!failure.message().contains("did not go through"));
                assert!(matches!(
                    agent_pay_failure(&error),
                    AgentPayFailure::Unknown(_)
                ));
            } else {
                assert!(!failure.message().contains("may have gone through"));
                assert!(!matches!(
                    agent_pay_failure(&error),
                    AgentPayFailure::Unknown(_)
                ));
            }
        }
    }

    #[test]
    fn insufficient_funds_comes_from_the_variant_not_the_text() {
        let short = SdkError::InsufficientFunds {
            token_identifier: None,
        };
        assert_eq!(
            agent_pay_failure(&short),
            AgentPayFailure::InsufficientFunds
        );
        assert_eq!(prepare_failure(&short), AgentPayFailure::InsufficientFunds);
        // A network error that merely mentions the word is not a shortfall,
        // and may have paid.
        let network = SdkError::NetworkError("insufficient bandwidth".into());
        assert!(matches!(
            agent_pay_failure(&network),
            AgentPayFailure::Unknown(_)
        ));
        assert!(pay_failure(&network).outcome_unknown());
    }

    #[test]
    fn preparing_never_reports_an_unknown_outcome() {
        for (error, _) in every_variant() {
            assert!(!matches!(
                prepare_failure(&error),
                AgentPayFailure::Unknown(_)
            ));
        }
    }
}

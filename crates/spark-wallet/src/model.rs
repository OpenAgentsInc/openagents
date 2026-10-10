//! What a wallet screen or command sees of a running Spark wallet: the
//! [`Node`] trait and the values it returns. The phone's Wallet tab
//! (`crates/openagents-mobile`) and `openagents wallet` on computers share
//! them.

use bitcoin_amount::Format;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// What a wallet screen or command needs from a running wallet.
/// [`crate::SparkNode`] is the real one; tests supply their own. Every call
/// blocks.
pub trait Node: Send + Sync {
    /// The balance in base units, as last synced.
    fn balance(&self) -> Result<u64, String>;
    fn sync(&self) -> Result<(), String>;
    /// The wallet's static Spark address.
    fn spark_address(&self) -> Result<String, String>;
    /// The wallet's static Bitcoin deposit address.
    fn bitcoin_address(&self) -> Result<String, String>;
    /// A new Lightning invoice, with an amount or without one.
    fn invoice(&self, amount_sats: Option<u64>, description: &str) -> Result<String, String>;
    /// Prepare a payment and quote its fee. The node keeps only the latest
    /// quote.
    fn quote(&self, request: &SendRequest) -> Result<Quote, QuoteFailure>;
    /// Pay a quote once; a repeat with the same key returns the same payment.
    /// A [`PayFailure::Unknown`] may have paid: retry it only with the same
    /// key, never a fresh one.
    fn pay(&self, quote: u64, idempotency_key: &str) -> Result<Paid, PayFailure>;
    /// Recent payments, newest first.
    fn payments(&self, limit: u32) -> Result<Vec<PaymentRow>, String>;
    /// Start a purchase with a provider; the URL for the person to open.
    fn buy(&self, provider: Provider, amount_sats: u64) -> Result<String, String>;
    /// On-chain deposits not yet claimed into the balance.
    fn deposits(&self) -> Result<Vec<DepositRow>, String>;
    /// What claiming a deposit now would cost.
    fn claim_quote(&self, txid: &str, vout: u32) -> Result<ClaimQuote, String>;
    /// Claim a deposit for at most `max_fee_sats` base units; what happened,
    /// in words.
    fn claim(&self, txid: &str, vout: u32, max_fee_sats: u64) -> Result<String, String>;
    /// The network's recommended on-chain fee rates.
    fn fee_rates(&self) -> Result<FeeRates, String>;
    /// Send a deposit back on-chain to `address` at `sat_per_vbyte`; the
    /// refund transaction's ID.
    fn refund(
        &self,
        txid: &str,
        vout: u32,
        address: &str,
        sat_per_vbyte: u64,
    ) -> Result<String, String>;
    /// Choose how fast a quoted on-chain withdrawal confirms; its fee.
    fn set_speed(&self, quote: u64, speed: Speed) -> Result<u64, String>;
    /// Saved contacts: Lightning addresses with names, from the SDK's own
    /// contact list.
    fn contacts(&self) -> Result<Vec<Contact>, String>;
    /// Save a Lightning address as a contact.
    fn add_contact(&self, name: &str, address: &str) -> Result<(), String>;
    /// The unilateral-exit state (Breez's `export_unilateral_exit_state`):
    /// what lets the recovery words take the balance out on-chain while
    /// Spark's operators are down. It holds no keys. Read locally.
    fn exit_state(&self) -> Result<String, String>;
    /// Call `notify` when the wallet syncs, a payment changes, or a deposit
    /// arrives.
    fn subscribe(&self, notify: Arc<dyn Fn() + Send + Sync>);
    /// Quote the fee, in sats, to pay a BOLT11 invoice now, keeping no quote
    /// (an agent's payment request; see `crate::spend`).
    fn invoice_fee(&self, invoice: &str) -> Result<u64, String> {
        let _ = invoice;
        Err("This wallet can't pay an agent's request.".into())
    }
    /// Pay a BOLT11 invoice for at most `max_fee_sats`, once per
    /// `idempotency_key` (a UUID): a repeat returns the same payment.
    fn pay_invoice(
        &self,
        invoice: &str,
        max_fee_sats: u64,
        idempotency_key: &str,
    ) -> Result<InvoicePayment, AgentPayFailure> {
        let _ = (invoice, max_fee_sats, idempotency_key);
        Err(AgentPayFailure::Failed(
            "This wallet can't pay an agent's request.".into(),
        ))
    }
}

/// An invoice the wallet paid for an agent: the payment and, once the
/// payee released it, its preimage (lowercase hex).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvoicePayment {
    pub row: PaymentRow,
    pub preimage: Option<String>,
}

/// Why the wallet did not pay an agent's invoice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentPayFailure {
    /// The quoted fee, in sats, is above the ceiling.
    FeeTooHigh(u64),
    InsufficientFunds,
    /// The wallet refused before sending: nothing was paid.
    Failed(String),
    /// The send started and the wallet lost track of it (a network or
    /// storage error): it may have gone through. Retry only with the same
    /// idempotency key.
    Unknown(String),
}

/// Why a payment of a quote did not complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PayFailure {
    /// The wallet refused before anything was sent: nothing was paid.
    NotSent(String),
    /// The send started and the wallet lost track of it (a network or
    /// storage error): it may have gone through. Retry only with the same
    /// idempotency key; the payment history says what happened.
    Unknown(String),
}

impl PayFailure {
    /// The failure in words for the person.
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::NotSent(message) | Self::Unknown(message) => message,
        }
    }

    /// Whether the payment may have gone through.
    #[must_use]
    pub fn outcome_unknown(&self) -> bool {
        matches!(self, Self::Unknown(_))
    }
}

impl std::fmt::Display for PayFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

/// Who sells bitcoin for dollars. Both are Breez integrations on mainnet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// Card or Apple Pay; MoonPay sends bitcoin on-chain to the deposit
    /// address, which the wallet claims.
    Moonpay,
    /// A fixed-amount Lightning invoice that Cash App pays from the
    /// person's cash balance or debit card.
    CashApp,
}

impl Provider {
    /// `moonpay` or `cashapp`.
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "moonpay" => Some(Self::Moonpay),
            "cashapp" => Some(Self::CashApp),
            _ => None,
        }
    }
}

/// An on-chain deposit waiting to be claimed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepositRow {
    pub txid: String,
    pub vout: u32,
    pub amount_sats: u64,
    /// It has the confirmations a claim at maturity needs.
    pub mature: bool,
    /// Why the last claim failed.
    pub problem: Option<DepositProblem>,
    /// A refund of it was broadcast in this transaction.
    #[serde(default)]
    pub refund_txid: Option<String>,
}

/// Why the SDK's last automatic claim of a deposit failed. The screen words
/// it in the person's amount format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DepositProblem {
    /// Claiming costs this many base units, above the automatic limit.
    FeeAboveLimit(u64),
    /// The deposit wasn't found on the chain.
    Missing,
    /// Another failure, in the SDK's words.
    Failed(String),
}

/// A saved contact: a name and a Lightning address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contact {
    pub name: String,
    pub address: String,
}

/// Recommended on-chain fee rates, in sat/vB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeRates {
    pub fastest: u64,
    pub half_hour: u64,
    pub hour: u64,
}

impl FeeRates {
    /// The rate for `speed`, at least 1 sat/vB.
    pub fn rate(self, speed: Speed) -> u64 {
        match speed {
            Speed::Fast => self.fastest,
            Speed::Medium => self.half_hour,
            Speed::Slow => self.hour,
        }
        .max(1)
    }
}

/// How fast an on-chain transaction should confirm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    Slow,
    Medium,
    Fast,
}

impl Speed {
    pub const ALL: [Self; 3] = [Self::Slow, Self::Medium, Self::Fast];

    pub fn id(self) -> &'static str {
        match self {
            Self::Slow => "slow",
            Self::Medium => "medium",
            Self::Fast => "fast",
        }
    }

    /// `slow`, `medium`, or `fast`.
    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|speed| speed.id() == id)
    }

    /// The speed and about how long it takes, for a screen.
    pub fn label(self) -> &'static str {
        match self {
            Self::Slow => "Slow · about an hour or more",
            Self::Medium => "Medium · about half an hour",
            Self::Fast => "Fast · the next block or two",
        }
    }
}

/// The cost of claiming a deposit now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClaimQuote {
    pub fee_sats: u64,
    pub credit_sats: u64,
    /// Claimed ahead of maturity, for a provider's spread.
    pub early: bool,
    pub confirmations: u32,
    pub confirmations_required: u32,
}

/// Where a payment goes, as its request decoded it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    Lightning(String),
    LightningAddress(String),
    Spark(String),
    Bitcoin(String),
}

/// What the person asked to pay: a request as pasted or scanned, the amount
/// they typed when the request carries none (base units), a comment for a
/// recipient that takes one, and the format to word amounts in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SendRequest {
    pub input: String,
    pub amount_sats: Option<u64>,
    pub comment: Option<String>,
    pub format: Format,
}

/// A prepared payment and its fee.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quote {
    pub id: u64,
    pub destination: Destination,
    pub amount_sats: u64,
    pub fee_sats: u64,
    /// The payment request's own description, or an LNURL recipient's.
    pub note: Option<String>,
    /// The comment sent to an LNURL recipient.
    pub comment: Option<String>,
    /// For an on-chain withdrawal: each speed and its fee, and the one
    /// chosen. Empty otherwise.
    pub speeds: Vec<(Speed, u64)>,
    pub speed: Option<Speed>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuoteFailure {
    /// The request has no amount, or the amount is out of its range.
    NeedsAmount(Ask),
    Refused(String),
}

/// What the screen asks for before a payment can be quoted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    pub message: String,
    /// Who is paid, as the request names them: "alice@example.com".
    pub recipient: Option<String>,
    /// The recipient's own description of the payment.
    pub description: Option<String>,
    /// The longest comment the recipient takes, in characters; 0 takes none.
    pub comment_max: u16,
}

impl Ask {
    /// Ask for an amount, and nothing else.
    pub fn amount(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            recipient: None,
            description: None,
            comment_max: 0,
        }
    }
}

/// A payment that went out, and what the recipient said about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paid {
    pub row: PaymentRow,
    /// An LNURL recipient's message after the payment, as plain text.
    pub message: Option<String>,
}

/// What an LNURL-pay recipient (a Lightning address or an `lnurl` code)
/// accepts, read from its pay request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LnurlTerms {
    /// The Lightning address, or the service's domain.
    pub recipient: String,
    /// Whole base units: the minimum rounded up, the maximum rounded down.
    /// LNURL states them in msat (LUD-06), which stays the wire unit.
    pub min_sats: u64,
    pub max_sats: u64,
    /// The longest comment it takes (LUD-12); 0 takes none.
    pub comment_max: u16,
    /// Its `text/plain` metadata (LUD-06), as plain text.
    pub description: Option<String>,
}

impl LnurlTerms {
    pub fn of(
        recipient: String,
        min_msat: u64,
        max_msat: u64,
        comment_max: u16,
        metadata: &str,
    ) -> Self {
        Self {
            recipient,
            min_sats: bitcoin_amount::from_msat_ceil(min_msat),
            max_sats: bitcoin_amount::from_msat_floor(max_msat),
            comment_max,
            description: lnurl_description(metadata),
        }
    }

    fn ask(&self, message: String) -> QuoteFailure {
        QuoteFailure::NeedsAmount(Ask {
            message,
            recipient: Some(self.recipient.clone()),
            description: self.description.clone(),
            comment_max: self.comment_max,
        })
    }

    /// The amount and comment to prepare, or what to ask the person. A
    /// recipient that takes one amount only is paid that amount. Amounts in
    /// what it asks are worded in `format`.
    pub fn check(
        &self,
        amount: Option<u64>,
        comment: Option<&str>,
        format: Format,
    ) -> Result<(u64, Option<String>), QuoteFailure> {
        if self.max_sats == 0 || self.min_sats > self.max_sats {
            return Err(QuoteFailure::Refused(format!(
                "{} isn't taking payments right now.",
                self.recipient
            )));
        }
        let range = if self.min_sats == self.max_sats {
            format!(
                "{} takes exactly {}.",
                self.recipient,
                format.show(self.min_sats)
            )
        } else {
            format!(
                "{} takes from {} to {}.",
                self.recipient,
                format.show(self.min_sats),
                format.show(self.max_sats)
            )
        };
        let amount = match amount {
            Some(amount) => amount,
            None if self.min_sats == self.max_sats => self.min_sats,
            None => return Err(self.ask(format!("Enter an amount. {range}"))),
        };
        if amount < self.min_sats || amount > self.max_sats {
            return Err(self.ask(range));
        }
        let comment = comment
            .map(str::trim)
            .filter(|comment| !comment.is_empty() && self.comment_max > 0);
        if let Some(comment) = comment
            && comment.chars().count() > usize::from(self.comment_max)
        {
            return Err(self.ask(format!(
                "{} takes a comment of up to {} characters.",
                self.recipient, self.comment_max
            )));
        }
        Ok((amount, comment.map(str::to_owned)))
    }
}

/// The `text/plain` entry of LNURL-pay metadata: a JSON array of
/// `[type, value]` pairs.
pub fn lnurl_description(metadata: &str) -> Option<String> {
    let entries: Vec<Vec<serde_json::Value>> = serde_json::from_str(metadata).ok()?;
    entries.iter().find_map(|entry| match entry.as_slice() {
        [kind, value] if kind.as_str() == Some("text/plain") => {
            value.as_str().and_then(|text| plain_text(text, 200))
        }
        _ => None,
    })
}

/// Text from a stranger, fit for one line of the screen: control characters
/// become spaces, runs of space collapse, and it stops at `limit` characters.
pub fn plain_text(text: &str, limit: usize) -> Option<String> {
    let cleaned: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        return None;
    }
    let mut chars = cleaned.chars();
    let head: String = chars.by_ref().take(limit).collect();
    Some(if chars.next().is_some() {
        format!("{head}…")
    } else {
        head
    })
}

/// A payment as the SDK reported it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentRow {
    pub id: String,
    pub received: bool,
    pub amount_sats: u64,
    pub fee_sats: u64,
    pub method: String,
    /// `completed`, `pending`, or `failed`.
    pub status: String,
    /// Unix seconds.
    pub at: u64,
}

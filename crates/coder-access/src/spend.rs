//! Agent spending, phase 1: an agent on a Coder host asks, and the owner
//! approves each payment on the phone (`mode: request`).
//!
//! Three formats, written up in `docs/breez/spend-protocol.md`:
//!
//! - [`Grant`] (`openagents.spend-grant.v1`): the phone's statement of what a
//!   host may *ask* it to pay. In `request` mode it moves nothing by itself:
//!   every payment still needs the owner's tap on the phone.
//! - [`SpendRequest`] (`openagents.spend-request.v1`): one payment a host
//!   asks for, under the grant it holds. Its ID is the idempotency key.
//! - [`Receipt`] (`openagents.spend-receipt.v1`): what the phone did with a
//!   request: paid (with the preimage), refused (with a [`Refusal`] code),
//!   pending, or unknown.
//!
//! They travel inside NIP-HOST requests the phone signs and encrypts to the
//! host (`spend.list` carries the grant, `spend.settle` the receipt), and in
//! the host's signed replies (`spend.list` returns the requests), so each is
//! authenticated by the envelope that carries it.
//!
//! [`Ledger`] is the phone's budget: it checks a request against the grant
//! and what the grant has already reserved or spent, reserves the amount
//! plus the fee ceiling before the wallet is called, and settles or releases
//! the reservation from the wallet's evidence. Pending payments stay
//! reserved. It is pure; the phone persists it.
use crate::{Code, Error, Result, fail};
use nostr::x402::{PaymentRequest, decode_payment_request};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const GRANT: &str = "openagents.spend-grant.v1";
pub const REQUEST: &str = "openagents.spend-request.v1";
pub const RECEIPT: &str = "openagents.spend-receipt.v1";
/// A grant lives at most 30 days, as a host grant does.
pub const MAX_GRANT_LIFETIME: u64 = 30 * 24 * 60 * 60;
/// A request lives at most one hour, and never past its invoice's expiry.
pub const MAX_REQUEST_LIFETIME: u64 = 60 * 60;
/// The most requests one `spend.list` answer carries.
pub const MAX_LISTED: usize = 16;
/// The only unit phase 1 uses. Amounts are exact; none is ever inferred.
pub const UNIT: &str = "msat";
/// The only network phase 1 pays on.
pub const NETWORK: &str = "bitcoin";
/// The only wallet kind phase 1 draws on.
pub const WALLET_KIND: &str = "spark";
/// The longest BOLT11 text a request carries.
pub const MAX_PAYMENT: usize = 4096;
const MAX_SAFE: u64 = 9_007_199_254_740_991;

/// How a grant's payments are approved. Phase 1 has only `request`: the
/// owner approves each payment on the phone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Request,
}

/// Why a payment is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    X402Purchase,
    LaborPayment,
    Tip,
    Transfer,
}
impl Purpose {
    pub const ALL: [Purpose; 4] = [
        Purpose::X402Purchase,
        Purpose::LaborPayment,
        Purpose::Tip,
        Purpose::Transfer,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::X402Purchase => "x402_purchase",
            Self::LaborPayment => "labor_payment",
            Self::Tip => "tip",
            Self::Transfer => "transfer",
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.as_str() == text)
    }
    /// How the approval sheet names it.
    pub fn label(self) -> &'static str {
        match self {
            Self::X402Purchase => "Paid tool or API (x402)",
            Self::LaborPayment => "Payment for work",
            Self::Tip => "Tip",
            Self::Transfer => "Transfer",
        }
    }
}

/// How a payment moves. Phase 1 pays BOLT11 invoices only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rail {
    Lightning,
}

/// Fee ceilings. A quote above either refuses as `fee_too_high`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeeMax {
    /// Absolute ceiling in msat.
    pub absolute: u64,
    /// Proportional ceiling in parts per million of the amount.
    pub ppm: u32,
}
impl FeeMax {
    /// The lower of the two ceilings for a payment of `amount_msat`.
    #[must_use]
    pub fn ceiling(&self, amount_msat: u64) -> u64 {
        let proportional =
            u64::try_from(u128::from(amount_msat) * u128::from(self.ppm) / 1_000_000)
                .unwrap_or(u64::MAX);
        self.absolute.min(proportional)
    }
}

/// The wallet a grant draws on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WalletRef {
    pub kind: String,
    pub network: String,
}

/// `openagents.spend-grant.v1`: what a host may ask the phone to pay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub v: String,
    pub requires: Vec<String>,
    /// 32-byte ID, lowercase hex.
    pub grant: String,
    /// The phone's device key.
    pub issuer: String,
    /// The host's key: the agent side that asks.
    pub grantee: String,
    pub wallet: WalletRef,
    pub mode: Mode,
    pub unit: String,
    /// Ceiling for one payment, fees included.
    pub per_payment_max: u64,
    /// Rolling window in seconds and its ceiling, fees included.
    pub period: u64,
    pub period_max: u64,
    /// Lifetime ceiling, fees included.
    pub total_max: u64,
    pub fee_max: FeeMax,
    pub rails: Vec<Rail>,
    /// Allowed payees: Lightning node keys (compressed, hex). Empty means
    /// none, unless `any_payee` is set.
    pub payees: Vec<String>,
    /// Any payee may be asked for. Only in `request` mode, where the owner
    /// sees the payee decoded from the invoice and approves each payment.
    pub any_payee: bool,
    pub purposes: Vec<Purpose>,
    /// The grantee's epoch at the phone. Revocation advances it.
    pub epoch: u64,
    pub issued_at: u64,
    pub expires_at: u64,
}

/// What the phone issues each host by default in phase 1. In `request`
/// mode these only bound what a host can ask for; nothing pays without a tap.
pub mod defaults {
    /// 10,000 sats.
    pub const PER_PAYMENT_MAX: u64 = 10_000_000;
    /// 24 hours.
    pub const PERIOD: u64 = 24 * 60 * 60;
    /// 50,000 sats a day.
    pub const PERIOD_MAX: u64 = 50_000_000;
    /// 500,000 sats in the grant's life.
    pub const TOTAL_MAX: u64 = 500_000_000;
    /// 100 sats.
    pub const FEE_ABSOLUTE: u64 = 100_000;
    /// Never more than the amount itself.
    pub const FEE_PPM: u32 = 1_000_000;
    /// 30 days.
    pub const LIFETIME: u64 = super::MAX_GRANT_LIFETIME;
}

impl Grant {
    /// A `request`-mode grant with the phase 1 defaults: any payee (each
    /// shown and approved on the phone), every purpose, Lightning only.
    #[must_use]
    pub fn request_mode(grant: String, issuer: &str, grantee: &str, epoch: u64, now: u64) -> Self {
        Self {
            v: GRANT.into(),
            requires: vec![],
            grant,
            issuer: issuer.into(),
            grantee: grantee.into(),
            wallet: WalletRef {
                kind: WALLET_KIND.into(),
                network: NETWORK.into(),
            },
            mode: Mode::Request,
            unit: UNIT.into(),
            per_payment_max: defaults::PER_PAYMENT_MAX,
            period: defaults::PERIOD,
            period_max: defaults::PERIOD_MAX,
            total_max: defaults::TOTAL_MAX,
            fee_max: FeeMax {
                absolute: defaults::FEE_ABSOLUTE,
                ppm: defaults::FEE_PPM,
            },
            rails: vec![Rail::Lightning],
            payees: vec![],
            any_payee: true,
            purposes: Purpose::ALL.to_vec(),
            epoch,
            issued_at: now,
            expires_at: now + defaults::LIFETIME,
        }
    }

    pub fn validate(&self) -> Result<()> {
        schema(&self.v, GRANT, &self.requires)?;
        crate::protocol::identity(&self.grant).map_err(Error::from)?;
        crate::protocol::public(&self.issuer)?;
        crate::protocol::public(&self.grantee)?;
        if self.issuer == self.grantee {
            return fail(Code::Forbidden, "a spend grant's issuer and grantee differ");
        }
        if self.wallet.kind != WALLET_KIND || self.wallet.network != NETWORK || self.unit != UNIT {
            return fail(Code::Unsupported, "unsupported spend wallet or unit");
        }
        if self.rails != [Rail::Lightning] {
            return fail(
                Code::Unsupported,
                "phase 1 grants name the Lightning rail only",
            );
        }
        if self.purposes.is_empty() || !self.purposes.windows(2).all(|w| w[0] < w[1]) {
            return fail(
                Code::Malformed,
                "purposes must be nonempty, sorted, and distinct",
            );
        }
        if self.payees.len() > 64 || !self.payees.windows(2).all(|w| w[0] < w[1]) {
            return fail(
                Code::Malformed,
                "payees must be sorted, distinct, and bounded",
            );
        }
        for payee in &self.payees {
            node_key(payee)?;
        }
        if self.any_payee && self.mode != Mode::Request {
            return fail(
                Code::Forbidden,
                "only a request-mode grant admits any payee",
            );
        }
        for value in [
            self.per_payment_max,
            self.period,
            self.period_max,
            self.total_max,
            self.fee_max.absolute,
            self.epoch,
            self.issued_at,
            self.expires_at,
        ] {
            safe(value)?;
        }
        if self.per_payment_max == 0
            || self.per_payment_max > self.period_max
            || self.period_max > self.total_max
            || self.period == 0
            || self.period > MAX_GRANT_LIFETIME
            || self.fee_max.ppm > 1_000_000
        {
            return fail(Code::Bounds, "spend grant ceilings are inconsistent");
        }
        if self.expires_at <= self.issued_at
            || self.expires_at - self.issued_at > MAX_GRANT_LIFETIME
        {
            return fail(Code::Malformed, "invalid spend grant lifetime");
        }
        Ok(())
    }

    fn admits_payee(&self, payee: &str) -> bool {
        self.any_payee || self.payees.iter().any(|p| p == payee)
    }
}

/// References a request carries for the receipt and the approval sheet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// The host task the agent works in, when it has one.
    pub task: Option<String>,
    /// A host-authored title: the task's title or the tool's name.
    pub title: Option<String>,
    /// The x402 resource bought, as a URI.
    pub resource: Option<String>,
    /// The agent's short note. The sheet shows it as the agent's words,
    /// never as the payee.
    pub note: Option<String>,
}
impl Context {
    fn validate(&self) -> Result<()> {
        if let Some(task) = &self.task {
            crate::protocol::identity(task).map_err(Error::from)?;
        }
        for (value, max) in [(&self.title, 200), (&self.resource, 512), (&self.note, 280)] {
            if let Some(value) = value
                && (value.trim().is_empty()
                    || value.len() > max
                    || value.chars().any(char::is_control))
            {
                return fail(Code::Bounds, "spend request context exceeds its bound");
            }
        }
        Ok(())
    }
}

/// `openagents.spend-request.v1`: one payment a host asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpendRequest {
    pub v: String,
    pub requires: Vec<String>,
    /// 32-byte ID and the idempotency key. Different bytes under the same ID
    /// are a conflict.
    pub request: String,
    /// The grant it draws on and that grant's epoch.
    pub grant: String,
    pub epoch: u64,
    /// The host that asks: the grant's grantee.
    pub grantee: String,
    /// The exact BOLT11 invoice to pay.
    pub payment: String,
    /// The invoice's exact amount.
    pub amount_msat: u64,
    /// The caller's fee ceiling, never above the grant's.
    pub fee_max_msat: u64,
    pub purpose: Purpose,
    pub context: Context,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl SpendRequest {
    /// Check the request's own shape and its invoice: a mainnet BOLT11 with
    /// exactly `amount_msat`, which outlives the request.
    pub fn validate(&self) -> Result<PaymentRequest> {
        schema(&self.v, REQUEST, &self.requires)?;
        crate::protocol::identity(&self.request).map_err(Error::from)?;
        crate::protocol::identity(&self.grant).map_err(Error::from)?;
        crate::protocol::public(&self.grantee)?;
        for value in [
            self.epoch,
            self.amount_msat,
            self.fee_max_msat,
            self.issued_at,
            self.expires_at,
        ] {
            safe(value)?;
        }
        if self.expires_at <= self.issued_at
            || self.expires_at - self.issued_at > MAX_REQUEST_LIFETIME
        {
            return fail(Code::Malformed, "invalid spend request lifetime");
        }
        self.context.validate()?;
        let invoice = self.invoice()?;
        if invoice.amount_msat != self.amount_msat || self.amount_msat == 0 {
            return fail(Code::Malformed, "the amount differs from the invoice's");
        }
        if self.expires_at > invoice.expires_at() {
            return fail(Code::Malformed, "a spend request outlives its invoice");
        }
        Ok(invoice)
    }

    /// The authenticated invoice. Only mainnet BOLT11 decodes here.
    pub fn invoice(&self) -> Result<PaymentRequest> {
        if self.payment.len() > MAX_PAYMENT {
            return fail(Code::Bounds, "the payment request exceeds its bound");
        }
        let invoice = decode_payment_request(&self.payment).map_err(|_| {
            Error::new(
                Code::Malformed,
                "the payment request is not a valid invoice",
            )
        })?;
        if invoice.currency != "bc" {
            return fail(Code::Unsupported, "only mainnet invoices are paid");
        }
        Ok(invoice)
    }

    /// The digest of the request's exact bytes, which the ledger keeps to
    /// tell a retry from a conflicting reuse of the ID.
    pub fn digest(&self) -> Result<String> {
        Ok(nostr::contracts::digest_bytes(&crate::protocol::encoded(
            self,
        )?))
    }

    /// A request's ID for an invoice: repeated asks for the same invoice are
    /// one request.
    #[must_use]
    pub fn id_for(payment: &str) -> String {
        let digest =
            nostr::contracts::digest_bytes(format!("{REQUEST}:{}", payment.trim()).as_bytes());
        digest.trim_start_matches("sha256:").to_owned()
    }
}

/// What a receipt reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Settlement {
    Paid,
    Refused,
    Pending,
    Unknown,
}

/// Why a request was not paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    Expired,
    Revoked,
    Stale,
    OverPaymentCap,
    OverPeriodCap,
    OverTotalCap,
    FeeTooHigh,
    PayeeNotAllowed,
    PurposeNotAllowed,
    RailNotAllowed,
    DeclinedByOwner,
    PhoneUnreachable,
    InsufficientFunds,
    /// The request or its invoice is malformed, or its amount differs.
    Malformed,
    /// The request ID was reused with different bytes.
    Conflict,
    /// The wallet tried and the payment failed.
    PaymentFailed,
}
impl Refusal {
    /// The plain words the phone and the host show for it.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Expired => "The request expired before it was paid.",
            Self::Revoked => "This computer may not ask for payments.",
            Self::Stale => "The request was made under a grant the phone has since replaced.",
            Self::OverPaymentCap => "The amount is above this computer's limit for one payment.",
            Self::OverPeriodCap => "This computer's daily payment limit is used up.",
            Self::OverTotalCap => "This computer's total payment limit is used up.",
            Self::FeeTooHigh => "The fee is above the ceiling.",
            Self::PayeeNotAllowed => "The payee isn't allowed.",
            Self::PurposeNotAllowed => "The purpose isn't allowed.",
            Self::RailNotAllowed => "That kind of payment isn't allowed.",
            Self::DeclinedByOwner => "Declined on the phone.",
            Self::PhoneUnreachable => "The phone didn't answer before the request expired.",
            Self::InsufficientFunds => "The phone's wallet doesn't hold enough.",
            Self::Malformed => "The request or its invoice can't be read.",
            Self::Conflict => "The request ID was reused for a different payment.",
            Self::PaymentFailed => "The payment didn't go through.",
        }
    }
}

/// Budget left under a grant, fees included.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remaining {
    pub period_msat: u64,
    pub total_msat: u64,
}

/// `openagents.spend-receipt.v1`: what the phone did with a request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub v: String,
    pub requires: Vec<String>,
    pub request: String,
    pub grant: String,
    pub outcome: Settlement,
    /// Present exactly when the outcome is `refused`.
    pub code: Option<Refusal>,
    /// The wallet's payment ID.
    pub payment_id: Option<String>,
    pub amount_msat: Option<u64>,
    /// Unknown fees stay unknown.
    pub fees_msat: Option<u64>,
    /// The Lightning preimage, lowercase hex. Only in a `paid` receipt,
    /// which travels only inside encrypted envelopes; general traces carry
    /// its digest.
    pub proof: Option<String>,
    pub remaining: Option<Remaining>,
    pub at: u64,
}
impl Receipt {
    /// A refusal receipt.
    #[must_use]
    pub fn refused(request: &str, grant: &str, code: Refusal, at: u64) -> Self {
        Self {
            v: RECEIPT.into(),
            requires: vec![],
            request: request.into(),
            grant: grant.into(),
            outcome: Settlement::Refused,
            code: Some(code),
            payment_id: None,
            amount_msat: None,
            fees_msat: None,
            proof: None,
            remaining: None,
            at,
        }
    }

    pub fn validate(&self) -> Result<()> {
        schema(&self.v, RECEIPT, &self.requires)?;
        crate::protocol::identity(&self.request).map_err(Error::from)?;
        crate::protocol::identity(&self.grant).map_err(Error::from)?;
        safe(self.at)?;
        if self.code.is_some() != (self.outcome == Settlement::Refused) {
            return fail(
                Code::Malformed,
                "a refusal code goes with a refused outcome only",
            );
        }
        if self.proof.is_some() != (self.outcome == Settlement::Paid) {
            return fail(Code::Malformed, "a proof goes with a paid outcome only");
        }
        if let Some(proof) = &self.proof {
            crate::protocol::identity(proof).map_err(Error::from)?;
            if self.amount_msat.is_none() {
                return fail(Code::Malformed, "a paid receipt names its amount");
            }
        }
        if self
            .payment_id
            .as_ref()
            .is_some_and(|id| id.is_empty() || id.len() > 256 || id.chars().any(char::is_control))
        {
            return fail(Code::Bounds, "payment ID exceeds its bound");
        }
        Ok(())
    }

    /// Whether the outcome is final: paid or refused.
    #[must_use]
    pub fn is_final(&self) -> bool {
        matches!(self.outcome, Settlement::Paid | Settlement::Refused)
    }

    /// Check that the receipt answers `request`, and that a `paid` one
    /// proves it: the preimage hashes to the invoice's payment hash and the
    /// amount is the request's.
    pub fn answers(&self, request: &SpendRequest) -> Result<()> {
        self.validate()?;
        if self.request != request.request || self.grant != request.grant {
            return fail(Code::Forbidden, "the receipt answers another request");
        }
        if self.outcome == Settlement::Paid {
            let invoice = request.invoice()?;
            let preimage = hex32(self.proof.as_deref().unwrap_or_default())?;
            let hash = nostr::contracts::digest_bytes(&preimage);
            if hash.trim_start_matches("sha256:") != hex(&invoice.payment_hash)
                || self.amount_msat != Some(request.amount_msat)
            {
                return fail(Code::Forbidden, "the receipt does not prove the payment");
            }
        }
        Ok(())
    }
}

/// A request as the host lists it, with the receipt it holds, if any.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub request: SpendRequest,
    pub receipt: Option<Receipt>,
}

/// Where a ledger entry stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Checked and approved; the amount and fee ceiling are held while the
    /// wallet pays.
    Reserved,
    /// The wallet reported the payment pending. Still held.
    Pending,
    Paid,
    Refused,
}

/// One request in the phone's ledger, tagged with the host and task that
/// asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LedgerEntry {
    pub request: String,
    pub digest: String,
    pub grant: String,
    pub host: String,
    pub task: Option<String>,
    pub title: Option<String>,
    pub purpose: Purpose,
    /// The payee as decoded from the invoice.
    pub payee: String,
    pub amount_msat: u64,
    /// The fee ceiling held while unsettled.
    pub fee_reserved_msat: u64,
    /// The fee the wallet reported, once known.
    pub fee_msat: Option<u64>,
    pub state: State,
    pub code: Option<Refusal>,
    pub payment_id: Option<String>,
    pub at: u64,
    pub settled_at: Option<u64>,
    /// The host has recorded this entry's final receipt.
    #[serde(default)]
    pub delivered: bool,
}
impl LedgerEntry {
    /// What the entry holds against its grant: nothing when refused, the
    /// fee paid once known, else the fee ceiling.
    #[must_use]
    pub fn held(&self) -> u64 {
        match self.state {
            State::Refused => 0,
            State::Paid => self
                .amount_msat
                .saturating_add(self.fee_msat.unwrap_or(self.fee_reserved_msat)),
            State::Reserved | State::Pending => {
                self.amount_msat.saturating_add(self.fee_reserved_msat)
            }
        }
    }
}

/// A request that passed every check: its decoded invoice and what the
/// grant has left before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Admitted {
    pub invoice: PaymentRequest,
    pub payee: String,
    pub remaining: Remaining,
    /// No earlier entry paid this payee under this host.
    pub new_payee: bool,
}

/// The phone's spend ledger for one wallet. Restarting never resets a
/// period: entries keep their times and are pruned only long after.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub entries: BTreeMap<String, LedgerEntry>,
}

/// Settled entries are kept for this long, for history.
pub const LEDGER_RETENTION: u64 = 90 * 24 * 60 * 60;
/// The most entries a ledger keeps.
pub const LEDGER_MAX: usize = 1024;

impl Ledger {
    /// Everything held under `grant` since `since`.
    fn held_since(&self, grant: &str, since: u64) -> u64 {
        self.entries
            .values()
            .filter(|e| e.grant == grant && e.at >= since)
            .map(LedgerEntry::held)
            .fold(0, u64::saturating_add)
    }

    /// What `grant` has left at `now`.
    #[must_use]
    pub fn remaining(&self, grant: &Grant, now: u64) -> Remaining {
        let period = self.held_since(&grant.grant, now.saturating_sub(grant.period));
        let total = self.held_since(&grant.grant, 0);
        Remaining {
            period_msat: grant.period_max.saturating_sub(period),
            total_msat: grant.total_max.saturating_sub(total),
        }
    }

    /// Check `request` against the phone's current grant for its host
    /// (`None` when the phone holds none or the owner blocked the host) and
    /// the ledger, without changing anything. The first failing check names
    /// the refusal.
    pub fn check(
        &self,
        grant: Option<&Grant>,
        request: &SpendRequest,
        now: u64,
    ) -> std::result::Result<Admitted, Refusal> {
        let invoice = request.validate().map_err(|error| match error.code {
            Code::Unsupported => Refusal::RailNotAllowed,
            _ => Refusal::Malformed,
        })?;
        let grant = grant.ok_or(Refusal::Revoked)?;
        if request.grantee != grant.grantee {
            return Err(Refusal::Revoked);
        }
        if request.grant != grant.grant {
            // A request under an older epoch was overtaken by revocation or
            // replacement; anything else names no grant the phone holds.
            return Err(if request.epoch < grant.epoch {
                Refusal::Stale
            } else {
                Refusal::Revoked
            });
        }
        if request.epoch != grant.epoch {
            return Err(Refusal::Stale);
        }
        if now >= request.expires_at || now >= grant.expires_at || now >= invoice.expires_at() {
            return Err(Refusal::Expired);
        }
        if let Some(existing) = self.entries.get(&request.request)
            && existing.digest != request.digest().map_err(|_| Refusal::Malformed)?
        {
            return Err(Refusal::Conflict);
        }
        if !grant.purposes.contains(&request.purpose) {
            return Err(Refusal::PurposeNotAllowed);
        }
        let payee = hex(&invoice.payee);
        if !grant.admits_payee(&payee) {
            return Err(Refusal::PayeeNotAllowed);
        }
        if request.fee_max_msat > grant.fee_max.ceiling(request.amount_msat) {
            return Err(Refusal::FeeTooHigh);
        }
        let cost = request.amount_msat.saturating_add(request.fee_max_msat);
        if cost > grant.per_payment_max {
            return Err(Refusal::OverPaymentCap);
        }
        // A request already reserved holds its own amount: a retry is not
        // counted twice.
        let own = self
            .entries
            .get(&request.request)
            .map_or(0, LedgerEntry::held);
        let remaining = self.remaining(grant, now);
        if cost > remaining.period_msat.saturating_add(own) {
            return Err(Refusal::OverPeriodCap);
        }
        if cost > remaining.total_msat.saturating_add(own) {
            return Err(Refusal::OverTotalCap);
        }
        let new_payee = !self
            .entries
            .values()
            .any(|e| e.host == grant.grantee && e.payee == payee && e.state == State::Paid);
        Ok(Admitted {
            invoice,
            payee,
            remaining,
            new_payee,
        })
    }

    /// Reserve an approved request: check it again and hold its amount plus
    /// fee ceiling. A retry of an entry already reserved, pending, or paid
    /// returns it unchanged; it never reserves twice.
    pub fn reserve(
        &mut self,
        grant: Option<&Grant>,
        request: &SpendRequest,
        now: u64,
    ) -> std::result::Result<LedgerEntry, Refusal> {
        let digest = request.digest().map_err(|_| Refusal::Malformed)?;
        if let Some(existing) = self.entries.get(&request.request) {
            if existing.digest != digest {
                return Err(Refusal::Conflict);
            }
            if existing.state != State::Refused {
                return Ok(existing.clone());
            }
            return Err(existing.code.unwrap_or(Refusal::Malformed));
        }
        let admitted = self.check(grant, request, now)?;
        self.prune(now);
        if self.entries.len() >= LEDGER_MAX {
            return Err(Refusal::OverTotalCap);
        }
        let entry = LedgerEntry {
            request: request.request.clone(),
            digest,
            grant: request.grant.clone(),
            host: request.grantee.clone(),
            task: request.context.task.clone(),
            title: request.context.title.clone(),
            purpose: request.purpose,
            payee: admitted.payee,
            amount_msat: request.amount_msat,
            fee_reserved_msat: request.fee_max_msat,
            fee_msat: None,
            state: State::Reserved,
            code: None,
            payment_id: None,
            at: now,
            settled_at: None,
            delivered: false,
        };
        self.entries.insert(request.request.clone(), entry.clone());
        Ok(entry)
    }

    /// Record a refusal without a reservation (declined, expired, or failed
    /// checks), or release a reservation the wallet never paid.
    pub fn refuse(&mut self, request: &SpendRequest, code: Refusal, now: u64) {
        if let Some(entry) = self.entries.get_mut(&request.request) {
            // A paid or pending payment is never released by a refusal.
            if matches!(entry.state, State::Reserved) {
                entry.state = State::Refused;
                entry.code = Some(code);
                entry.settled_at = Some(now);
            }
            return;
        }
        let payee = request
            .invoice()
            .map(|invoice| hex(&invoice.payee))
            .unwrap_or_default();
        self.prune(now);
        self.entries.insert(
            request.request.clone(),
            LedgerEntry {
                request: request.request.clone(),
                digest: request.digest().unwrap_or_default(),
                grant: request.grant.clone(),
                host: request.grantee.clone(),
                task: request.context.task.clone(),
                title: request.context.title.clone(),
                purpose: request.purpose,
                payee,
                amount_msat: request.amount_msat,
                fee_reserved_msat: 0,
                fee_msat: None,
                state: State::Refused,
                code: Some(code),
                payment_id: None,
                at: now,
                settled_at: Some(now),
                delivered: false,
            },
        );
    }

    /// The wallet reported a reserved or pending payment failed: release it
    /// as `payment_failed`. Only the wallet's evidence does this.
    pub fn fail(&mut self, request: &str, now: u64) {
        if let Some(entry) = self.entries.get_mut(request)
            && matches!(entry.state, State::Reserved | State::Pending)
        {
            entry.state = State::Refused;
            entry.code = Some(Refusal::PaymentFailed);
            entry.settled_at = Some(now);
        }
    }

    /// Record the wallet's report on a reserved or pending entry: paid with
    /// its fee, or still pending. Pending keeps the reservation.
    pub fn settle(
        &mut self,
        request: &str,
        paid: bool,
        payment_id: Option<String>,
        fee_msat: Option<u64>,
        now: u64,
    ) {
        if let Some(entry) = self.entries.get_mut(request)
            && matches!(entry.state, State::Reserved | State::Pending)
        {
            entry.payment_id = payment_id.or(entry.payment_id.take());
            if paid {
                entry.state = State::Paid;
                entry.fee_msat = fee_msat;
                entry.settled_at = Some(now);
            } else {
                entry.state = State::Pending;
            }
        }
    }

    /// Keep at most `max` entries, forgetting the oldest settled ones first.
    /// Reserved and pending entries are always kept.
    pub fn trim(&mut self, max: usize) {
        if self.entries.len() <= max {
            return;
        }
        let mut settled: Vec<(u64, String)> = self
            .entries
            .values()
            .filter(|e| matches!(e.state, State::Paid | State::Refused))
            .map(|e| (e.at, e.request.clone()))
            .collect();
        settled.sort();
        let excess = self.entries.len() - max;
        for (_, request) in settled.into_iter().take(excess) {
            self.entries.remove(&request);
        }
    }

    /// Forget settled entries past retention, oldest first, within bounds.
    /// Reserved and pending entries are never pruned.
    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(LEDGER_RETENTION);
        self.entries
            .retain(|_, e| matches!(e.state, State::Reserved | State::Pending) || e.at >= cutoff);
    }
}

pub(crate) fn schema(actual: &str, expected: &str, requires: &[String]) -> Result<()> {
    if actual != expected || !requires.is_empty() {
        return fail(Code::Unsupported, "unsupported spend schema or feature");
    }
    Ok(())
}
fn safe(value: u64) -> Result<()> {
    if value > MAX_SAFE {
        return fail(Code::Malformed, "integer exceeds the safe range");
    }
    Ok(())
}
fn node_key(text: &str) -> Result<()> {
    if text.len() != 66
        || !(text.starts_with("02") || text.starts_with("03"))
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return fail(Code::Malformed, "a payee is a compressed node key in hex");
    }
    Ok(())
}
/// Lowercase hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hex32(text: &str) -> Result<[u8; 32]> {
    crate::protocol::identity(text).map_err(Error::from)?;
    let mut out = [0_u8; 32];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| Error::new(Code::Malformed, "invalid hex"))?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests;

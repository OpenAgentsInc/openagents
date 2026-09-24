//! Plans, subscriptions, checkout sessions, invoices, and the provider
//! event journal — the billing book.
//!
//! A [`Plan`] is an operator-declared, versioned offer: what it costs,
//! what each paid period grants, how many seats it covers, and which
//! doors it may name. Plans live in deployment configuration, not in
//! this store — the store records what customers did with them:
//! [`Subscription`]s, pending [`Checkout`]s, [`Invoice`]s, the
//! deduplicated [`Event`] journal, and the [`Grant`]s already issued.
//!
//! Provider events are the only way money moves: a checkout completing,
//! an invoice paying or failing, a charge refunded or disputed. Every
//! event carries the references its effect needs, so a duplicate, a
//! replay, or an out-of-order delivery decides against committed state
//! rather than against arrival order. Applying an event returns
//! [`Effect`]s — the cross-store mutations an adapter owes the money
//! ledger and the account store — which are idempotent on their
//! `source` keys, so a crash between the ledger append and this store's
//! seal replays safely through [`BillingBook::reconcile`].
//!
//! # What this is not
//!
//! There is no payment processor, no transport, and no clock here:
//! every timestamp arrives as an argument in Unix seconds, and the only
//! entropy is minting record ids. No price is chosen — a plan's price
//! is the operator's declaration, and this module never invents one.
//! Grant credit is consumed last: an expired or refunded allowance
//! debits the lesser of its original amount and the available balance,
//! so a refund can never remove credit already committed to work.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::accounts::Trouble;
use crate::sessions::{Access, push_access};

/// The store's schema tag.
pub const SCHEMA: &str = "openagents.billing.v1";

const BILLING: &str = "billing.json";
const HISTORY_DIR: &str = "billing-history";
const LOCK: &str = "billing.lock";
const LOCK_RETRIES: u32 = 200;
const STORE_BYTES: u64 = 16 * 1024 * 1024;
const ACCESS_MAX: usize = 1024;
const EVENTS_MAX: usize = 4096;
const GRANTS_MAX: usize = 8192;

/// Which doors a plan may name in a request's `model` field.
///
/// The config spells it `"models": "all"` for every configured door,
/// or `"models": {"listed": ["door-a", "door-b"]}` for a subset.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelAccess {
    /// Every configured door — the plan binds no subset.
    All,
    /// An explicit list of door names.
    Listed(Vec<String>),
}

impl ModelAccess {
    /// Whether the plan's doors include `model`.
    #[must_use]
    pub fn permits(&self, model: &str) -> bool {
        match self {
            Self::All => true,
            Self::Listed(doors) => doors.iter().any(|door| door == model),
        }
    }
}

/// What a plan costs for one period. `amount` is millionths of
/// `currency`; a free plan is `amount: 0` — never a missing price.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Price {
    pub amount: u64,
    pub currency: String,
    /// The period the price and allowance recur over, in seconds.
    pub period_secs: u64,
}

impl Price {
    /// Whether this is a free plan — `subscribe` may take it directly;
    /// a nonzero amount always travels through checkout.
    #[must_use]
    pub fn is_free(&self) -> bool {
        self.amount == 0
    }
}

/// An operator-declared offer — the published, versioned price
/// configuration a deployment stands behind.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    /// The plan's kebab-case id — what subscriptions and requests name.
    pub id: String,
    /// The plan revision — `subscription.plan_version` pins which offer
    /// the customer took so a later plan edit cannot relabel it.
    pub version: String,
    /// The display name.
    pub name: String,
    /// What one period costs, and how long a period is.
    pub price: Price,
    /// The credit each paid period grants, in millionths of the price's
    /// currency — granted once per period through the ledger's
    /// idempotent `Credit` mutation.
    pub allowance: u64,
    /// The one-time credit a workspace's first subscription grants.
    #[serde(default)]
    pub signup_credit: u64,
    /// The seats the plan covers; applied to the workspace on
    /// activation and each renewal. `None` leaves seats untouched.
    #[serde(default)]
    pub seats: Option<u32>,
    /// The doors the plan may name.
    pub models: ModelAccess,
    /// The workspace's spend ceiling, matching the money account's
    /// `spend_limit`. `u64::MAX` is unlimited.
    #[serde(default = "max_spend")]
    pub spend_limit: u64,
    /// How long period-allowance credit stands before it expires.
    /// Absent means allowance does not expire — a stated policy, not
    /// an oversight.
    #[serde(default)]
    pub credit_expiry_secs: Option<u64>,
    /// Whether purchased top-ups are allowed while subscribed.
    #[serde(default = "yes")]
    pub topups_allowed: bool,
}

fn max_spend() -> u64 {
    u64::MAX
}
fn yes() -> bool {
    true
}

/// What a checkout session is for.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Intent {
    /// Start or move a subscription onto `plan`.
    Subscribe {
        /// The plan id the session pays for.
        plan: String,
    },
    /// A one-time credit purchase of `amount` millionths of `currency`.
    TopUp {
        /// The purchased amount.
        amount: u64,
        /// The purchase currency.
        currency: String,
    },
}

/// A checkout's standing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckoutState {
    /// Created and unpaid — the browser may still be on the provider's page.
    Pending,
    /// The provider confirmed payment through a verified event.
    Complete,
    /// The pending session outlived its deadline without a completion.
    Expired,
}

impl std::fmt::Display for CheckoutState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pending => "pending",
            Self::Complete => "complete",
            Self::Expired => "expired",
        })
    }
}

/// A checkout session: the intent, the provider's reference, and the
/// standing. The browser's return displays this record — it never
/// moves it; only a verified provider event does.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Checkout {
    /// `cko_<hex>` — minted here, returned to the browser.
    pub id: String,
    /// The workspace the session belongs to.
    pub workspace: String,
    /// What the session pays for.
    pub intent: Intent,
    /// The plan version the session was created under — a checkout
    /// completes against the offer the customer saw.
    pub plan_version: String,
    /// The provider-side session reference — what the provider's
    /// events name back.
    pub provider_ref: String,
    /// The current standing.
    pub state: CheckoutState,
    /// When the session was created.
    pub created: u64,
    /// When the session stops being payable.
    pub expires_at: u64,
    /// When it resolved, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<u64>,
}

/// A subscription's standing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionState {
    /// Paid through the current period's end.
    Active,
    /// The current period's renewal payment failed — inside the grace
    /// window the deployment declares, still entitled.
    PastDue,
    /// Cancelled — entitled until `cancel_at`, then `expired`.
    Cancelled,
    /// Entitlement ended — expiry, unrecovered failure, or elapsed
    /// cancellation.
    Expired,
}

impl std::fmt::Display for SubscriptionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Active => "active",
            Self::PastDue => "past-due",
            Self::Cancelled => "cancelled",
            Self::Expired => "expired",
        })
    }
}

/// A recorded transition on a subscription — the audit the workspace's
/// billing read returns.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Transition {
    /// When it happened.
    pub at: u64,
    /// What changed — `created`, `renewed`, `plan-scheduled`,
    /// `plan-changed`, `past-due`, `recovered`, `cancelled`, `expired`.
    pub action: String,
    /// The plan after the change.
    pub plan: String,
    /// The period after the change.
    pub period: u32,
    /// The event or route that caused it.
    pub source: String,
}

/// A workspace's subscription: the plan it's on, the period it stands
/// in, and the transition journal.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Subscription {
    /// `sub_<hex>` — minted here.
    pub id: String,
    /// The workspace it covers — one active subscription per workspace.
    pub workspace: String,
    /// The current plan id.
    pub plan: String,
    /// The plan revision the customer is on.
    pub plan_version: String,
    /// The current standing.
    pub state: SubscriptionState,
    /// The paid period number the subscription stands in — one allowance
    /// grant per period, keyed `billing:<sub>:period:<n>`.
    pub period: u32,
    /// When the current period started.
    pub period_started: u64,
    /// When the current period ends.
    pub period_ends: u64,
    /// A scheduled plan change that lands at the next paid renewal —
    /// the downgrade path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_plan: Option<String>,
    /// When cancellation takes effect — always a period end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_at: Option<u64>,
    /// The provider-side subscription reference, when the provider
    /// tracks one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_ref: Option<String>,
    /// A failed payment's deadline — past it the subscription expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_ends: Option<u64>,
    /// The transition journal, bounded.
    #[serde(default)]
    pub history: Vec<Transition>,
}

/// An invoice's standing.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InvoiceState {
    /// Issued, unpaid.
    Open,
    /// Confirmed paid by a provider event.
    Paid,
    /// Payment attempted and failed.
    Failed,
    /// The charge was refunded after payment.
    Refunded,
    /// The charge is under dispute.
    Disputed,
}

impl std::fmt::Display for InvoiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Open => "open",
            Self::Paid => "paid",
            Self::Failed => "failed",
            Self::Refunded => "refunded",
            Self::Disputed => "disputed",
        })
    }
}

/// A charge against a workspace — the link between a provider's
/// attempt and the ledger's grants and debits.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Invoice {
    /// `inv_<hex>` — minted here on first sight.
    pub id: String,
    /// The workspace charged.
    pub workspace: String,
    /// The subscription it belongs to, when it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<String>,
    /// The period it pays for, when it names one.
    #[serde(default)]
    pub period: u32,
    /// Millionths of `currency`.
    pub amount: u64,
    /// The charge currency.
    pub currency: String,
    /// The standing.
    pub state: InvoiceState,
    /// The provider-side charge reference.
    pub provider_ref: String,
    /// When the invoice was issued.
    pub created: u64,
    /// When it last changed standing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<u64>,
}

/// A provider event as the webhook verified it — the deduplicated
/// journal entry. `kind` and its references travel as flat fields so
/// the journal reads plainly.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Event {
    /// The provider that emitted it — `sandbox` today.
    pub provider: String,
    /// The provider's event id — the dedup key.
    pub id: String,
    /// `checkout-completed`, `invoice-paid`, `invoice-failed`,
    /// `subscription-cancelled`, `charge-refunded`, `charge-disputed`.
    pub kind: String,
    /// The checkout the event names, when it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<String>,
    /// The subscription the event names, when it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<String>,
    /// The invoice the event names, when it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice: Option<String>,
    /// The period the event pays for, when it names one.
    #[serde(default)]
    pub period: u32,
    /// Millionths of currency the event moves, when it names one.
    #[serde(default)]
    pub amount: u64,
    /// The event's currency, when it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Whether cancellation lands at the period end — the only kind
    /// supported.
    #[serde(default)]
    pub at_period_end: bool,
    /// The provider-side reference the event carries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_ref: Option<String>,
    /// When the webhook accepted it — the server writes this; a
    /// provider body does not carry it.
    #[serde(default)]
    pub received: u64,
    /// Whether its effects were applied to the book — the server
    /// writes this too.
    #[serde(default)]
    pub applied: bool,
    /// The outcome — `applied`, `duplicate`, `superseded`, or
    /// `ignored:` with a reason. The journal writes this.
    #[serde(default)]
    pub outcome: String,
}

/// A credit this store caused the ledger to grant — recorded so the
/// grant is provably once and auditable.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Grant {
    /// The idempotent ledger source — `billing:signup:<workspace>`,
    /// `billing:<sub>:period:<n>`, or `billing:topup:<checkout>`.
    pub source: String,
    /// The workspace credited.
    pub workspace: String,
    /// `signup-credit`, `period-allowance`, or `top-up`.
    pub kind: String,
    /// Millionths of `currency`.
    pub amount: u64,
    /// The grant's currency.
    pub currency: String,
    /// When it was granted.
    pub issued: u64,
    /// When the credit expires, when the plan declares an expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// The invoice that paid for it, when there is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoice: Option<String>,
    /// The audit line the ledger mutation carries — stored so a
    /// reconcile replay issues byte-identical mutations, which is
    /// what makes a replayed grant a no-op rather than a conflict.
    #[serde(default)]
    pub audit: String,
}

/// A debit this store caused the ledger to apply — recorded like a
/// grant so a crash between the book's commit and the ledger's append
/// replays the same claim.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Clawback {
    /// The idempotent ledger source — `billing:refund:<invoice>`,
    /// `billing:dispute:<invoice>`, or `expired:<grant-source>`.
    pub source: String,
    /// The workspace debited.
    pub workspace: String,
    /// The named amount — the ledger takes the lesser of this and the
    /// available balance, so a replayed claim clamps the same way.
    pub amount: u64,
    /// `refund`, `dispute`, or `expiry`.
    pub kind: String,
    /// When the claim was recorded.
    pub issued: u64,
    /// The audit line the ledger mutation carries.
    pub audit: String,
}

/// A mutation an adapter owes another store — the ledger or the
/// account store. Every effect is idempotent on its own key, so
/// applying it twice costs nothing.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Effect {
    /// Create the workspace's money account if absent.
    CreateAccount {
        /// The workspace the account belongs to.
        workspace: String,
        /// The account currency.
        currency: String,
        /// The account's spend ceiling.
        spend_limit: u64,
        /// Whether top-ups are allowed.
        topups_allowed: bool,
    },
    /// Credit the workspace — a grant or a top-up.
    Credit {
        /// The workspace credited.
        workspace: String,
        /// The idempotent mutation source.
        source: String,
        /// Millionths.
        amount: u64,
        /// `grant` or `top-up`.
        credit_kind: String,
        /// The audit line.
        audit: String,
    },
    /// Debit the workspace — a refund, a dispute clawback, or an
    /// expired allowance taking back what was not spent.
    Debit {
        /// The workspace debited.
        workspace: String,
        /// The idempotent mutation source.
        source: String,
        /// Millionths — the adapter clamps to the available balance,
        /// so a clawback never removes credit already spent.
        amount: u64,
        /// The audit line.
        audit: String,
    },
    /// Apply a plan's seats to the workspace.
    SetSeats {
        /// The workspace.
        workspace: String,
        /// The new seat count.
        seats: Option<u32>,
    },
}

/// How an event landed — what `apply_event` reports and the journal
/// records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    /// The event changed state; these effects owe other stores.
    Applied(Vec<Effect>),
    /// The event id was already journaled — a replay, never a second
    /// effect.
    Duplicate,
    /// A different event already moved the record past this one's
    /// claim — out-of-order delivery absorbed.
    Superseded(String),
    /// The event names nothing this store holds, or claims a state
    /// transition that cannot follow from committed state.
    Ignored(String),
}

impl Outcome {
    /// The outcome's journal name — `applied`, `duplicate`,
    /// `superseded:<why>`, or `ignored:<why>`.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Applied(_) => "applied".to_string(),
            Self::Duplicate => "duplicate".to_string(),
            Self::Superseded(why) => format!("superseded:{why}"),
            Self::Ignored(why) => format!("ignored:{why}"),
        }
    }
}

/// The set `reconcile` returns — what a restart or a lost delivery
/// must still answer for.
#[derive(Clone, Debug, Default)]
pub struct Reconciliation {
    /// Provider references worth re-fetching — pending checkouts past
    /// their deadline.
    pub stale_refs: Vec<String>,
    /// Events journaled `received` but not `applied` — a crash
    /// mid-application left these; replaying them is safe.
    pub unapplied: Vec<String>,
}

/// The lifecycle book: plans stay in configuration; everything the
/// customers did lives here.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct BillingBook {
    /// Subscriptions by id.
    #[serde(default)]
    pub subscriptions: BTreeMap<String, Subscription>,
    /// The active (or last) subscription per workspace.
    #[serde(default)]
    pub by_workspace: BTreeMap<String, String>,
    /// Checkout sessions by id.
    #[serde(default)]
    pub checkouts: BTreeMap<String, Checkout>,
    /// Invoices by id.
    #[serde(default)]
    pub invoices: BTreeMap<String, Invoice>,
    /// The deduplicated event journal, by `provider:id`.
    #[serde(default)]
    pub events: BTreeMap<String, Event>,
    /// Credits issued, by their idempotent source key.
    #[serde(default)]
    pub grants: BTreeMap<String, Grant>,
    /// Debits claimed, by their idempotent source key.
    #[serde(default)]
    pub clawbacks: BTreeMap<String, Clawback>,
    /// Record ids already minted — collision defense.
    #[serde(default)]
    ids: Vec<String>,
}

impl BillingBook {
    fn mint(&mut self, prefix: &str, now: u64) -> Result<String, Refusal> {
        let entropy = fresh()?;
        for nonce in 0_u64.. {
            let mut hasher = Sha256::new();
            hasher.update(prefix.as_bytes());
            hasher.update(now.to_be_bytes());
            hasher.update(entropy.as_bytes());
            hasher.update(nonce.to_be_bytes());
            let hex = format!("{:x}", hasher.finalize());
            let id = format!("{prefix}_{}", &hex[..12]);
            if !self.ids.contains(&id) {
                self.ids.push(id.clone());
                return Ok(id);
            }
        }
        unreachable!()
    }

    /// The workspace's current subscription, when it has one.
    #[must_use]
    pub fn subscription_for(&self, workspace: &str) -> Option<&Subscription> {
        self.by_workspace
            .get(workspace)
            .and_then(|id| self.subscriptions.get(id))
    }

    /// The checkout a provider reference names.
    #[must_use]
    pub fn checkout_by_ref(&self, provider_ref: &str) -> Option<&Checkout> {
        self.checkouts
            .values()
            .find(|checkout| checkout.provider_ref == provider_ref)
    }

    /// What the workspace is entitled to right now: the configured plan
    /// its active, past-due, or cancelled-but-unexpired subscription
    /// holds — refused when no subscription stands or the plan does not
    /// cover the named door.
    pub fn entitled<'p>(
        &self,
        workspace: &str,
        plans: &'p [Plan],
        model: &str,
        now: u64,
    ) -> Result<&'p Plan, Refusal> {
        let subscription = self
            .subscription_for(workspace)
            .ok_or_else(|| Refusal::NoSubscription(workspace.to_string()))?;
        match subscription.state {
            SubscriptionState::Expired => {
                return Err(Refusal::SubscriptionExpired(workspace.to_string()));
            }
            SubscriptionState::Cancelled if subscription.cancel_at.is_some_and(|at| at <= now) => {
                return Err(Refusal::SubscriptionExpired(workspace.to_string()));
            }
            _ => {}
        }
        let plan = plans
            .iter()
            .find(|plan| plan.id == subscription.plan)
            .ok_or_else(|| Refusal::UnknownPlan(subscription.plan.clone()))?;
        if !plan.models.permits(model) {
            return Err(Refusal::PlanExcludesModel {
                workspace: workspace.to_string(),
                plan: plan.id.clone(),
                model: model.to_string(),
            });
        }
        Ok(plan)
    }

    /// Open a checkout session — one pending session per workspace.
    pub fn open_checkout(
        &mut self,
        workspace: &str,
        intent: Intent,
        plan: &Plan,
        provider_ref: String,
        now: u64,
        ttl_secs: u64,
    ) -> Result<Checkout, Refusal> {
        if self.checkouts.values().any(|checkout| {
            checkout.workspace == workspace && checkout.state == CheckoutState::Pending
        }) {
            return Err(Refusal::CheckoutPending(workspace.to_string()));
        }
        if let Intent::Subscribe { plan: wanted } = &intent {
            if wanted != &plan.id {
                return Err(Refusal::UnknownPlan(wanted.clone()));
            }
            if plan.price.is_free() {
                return Err(Refusal::FreePlan(plan.id.clone()));
            }
            if self
                .subscription_for(workspace)
                .is_some_and(|sub| sub.plan == plan.id && sub.state != SubscriptionState::Expired)
            {
                return Err(Refusal::AlreadySubscribed {
                    workspace: workspace.to_string(),
                    plan: plan.id.clone(),
                });
            }
        }
        let checkout = Checkout {
            id: self.mint("cko", now)?,
            workspace: workspace.to_string(),
            intent,
            plan_version: plan.version.clone(),
            provider_ref,
            state: CheckoutState::Pending,
            created: now,
            expires_at: now.saturating_add(ttl_secs),
            resolved: None,
        };
        self.checkouts.insert(checkout.id.clone(), checkout.clone());
        Ok(checkout)
    }

    /// Subscribe a workspace to a free plan directly — no checkout, no
    /// payment, the same record shape a completed checkout produces.
    pub fn subscribe(
        &mut self,
        workspace: &str,
        plan: &Plan,
        now: u64,
    ) -> Result<(Subscription, Vec<Effect>), Refusal> {
        if !plan.price.is_free() {
            return Err(Refusal::CheckoutRequired(plan.id.clone()));
        }
        if self
            .subscription_for(workspace)
            .is_some_and(|sub| sub.state != SubscriptionState::Expired)
        {
            return Err(Refusal::AlreadySubscribed {
                workspace: workspace.to_string(),
                plan: self
                    .subscription_for(workspace)
                    .map(|sub| sub.plan.clone())
                    .unwrap_or_default(),
            });
        }
        let mut subscription = Subscription {
            id: self.mint("sub", now)?,
            workspace: workspace.to_string(),
            plan: plan.id.clone(),
            plan_version: plan.version.clone(),
            state: SubscriptionState::Active,
            period: 1,
            period_started: now,
            period_ends: now.saturating_add(plan.price.period_secs),
            pending_plan: None,
            cancel_at: None,
            provider_ref: None,
            grace_ends: None,
            history: Vec::new(),
        };
        subscription.history.push(Transition {
            at: now,
            action: "created".to_string(),
            plan: plan.id.clone(),
            period: 1,
            source: "subscribe".to_string(),
        });
        let effects = self.activation_effects(&subscription, plan, now, "subscribe");
        self.subscriptions
            .insert(subscription.id.clone(), subscription.clone());
        self.by_workspace
            .insert(workspace.to_string(), subscription.id.clone());
        Ok((subscription, effects))
    }

    /// The grant and account effects a fresh or renewed subscription
    /// owes: the money account itself, the sign-up credit once, the
    /// period's allowance once, and the plan's seats.
    fn activation_effects(
        &mut self,
        subscription: &Subscription,
        plan: &Plan,
        now: u64,
        source: &str,
    ) -> Vec<Effect> {
        let mut effects = vec![
            Effect::CreateAccount {
                workspace: subscription.workspace.clone(),
                currency: plan.price.currency.clone(),
                spend_limit: plan.spend_limit,
                topups_allowed: plan.topups_allowed,
            },
            Effect::SetSeats {
                workspace: subscription.workspace.clone(),
                seats: plan.seats,
            },
        ];
        if plan.signup_credit > 0 {
            let grant_source = format!("billing:signup:{}", subscription.workspace);
            if let std::collections::btree_map::Entry::Vacant(slot) =
                self.grants.entry(grant_source.clone())
            {
                let audit = format!("{source}: sign-up credit on plan {}", plan.id);
                slot.insert(Grant {
                    source: grant_source.clone(),
                    workspace: subscription.workspace.clone(),
                    kind: "signup-credit".to_string(),
                    amount: plan.signup_credit,
                    currency: plan.price.currency.clone(),
                    issued: now,
                    expires_at: None,
                    invoice: None,
                    audit: audit.clone(),
                });
                effects.push(Effect::Credit {
                    workspace: subscription.workspace.clone(),
                    source: grant_source,
                    amount: plan.signup_credit,
                    credit_kind: "grant".to_string(),
                    audit,
                });
            }
        }
        if plan.allowance > 0 {
            let grant_source =
                format!("billing:{}:period:{}", subscription.id, subscription.period);
            let audit = format!(
                "{source}: {} period {} allowance",
                plan.id, subscription.period
            );
            self.grants.insert(
                grant_source.clone(),
                Grant {
                    source: grant_source.clone(),
                    workspace: subscription.workspace.clone(),
                    kind: "period-allowance".to_string(),
                    amount: plan.allowance,
                    currency: plan.price.currency.clone(),
                    issued: now,
                    expires_at: plan.credit_expiry_secs.map(|secs| now.saturating_add(secs)),
                    invoice: None,
                    audit: audit.clone(),
                },
            );
            effects.push(Effect::Credit {
                workspace: subscription.workspace.clone(),
                source: grant_source,
                amount: plan.allowance,
                credit_kind: "grant".to_string(),
                audit,
            });
        }
        effects
    }

    /// Journal an event as received — the dedup decision happens here,
    /// before any effect is computed.
    pub fn receive(&mut self, mut event: Event, now: u64) -> Result<Event, Refusal> {
        let key = format!("{}:{}", event.provider, event.id);
        if self.events.contains_key(&key) {
            return Err(Refusal::DuplicateEvent(key));
        }
        if self.events.len() >= EVENTS_MAX {
            return Err(Refusal::EventsBounded(EVENTS_MAX));
        }
        event.received = now;
        event.applied = false;
        event.outcome = "received".to_string();
        self.events.insert(key, event.clone());
        Ok(event)
    }

    /// Mark a journaled event applied with its outcome.
    pub fn conclude(&mut self, provider: &str, id: &str, outcome: &Outcome) -> Result<(), Refusal> {
        let key = format!("{provider}:{id}");
        let event = self
            .events
            .get_mut(&key)
            .ok_or_else(|| Refusal::UnknownEvent(key.clone()))?;
        event.applied = true;
        event.outcome = outcome.name();
        Ok(())
    }

    /// Apply a provider event to the book. The caller journals first
    /// through `receive`, applies the returned [`Effect`]s, then calls
    /// `conclude` — so a crash mid-application leaves the event
    /// `received` and `reconcile` replays it against idempotent stores.
    pub fn apply_event(&mut self, event: &Event, plans: &[Plan], now: u64) -> Outcome {
        match event.kind.as_str() {
            "checkout-completed" => self.checkout_completed(event, plans, now),
            "invoice-paid" => self.invoice_paid(event, plans, now),
            "invoice-failed" => self.invoice_failed(event, now),
            "subscription-cancelled" => self.subscription_cancelled(event, now),
            "charge-refunded" => self.charge_refunded(event, now, false),
            "charge-disputed" => self.charge_refunded(event, now, true),
            other => Outcome::Ignored(format!("unknown event kind `{other}`")),
        }
    }

    fn checkout_completed(&mut self, event: &Event, plans: &[Plan], now: u64) -> Outcome {
        let checkout_id = match &event.checkout {
            Some(id) => id.clone(),
            None => return Outcome::Ignored("no checkout named".into()),
        };
        let checkout = match self.checkouts.get_mut(&checkout_id) {
            Some(checkout) => checkout,
            None => return Outcome::Ignored(format!("unknown checkout `{checkout_id}`")),
        };
        if checkout.state == CheckoutState::Complete {
            return Outcome::Superseded("checkout already complete".into());
        }
        if checkout.state == CheckoutState::Expired {
            return Outcome::Ignored("checkout expired before payment".into());
        }
        if let Some(reference) = &event.provider_ref
            && reference != &checkout.provider_ref
        {
            return Outcome::Ignored("provider reference mismatch".into());
        }
        checkout.state = CheckoutState::Complete;
        checkout.resolved = Some(now);
        let checkout = checkout.clone();
        match &checkout.intent {
            Intent::TopUp { amount, currency } => {
                if *amount == 0 {
                    return Outcome::Ignored("zero-amount top-up".into());
                }
                let source = format!("billing:topup:{}", checkout.id);
                let audit = format!("top-up via checkout {}", checkout.id);
                self.grants.insert(
                    source.clone(),
                    Grant {
                        source: source.clone(),
                        workspace: checkout.workspace.clone(),
                        kind: "top-up".to_string(),
                        amount: *amount,
                        currency: currency.clone(),
                        issued: now,
                        expires_at: None,
                        invoice: event.invoice.clone(),
                        audit: audit.clone(),
                    },
                );
                Outcome::Applied(vec![Effect::Credit {
                    workspace: checkout.workspace.clone(),
                    source,
                    amount: *amount,
                    credit_kind: "top-up".to_string(),
                    audit,
                }])
            }
            Intent::Subscribe { plan: plan_id } => {
                let plan = match plans.iter().find(|plan| &plan.id == plan_id) {
                    Some(plan) => plan,
                    None => return Outcome::Ignored(format!("unknown plan `{plan_id}`")),
                };
                if self
                    .subscription_for(&checkout.workspace)
                    .is_some_and(|sub| {
                        sub.plan == plan.id && sub.state != SubscriptionState::Expired
                    })
                {
                    return Outcome::Superseded("workspace already on that plan".into());
                }
                let prior = self.by_workspace.get(&checkout.workspace).cloned();
                let subscription_id = match self.mint("sub", now) {
                    Ok(id) => id,
                    Err(_) => return Outcome::Ignored("unavailable".into()),
                };
                let mut subscription = Subscription {
                    id: subscription_id.clone(),
                    workspace: checkout.workspace.clone(),
                    plan: plan.id.clone(),
                    plan_version: checkout.plan_version.clone(),
                    state: SubscriptionState::Active,
                    period: 1,
                    period_started: now,
                    period_ends: now.saturating_add(plan.price.period_secs),
                    pending_plan: None,
                    cancel_at: None,
                    provider_ref: event.provider_ref.clone(),
                    grace_ends: None,
                    history: Vec::new(),
                };
                subscription.history.push(Transition {
                    at: now,
                    action: "created".to_string(),
                    plan: plan.id.clone(),
                    period: 1,
                    source: format!("checkout {}", checkout.id),
                });
                let effects = self.activation_effects(
                    &subscription,
                    plan,
                    now,
                    &format!("checkout {}", checkout.id),
                );
                self.subscriptions
                    .insert(subscription_id.clone(), subscription);
                self.by_workspace
                    .insert(checkout.workspace.clone(), subscription_id.clone());
                if let Some(prior_id) = prior
                    && prior_id != subscription_id
                    && let Some(old) = self.subscriptions.get_mut(&prior_id)
                {
                    old.state = SubscriptionState::Expired;
                }
                Outcome::Applied(effects)
            }
        }
    }

    fn invoice_paid(&mut self, event: &Event, plans: &[Plan], now: u64) -> Outcome {
        let subscription_id = match &event.subscription {
            Some(id) => id.clone(),
            None => return Outcome::Ignored("no subscription named".into()),
        };
        let invoice_id = match &event.invoice {
            Some(id) => id.clone(),
            None => match self.mint("inv", now) {
                Ok(id) => id,
                Err(refusal) => return Outcome::Ignored(refusal.code().to_string()),
            },
        };
        let subscription = match self.subscriptions.get_mut(&subscription_id) {
            Some(subscription) => subscription,
            None => {
                return Outcome::Ignored(format!("unknown subscription `{subscription_id}`"));
            }
        };
        let plan = match plans.iter().find(|plan| plan.id == subscription.plan) {
            Some(plan) => plan.clone(),
            None => {
                return Outcome::Ignored(format!("unknown plan `{}`", subscription.plan));
            }
        };
        // Upsert the invoice first — refund and dispute events name it later.
        let workspace = subscription.workspace.clone();
        self.invoices
            .entry(invoice_id.clone())
            .or_insert_with(|| Invoice {
                id: invoice_id.clone(),
                workspace: workspace.clone(),
                subscription: Some(subscription_id.clone()),
                period: event.period,
                amount: event.amount,
                currency: event
                    .currency
                    .clone()
                    .unwrap_or_else(|| plan.price.currency.clone()),
                state: InvoiceState::Open,
                provider_ref: event.provider_ref.clone().unwrap_or_default(),
                created: now,
                resolved: None,
            });
        if event.period <= subscription.period && subscription.state == SubscriptionState::Active {
            self.invoices.get_mut(&invoice_id).unwrap().state = InvoiceState::Paid;
            return Outcome::Superseded(format!(
                "period already stands at {}",
                subscription.period
            ));
        }
        // A paid invoice for a later period renews the subscription.
        if event.period > subscription.period {
            let mut next_plan = plan.clone();
            if let Some(pending) = subscription.pending_plan.take()
                && let Some(plan) = plans.iter().find(|plan| plan.id == pending)
            {
                next_plan = plan.clone();
                subscription.history.push(Transition {
                    at: now,
                    action: "plan-changed".to_string(),
                    plan: next_plan.id.clone(),
                    period: event.period,
                    source: format!("invoice {invoice_id}"),
                });
            }
            subscription.period = event.period;
            subscription.period_started = now;
            subscription.period_ends = now.saturating_add(next_plan.price.period_secs);
            subscription.plan = next_plan.id.clone();
            subscription.plan_version = next_plan.version.clone();
            subscription.state = SubscriptionState::Active;
            subscription.grace_ends = None;
            subscription.history.push(Transition {
                at: now,
                action: "renewed".to_string(),
                plan: next_plan.id.clone(),
                period: event.period,
                source: format!("invoice {invoice_id}"),
            });
            let invoice = self.invoices.get_mut(&invoice_id).unwrap();
            invoice.state = InvoiceState::Paid;
            invoice.resolved = Some(now);
            let subscription = subscription.clone();
            let effects = self.activation_effects(
                &subscription,
                &next_plan,
                now,
                &format!("invoice {invoice_id}"),
            );
            return Outcome::Applied(effects);
        }
        // Same-period paid after a failure — recovery inside the period.
        if subscription.state == SubscriptionState::PastDue {
            subscription.state = SubscriptionState::Active;
            subscription.grace_ends = None;
            subscription.history.push(Transition {
                at: now,
                action: "recovered".to_string(),
                plan: plan.id.clone(),
                period: subscription.period,
                source: format!("invoice {invoice_id}"),
            });
            let invoice = self.invoices.get_mut(&invoice_id).unwrap();
            invoice.state = InvoiceState::Paid;
            invoice.resolved = Some(now);
            return Outcome::Applied(Vec::new());
        }
        Outcome::Superseded("invoice already resolved".into())
    }

    fn invoice_failed(&mut self, event: &Event, now: u64) -> Outcome {
        let subscription_id = match &event.subscription {
            Some(id) => id.clone(),
            None => return Outcome::Ignored("no subscription named".into()),
        };
        let subscription = match self.subscriptions.get_mut(&subscription_id) {
            Some(subscription) => subscription,
            None => {
                return Outcome::Ignored(format!("unknown subscription `{subscription_id}`"));
            }
        };
        if let Some(invoice_id) = &event.invoice {
            let workspace = subscription.workspace.clone();
            let invoice = self
                .invoices
                .entry(invoice_id.clone())
                .or_insert_with(|| Invoice {
                    id: invoice_id.clone(),
                    workspace,
                    subscription: Some(subscription_id.clone()),
                    period: event.period,
                    amount: event.amount,
                    currency: event.currency.clone().unwrap_or_else(|| "usd".into()),
                    state: InvoiceState::Open,
                    provider_ref: event.provider_ref.clone().unwrap_or_default(),
                    created: now,
                    resolved: None,
                });
            invoice.state = InvoiceState::Failed;
            invoice.resolved = Some(now);
        }
        if subscription.state == SubscriptionState::Expired {
            return Outcome::Superseded("subscription already expired".into());
        }
        if subscription.state == SubscriptionState::PastDue {
            return Outcome::Superseded("already past due".into());
        }
        subscription.state = SubscriptionState::PastDue;
        subscription.grace_ends = Some(subscription.period_ends);
        subscription.history.push(Transition {
            at: now,
            action: "past-due".to_string(),
            plan: subscription.plan.clone(),
            period: subscription.period,
            source: event
                .invoice
                .clone()
                .map(|id| format!("invoice {id}"))
                .unwrap_or_else(|| "provider".to_string()),
        });
        Outcome::Applied(Vec::new())
    }

    fn subscription_cancelled(&mut self, event: &Event, now: u64) -> Outcome {
        let subscription_id = match &event.subscription {
            Some(id) => id.clone(),
            None => return Outcome::Ignored("no subscription named".into()),
        };
        let subscription = match self.subscriptions.get_mut(&subscription_id) {
            Some(subscription) => subscription,
            None => {
                return Outcome::Ignored(format!("unknown subscription `{subscription_id}`"));
            }
        };
        if subscription.state == SubscriptionState::Expired
            || subscription.state == SubscriptionState::Cancelled
        {
            return Outcome::Superseded("subscription already closed".into());
        }
        subscription.state = SubscriptionState::Cancelled;
        subscription.cancel_at = Some(if event.at_period_end {
            subscription.period_ends
        } else {
            now
        });
        subscription.pending_plan = None;
        subscription.history.push(Transition {
            at: now,
            action: "cancelled".to_string(),
            plan: subscription.plan.clone(),
            period: subscription.period,
            source: event.provider.clone(),
        });
        Outcome::Applied(Vec::new())
    }

    fn charge_refunded(&mut self, event: &Event, now: u64, disputed: bool) -> Outcome {
        let invoice_id = match &event.invoice {
            Some(id) => id.clone(),
            None => return Outcome::Ignored("no invoice named".into()),
        };
        let invoice = match self.invoices.get_mut(&invoice_id) {
            Some(invoice) => invoice,
            None => return Outcome::Ignored(format!("unknown invoice `{invoice_id}`")),
        };
        if invoice.state == InvoiceState::Refunded || invoice.state == InvoiceState::Disputed {
            return Outcome::Superseded("invoice already closed".into());
        }
        invoice.state = if disputed {
            InvoiceState::Disputed
        } else {
            InvoiceState::Refunded
        };
        invoice.resolved = Some(now);
        let amount = if event.amount > 0 {
            event.amount
        } else {
            invoice.amount
        };
        let kind = if disputed { "dispute" } else { "refund" };
        let source = format!("billing:{kind}:{invoice_id}");
        let audit = format!("{kind} on invoice {invoice_id}");
        self.clawbacks.insert(
            source.clone(),
            Clawback {
                source: source.clone(),
                workspace: invoice.workspace.clone(),
                amount,
                kind: kind.to_string(),
                issued: now,
                audit: audit.clone(),
            },
        );
        Outcome::Applied(vec![Effect::Debit {
            workspace: invoice.workspace.clone(),
            source,
            amount,
            audit,
        }])
    }

    /// Cancel through the management route — identical state change to
    /// a provider `subscription-cancelled` event, different source.
    pub fn cancel(&mut self, workspace: &str, now: u64) -> Result<Subscription, Refusal> {
        let subscription = self
            .subscription_for(workspace)
            .ok_or_else(|| Refusal::NoSubscription(workspace.to_string()))?;
        if subscription.state != SubscriptionState::Active
            && subscription.state != SubscriptionState::PastDue
        {
            return Err(Refusal::SubscriptionClosed(subscription.id.clone()));
        }
        let id = subscription.id.clone();
        let subscription = self.subscriptions.get_mut(&id).unwrap();
        subscription.state = SubscriptionState::Cancelled;
        subscription.cancel_at = Some(subscription.period_ends);
        subscription.pending_plan = None;
        subscription.history.push(Transition {
            at: now,
            action: "cancelled".to_string(),
            plan: subscription.plan.clone(),
            period: subscription.period,
            source: "workspace".to_string(),
        });
        Ok(subscription.clone())
    }

    /// Schedule a plan change at the next paid renewal — the downgrade
    /// path. An immediate upgrade travels through checkout instead.
    pub fn change_plan(
        &mut self,
        workspace: &str,
        plan: &Plan,
        now: u64,
    ) -> Result<Subscription, Refusal> {
        let subscription = self
            .subscription_for(workspace)
            .ok_or_else(|| Refusal::NoSubscription(workspace.to_string()))?;
        if subscription.plan == plan.id && subscription.pending_plan.is_none() {
            return Err(Refusal::AlreadySubscribed {
                workspace: workspace.to_string(),
                plan: plan.id.clone(),
            });
        }
        if subscription.state == SubscriptionState::Expired
            || subscription.state == SubscriptionState::Cancelled
        {
            return Err(Refusal::SubscriptionClosed(subscription.id.clone()));
        }
        let id = subscription.id.clone();
        let subscription = self.subscriptions.get_mut(&id).unwrap();
        subscription.pending_plan = Some(plan.id.clone());
        subscription.history.push(Transition {
            at: now,
            action: "plan-scheduled".to_string(),
            plan: plan.id.clone(),
            period: subscription.period,
            source: "workspace".to_string(),
        });
        Ok(subscription.clone())
    }

    /// The time-based sweep every mutation runs: pending checkouts past
    /// their deadline expire, cancelled subscriptions past their
    /// period end expire, and past-due subscriptions past grace expire.
    pub fn expire(&mut self, now: u64) {
        for checkout in self.checkouts.values_mut() {
            if checkout.state == CheckoutState::Pending && checkout.expires_at <= now {
                checkout.state = CheckoutState::Expired;
                checkout.resolved = Some(now);
            }
        }
        for subscription in self.subscriptions.values_mut() {
            let expires = match subscription.state {
                SubscriptionState::Cancelled => subscription.cancel_at.is_some_and(|at| at <= now),
                SubscriptionState::PastDue => subscription.grace_ends.is_some_and(|at| at <= now),
                _ => false,
            };
            if expires {
                subscription.state = SubscriptionState::Expired;
                subscription.history.push(Transition {
                    at: now,
                    action: "expired".to_string(),
                    plan: subscription.plan.clone(),
                    period: subscription.period,
                    source: "sweep".to_string(),
                });
            }
        }
    }

    /// Grants whose allowance expiry has passed and whose clawback was
    /// not yet applied — one debit effect each, idempotent by source.
    pub fn expired_grants(&self, now: u64) -> Vec<Effect> {
        self.grants
            .values()
            .filter(|grant| {
                grant.expires_at.is_some_and(|at| at <= now)
                    && !self
                        .grants
                        .contains_key(&format!("expired:{}", grant.source))
            })
            .map(|grant| Effect::Debit {
                workspace: grant.workspace.clone(),
                source: format!("expired:{}", grant.source),
                amount: grant.amount,
                audit: format!("expired {} allowance", grant.kind),
            })
            .collect()
    }

    /// Mark an expired-grant clawback recorded — after the adapter
    /// applies the debit.
    pub fn close_grant(&mut self, source: &str, now: u64) -> Result<(), Refusal> {
        let grant = self
            .grants
            .get(source)
            .ok_or_else(|| Refusal::UnknownGrant(source.to_string()))?
            .clone();
        self.grants.insert(
            format!("expired:{source}"),
            Grant {
                source: format!("expired:{source}"),
                kind: "expiry".to_string(),
                amount: 0,
                issued: now,
                expires_at: Some(now),
                invoice: grant.invoice.clone(),
                workspace: grant.workspace.clone(),
                currency: grant.currency.clone(),
                audit: grant.audit.clone(),
            },
        );
        Ok(())
    }

    /// What a restart or a lost delivery still owes: received-but-
    /// unapplied events to replay, and pending checkouts whose provider
    /// references are worth re-fetching.
    #[must_use]
    pub fn reconcile(&self) -> Reconciliation {
        Reconciliation {
            stale_refs: self
                .checkouts
                .values()
                .filter(|checkout| checkout.state == CheckoutState::Pending)
                .map(|checkout| checkout.provider_ref.clone())
                .collect(),
            unapplied: self
                .events
                .values()
                .filter(|event| !event.applied)
                .map(|event| format!("{}:{}", event.provider, event.id))
                .collect(),
        }
    }
}

/// A typed refusal from the billing book.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The workspace holds no subscription.
    NoSubscription(String),
    /// The workspace's subscription ended.
    SubscriptionExpired(String),
    /// The subscription is already closed for the attempted change.
    SubscriptionClosed(String),
    /// The workspace is already on the plan named.
    AlreadySubscribed {
        /// The workspace.
        workspace: String,
        /// The plan it already holds.
        plan: String,
    },
    /// A paid plan's change must travel through checkout.
    CheckoutRequired(String),
    /// A pending checkout already stands for the workspace.
    CheckoutPending(String),
    /// A free plan subscribes directly — checkout is a payment page.
    FreePlan(String),
    /// The plan id names no configured plan.
    UnknownPlan(String),
    /// The workspace's plan does not cover the named door.
    PlanExcludesModel {
        /// The workspace.
        workspace: String,
        /// The plan held.
        plan: String,
        /// The door refused.
        model: String,
    },
    /// The event id is already journaled.
    DuplicateEvent(String),
    /// The event journal hit its bound.
    EventsBounded(usize),
    /// The event names nothing journaled.
    UnknownEvent(String),
    /// The grant source names nothing issued.
    UnknownGrant(String),
    /// The mint's entropy source failed.
    Unavailable,
    /// The store could not be read or written.
    Store(String),
}

impl Refusal {
    /// The stable refusal code — what the adapter maps to HTTP.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoSubscription(_) => "no_subscription",
            Self::SubscriptionExpired(_) => "subscription_expired",
            Self::SubscriptionClosed(_) => "subscription_closed",
            Self::AlreadySubscribed { .. } => "already_subscribed",
            Self::CheckoutRequired(_) => "checkout_required",
            Self::CheckoutPending(_) => "checkout_pending",
            Self::FreePlan(_) => "free_plan",
            Self::UnknownPlan(_) => "unknown_plan",
            Self::PlanExcludesModel { .. } => "plan_excludes_model",
            Self::DuplicateEvent(_) => "duplicate_event",
            Self::EventsBounded(_) => "events_bounded",
            Self::UnknownEvent(_) => "unknown_event",
            Self::UnknownGrant(_) => "unknown_grant",
            Self::Unavailable => "unavailable",
            Self::Store(_) => "billing_unavailable",
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSubscription(workspace) => {
                write!(f, "workspace `{workspace}` has no subscription")
            }
            Self::SubscriptionExpired(workspace) => {
                write!(f, "the subscription on `{workspace}` has expired")
            }
            Self::SubscriptionClosed(workspace) => {
                write!(
                    f,
                    "the subscription on `{workspace}` can't be changed in its current state"
                )
            }
            Self::AlreadySubscribed { workspace, plan } => {
                write!(f, "workspace `{workspace}` is already on plan `{plan}`")
            }
            Self::CheckoutRequired(plan) => {
                write!(
                    f,
                    "plan `{plan}` is a paid plan; start a checkout to subscribe"
                )
            }
            Self::CheckoutPending(workspace) => {
                write!(f, "workspace `{workspace}` already has a pending checkout")
            }
            Self::FreePlan(plan) => {
                write!(
                    f,
                    "plan `{plan}` is free; subscribe to it directly without a checkout"
                )
            }
            Self::UnknownPlan(plan) => write!(f, "plan `{plan}` doesn't exist"),
            Self::PlanExcludesModel {
                workspace,
                plan,
                model,
            } => write!(
                f,
                "workspace `{workspace}` is on plan `{plan}`, which doesn't include model `{model}`"
            ),
            Self::DuplicateEvent(key) => write!(f, "event `{key}` was already recorded"),
            Self::EventsBounded(bound) => {
                write!(f, "the event log is full at its {bound}-event limit")
            }
            Self::UnknownEvent(key) => write!(f, "event `{key}` isn't recorded"),
            Self::UnknownGrant(source) => write!(f, "credit grant `{source}` doesn't exist"),
            Self::Unavailable => write!(f, "the service couldn't generate a secure ID; try again"),
            Self::Store(detail) => {
                write!(f, "the billing records couldn't be read or saved: {detail}")
            }
        }
    }
}

/// The persisted store envelope — same discipline as `sessions.json`:
/// schema tag, sequence, `supersedes`, self-recomputing digest, and an
/// archive of every sealed revision under `billing-history/`.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Store {
    /// The schema tag.
    pub v: String,
    /// The revision number.
    pub sequence: u64,
    /// The digest this revision replaced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// The lifecycle book.
    pub book: BillingBook,
    /// The bounded billing access log.
    #[serde(default)]
    pub access: Vec<Access>,
    /// The digest over every field above.
    pub digest: String,
}

impl Store {
    fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a store serializes");
        value
            .as_object_mut()
            .expect("a store is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// The checks a store must pass before anything reads it.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if self.v != SCHEMA {
            return Err(format!("{name}: schema `{}` is not `{SCHEMA}`", self.v));
        }
        if self.digest != self.compute_digest() {
            return Err(format!(
                "{name}: the store's digest does not recompute over its contents"
            ));
        }
        for (id, subscription) in &self.book.subscriptions {
            if *id != subscription.id {
                return Err(format!(
                    "{name}: subscription `{id}` is filed under the wrong id"
                ));
            }
        }
        for (id, checkout) in &self.book.checkouts {
            if *id != checkout.id {
                return Err(format!(
                    "{name}: checkout `{id}` is filed under the wrong id"
                ));
            }
        }
        for (workspace, id) in &self.book.by_workspace {
            if !self.book.subscriptions.contains_key(id) {
                return Err(format!(
                    "{name}: workspace `{workspace}` points at a missing subscription"
                ));
            }
        }
        if self.access.len() > ACCESS_MAX {
            return Err(format!(
                "{name}: the access log exceeds {ACCESS_MAX} events"
            ));
        }
        if self.book.events.len() > EVENTS_MAX {
            return Err(format!(
                "{name}: the event journal exceeds {EVENTS_MAX} records"
            ));
        }
        if self.book.grants.len() > GRANTS_MAX {
            return Err(format!(
                "{name}: the grant journal exceeds {GRANTS_MAX} records"
            ));
        }
        Ok(())
    }

    fn parse(text: &str, name: &str) -> Result<Self, String> {
        let store: Self = serde_json::from_str(text).map_err(|error| format!("{name}: {error}"))?;
        store.validate(name)?;
        Ok(store)
    }
}

/// The billing store handle — a directory; every query re-reads and
/// re-validates `billing.json`, and every mutation locks, re-reads,
/// sweeps, applies, seals, and writes in one rename.
#[derive(Clone, Debug)]
pub struct Billing {
    dir: PathBuf,
}

impl Billing {
    /// Create the store's genesis revision in a directory that does not
    /// already hold one.
    pub fn install(dir: &Path) -> Result<Self, Trouble> {
        std::fs::create_dir_all(dir)?;
        let _lock = BillingLock::acquire(dir)?;
        if dir.join(BILLING).exists()
            || (dir.join(HISTORY_DIR).exists()
                && std::fs::read_dir(dir.join(HISTORY_DIR))?.next().is_some())
        {
            return Err(Trouble::Invalid(format!(
                "{} already holds a billing store; open it rather than reinstalling",
                dir.display()
            )));
        }
        let mut store = Store {
            v: SCHEMA.to_string(),
            sequence: 0,
            supersedes: None,
            book: BillingBook::default(),
            access: Vec::new(),
            digest: String::new(),
        };
        store.seal();
        store
            .validate(&dir.join(BILLING).display().to_string())
            .map_err(Trouble::Invalid)?;
        save(dir, &store)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Open the store in a directory, validating it end to end.
    pub fn open(dir: &Path) -> Result<Self, Trouble> {
        load(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Read the current store — the fresh read every query makes.
    pub fn store(&self) -> Result<Store, Trouble> {
        load(&self.dir)
    }

    /// One mutation: lock, re-read inside the lock, run the expiry
    /// sweep, apply `f`, seal, validate, write.
    pub fn mutate<T>(
        &self,
        f: impl FnOnce(&mut BillingBook, &mut Vec<Access>, u64) -> Result<T, Refusal>,
    ) -> Result<T, Refusal> {
        let _lock = BillingLock::acquire(&self.dir).map_err(|t| Refusal::Store(t.to_string()))?;
        let mut store = load(&self.dir).map_err(|t| Refusal::Store(t.to_string()))?;
        let now = unix_now();
        store.book.expire(now);
        let supersedes = store.digest.clone();
        let out = f(&mut store.book, &mut store.access, now)?;
        store.sequence += 1;
        store.supersedes = Some(supersedes);
        store.seal();
        store
            .validate(&self.dir.join(BILLING).display().to_string())
            .map_err(Refusal::Store)?;
        save(&self.dir, &store).map_err(|t| Refusal::Store(t.to_string()))?;
        Ok(out)
    }

    /// Append an access event inside a mutation that does nothing else.
    pub fn record(
        &self,
        actor: &str,
        action: &str,
        workspace: Option<&str>,
        detail: Option<String>,
    ) -> Result<(), Refusal> {
        self.mutate(|_, access, now| {
            push_access(
                access,
                Access {
                    at: now,
                    actor: actor.to_string(),
                    action: action.to_string(),
                    workspace: workspace.map(str::to_string),
                    session: None,
                    detail,
                },
            );
            Ok(())
        })
    }
}

fn load(dir: &Path) -> Result<Store, Trouble> {
    let path = dir.join(BILLING);
    let text = read_billing(&path)?;
    Store::parse(&text, &path.display().to_string()).map_err(Trouble::Invalid)
}

struct BillingLock {
    path: PathBuf,
}

impl BillingLock {
    fn acquire(dir: &Path) -> Result<Self, Trouble> {
        let path = dir.join(LOCK);
        for _ in 0..LOCK_RETRIES {
            match std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
            {
                Ok(mut file) => {
                    writeln!(file, "pid {}", std::process::id()).ok();
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(Trouble::Io(error)),
            }
        }
        Err(Trouble::Locked(path.display().to_string()))
    }
}

impl Drop for BillingLock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

fn read_billing(path: &Path) -> Result<String, Trouble> {
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(STORE_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > STORE_BYTES {
        return Err(Trouble::Invalid("billing store exceeds 16 MiB".into()));
    }
    Ok(text)
}

fn write_synced(path: &Path, text: &str) -> Result<(), Trouble> {
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

/// Write a sealed store: archive it by digest, then replace
/// `billing.json` in one rename.
fn save(dir: &Path, store: &Store) -> Result<(), Trouble> {
    let history = dir.join(HISTORY_DIR);
    std::fs::create_dir_all(&history)?;
    let text =
        serde_json::to_string_pretty(store).map_err(|error| Trouble::Invalid(error.to_string()))?;
    let archived = history.join(format!("{}.json", store.digest));
    if text.len() as u64 + 1 > STORE_BYTES {
        return Err(Trouble::Invalid("billing store exceeds 16 MiB".into()));
    }
    if !archived.exists() {
        write_synced(&archived, &format!("{text}\n"))?;
    } else if read_billing(&archived)? != format!("{text}\n") {
        return Err(Trouble::Invalid(
            "archived revision content mismatch".into(),
        ));
    }
    std::fs::File::open(&history)?.sync_all()?;
    let staged = dir.join(format!(".{BILLING}.{}.tmp", fresh_id()?));
    write_synced(&staged, &format!("{text}\n"))?;
    std::fs::rename(&staged, dir.join(BILLING))?;
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

fn fresh_id() -> Result<String, Trouble> {
    fresh().map_err(|_| Trouble::Invalid("entropy unavailable".into()))
}

/// Canonical JSON: keys sorted, whitespace gone — the same
/// canonicalization `accounts.json` digests under.
fn canonicalize(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        serde_json::Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a value serializes"),
    }
}

/// Fresh random material for record ids — 32 bytes of hex.
fn fresh() -> Result<String, Refusal> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| Refusal::Unavailable)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Fresh random material for a provider reference — 16 hex characters.
/// The adapter mints a checkout's provider-side reference through this
/// so it carries no account or key material.
pub fn fresh_ref() -> Result<String, Refusal> {
    fresh().map(|hex| hex[..16].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(id: &str, amount: u64) -> Plan {
        Plan {
            id: id.to_string(),
            version: "2026-09".to_string(),
            name: id.to_string(),
            price: Price {
                amount,
                currency: "usd".to_string(),
                period_secs: 2_592_000,
            },
            allowance: 5_000_000,
            signup_credit: 1_000_000,
            seats: Some(5),
            models: ModelAccess::Listed(vec!["shared-kev".to_string()]),
            spend_limit: u64::MAX,
            credit_expiry_secs: None,
            topups_allowed: true,
        }
    }

    fn event(kind: &str) -> Event {
        Event {
            provider: "sandbox".to_string(),
            id: "evt_1".to_string(),
            kind: kind.to_string(),
            checkout: None,
            subscription: None,
            invoice: None,
            period: 0,
            amount: 0,
            currency: None,
            at_period_end: true,
            provider_ref: None,
            received: 0,
            applied: false,
            outcome: String::new(),
        }
    }

    #[test]
    fn a_free_subscription_grants_once() {
        let mut book = BillingBook::default();
        let free = plan("free", 0);
        let (subscription, effects) = book.subscribe("ws_1", &free, 100).unwrap();
        assert_eq!(subscription.state, SubscriptionState::Active);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Credit {
                amount: 1_000_000,
                ..
            }
        )));
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::Credit {
                amount: 5_000_000,
                ..
            }
        )));
        assert!(book.subscribe("ws_1", &free, 200).is_err());
    }

    #[test]
    fn a_checkout_completes_into_a_subscription() {
        let mut book = BillingBook::default();
        let pro = plan("pro", 9_000_000);
        let checkout = book
            .open_checkout(
                "ws_1",
                Intent::Subscribe {
                    plan: "pro".to_string(),
                },
                &pro,
                "provider-session-1".to_string(),
                100,
                3_600,
            )
            .unwrap();
        let mut completed = event("checkout-completed");
        completed.checkout = Some(checkout.id.clone());
        completed.provider_ref = Some("provider-session-1".to_string());
        let outcome = book.apply_event(&completed, std::slice::from_ref(&pro), 200);
        assert!(matches!(outcome, Outcome::Applied(_)));
        let subscription = book.subscription_for("ws_1").unwrap();
        assert_eq!(subscription.plan, "pro");
        assert_eq!(subscription.state, SubscriptionState::Active);
        // A replay of the same transition supersedes.
        let mut again = event("checkout-completed");
        again.id = "evt_2".to_string();
        again.checkout = Some(checkout.id.clone());
        assert!(matches!(
            book.apply_event(&again, std::slice::from_ref(&pro), 300),
            Outcome::Superseded(_)
        ));
    }

    #[test]
    fn renewal_grants_once_per_period() {
        let mut book = BillingBook::default();
        let pro = plan("pro", 9_000_000);
        let checkout = book
            .open_checkout(
                "ws_1",
                Intent::Subscribe {
                    plan: "pro".to_string(),
                },
                &pro,
                "ref".to_string(),
                100,
                3_600,
            )
            .unwrap();
        let mut completed = event("checkout-completed");
        completed.checkout = Some(checkout.id.clone());
        book.apply_event(&completed, std::slice::from_ref(&pro), 200);
        let subscription = book.subscription_for("ws_1").unwrap().clone();
        let mut paid = event("invoice-paid");
        paid.subscription = Some(subscription.id.clone());
        paid.invoice = Some("inv_p2".to_string());
        paid.period = 2;
        paid.amount = 9_000_000;
        let outcome = book.apply_event(&paid, std::slice::from_ref(&pro), 300);
        let Outcome::Applied(effects) = outcome else {
            panic!("renewal must apply");
        };
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Credit { source, .. } if source.contains("period:2")))
        );
        assert_eq!(book.subscription_for("ws_1").unwrap().period, 2);
        // The same period's payment again is superseded, never a second grant.
        let mut replay = event("invoice-paid");
        replay.subscription = Some(subscription.id.clone());
        replay.invoice = Some("inv_p2b".to_string());
        replay.period = 2;
        assert!(matches!(
            book.apply_event(&replay, std::slice::from_ref(&pro), 400),
            Outcome::Superseded(_)
        ));
    }

    #[test]
    fn failure_then_recovery_inside_the_period() {
        let mut book = BillingBook::default();
        let free = plan("free", 0);
        book.subscribe("ws_1", &free, 100).unwrap();
        let sub = book.subscription_for("ws_1").unwrap().clone();
        let mut failed = event("invoice-failed");
        failed.subscription = Some(sub.id.clone());
        failed.period = 1;
        book.apply_event(&failed, std::slice::from_ref(&free), 200);
        assert_eq!(
            book.subscription_for("ws_1").unwrap().state,
            SubscriptionState::PastDue
        );
        let mut paid = event("invoice-paid");
        paid.subscription = Some(sub.id.clone());
        paid.period = 1;
        book.apply_event(&paid, std::slice::from_ref(&free), 300);
        assert_eq!(
            book.subscription_for("ws_1").unwrap().state,
            SubscriptionState::Active
        );
    }

    #[test]
    fn a_cancel_expires_at_period_end() {
        let mut book = BillingBook::default();
        let free = plan("free", 0);
        book.subscribe("ws_1", &free, 100).unwrap();
        book.cancel("ws_1", 200).unwrap();
        let subscription = book.subscription_for("ws_1").unwrap();
        assert_eq!(subscription.state, SubscriptionState::Cancelled);
        assert!(
            book.entitled("ws_1", std::slice::from_ref(&free), "shared-kev", 300)
                .is_ok()
        );
        book.expire(free.price.period_secs + 200);
        assert_eq!(
            book.subscription_for("ws_1").unwrap().state,
            SubscriptionState::Expired
        );
        assert!(
            book.entitled(
                "ws_1",
                std::slice::from_ref(&free),
                "shared-kev",
                free.price.period_secs + 300
            )
            .is_err()
        );
    }

    #[test]
    fn a_plan_excludes_a_door() {
        let mut book = BillingBook::default();
        let free = plan("free", 0);
        book.subscribe("ws_1", &free, 100).unwrap();
        assert!(
            book.entitled("ws_1", std::slice::from_ref(&free), "shared-kev", 200)
                .is_ok()
        );
        let refusal = book
            .entitled("ws_1", std::slice::from_ref(&free), "acme-kev", 200)
            .unwrap_err();
        assert_eq!(refusal.code(), "plan_excludes_model");
    }

    #[test]
    fn a_refund_debits_once() {
        let mut book = BillingBook::default();
        let pro = plan("pro", 9_000_000);
        let checkout = book
            .open_checkout(
                "ws_1",
                Intent::Subscribe {
                    plan: "pro".to_string(),
                },
                &pro,
                "ref".to_string(),
                100,
                3_600,
            )
            .unwrap();
        let mut completed = event("checkout-completed");
        completed.checkout = Some(checkout.id.clone());
        completed.invoice = Some("inv_p1".to_string());
        book.apply_event(&completed, std::slice::from_ref(&pro), 200);
        let sub = book.subscription_for("ws_1").unwrap().clone();
        let mut paid = event("invoice-paid");
        paid.subscription = Some(sub.id.clone());
        paid.invoice = Some("inv_p1".to_string());
        paid.period = 2;
        paid.amount = 9_000_000;
        book.apply_event(&paid, std::slice::from_ref(&pro), 300);
        let mut refund = event("charge-refunded");
        refund.invoice = Some("inv_p1".to_string());
        refund.amount = 9_000_000;
        let outcome = book.apply_event(&refund, std::slice::from_ref(&pro), 400);
        assert!(matches!(outcome, Outcome::Applied(_)));
        assert_eq!(book.invoices["inv_p1"].state, InvoiceState::Refunded);
        let mut replay = event("charge-refunded");
        replay.invoice = Some("inv_p1".to_string());
        assert!(matches!(
            book.apply_event(&replay, std::slice::from_ref(&pro), 500),
            Outcome::Superseded(_)
        ));
    }

    #[test]
    fn a_store_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("billing-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Billing::install(&dir).unwrap();
        store
            .mutate(|book, _, now| {
                let free = plan("free", 0);
                book.subscribe("ws_1", &free, now)
            })
            .unwrap();
        let read = Billing::open(&dir).unwrap().store().unwrap();
        assert!(read.book.subscription_for("ws_1").is_some());
        std::fs::remove_dir_all(&dir).ok();
    }
}

//! Native prepaid checkout state in the existing billing journal.
use crate::money::funding::{Quote, Snapshot};
use receipts::purchase::Context;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const RECORDS: usize = 128;
const EVENTS: usize = 4096;
// A shorter bound than the provider's minimum retention leaves recovery margin.
const CREATE_RETRY_SECONDS: u64 = 23 * 3600;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub context: Context,
    pub quote: Quote,
    pub quoted_at: u64,
    pub policy_digest: String,
    pub merchant: String,
    pub live: bool,
    pub deployment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    pub return_origin: String,
    pub customer_reference: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub idempotency: String,
    pub started_at: u64,
    pub native: Option<String>,
}
impl Create {
    pub fn retry(&self, now: u64) -> Result<(), String> {
        if self.native.is_some()
            || now < self.started_at
            || now
                >= self
                    .started_at
                    .checked_add(CREATE_RETRY_SECONDS)
                    .ok_or("Create window overflow.")?
        {
            return Err("The original provider create cannot be replayed in this window.".into());
        }
        Ok(())
    }
    pub fn retain(&mut self, native: &str) -> Result<(), String> {
        if let Some(old) = &self.native {
            if old != native {
                return Err("Provider create differs from its original reference.".into());
            }
        } else {
            self.native = Some(native.into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub checkout: String,
    pub customer: String,
    pub intent: String,
    pub charge: String,
    pub refunds: Vec<String>,
    pub disputes: Vec<String>,
    pub transactions: Vec<String>,
    pub snapshot: Snapshot,
    pub adjustment_fee_units: i64,
    pub excess_removed_units: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Checkout {
    /// Verified unpaid native status. Absence remains unknown, never failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unpaid_status: Option<String>,
    pub binding: Binding,
    pub customer: Option<Create>,
    pub checkout: Option<Create>,
    pub hosted_url: Option<String>,
    pub applied: Option<Observation>,
    pub applying: Option<Observation>,
}
impl Checkout {
    pub fn start_customer(&mut self, idempotency: String, now: u64) -> Result<Create, String> {
        if let Some(old) = &self.customer {
            if old.native.is_none() {
                old.retry(now)?;
            }
            return Ok(old.clone());
        }
        if now >= self.binding.quote.expires_at {
            return Err("The original funding quote expired.".into());
        }
        opaque(&idempotency)?;
        let create = Create {
            idempotency,
            started_at: now,
            native: None,
        };
        self.customer = Some(create.clone());
        Ok(create)
    }
    pub fn start_checkout(&mut self, idempotency: String, now: u64) -> Result<Create, String> {
        if let Some(old) = &self.checkout {
            if old.native.is_none() {
                old.retry(now)?;
            }
            return Ok(old.clone());
        }
        if self
            .customer
            .as_ref()
            .and_then(|c| c.native.as_ref())
            .is_none()
            || now
                .checked_add(1800)
                .is_none_or(|t| t > self.binding.quote.expires_at)
        {
            return Err("The original customer or checkout window is unavailable.".into());
        }
        opaque(&idempotency)?;
        let create = Create {
            idempotency,
            started_at: now,
            native: None,
        };
        self.checkout = Some(create.clone());
        Ok(create)
    }
    /// Persist this result before the adapter sends the stable Money mutation.
    /// A new lookup cannot replace an interrupted original application.
    pub fn stage(&mut self, observation: Observation) -> Result<bool, String> {
        if let Some(pending) = &self.applying {
            if pending == &observation {
                return Ok(false);
            }
            return Err("The original monetary application must reconcile first.".into());
        }
        self.check_observation(&observation)?;
        if let Some(applied) = &self.applied {
            let mut normalized = observation.clone();
            normalized.snapshot.revision = applied.snapshot.revision;
            if &normalized == applied {
                return Ok(false);
            }
        }
        let revision = self
            .applied
            .as_ref()
            .map(|o| o.snapshot.revision)
            .unwrap_or(0);
        if observation.snapshot.revision != revision.checked_add(1).ok_or("Snapshot overflow.")? {
            return Err("Collection revision differs from its original application.".into());
        }
        self.applying = Some(observation);
        Ok(true)
    }
    /// Call only after the ledger accepts or replays that exact mutation.
    pub fn applied(&mut self, snapshot: &Snapshot) -> Result<(), String> {
        if let Some(pending) = &self.applying {
            if &pending.snapshot != snapshot {
                return Err("The original monetary application changed.".into());
            }
            self.applied = self.applying.take();
            return Ok(());
        }
        if self
            .applied
            .as_ref()
            .is_some_and(|o| &o.snapshot == snapshot)
        {
            return Ok(());
        }
        Err("No original monetary application exists.".into())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub body_sha256: String,
    pub kind: String,
    pub object: String,
    pub created_at: u64,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub checkouts: BTreeMap<String, Checkout>,
    pub events: BTreeMap<String, Event>,
}
impl Book {
    pub fn is_empty(&self) -> bool {
        self.checkouts.is_empty() && self.events.is_empty()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.checkouts.len() > RECORDS || self.events.len() > EVENTS {
            return Err("Native prepaid journal exceeds its retained bounds.".into());
        }
        let mut creates = BTreeSet::new();
        let mut checkouts = BTreeSet::new();
        for (id, record) in &self.checkouts {
            record.binding.validate()?;
            if id != &record.binding.quote.id {
                return Err("Native checkout is filed under another original quote.".into());
            }
            for (create, prefix) in [
                (&record.customer, "cus_"),
                (
                    &record.checkout,
                    if record.binding.live {
                        "cs_live_"
                    } else {
                        "cs_test_"
                    },
                ),
            ] {
                if let Some(create) = create {
                    opaque(&create.idempotency)?;
                    if !creates.insert(&create.idempotency) {
                        return Err(
                            "Provider create key belongs to another original operation.".into()
                        );
                    }
                    if create.started_at < record.binding.quoted_at
                        || create.started_at >= record.binding.quote.expires_at
                    {
                        return Err(
                            "Provider create was not admitted in the original quote window.".into(),
                        );
                    }
                    if let Some(native) = &create.native {
                        native_id(native, prefix)?;
                    }
                }
            }
            if record.checkout.is_some()
                && record
                    .customer
                    .as_ref()
                    .and_then(|c| c.native.as_ref())
                    .is_none()
            {
                return Err("Checkout has no original native customer.".into());
            }
            if let Some(native) = record.checkout.as_ref().and_then(|c| c.native.as_ref()) {
                if !checkouts.insert(native) {
                    return Err("Native checkout belongs to another original quote.".into());
                }
            }
            if record.hosted_url.is_some()
                && record
                    .checkout
                    .as_ref()
                    .and_then(|c| c.native.as_ref())
                    .is_none()
            {
                return Err("Hosted checkout has no original native reference.".into());
            }
            if record.unpaid_status.is_some()
                && record
                    .checkout
                    .as_ref()
                    .and_then(|c| c.native.as_ref())
                    .is_none()
            {
                return Err("Unpaid status has no original native checkout.".into());
            }
            if record
                .unpaid_status
                .as_deref()
                .is_some_and(|s| !matches!(s, "pending" | "expired"))
            {
                return Err("Native unpaid status is invalid.".into());
            }
            if let Some(observation) = &record.applied {
                record.check_observation(observation)?;
            }
            if let Some(observation) = &record.applying {
                record.check_observation(observation)?;
                if observation.snapshot.revision
                    != record
                        .applied
                        .as_ref()
                        .map(|o| o.snapshot.revision)
                        .unwrap_or(0)
                        .checked_add(1)
                        .ok_or("Snapshot overflow.")?
                {
                    return Err("Pending collection lost its original predecessor.".into());
                }
            }
        }
        for (id, event) in &self.events {
            native_id(id, "evt_")?;
            if id != &event.id
                || event.body_sha256.len() != 64
                || !event.body_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("Native event identity or body digest is invalid.".into());
            }
        }
        Ok(())
    }
    pub fn admit(&mut self, binding: Binding) -> Result<(), String> {
        binding.validate()?;
        let id = &binding.quote.id;
        if let Some(old) = self.checkouts.get(id) {
            return if old.binding == binding {
                Ok(())
            } else {
                Err("Checkout identity already names different original terms.".into())
            };
        }
        if self.checkouts.len() >= RECORDS {
            return Err("The retained checkout bound is full.".into());
        }
        self.checkouts.insert(
            id.clone(),
            Checkout {
                unpaid_status: None,
                binding,
                customer: None,
                checkout: None,
                hosted_url: None,
                applied: None,
                applying: None,
            },
        );
        Ok(())
    }
    pub fn receive(&mut self, event: Event) -> Result<bool, String> {
        if let Some(old) = self.events.get(&event.id) {
            return if old == &event {
                Ok(false)
            } else {
                Err("Native event identity names different bytes or references.".into())
            };
        }
        if self.events.len() >= EVENTS {
            return Err("The retained native event bound is full.".into());
        }
        self.events.insert(event.id.clone(), event);
        Ok(true)
    }
}
fn opaque(value: &str) -> Result<(), String> {
    if !(16..=128).contains(&value.len())
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("Invalid opaque prepaid reference.".into());
    }
    Ok(())
}

impl Binding {
    /// Old journal records remain readable, but cannot replay a provider call
    /// without the API version admitted before their original create.
    pub fn provider_version(&self) -> Result<&str, String> {
        self.api_version
            .as_deref()
            .ok_or_else(|| "The original provider API version is unavailable.".into())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.api_version.as_deref().is_some_and(|v| {
            v.is_empty()
                || v.len() > 64
                || !v
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        }) {
            return Err("Invalid original provider API version.".into());
        }
        self.context.validate().map_err(str::to_string)?;
        opaque(&self.quote.id)?;
        opaque(&self.customer_reference)?;
        native_id(&self.merchant, "acct_")?;
        if !self.context.can_invoke
            || !matches!(self.context.role.as_str(), "owner" | "admin")
            || self.context.workspace != self.context.payer_workspace
            || self.context.price.currency != "USD"
            || self.quoted_at >= self.quote.expires_at
            || self.quote.expires_at - self.quoted_at > 86_400
            || self.quote.gross_units % 10_000 != 0
            || !(50..=99_999_999).contains(&(self.quote.gross_units / 10_000))
            || self.quote.maximum_fee_units >= self.quote.gross_units
            || !digest(&self.deployment)
            || !digest(&self.policy_digest)
        {
            return Err(
                "Prepaid checkout requires its original USD payer, deployment, and quote.".into(),
            );
        }
        Ok(())
    }
}
impl Checkout {
    fn check_observation(&self, observation: &Observation) -> Result<(), String> {
        let quote = &self.binding.quote;
        let snapshot = &observation.snapshot;
        let funding = &snapshot.funding;
        native_id(&observation.intent, "pi_")?;
        native_id(&observation.charge, "ch_")?;
        if snapshot.quote != quote.id
            || funding.id != quote.id
            || funding.origin != quote.origin
            || funding.policy != quote.policy
            || funding.conversion != quote.conversion
            || funding.gross_units != quote.gross_units
            || funding.fee_units > quote.maximum_fee_units
            || funding.payment != format!("stripe:{}:{}", self.binding.merchant, observation.charge)
            || snapshot.paid_at < self.binding.quoted_at
            || snapshot.paid_at >= quote.expires_at
            || snapshot.revision == 0
            || !digest(&snapshot.evidence)
            || snapshot.processor_expense_units != observation.adjustment_fee_units
            || self.customer.as_ref().and_then(|c| c.native.as_deref())
                != Some(observation.customer.as_str())
            || self.checkout.as_ref().and_then(|c| c.native.as_deref())
                != Some(observation.checkout.as_str())
        {
            return Err("Collection differs from the original admitted checkout.".into());
        }
        for (values, prefix, bound) in [
            (&observation.refunds, "re_", 100),
            (&observation.disputes, "du_", 100),
            (&observation.transactions, "txn_", 401),
        ] {
            if values.len() > bound || values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(
                    "Native collection references exceed their exact retained bounds.".into(),
                );
            }
            for id in values {
                native_id(
                    id,
                    if prefix == "du_" && id.starts_with("dp_") {
                        "dp_"
                    } else {
                        prefix
                    },
                )?;
            }
        }
        Ok(())
    }
}
fn native_id(value: &str, prefix: &str) -> Result<(), String> {
    let suffix = value
        .strip_prefix(prefix)
        .ok_or("Invalid native prepaid reference.")?;
    if suffix.is_empty()
        || value.len() > 128
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err("Invalid native prepaid reference.".into());
    }
    Ok(())
}
fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        billing::Billing,
        money::{Ledger, Mutation, Operation, funding::*},
    };
    fn hash() -> String {
        format!("sha256:{}", "a".repeat(64))
    }
    fn binding(now: u64) -> Binding {
        Binding {
            context: Context {
                schema: receipts::purchase::SCHEMA.into(),
                account: "account".into(),
                workspace: "buyer".into(),
                payer_workspace: "buyer".into(),
                tenant: "tenant".into(),
                credential_reference: "credential".into(),
                membership_epoch: 1,
                workspace_members_epoch: 1,
                role: "owner".into(),
                door: "fixture".into(),
                registry_digest: hash(),
                artifact_digest: hash(),
                price: receipts::purchase::PriceReference {
                    version: "fixture-price".into(),
                    currency: "USD".into(),
                    policy: "fixture-use-v1".into(),
                    terms_digest: hash(),
                    maximum_usage_digest: hash(),
                    maximum_charge: 100,
                },
                can_invoke: true,
                commercial: None,
                team_policy: None,
            },
            quote: Quote {
                id: "native_quote_fixture_001".into(),
                origin: "stripe-card-fixture".into(),
                policy: "native-fixture-v1".into(),
                conversion: "native-usd-v1".into(),
                gross_units: 100_000_000,
                maximum_fee_units: 5_000_000,
                expires_at: now + 3600,
            },
            quoted_at: now,
            policy_digest: hash(),
            merchant: "acct_fixture".into(),
            live: false,
            deployment: hash(),
            api_version: Some("fixture.v1".into()),
            return_origin: "https://fixture.invalid".into(),
            customer_reference: "opaque_customer_fixture_001".into(),
        }
    }
    #[test]
    fn original_provider_version_survives_restart_and_legacy_records_cannot_replay() {
        let original = binding(100);
        let mut book = Book::default();
        book.admit(original.clone()).unwrap();
        let restarted: Book = serde_json::from_slice(&serde_json::to_vec(&book).unwrap()).unwrap();
        restarted.validate().unwrap();
        assert_eq!(
            restarted.checkouts[&original.quote.id]
                .binding
                .provider_version()
                .unwrap(),
            "fixture.v1"
        );
        let mut successor = original.clone();
        successor.api_version = Some("fixture.v2".into());
        assert!(book.admit(successor).is_err());
        let mut legacy = original;
        legacy.api_version = None;
        legacy.validate().unwrap();
        assert!(legacy.provider_version().is_err());
        let value = serde_json::to_value(&legacy).unwrap();
        assert!(value.get("api_version").is_none());
        let read: Binding = serde_json::from_value(value).unwrap();
        assert_eq!(read, legacy);
        assert!(read.provider_version().is_err());
    }
    fn ready(record: &mut Checkout, now: u64) {
        record
            .start_customer("customer_create_fixture_001".into(), now)
            .unwrap();
        record
            .customer
            .as_mut()
            .unwrap()
            .retain("cus_fixture")
            .unwrap();
        record
            .start_checkout("checkout_create_fixture_001".into(), now)
            .unwrap();
        record
            .checkout
            .as_mut()
            .unwrap()
            .retain("cs_test_fixture")
            .unwrap();
    }
    fn observation(record: &Checkout) -> Observation {
        let quote = &record.binding.quote;
        Observation {
            checkout: "cs_test_fixture".into(),
            customer: "cus_fixture".into(),
            intent: "pi_fixture".into(),
            charge: "ch_fixture".into(),
            refunds: vec![],
            disputes: vec![],
            transactions: vec!["txn_original".into()],
            snapshot: Snapshot {
                quote: quote.id.clone(),
                funding: Funding {
                    id: quote.id.clone(),
                    origin: quote.origin.clone(),
                    payment: "stripe:acct_fixture:ch_fixture".into(),
                    policy: quote.policy.clone(),
                    conversion: quote.conversion.clone(),
                    gross_units: quote.gross_units,
                    fee_units: 3_000_000,
                },
                paid_at: record.binding.quoted_at,
                finality: Finality::Final,
                evidence: hash(),
                revision: 1,
                refunded_source_units: 0,
                refund_recovery_proofs: Default::default(),
                disputed_source_units: 0,
                reconciliation_pending: false,
                processor_expense_units: 0,
            },
            adjustment_fee_units: 0,
            excess_removed_units: 0,
        }
    }
    #[test]
    fn journal_retains_original_create_and_scrubbed_event_across_restart() {
        let root = tempfile::tempdir().unwrap();
        let billing = Billing::install(root.path()).unwrap();
        let empty = billing.store().unwrap();
        let json = serde_json::to_value(&empty).unwrap();
        assert!(json["book"].get("prepaid").is_none());
        let now = 100;
        let binding = binding(now);
        let id = binding.quote.id.clone();
        billing
            .mutate(|book, _, _| {
                book.prepaid.admit(binding.clone()).unwrap();
                let record = book.prepaid.checkouts.get_mut(&id).unwrap();
                record
                    .start_customer("customer_create_fixture_001".into(), now)
                    .unwrap();
                Ok(())
            })
            .unwrap();
        drop(billing);
        let billing = Billing::open(root.path()).unwrap();
        let mut book = billing.store().unwrap().book.prepaid;
        let record = book.checkouts.get_mut(&id).unwrap();
        let original = record
            .start_customer("different_key_fixture_001".into(), now + 1)
            .unwrap();
        assert_eq!(original.idempotency, "customer_create_fixture_001");
        assert!(original.retry(now - 1).is_err());
        assert!(original.retry(now + CREATE_RETRY_SECONDS).is_err());
        assert!(
            record
                .start_checkout("checkout_create_fixture_001".into(), now)
                .is_err()
        );
        record
            .customer
            .as_mut()
            .unwrap()
            .retain("cus_fixture")
            .unwrap();
        record
            .start_checkout("checkout_create_fixture_001".into(), now)
            .unwrap();
        record
            .checkout
            .as_mut()
            .unwrap()
            .retain("cs_test_fixture")
            .unwrap();
        assert!(
            record
                .checkout
                .as_mut()
                .unwrap()
                .retain("cs_test_other")
                .is_err()
        );
        let event = Event {
            id: "evt_fixture".into(),
            kind: "checkout.session.completed".into(),
            object: "cs_test_fixture".into(),
            body_sha256: "b".repeat(64),
            created_at: now,
        };
        assert!(book.receive(event.clone()).unwrap());
        assert!(!book.receive(event.clone()).unwrap());
        let mut conflict = event;
        conflict.body_sha256 = "c".repeat(64);
        assert!(book.receive(conflict).is_err());
        book.validate().unwrap();
        billing
            .mutate(|all, _, _| {
                all.prepaid = book.clone();
                Ok(())
            })
            .unwrap();
        assert_eq!(
            Billing::open(root.path())
                .unwrap()
                .store()
                .unwrap()
                .book
                .prepaid
                .events
                .len(),
            1
        );
        assert!(
            serde_json::to_string(&book)
                .unwrap()
                .contains("body_sha256")
        );
    }
    #[test]
    fn interrupted_money_application_replays_original_snapshot_once_before_advancing() {
        let root = tempfile::tempdir().unwrap();
        let billing = Billing::install(root.path()).unwrap();
        let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let apply = |ledger: &mut Ledger, source: &str, operation: Operation| {
            ledger.apply(Mutation {
                workspace: "buyer".into(),
                source: source.into(),
                audit: "isolated native billing fixture".into(),
                operation,
            })
        };
        apply(
            &mut ledger,
            "create",
            Operation::Create {
                currency: "USD".into(),
                spend_limit: 1_000_000_000,
                topups_allowed: true,
            },
        )
        .unwrap();
        let unit = Unit::CurrencyMillionths {
            currency: "USD".into(),
        };
        let policy = Policy {
            schema: POLICY_SCHEMA.into(),
            version: "native-fixture-v1".into(),
            unit: unit.clone(),
            conversions: vec![Conversion {
                version: "native-usd-v1".into(),
                source: unit.clone(),
                target: unit,
                numerator: 1,
                denominator: 1,
                source_ref: "fixture:no-real-money".into(),
                valid_from: 0,
                valid_until: u64::MAX,
                rounding: Rounding::Exact,
                fee_payer: FeePayer::Customer,
                max_fee_units: 5_000_000,
            }],
            purchases: PurchaseTerms {
                required_finality: Finality::Final,
                refunds_allowed: true,
                disputes_allowed: true,
                spent_credit_loss: SpentCreditLoss::Operator,
            },
            promotions: PromotionTerms {
                total_cap: 1,
                grant_cap: 1,
                max_lifetime_seconds: 1,
                max_admissions: 1,
                price_policies: ["fixture-use-v1".into()].into(),
                reversible: true,
            },
        };
        apply(&mut ledger, "policy", Operation::FundingPolicy { policy }).unwrap();
        let mut binding = binding(now);
        binding.policy_digest = ledger.funding_policy("buyer").unwrap().digest().unwrap();
        let id = binding.quote.id.clone();
        apply(
            &mut ledger,
            "quote",
            Operation::QuoteFunding {
                quote: binding.quote.clone(),
            },
        )
        .unwrap();
        binding.quoted_at = ledger
            .statement("buyer")
            .unwrap()
            .funding_quotes
            .iter()
            .find(|q| q.quote.id == id)
            .unwrap()
            .quoted_at;
        let observation = billing
            .mutate(|all, _, _| {
                all.prepaid.admit(binding.clone()).unwrap();
                let record = all.prepaid.checkouts.get_mut(&id).unwrap();
                ready(record, binding.quoted_at);
                let observation = observation(record);
                assert!(record.stage(observation.clone()).unwrap());
                Ok(observation)
            })
            .unwrap();
        let operation = Operation::ReconcileQuotedFunding {
            snapshot: observation.snapshot.clone(),
        };
        assert!(
            apply(
                &mut ledger,
                "billing-card-fixture-revision-1",
                operation.clone()
            )
            .unwrap()
        );
        // Crash after Money committed, before the billing seal acknowledged it.
        drop(ledger);
        drop(billing);
        let billing = Billing::open(root.path()).unwrap();
        let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
        let retained = billing.store().unwrap().book.prepaid.checkouts[&id]
            .applying
            .clone()
            .unwrap();
        assert_eq!(retained, observation);
        assert!(!apply(&mut ledger, "billing-card-fixture-revision-1", operation).unwrap());
        billing
            .mutate(|all, _, _| {
                let record = all.prepaid.checkouts.get_mut(&id).unwrap();
                let mut newer = observation.clone();
                newer.snapshot.revision = 2;
                assert!(record.stage(newer.clone()).is_err());
                record.applied(&observation.snapshot).unwrap();
                assert!(!record.stage(newer).unwrap());
                Ok(())
            })
            .unwrap();
        assert_eq!(ledger.balance("buyer").unwrap().available, 97_000_000);
        let book = billing.store().unwrap().book.prepaid;
        assert!(book.checkouts[&id].applying.is_none());
        assert_eq!(
            book.checkouts[&id]
                .applied
                .as_ref()
                .unwrap()
                .snapshot
                .revision,
            1
        );
    }
}

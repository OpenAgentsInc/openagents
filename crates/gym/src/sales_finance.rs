//! Private operating reports over retained delivery, payment, and bill evidence.
//! This projection transfers no money and never turns activity into revenue.

use crate::sales_evidence::{self, Reference};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub const SCHEMA: &str = "openagents.gym.sales-finance.v1";
const MAX_MANIFEST: usize = 1024 * 1024;
const MAX_REPORT: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 512;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub owner: String,
    pub period_start: u64,
    pub period_end: u64,
    pub inventory: Reference,
    pub comparisons: BTreeMap<String, Reference>,
    pub offers: Vec<Offer>,
    pub gaps: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub schema: String,
    pub entries: Vec<String>,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub id: String,
    pub version: String,
    pub account: String,
    pub cohort: String,
    pub entries: Vec<Entry>,
    /// A retained owner declaration that this class adds no operating expense
    /// beyond costs already reflected in the settlement allocation.
    pub no_cost: BTreeMap<ExpenseClass, Reference>,
    pub assumptions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub version: String,
    pub evidence: Reference,
    pub unit: String,
    pub contractual_charge: u64,
    /// The retained contract explicitly bills this failed work.
    pub billable_failure: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Settlement {
        ledger: Reference,
        key: String,
        attribution: Reference,
    },
    Commercial {
        receipt: Reference,
    },
}
/// A retained invoice/payment or funding statement from the chosen lane's
/// owner. This is an input attestation, not another invoice or payment engine.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommercialReceipt {
    pub schema: String,
    pub id: String,
    pub account: String,
    pub offer_version: String,
    pub at: u64,
    pub kind: CollectionKind,
    pub unit: String,
    pub contractual_charge: u64,
    /// Gross invoice/funding collection. Processing fees use retained expense
    /// rows, rather than masquerading as unpaid invoice balances.
    pub collected: u64,
    pub terms_digest: String,
    pub evidence: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribution {
    pub schema: String,
    pub ledger_digest: String,
    pub settlement: String,
    pub account: String,
    pub offer_version: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CollectionKind {
    Service,
    TopUp,
    Agreement,
    FreeTrial,
    UnspentFunding,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Accepted,
    Failed,
    Pending,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Payer {
    OpenAgents,
    Customer,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskLink {
    pub comparison: String,
    pub task: String,
    pub payer: Payer,
    pub include_baseline_costs: bool,
    pub allocation_evidence: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub at: u64,
    pub terms: Terms,
    pub source: Source,
    pub delivery: Delivery,
    pub delivery_evidence: Reference,
    pub task: Option<TaskLink>,
    pub expenses: Vec<Expense>,
    pub adjustments: Vec<Adjustment>,
    pub incidents: Vec<Incident>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ExpenseClass {
    Provider,
    Compute,
    Payment,
    Setup,
    Onboarding,
    Support,
    Repair,
    Promotion,
    Commission,
}
const REQUIRED_COSTS: [ExpenseClass; 8] = [
    ExpenseClass::Provider,
    ExpenseClass::Compute,
    ExpenseClass::Payment,
    ExpenseClass::Setup,
    ExpenseClass::Onboarding,
    ExpenseClass::Support,
    ExpenseClass::Repair,
    ExpenseClass::Promotion,
];
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Billed,
    Estimate,
    SubscriptionCapacity,
    ProviderGrant,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expense {
    pub id: String,
    pub class: ExpenseClass,
    pub basis: Basis,
    pub unit: String,
    pub amount: Option<u64>,
    pub payer: Payer,
    pub evidence: Option<Reference>,
    pub price: Option<sales_evidence::Price>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdjustmentKind {
    Refund,
    RefundReversal,
    Loss,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdjustmentTarget {
    EarnedCharge,
    Funding,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adjustment {
    pub id: String,
    pub kind: AdjustmentKind,
    pub target: AdjustmentTarget,
    /// The actual OpenAgents economic effect, in this entry's charge unit.
    /// A refunded pass-through is not automatically recovered from its payee.
    pub amount: u64,
    pub evidence: Reference,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IncidentKind {
    FailedProvisioning,
    Replacement,
    Repair,
    Setup,
    Onboarding,
    Support,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Incident {
    pub id: String,
    pub kind: IncidentKind,
    pub responsible_human: String,
    pub elapsed_ms: u64,
    pub evidence: Reference,
    pub expense_ids: Vec<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Revenue {
    pub contractual_charge: u64,
    pub agreed_future_charge: u64,
    /// External collection before the receiver's inbound fee.
    pub gross_collected: u64,
    /// Net wallet collection for ledger sources; inbound fees stay separate.
    pub collected: u64,
    pub funding_collected: u64,
    pub funding_refunds: u64,
    pub funding_refund_reversals: u64,
    pub purchased_balance_consumed: u64,
    pub unspent_funding: u64,
    pub unearned_collection: u64,
    pub unearned_charge: u64,
    pub author_liability: u64,
    pub resource_liability: u64,
    pub promotion_liability: u64,
    pub author_allocated: u64,
    pub resource_allocated: u64,
    pub promotion_allocated: u64,
    /// The recorded OpenAgents share, before separately funded first-call awards.
    pub earned_openagents: u64,
    pub earned_uncollected: u64,
    pub inbound_fees_already_net: u64,
    pub promotions_already_allocated: u64,
    pub funded_promotions: u64,
    pub refunds: u64,
    pub refund_reversals: u64,
    pub losses: u64,
    pub billable_failures: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CostTotal {
    pub class: ExpenseClass,
    pub basis: Basis,
    pub unit: String,
    pub payer: Payer,
    pub known_subtotal: u64,
    pub unknown_items: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperatingView {
    pub offer: String,
    pub cohort: String,
    pub revenue: BTreeMap<String, Revenue>,
    pub costs: Vec<CostTotal>,
    pub missing_cost_classes: Vec<ExpenseClass>,
    pub contribution_known_subtotal: BTreeMap<String, i128>,
    /// None means unknown/incomplete costs or incompatible denominations.
    pub profitable: Option<bool>,
    pub incident_count: u64,
    pub incident_ms: u64,
    pub unresolved_deliveries: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub manifest_digest: String,
    pub manifest: Manifest,
    pub inventory_complete: bool,
    pub offers: Vec<OperatingView>,
    pub cohorts: Vec<OperatingView>,
    pub source_digests: BTreeMap<String, String>,
    pub commissions: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub schema: String,
    pub report_digest: String,
    pub owner: String,
    pub approved: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Aggregate {
    pub schema: String,
    pub views: Vec<OperatingView>,
    pub limitations: Vec<String>,
}

fn text(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        Err("invalid bounded finance identifier".into())
    } else {
        Ok(())
    }
}
fn add(to: &mut u64, amount: u64) -> Result<(), String> {
    *to = to.checked_add(amount).ok_or("finance amount overflow")?;
    Ok(())
}
fn amount(value: i64) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| "negative ledger amount".into())
}
fn cash_unit(unit: &str) -> bool {
    matches!(unit, "msat" | "USD_millionths")
}
fn private_root(root: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(root).map_err(|_| "private finance root is unavailable")?;
    if !meta.is_dir() || meta.file_type().is_symlink() || meta.permissions().mode() & 0o077 != 0 {
        return Err("finance source root must be a private directory".into());
    }
    Ok(())
}
fn expense(
    reader: &mut sales_evidence::Reader<'_>,
    e: &Expense,
    totals: &mut BTreeMap<(ExpenseClass, Basis, String, Payer), CostTotal>,
    seen: &mut BTreeSet<String>,
) -> Result<(), String> {
    text(&e.id)?;
    text(&e.unit)?;
    if !seen.insert(format!("expense:{}", e.id)) {
        return Err("duplicate expense identity".into());
    }
    if e.class == ExpenseClass::Commission {
        return Err(
            "authoritative commission enrichment is unavailable; do not enable it implicitly"
                .into(),
        );
    }
    match e.basis {
        Basis::Billed | Basis::Estimate => {
            if !cash_unit(&e.unit) || e.amount.is_none() || e.evidence.is_none() {
                return Err("cash costs require an exact amount, supported denomination, and retained bill/allocation".into());
            }
        }
        Basis::SubscriptionCapacity if e.unit != "subscription_capacity_units" => {
            return Err("subscription capacity is not cash".into());
        }
        Basis::ProviderGrant if e.unit != "provider_grant_units" => {
            return Err("provider grants are not cash expenses".into());
        }
        Basis::Unknown if e.amount.is_some() => {
            return Err("unknown cost cannot contain an amount".into());
        }
        _ => {}
    }
    if let Some(reference) = &e.evidence {
        reader.read(reference)?;
        if e.basis == Basis::Billed && !seen.insert(format!("bill:{}", reference.sha256)) {
            return Err("duplicate billed source; retain distinct allocated line evidence".into());
        }
    } else if e.basis != Basis::Unknown {
        return Err("known cost requires retained evidence".into());
    }
    if e.basis == Basis::Estimate {
        let price = e
            .price
            .as_ref()
            .ok_or("estimate requires pinned price terms")?;
        text(&price.version)?;
        reader.read(&price.provenance)?;
    }
    let key = (e.class, e.basis, e.unit.clone(), e.payer);
    let total = totals.entry(key).or_insert(CostTotal {
        class: e.class,
        basis: e.basis,
        unit: e.unit.clone(),
        payer: e.payer,
        known_subtotal: 0,
        unknown_items: 0,
    });
    match e.amount {
        Some(value) => add(&mut total.known_subtotal, value)?,
        None => add(&mut total.unknown_items, 1)?,
    }
    Ok(())
}

/// Rebuild the chosen lanes from frozen source bytes. Human attestations prove
/// attribution, not remote delivery or the absence of unrecorded expenses.
pub fn rebuild(root: &Path, bytes: &[u8]) -> Result<Report, String> {
    private_root(root)?;
    if bytes.len() > MAX_MANIFEST {
        return Err("finance manifest exceeds bound".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(bytes).map_err(|_| "malformed finance manifest")?;
    if manifest.schema != SCHEMA
        || manifest.period_start >= manifest.period_end
        || manifest.offers.is_empty()
        || manifest.offers.len() > 64
        || manifest.comparisons.len() > 32
        || manifest.gaps.len() > 64
    {
        return Err("unsupported or oversized finance manifest".into());
    }
    text(&manifest.owner)?;
    for gap in &manifest.gaps {
        text(gap)?;
    }
    let mut reader = sales_evidence::Reader {
        root,
        bytes: 0,
        snapshots: BTreeMap::new(),
    };
    let inventory: Inventory = serde_json::from_slice(&reader.read(&manifest.inventory)?)
        .map_err(|_| "malformed finance inventory")?;
    if inventory.schema != "openagents.gym.sales-finance-inventory.v1"
        || inventory.entries.len() > MAX_ENTRIES
    {
        return Err("unsupported finance inventory".into());
    }
    let mut comparisons = BTreeMap::new();
    let mut comparison_digests = BTreeSet::new();
    for (id, reference) in &manifest.comparisons {
        text(id)?;
        if !comparison_digests.insert(&reference.sha256) {
            return Err("duplicate comparison source".into());
        }
        let study = reader.read(reference)?;
        let report = sales_evidence::rebuild(root, &study)
            .map_err(|_| "retained comparison evidence failed validation")?;
        // Bind the report to every retained source, including failed attempts.
        reader.read(&report.manifest.inventory)?;
        if let Some(r) = &report.manifest.gym_store {
            reader.read(r)?;
        }
        for task in &report.manifest.tasks {
            for attempt in task.baseline.iter().chain(&task.candidate) {
                reader.read(&attempt.trace)?;
                reader.read(&attempt.artifact)?;
                for check in attempt.checks.values() {
                    if let Some(r) = &check.evidence {
                        reader.read(r)?;
                    }
                }
                if let Some(c) = &attempt.compute {
                    reader.read(&c.evidence)?;
                }
                if let Some(a) = &attempt.acceptance {
                    reader.read(&a.check_review)?;
                    reader.read(&a.customer_decision)?;
                }
                for cost in &attempt.costs {
                    if let Some(r) = &cost.evidence {
                        reader.read(r)?;
                    }
                    if let Some(p) = &cost.price {
                        reader.read(&p.provenance)?;
                    }
                }
            }
        }
        comparisons.insert(id.clone(), report);
    }
    let mut ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut tasks = BTreeSet::new();
    let mut bill_sources = BTreeSet::new();
    let mut views = Vec::new();
    let mut offer_ids = BTreeSet::new();
    for offer in &manifest.offers {
        for value in [&offer.id, &offer.version, &offer.account, &offer.cohort] {
            text(value)?;
        }
        if !offer_ids.insert(&offer.id) || offer.assumptions.len() > 32 {
            return Err("duplicate offer or oversized assumptions".into());
        }
        for value in &offer.assumptions {
            text(value)?;
        }
        let mut revenue = BTreeMap::<String, Revenue>::new();
        let mut costs = BTreeMap::new();
        let mut expense_ids = BTreeSet::new();
        let mut incident_count = 0;
        let mut incident_ms = 0;
        let mut unresolved_deliveries = 0;
        for entry in &offer.entries {
            let mut entry_expense_ids = BTreeSet::new();
            text(&entry.id)?;
            text(&entry.terms.version)?;
            text(&entry.terms.unit)?;
            if !ids.insert(entry.id.clone())
                || ids.len() > MAX_ENTRIES
                || entry.at < manifest.period_start
                || entry.at >= manifest.period_end
                || !cash_unit(&entry.terms.unit)
                || entry.expenses.len() > 64
                || entry.adjustments.len() > 32
                || entry.incidents.len() > 32
            {
                return Err("duplicate entry or invalid finance bounds".into());
            }
            reader.read(&entry.terms.evidence)?;
            reader.read(&entry.delivery_evidence)?;
            let mut r = Revenue::default();
            let kind;
            match &entry.source {
                Source::Settlement {
                    ledger,
                    key,
                    attribution,
                } => {
                    text(key)?;
                    // Payment hashes and debit IDs identify the financial event;
                    // a newer snapshot must not turn it into another collection.
                    if !sources.insert(format!("ledger-settlement:{key}")) {
                        return Err("duplicate settlement source".into());
                    }
                    let before = reader.read(ledger)?;
                    let binding: Attribution = serde_json::from_slice(&reader.read(attribution)?)
                        .map_err(|_| "malformed financial attribution")?;
                    if binding.schema != "openagents.sales.financial-attribution.v1"
                        || binding.ledger_digest != ledger.sha256
                        || binding.settlement != *key
                        || binding.account != offer.account
                        || binding.offer_version != offer.version
                    {
                        return Err(
                            "settlement attribution disagrees with customer/offer/source".into(),
                        );
                    }
                    let path = root.join(&ledger.path);
                    for suffix in ["-wal", "-journal"] {
                        if fs::symlink_metadata(format!("{}{suffix}", path.display())).is_ok() {
                            return Err(
                                "retain a checkpointed ledger snapshot without journal sidecars"
                                    .into(),
                            );
                        }
                    }
                    let ledger = pay_ledger::Ledger::open_read_only(&path)
                        .map_err(|_| "retained settlement ledger is unavailable")?;
                    let record = ledger
                        .settlement(key)
                        .map_err(|_| "settlement read refused")?
                        .ok_or("settlement source is missing")?;
                    if entry.terms.unit != "msat"
                        || amount(record.price_msat)? != entry.terms.contractual_charge
                        || u64::try_from(record.settled_at).ok() != Some(entry.at)
                    {
                        return Err("settlement and contract terms disagree".into());
                    }
                    kind = CollectionKind::Service;
                    r.contractual_charge = amount(record.price_msat)?;
                    if record.rail == pay_ledger::Rail::Balance {
                        r.purchased_balance_consumed = amount(record.received_msat)?;
                    } else {
                        r.collected = amount(record.received_msat)?;
                        r.gross_collected = r.collected;
                        add(&mut r.gross_collected, amount(record.lsp_fee_msat)?)?;
                    }
                    r.inbound_fees_already_net = amount(record.lsp_fee_msat)?;
                    for share in &record.shares {
                        let value = amount(share.amount_msat)?;
                        match share.role.as_str() {
                            "openagents" if share.party == pay_ledger::OPENAGENTS => {
                                add(&mut r.earned_openagents, value)?
                            }
                            "author" => add(&mut r.author_allocated, value)?,
                            "resource" | "provider" | "balance_credit" => {
                                add(&mut r.resource_allocated, value)?
                            }
                            "bonus" => {
                                add(&mut r.promotion_allocated, value)?;
                                add(&mut r.promotions_already_allocated, value)?;
                            }
                            "lsp_fee" => {}
                            _ => return Err("unsupported settlement allocation".into()),
                        }
                    }
                    for bonus in &record.bonuses {
                        if bonus.kind == "first_paid_call" {
                            add(&mut r.funded_promotions, amount(bonus.amount_msat)?)?;
                            add(&mut r.promotion_allocated, amount(bonus.amount_msat)?)?;
                        }
                    }
                    for claim in ledger
                        .settlement_liabilities(key)
                        .map_err(|_| "settlement liability read refused")?
                    {
                        match claim.role.as_str() {
                            "author" => add(&mut r.author_liability, amount(claim.amount_msat)?)?,
                            "resource" | "provider" | "balance_credit" => {
                                add(&mut r.resource_liability, amount(claim.amount_msat)?)?
                            }
                            "bonus" | "first_paid_call" => {
                                add(&mut r.promotion_liability, amount(claim.amount_msat)?)?
                            }
                            "openagents" | "lsp_fee" => {}
                            _ => return Err("unsupported settlement liability".into()),
                        }
                    }
                    if sales_evidence::digest(&before)
                        != sales_evidence::digest(&bounded_file(&path, 8 * 1024 * 1024)?)
                    {
                        return Err("ledger snapshot changed while reading".into());
                    }
                }
                Source::Commercial { receipt } => {
                    let record: CommercialReceipt = serde_json::from_slice(&reader.read(receipt)?)
                        .map_err(|_| "malformed commercial receipt")?;
                    if record.schema != "openagents.sales.commercial-receipt.v1"
                        || record.account != offer.account
                        || record.offer_version != offer.version
                        || record.at != entry.at
                        || record.unit != entry.terms.unit
                        || record.contractual_charge != entry.terms.contractual_charge
                        || record.terms_digest != entry.terms.evidence.sha256
                    {
                        return Err("commercial receipt and account/offer/terms disagree".into());
                    }
                    text(&record.id)?;
                    reader.read(&record.evidence)?;
                    if !sources.insert(format!("commercial:{}", record.id)) {
                        return Err("duplicate commercial source".into());
                    }
                    if matches!(record.kind, CollectionKind::Service | CollectionKind::TopUp)
                        && !sources
                            .insert(format!("collection-evidence:{}", record.evidence.sha256))
                    {
                        return Err("duplicate collected-payment evidence".into());
                    }
                    if record.kind == CollectionKind::UnspentFunding
                        && !sources.insert(format!(
                            "funding-position:{}:{}",
                            record.account, record.unit
                        ))
                    {
                        return Err(
                            "multiple unspent funding positions cannot be added as collections"
                                .into(),
                        );
                    }
                    kind = record.kind;
                    r.contractual_charge = record.contractual_charge;
                    r.collected = record.collected;
                    r.gross_collected = record.collected;
                    if kind == CollectionKind::Agreement && r.collected != 0 {
                        return Err("an agreement alone cannot contain a collection".into());
                    }
                    if kind == CollectionKind::FreeTrial
                        && (r.collected != 0 || r.contractual_charge != 0)
                    {
                        return Err("a free trial cannot contain a charge or collection".into());
                    }
                    r.earned_openagents = record.contractual_charge;
                    if kind == CollectionKind::Service
                        && record.collected > record.contractual_charge
                    {
                        return Err("service collection exceeds its contractual charge".into());
                    }
                    if kind == CollectionKind::Service {
                        r.earned_uncollected = record.contractual_charge - record.collected;
                    }
                    if kind == CollectionKind::UnspentFunding {
                        r.unspent_funding = r.collected;
                        r.collected = 0;
                        r.gross_collected = 0;
                    }
                    if kind != CollectionKind::Service && r.contractual_charge != 0 {
                        if kind == CollectionKind::Agreement {
                            r.agreed_future_charge = r.contractual_charge;
                            r.contractual_charge = 0;
                        } else {
                            return Err("funding cannot contain earned contractual charges".into());
                        }
                    }
                }
            }
            let mut accepted = false;
            let mut failed = false;
            if let Some(link) = &entry.task {
                reader.read(&link.allocation_evidence)?;
                let study = comparisons
                    .get(&link.comparison)
                    .ok_or("missing comparison source")?;
                if study.manifest.offer_version != offer.version {
                    return Err("task evidence names another offer version".into());
                }
                let task = study
                    .manifest
                    .tasks
                    .iter()
                    .find(|t| t.id == link.task)
                    .ok_or("task evidence is missing")?;
                if !tasks.insert(format!("{}:{}", study.manifest_digest, task.id)) {
                    return Err("task costs cannot be counted twice".into());
                }
                accepted = task.candidate.iter().any(|a| a.acceptance.is_some());
                failed = task.candidate.iter().any(|a| {
                    a.checks
                        .values()
                        .any(|c| c.status == sales_evidence::Status::Failed)
                });
                let attempts = task
                    .candidate
                    .iter()
                    .chain(task.baseline.iter().filter(|_| link.include_baseline_costs));
                for attempt in attempts {
                    for (index, cost) in attempt.costs.iter().enumerate() {
                        let e = Expense {
                            id: format!("{}:{}:{index}", entry.id, attempt.id),
                            class: match cost.component {
                                sales_evidence::CostComponent::Provider => ExpenseClass::Provider,
                                sales_evidence::CostComponent::Compute => ExpenseClass::Compute,
                                sales_evidence::CostComponent::Support => ExpenseClass::Support,
                            },
                            basis: match cost.basis {
                                sales_evidence::CostBasis::Billed => Basis::Billed,
                                sales_evidence::CostBasis::ListPrice => Basis::Estimate,
                                sales_evidence::CostBasis::SubscriptionCapacity => {
                                    Basis::SubscriptionCapacity
                                }
                                sales_evidence::CostBasis::Unknown => Basis::Unknown,
                            },
                            unit: cost.unit.clone(),
                            amount: cost.amount,
                            payer: link.payer,
                            evidence: cost.evidence.clone(),
                            price: cost.price.clone(),
                        };
                        entry_expense_ids.insert(e.id.clone());
                        expense(&mut reader, &e, &mut costs, &mut bill_sources)?;
                    }
                    for component in [
                        sales_evidence::CostComponent::Provider,
                        sales_evidence::CostComponent::Compute,
                        sales_evidence::CostComponent::Support,
                    ] {
                        if !attempt.costs.iter().any(|c| c.component == component) {
                            let e = Expense {
                                id: format!("{}:{}:missing:{component:?}", entry.id, attempt.id),
                                class: match component {
                                    sales_evidence::CostComponent::Provider => {
                                        ExpenseClass::Provider
                                    }
                                    sales_evidence::CostComponent::Compute => ExpenseClass::Compute,
                                    sales_evidence::CostComponent::Support => ExpenseClass::Support,
                                },
                                basis: Basis::Unknown,
                                unit: "undeclared".into(),
                                amount: None,
                                payer: link.payer,
                                evidence: None,
                                price: None,
                            };
                            entry_expense_ids.insert(e.id.clone());
                            expense(&mut reader, &e, &mut costs, &mut bill_sources)?;
                        }
                    }
                }
            }
            if entry.delivery == Delivery::Accepted && !accepted {
                return Err("accepted revenue requires independently/customer-accepted REV-03 task evidence".into());
            }
            if entry.delivery == Delivery::Failed && accepted {
                return Err("failed delivery contradicts accepted task evidence".into());
            }
            let billable_failed = entry.delivery == Delivery::Failed
                && entry.terms.billable_failure
                && (failed
                    || entry
                        .incidents
                        .iter()
                        .any(|i| i.kind == IncidentKind::FailedProvisioning));
            if kind != CollectionKind::Service {
                r.funding_collected = r.collected;
                r.earned_openagents = 0;
                r.earned_uncollected = 0;
            } else if entry.delivery != Delivery::Accepted && !billable_failed {
                r.unearned_collection = r.collected;
                r.earned_openagents = 0;
                r.earned_uncollected = 0;
                r.unearned_charge = r.contractual_charge;
            } else if billable_failed {
                r.billable_failures = 1;
            }
            if kind == CollectionKind::Service && entry.delivery == Delivery::Pending {
                add(&mut unresolved_deliveries, 1)?;
            }
            for adjustment in &entry.adjustments {
                text(&adjustment.id)?;
                reader.read(&adjustment.evidence)?;
                if !sources.insert(format!("adjustment:{}", adjustment.id)) {
                    return Err("duplicate adjustment source".into());
                }
                if !sources.insert(format!(
                    "adjustment-evidence:{}",
                    adjustment.evidence.sha256
                )) {
                    return Err("duplicate refund/loss evidence".into());
                }
                match (adjustment.target, adjustment.kind) {
                    (AdjustmentTarget::Funding, AdjustmentKind::Refund)
                        if kind != CollectionKind::Service =>
                    {
                        add(&mut r.funding_refunds, adjustment.amount)?
                    }
                    (AdjustmentTarget::Funding, AdjustmentKind::RefundReversal)
                        if kind != CollectionKind::Service =>
                    {
                        add(&mut r.funding_refund_reversals, adjustment.amount)?
                    }
                    (AdjustmentTarget::EarnedCharge, AdjustmentKind::Refund)
                        if kind == CollectionKind::Service =>
                    {
                        add(&mut r.refunds, adjustment.amount)?
                    }
                    (AdjustmentTarget::EarnedCharge, AdjustmentKind::RefundReversal)
                        if kind == CollectionKind::Service =>
                    {
                        add(&mut r.refund_reversals, adjustment.amount)?
                    }
                    (_, AdjustmentKind::Loss) => add(&mut r.losses, adjustment.amount)?,
                    _ => {
                        return Err("refund target disagrees with its funding/charge source".into());
                    }
                }
            }
            if r.refund_reversals > r.refunds || r.funding_refund_reversals > r.funding_refunds {
                return Err("refund reversal exceeds recorded refunds".into());
            }
            for e in &entry.expenses {
                if !expense_ids.insert(e.id.clone()) {
                    return Err("duplicate expense identity".into());
                }
                entry_expense_ids.insert(e.id.clone());
                expense(&mut reader, e, &mut costs, &mut bill_sources)?;
            }
            for incident in &entry.incidents {
                text(&incident.id)?;
                text(&incident.responsible_human)?;
                reader.read(&incident.evidence)?;
                if !sources.insert(format!("incident:{}", incident.id))
                    || incident.expense_ids.is_empty()
                    || incident.expense_ids.len() > 32
                    || incident
                        .expense_ids
                        .iter()
                        .any(|id| !entry_expense_ids.contains(id))
                {
                    return Err(
                        "incident must link its unique retained expense or explicit unknown cost"
                            .into(),
                    );
                }
                add(&mut incident_count, 1)?;
                add(&mut incident_ms, incident.elapsed_ms)?;
            }
            let total = revenue.entry(entry.terms.unit.clone()).or_default();
            merge_revenue(total, &r)?;
        }
        let mut missing = Vec::new();
        for class in REQUIRED_COSTS {
            if let Some(reference) = offer.no_cost.get(&class) {
                reader.read(reference)?;
                if costs.values().any(|c| c.class == class) {
                    return Err("no-cost declaration conflicts with recorded costs".into());
                }
            } else if !costs
                .values()
                .any(|c| c.class == class && matches!(c.basis, Basis::Billed | Basis::Unknown))
                && !(class == ExpenseClass::Promotion
                    && offer
                        .entries
                        .iter()
                        .all(|e| matches!(e.source, Source::Settlement { .. })))
            {
                missing.push(class);
            }
        }
        if offer.no_cost.contains_key(&ExpenseClass::Commission) {
            return Err("commission declarations require the future authoritative adapter".into());
        }
        let mut view = OperatingView {
            offer: offer.id.clone(),
            cohort: offer.cohort.clone(),
            revenue,
            costs: costs.into_values().collect(),
            missing_cost_classes: missing,
            contribution_known_subtotal: BTreeMap::new(),
            profitable: None,
            incident_count,
            incident_ms,
            unresolved_deliveries,
        };
        calculate(&mut view, inventory.complete && manifest.gaps.is_empty())?;
        views.push(view);
    }
    if ids != inventory.entries.iter().cloned().collect::<BTreeSet<_>>()
        || inventory.entries.len() != ids.len()
    {
        return Err("finance entries differ from the frozen inventory".into());
    }
    let mut cohorts = BTreeMap::<String, OperatingView>::new();
    for view in &views {
        let aggregate = cohorts.entry(view.cohort.clone()).or_insert(OperatingView {
            offer: String::new(),
            cohort: view.cohort.clone(),
            revenue: BTreeMap::new(),
            costs: Vec::new(),
            missing_cost_classes: Vec::new(),
            contribution_known_subtotal: BTreeMap::new(),
            profitable: None,
            incident_count: 0,
            incident_ms: 0,
            unresolved_deliveries: 0,
        });
        for (unit, r) in &view.revenue {
            merge_revenue(aggregate.revenue.entry(unit.clone()).or_default(), r)?;
        }
        for cost in &view.costs {
            if let Some(prior) = aggregate.costs.iter_mut().find(|p| {
                p.class == cost.class
                    && p.basis == cost.basis
                    && p.unit == cost.unit
                    && p.payer == cost.payer
            }) {
                add(&mut prior.known_subtotal, cost.known_subtotal)?;
                add(&mut prior.unknown_items, cost.unknown_items)?;
            } else {
                aggregate.costs.push(cost.clone());
            }
        }
        aggregate
            .missing_cost_classes
            .extend(&view.missing_cost_classes);
        add(&mut aggregate.incident_count, view.incident_count)?;
        add(&mut aggregate.incident_ms, view.incident_ms)?;
        add(
            &mut aggregate.unresolved_deliveries,
            view.unresolved_deliveries,
        )?;
    }
    for view in cohorts.values_mut() {
        view.missing_cost_classes.sort();
        view.missing_cost_classes.dedup();
        calculate(view, inventory.complete && manifest.gaps.is_empty())?;
    }
    Ok(Report {
        schema: SCHEMA.into(),
        manifest_digest: sales_evidence::digest(bytes),
        inventory_complete: inventory.complete,
        manifest,
        offers: views,
        cohorts: cohorts.into_values().collect(),
        source_digests: reader
            .snapshots
            .into_iter()
            .map(|(p, (h, _))| (p, h))
            .collect(),
        commissions: "unavailable".into(),
    })
}

fn merge_revenue(to: &mut Revenue, from: &Revenue) -> Result<(), String> {
    macro_rules! fields {($($field:ident),*)=>{$(add(&mut to.$field,from.$field)?;)*};}
    fields!(
        contractual_charge,
        agreed_future_charge,
        gross_collected,
        collected,
        funding_collected,
        funding_refunds,
        funding_refund_reversals,
        purchased_balance_consumed,
        unspent_funding,
        unearned_collection,
        unearned_charge,
        author_liability,
        resource_liability,
        promotion_liability,
        author_allocated,
        resource_allocated,
        promotion_allocated,
        earned_openagents,
        earned_uncollected,
        inbound_fees_already_net,
        promotions_already_allocated,
        funded_promotions,
        refunds,
        refund_reversals,
        losses,
        billable_failures
    );
    Ok(())
}
fn calculate(view: &mut OperatingView, coverage: bool) -> Result<(), String> {
    let mut complete =
        coverage && view.missing_cost_classes.is_empty() && view.unresolved_deliveries == 0;
    for (unit, r) in &view.revenue {
        let mut contribution = i128::from(r.earned_openagents)
            - i128::from(r.funded_promotions)
            - i128::from(r.refunds)
            - i128::from(r.losses)
            + i128::from(r.refund_reversals);
        for cost in &view.costs {
            if cost.unknown_items > 0 || cost.basis != Basis::Billed {
                complete = false;
            }
            if cost.payer == Payer::OpenAgents && cost.basis == Basis::Billed {
                if &cost.unit == unit {
                    contribution -= i128::from(cost.known_subtotal);
                } else {
                    complete = false;
                }
            }
        }
        view.contribution_known_subtotal
            .insert(unit.clone(), contribution);
    }
    // Different cash denominations never imply an exchange rate or net margin.
    view.profitable = (complete && view.revenue.len() == 1)
        .then(|| view.contribution_known_subtotal.values().all(|v| *v > 0));
    Ok(())
}

/// The owner reviews the exact report before receiving a scrubbed aggregate.
pub fn project(bytes: &[u8], review: &Review) -> Result<Aggregate, String> {
    if bytes.len() > MAX_REPORT {
        return Err("private finance report exceeds bound".into());
    }
    let report: Report =
        serde_json::from_slice(bytes).map_err(|_| "malformed private finance report")?;
    if report.schema != SCHEMA
        || review.schema != "openagents.gym.sales-finance-review.v1"
        || !review.approved
        || review.report_digest != sales_evidence::digest(bytes)
        || review.owner != report.manifest.owner
    {
        return Err("aggregate export requires the owner's approval of exact report bytes".into());
    }
    let mut views = report.cohorts;
    for (index, view) in views.iter_mut().enumerate() {
        view.offer.clear();
        view.cohort = format!("cohort_{}", index + 1);
        if view.revenue.keys().any(|unit| !cash_unit(unit))
            || view
                .contribution_known_subtotal
                .keys()
                .any(|unit| !cash_unit(unit))
        {
            return Err("aggregate contains an unsupported revenue denomination".into());
        }
        for cost in &mut view.costs {
            if !matches!(
                cost.unit.as_str(),
                "msat"
                    | "USD_millionths"
                    | "subscription_capacity_units"
                    | "provider_grant_units"
                    | "undeclared"
            ) {
                cost.unit = "withheld_denomination".into();
                cost.known_subtotal = 0;
                cost.unknown_items = cost.unknown_items.saturating_add(1);
                view.profitable = None;
            }
        }
    }
    Ok(Aggregate {schema:"openagents.gym.sales-finance-aggregate.v1".into(),views,limitations:vec!["Owner-reviewed retained evidence, not remote attestation or publication authority.".into(),"Net collections, funding, pass-through liabilities, earned shares, refunds, incentives, and expenses remain separate; amounts in different units are never combined.".into(),"Contribution subtracts billed OpenAgents costs and first-call promotions once. Author/resource shares, launch promotions, and inbound fees are already reflected in the recorded share.".into(),"Unknown costs, incomplete coverage, estimates, provider grants, and subscription capacity prevent an unqualified profitability claim. Future commission enrichment is unavailable.".into()]})
}
fn bounded_file(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "finance input is unavailable")?;
    let meta = file
        .metadata()
        .map_err(|_| "finance input metadata is unavailable")?;
    if !meta.is_file() {
        return Err("finance input must be a regular file".into());
    }
    file.take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "finance input read failed")?;
    if bytes.len() > max {
        return Err("finance input exceeds bound".into());
    }
    Ok(bytes)
}
/// Explicit offline CLI paths; neither private records nor credentials print.
pub fn command(args: &[String]) -> Result<(), String> {
    let mut flags = BTreeMap::new();
    for pair in args.chunks(2) {
        if pair.len() != 2
            || !matches!(
                pair[0].as_str(),
                "--root" | "--manifest" | "--output" | "--report" | "--review"
            )
            || flags.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err("use sales-finance --root DIR --manifest FILE --output FILE, or --report FILE --review FILE --output FILE".into());
        }
    }
    let output = flags
        .get("--output")
        .ok_or("private output path is required")?;
    let bytes = if let (Some(root), Some(manifest)) = (flags.get("--root"), flags.get("--manifest"))
    {
        if flags.len() != 3 {
            return Err("finance rebuild and export flags cannot mix".into());
        }
        serde_json::to_vec_pretty(&rebuild(
            Path::new(root),
            &bounded_file(Path::new(manifest), MAX_MANIFEST)?,
        )?)
        .map_err(|_| "cannot serialize finance report")?
    } else if let (Some(report), Some(review)) = (flags.get("--report"), flags.get("--review")) {
        if flags.len() != 3 {
            return Err("finance rebuild and export flags cannot mix".into());
        }
        let review: Review =
            serde_json::from_slice(&bounded_file(Path::new(review), MAX_MANIFEST)?)
                .map_err(|_| "malformed owner review")?;
        serde_json::to_vec_pretty(&project(
            &bounded_file(Path::new(report), MAX_REPORT)?,
            &review,
        )?)
        .map_err(|_| "cannot serialize finance aggregate")?
    } else {
        return Err("complete finance rebuild or reviewed export inputs are required".into());
    };
    if bytes.len() > MAX_REPORT {
        return Err("finance output exceeds bound".into());
    }
    sales_evidence::write_private(Path::new(output), &bytes)
}

#[cfg(test)]
#[path = "sales_finance_tests.rs"]
mod tests;

//! Private REV-25 evidence over the existing central settlement and retained costs.

use crate::{Host, Result, evaluate, private_path, source, types::Evaluation};
use gym::{
    sales_evidence::{Reference, digest},
    sales_finance as finance,
};
use openagents_wallet::LightningWallet;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
};

fn retain(root: &Path, name: &str, bytes: &[u8]) -> Result<Reference> {
    let path = root.join(name);
    private_path(&path)?;
    fs::create_dir_all(path.parent().ok_or("export parent is absent")?)
        .map_err(|_| "private finance export cannot be created")?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(|_| "finance export already exists or is unavailable")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "finance evidence cannot be retained")?;
    Ok(Reference {
        path: name.into(),
        sha256: digest(bytes),
    })
}

/// A private join between native REV-25 costs and one central contribution.
/// The cost report preserves its delivery classification. This view does not
/// manufacture coding-task acceptance or independent commercial qualification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContributionFinance {
    pub schema: String,
    pub account: String,
    pub offer: String,
    pub offer_version: String,
    pub evaluated: evaluate::Report,
    pub ledger: Reference,
    pub attribution: Reference,
    pub funding: Reference,
    pub frozen: Reference,
    pub acceptance: Reference,
    pub cost_manifest: Reference,
    pub customer_costs: finance::Report,
    pub settlement: String,
    pub gross_collected_msat: i64,
    pub platform_allocated_msat: i64,
    pub beneficiary_allocated_msat: i64,
    pub beneficiary_outstanding_msat: i64,
    pub known_operating_cost_msat: u64,
    pub known_customer_cost_msat: u64,
    pub payment_fee_msat: Option<u64>,
    pub operating_margin_msat: Option<i128>,
    pub profitable: Option<bool>,
    pub independent_commercial_qualification: String,
    pub serving_activated: bool,
}

impl Host {
    /// Export a fresh private customer cost view after the central liability
    /// exists. The SQLite file is a reporting snapshot, not a second money book.
    pub fn export_finance(
        &self,
        wallet: &impl LightningWallet,
        now: i64,
        output: &Path,
    ) -> Result<ContributionFinance> {
        let funding = self
            .retained()?
            .filter(|f| f.state == "funded_liability")
            .ok_or("a funded central contribution liability is required")?;
        let verified = evaluate::verify(&self.config, &self.ledger, &wallet.node_id(), now)?;
        if serde_json::to_vec(&verified.report).ok() != serde_json::to_vec(&funding.report).ok() {
            return Err("current contribution evidence differs from the funded obligation".into());
        }
        let key = &funding
            .invoice
            .as_ref()
            .ok_or("original invoice is absent")?
            .payment_hash;
        let settlement = self
            .ledger
            .settlement(key)
            .map_err(|_| "central settlement is unavailable")?
            .ok_or("funded contribution settlement is absent")?;
        if settlement.resource != funding.expected_resource
            || settlement.received_msat != funding.amount_msat
        {
            return Err("the payment hash belongs to another obligation".into());
        }
        let (_, evaluation): (_, Evaluation) = source::signed(
            &source::read(&self.config.protected_root, &self.config.evaluation)?,
            &verified.frozen.terms.protected_evaluator,
            now,
        )?;
        let original = finance::rebuild(
            &self.config.protected_root,
            &source::read(&self.config.protected_root, &evaluation.finance)?,
        )?;
        private_path(output)?;
        if output.starts_with(&self.config.worker_root)
            || self.config.worker_root.starts_with(output)
            || output.starts_with(&self.config.protected_root)
            || self.config.protected_root.starts_with(output)
        {
            return Err("finance export must have a separate private operator root".into());
        }
        fs::create_dir(output).map_err(|_| "finance output must be a new private directory")?;
        fs::set_permissions(output, fs::Permissions::from_mode(0o700))
            .map_err(|_| "finance output cannot be protected")?;
        for (name, sha256) in &original.source_digests {
            retain(
                output,
                name,
                &source::read(
                    &self.config.protected_root,
                    &Reference {
                        path: name.clone(),
                        sha256: sha256.clone(),
                    },
                )?,
            )?;
        }
        let terms = retain(
            output,
            "contribution/frozen.json",
            &source::read(&self.config.protected_root, &self.config.frozen)?,
        )?;
        let acceptance = retain(
            output,
            "contribution/acceptance.json",
            &source::read(&self.config.protected_root, &self.config.acceptance)?,
        )?;
        let ledger_path = output.join("contribution/central.sqlite");
        let connection =
            Connection::open_with_flags(&self.config.ledger, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|_| "central snapshot source is unavailable")?;
        connection
            .execute(
                "VACUUM INTO ?1",
                [ledger_path.to_str().ok_or("invalid snapshot path")?],
            )
            .map_err(|_| "central reporting snapshot cannot be retained")?;
        fs::set_permissions(&ledger_path, fs::Permissions::from_mode(0o600))
            .map_err(|_| "central snapshot cannot be protected")?;
        fs::File::open(&ledger_path)
            .and_then(|f| f.sync_all())
            .map_err(|_| "central snapshot cannot be synced")?;
        let ledger = Reference {
            path: "contribution/central.sqlite".into(),
            sha256: digest(&fs::read(&ledger_path).map_err(|_| "snapshot cannot be read")?),
        };
        let funding_reference = retain(
            output,
            "contribution/funding.json",
            &serde_json::to_vec(&funding).map_err(|_| "funding evidence cannot be encoded")?,
        )?;
        let mut manifest = original.manifest;
        let offer = manifest
            .offers
            .iter_mut()
            .find(|o| o.id == evaluation.finance_offer)
            .ok_or("selected customer offer is absent")?;
        let account = offer.account.clone();
        let offer_version = offer.version.clone();
        let attribution = retain(
            output,
            "contribution/attribution.json",
            &serde_json::to_vec(&finance::Attribution {
                schema: "openagents.sales.financial-attribution.v1".into(),
                ledger_digest: ledger.sha256.clone(),
                settlement: key.clone(),
                account: account.clone(),
                offer_version: offer_version.clone(),
            })
            .map_err(|_| "financial attribution cannot be encoded")?,
        )?;
        // A contribution is not a REV-03 coding task. Preserve the native
        // customer cost entries and add the receiver expense to the same offer.
        let fee_id = format!("contribution-funding-fee:{}", funding.obligation);
        if offer
            .entries
            .iter()
            .flat_map(|e| &e.expenses)
            .any(|e| e.id == fee_id)
        {
            return Err("contribution receiver expense already exists".into());
        }
        let cost_entry = offer
            .entries
            .iter_mut()
            .find(|e| e.expenses.len() < 64)
            .ok_or("customer cost entries cannot retain another expense")?;
        cost_entry.expenses.push(finance::Expense {
            id: fee_id,
            class: finance::ExpenseClass::Payment,
            basis: if funding.reported_payment_fee_msat.is_some() {
                finance::Basis::Billed
            } else {
                finance::Basis::Unknown
            },
            unit: "msat".into(),
            amount: funding.reported_payment_fee_msat,
            payer: finance::Payer::OpenAgents,
            evidence: Some(funding_reference.clone()),
            price: None,
        });
        let bytes =
            serde_json::to_vec(&manifest).map_err(|_| "finance manifest cannot be encoded")?;
        let cost_manifest = retain(output, "contribution/finance.json", &bytes)?;
        let customer_costs = finance::rebuild(output, &bytes)?;
        let view = customer_costs
            .offers
            .iter()
            .find(|o| o.offer == evaluation.finance_offer)
            .ok_or("customer cost view is absent")?;
        let (known_operating_cost_msat, known_customer_cost_msat) =
            view.costs
                .iter()
                .try_fold((0_u64, 0_u64), |(operator, customer), c| {
                    if c.unit != "msat" {
                        return Err("contribution costs changed denomination".to_string());
                    }
                    let add = |value: u64| {
                        value
                            .checked_add(c.known_subtotal)
                            .ok_or("operating cost overflow".to_string())
                    };
                    match c.payer {
                        finance::Payer::OpenAgents => Ok((add(operator)?, customer)),
                        finance::Payer::Customer => Ok((operator, add(customer)?)),
                    }
                })?;
        let platform_allocated_msat = settlement
            .shares
            .iter()
            .filter(|s| s.role == "openagents" && s.party == pay_ledger::OPENAGENTS)
            .try_fold(0_i64, |sum, s| {
                sum.checked_add(s.amount_msat)
                    .ok_or("platform allocation overflow")
            })?;
        let beneficiary_allocated_msat = settlement
            .shares
            .iter()
            .filter(|s| s.role == "author" && s.party == verified.frozen.terms.beneficiary)
            .try_fold(0_i64, |sum, s| {
                sum.checked_add(s.amount_msat)
                    .ok_or("contribution allocation overflow")
            })?;
        if platform_allocated_msat != verified.frozen.platform_fee_msat
            || beneficiary_allocated_msat != verified.frozen.terms.reward_msat
        {
            return Err("central contribution allocations disagree with frozen terms".into());
        }
        let beneficiary_outstanding_msat = self
            .ledger
            .settlement_liabilities(key)
            .map_err(|_| "central contribution liability is unavailable")?
            .iter()
            .filter(|s| s.role == "author" && s.party == verified.frozen.terms.beneficiary)
            .try_fold(0_i64, |sum, s| {
                sum.checked_add(s.amount_msat)
                    .ok_or("contribution liability overflow")
            })?;
        let operating_margin_msat = (view.costs.iter().all(|c| c.unknown_items == 0)
            && view.missing_cost_classes.is_empty())
        .then_some(i128::from(platform_allocated_msat) - i128::from(known_operating_cost_msat));
        let report = ContributionFinance {
            schema: "openagents.contribution-finance.v1".into(),
            account,
            offer: evaluation.finance_offer,
            offer_version,
            evaluated: verified.report,
            ledger,
            attribution,
            funding: funding_reference,
            frozen: terms,
            acceptance,
            cost_manifest,
            customer_costs,
            settlement: key.clone(),
            gross_collected_msat: settlement.received_msat,
            platform_allocated_msat,
            beneficiary_allocated_msat,
            beneficiary_outstanding_msat,
            known_operating_cost_msat,
            known_customer_cost_msat,
            payment_fee_msat: funding.reported_payment_fee_msat,
            operating_margin_msat,
            profitable: operating_margin_msat.map(|value| value > 0),
            independent_commercial_qualification: "unverified".into(),
            serving_activated: false,
        };
        retain(
            output,
            "contribution/report.json",
            &serde_json::to_vec(&report)
                .map_err(|_| "contribution finance view cannot be encoded")?,
        )?;
        fs::File::open(output)
            .and_then(|f| f.sync_all())
            .map_err(|_| "finance export directory cannot be synced")?;
        Ok(report)
    }
}

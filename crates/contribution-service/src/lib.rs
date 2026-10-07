//! Protected accepted contribution work funded through the existing central ledger.
//! Local signed records are attributable claims; independent commercial
//! qualification and real funding remain separate owner steps.

pub mod evaluate;
pub mod finance;
mod source;
pub mod types;

use fs2::FileExt;
use openagents_wallet::{
    IssuedInvoice, LightningWallet, PaymentDirection, PaymentStatus, parse_hash32,
};
use pay_ledger::{
    Ledger,
    markets::{contribution::Trust, worker::FundingReceipt},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path},
};
use types::{Config, SCHEMA};

pub type Result<T> = std::result::Result<T, String>;

/// Read an explicit private configuration without a home-directory fallback.
pub fn load_config(path: &Path) -> Result<Config> {
    private_path(path)?;
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "contribution configuration is unavailable")?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o077 != 0 {
        return Err("contribution configuration must be a private regular file".into());
    }
    let parent = path.parent().ok_or("configuration parent is absent")?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid configuration filename")?;
    serde_json::from_slice(&source::bytes(parent, name)?)
        .map_err(|_| "invalid contribution configuration".into())
}

/// Persisted funding transport state, never an alternative money balance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Funding {
    pub schema: String,
    pub obligation: String,
    pub request_hash: String,
    pub expected_resource: String,
    pub report: evaluate::Report,
    pub amount_msat: i64,
    pub central_node: String,
    pub intent_at: i64,
    pub admitted_expires_at: i64,
    pub invoice_expiry_secs: u32,
    pub created_at: i64,
    pub expires_at: i64,
    pub invoice: Option<IssuedInvoice>,
    pub reported_payment_fee_msat: Option<u64>,
    pub receiver_evidence: Option<ReceiverEvidence>,
    pub state: String,
}
/// A bounded digest of the owned receiver observation, without its preimage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReceiverEvidence {
    pub record_sha256: String,
    pub payment_hash: String,
    pub transferred_msat: u64,
    pub observed_at: i64,
    pub central_node: String,
}
pub struct Host {
    config: Config,
    ledger: Ledger,
    records: Connection,
    _lock: File,
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn private_path(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("an absolute contribution path without traversal is required".into());
    }
    let mut prefix = std::path::PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        if fs::symlink_metadata(&prefix).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("symlink contribution paths are refused".into());
        }
    }
    Ok(())
}
fn private_file(path: &Path) -> Result<File> {
    private_path(path)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "private contribution state is unavailable")?;
    if !file
        .metadata()
        .map_err(|_| "private state metadata is unavailable")?
        .is_file()
    {
        return Err("private contribution state must be a regular file".into());
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| "private contribution state cannot be protected")?;
    Ok(file)
}
impl Host {
    pub fn open(config: Config) -> Result<Self> {
        if config.schema != SCHEMA || parse_hash32(&config.authority).is_err() {
            return Err("unsupported contribution configuration".into());
        }
        for root in [&config.protected_root, &config.worker_root] {
            private_path(root)?;
            let meta = fs::metadata(root).map_err(|_| "contribution source root is unavailable")?;
            if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
                return Err("contribution source roots must be private directories".into());
            }
        }
        if config.protected_root.starts_with(&config.worker_root)
            || config.worker_root.starts_with(&config.protected_root)
            || config.state.starts_with(&config.worker_root)
            || config.ledger.starts_with(&config.worker_root)
        {
            return Err("protected evidence, funding state, and the central ledger must be outside worker custody".into());
        }
        private_path(&config.state)?;
        fs::create_dir_all(&config.state)
            .map_err(|_| "private contribution state cannot be created")?;
        fs::set_permissions(&config.state, fs::Permissions::from_mode(0o700))
            .map_err(|_| "private contribution state cannot be protected")?;
        let lock = private_file(&config.state.join("service.lock"))?;
        lock.try_lock_exclusive()
            .map_err(|_| "this contribution state already has an owner")?;
        private_path(&config.ledger)?;
        if !fs::metadata(&config.ledger)
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o077 == 0)
        {
            return Err("an existing private central ledger is required".into());
        }
        let db_path = config.state.join("funding.sqlite");
        private_file(&db_path)?;
        let records =
            Connection::open(db_path).map_err(|_| "contribution funding journal is unavailable")?;
        records
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|_| "durable funding journal is unavailable")?;
        records.execute_batch("CREATE TABLE IF NOT EXISTS obligation(id TEXT PRIMARY KEY,frozen TEXT NOT NULL,bytes TEXT NOT NULL);").map_err(|_| "contribution funding journal cannot be opened")?;
        fs::File::open(&config.state)
            .and_then(|f| f.sync_all())
            .map_err(|_| "private funding directory cannot be synced")?;
        if let Some(parent) = config.state.parent() {
            fs::File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(|_| "funding state parent cannot be synced")?;
        }
        let ledger = Ledger::open(&config.ledger).map_err(|_| "central ledger is unavailable")?;
        Ok(Self {
            config,
            ledger,
            records,
            _lock: lock,
        })
    }
    pub fn assess(&self, wallet: &impl LightningWallet, now: i64) -> Result<evaluate::Report> {
        evaluate::verify(&self.config, &self.ledger, &wallet.node_id(), now).map(|v| v.report)
    }
    fn retained(&self) -> Result<Option<Funding>> {
        let bytes: Option<String> = self
            .records
            .query_row("SELECT bytes FROM obligation LIMIT 1", [], |r| r.get(0))
            .optional()
            .map_err(|_| "funding record is unavailable")?;
        bytes
            .map(|b| serde_json::from_str(&b).map_err(|_| "invalid retained funding record".into()))
            .transpose()
    }
    fn save(&self, funding: &Funding) -> Result<()> {
        let changed = self
            .records
            .execute(
                "UPDATE obligation SET bytes=? WHERE id=?",
                params![
                    serde_json::to_string(funding).map_err(|_| "invalid funding state")?,
                    funding.obligation
                ],
            )
            .map_err(|_| "funding state cannot be retained")?;
        if changed != 1 {
            return Err("the original funding intent is absent".into());
        }
        Ok(())
    }
    /// Admit a single original invoice. An issue call whose reply is lost
    /// remains unknown; it never automatically creates a replacement invoice.
    pub fn prepare(&mut self, wallet: &impl LightningWallet, now: i64) -> Result<Funding> {
        if let Some(prior) = self.retained()? {
            if prior.state == "funded_liability" {
                return Ok(prior);
            }
            let report = self.assess(wallet, now)?;
            if report.frozen != prior.report.frozen
                || serde_json::to_vec(&report).ok() != serde_json::to_vec(&prior.report).ok()
            {
                return Err("the funded obligation cannot change its accepted evidence".into());
            }
            return Ok(prior);
        }
        let verified = evaluate::verify(&self.config, &self.ledger, &wallet.node_id(), now)?;
        let amount = verified
            .frozen
            .terms
            .reward_msat
            .checked_add(verified.frozen.platform_fee_msat)
            .ok_or("contribution reward funding overflow")?;
        let expires_at = verified.admitted_until;
        let expiry = u32::try_from(
            expires_at
                .checked_sub(now)
                .and_then(|n| n.checked_sub(60))
                .filter(|n| *n > 0)
                .ok_or("funding terms need the resident's bounded 60-second issue window")?,
        )
        .map_err(|_| "funding invoice expiry is unbounded")?;
        let request = serde_json::json!({"schema":"openagents.contribution-funding-request.v1","obligation":verified.frozen.terms.obligation,"frozen":verified.report.frozen,"acceptance":verified.report.acceptance,"report":gym::sales_evidence::digest(&serde_json::to_vec(&verified.report).map_err(|_|"report encoding failed")?),"amount_msat":amount,"central_node":verified.current.central_node,"intent_at":now,"invoice_expiry_secs":expiry,"admitted_expires_at":expires_at});
        let request_hash = gym::sales_evidence::digest(
            &nostr::contracts::jcs(&request)
                .map_err(|_| "funding request cannot be canonicalized")?,
        );
        let resource_bytes = nostr::contracts::jcs(&serde_json::json!({"terms":verified.frozen.terms,"acceptance":verified.accepted,"platform_fee_msat":verified.frozen.platform_fee_msat})).map_err(|_| "contribution resource cannot be canonicalized")?;
        let expected_resource = format!(
            "contribution:{}:{}",
            verified.frozen.terms.obligation,
            nostr::contracts::digest_bytes(&resource_bytes)
        );
        let mut funding = Funding {
            schema: "openagents.contribution-funding.v1".into(),
            obligation: verified.frozen.terms.obligation,
            request_hash,
            expected_resource,
            report: verified.report,
            amount_msat: amount,
            central_node: verified.current.central_node,
            intent_at: now,
            admitted_expires_at: expires_at,
            invoice_expiry_secs: expiry,
            created_at: now,
            expires_at,
            invoice: None,
            reported_payment_fee_msat: None,
            receiver_evidence: None,
            state: "issuance_unknown".into(),
        };
        self.records
            .execute(
                "INSERT INTO obligation(id,frozen,bytes) VALUES(?,?,?)",
                params![
                    funding.obligation,
                    funding.report.frozen,
                    serde_json::to_string(&funding).map_err(|_| "funding encoding failed")?
                ],
            )
            .map_err(|_| "the original funding intent cannot be retained")?;
        let invoice = match wallet.receive_exact_from_node(
            &funding.central_node,
            u64::try_from(amount).map_err(|_| "invalid funded amount")?,
            parse_hash32(&funding.request_hash).map_err(|_| "invalid request hash")?,
            expiry,
        ) {
            Ok(invoice) => invoice,
            Err(openagents_wallet::WalletError::NodeMismatch { .. }) => {
                funding.state = "receiver_refused".into();
                self.save(&funding)?;
                return Err("the approved central receiver changed; no invoice was issued".into());
            }
            Err(_) => {
                return Err("original invoice issuance is unknown; retain and reconcile it".into());
            }
        };
        let issued = nostr::x402::decode_invoice(&invoice.bolt11)
            .map_err(|_| "original funding invoice is invalid")?;
        funding.created_at =
            i64::try_from(issued.created_at()).map_err(|_| "invoice time overflow")?;
        funding.expires_at = funding
            .created_at
            .checked_add(i64::from(expiry))
            .ok_or("invoice expiry overflow")?;
        validate_invoice(&funding, &invoice)?;
        funding.invoice = Some(invoice);
        funding.state = "awaiting_funding".into();
        self.save(&funding)?;
        Ok(funding)
    }
    /// Verify the receiver's original invoice lookup and current protected
    /// sources. This cannot accept a customer callback or a second hash.
    pub fn reconcile(
        &mut self,
        wallet: &impl LightningWallet,
        mut clock: impl FnMut() -> Result<i64>,
    ) -> Result<Funding> {
        let started_at = clock()?;
        let now = started_at;
        let mut funding = self
            .retained()?
            .ok_or("contribution funding has not been admitted")?;
        let invoice = funding
            .invoice
            .as_ref()
            .ok_or("original invoice issuance remains unknown")?;
        validate_invoice(&funding, invoice)?;
        if wallet.node_id() != funding.central_node {
            return Err("the central receiver changed".into());
        }
        if let Some(record) = self
            .ledger
            .settlement(&invoice.payment_hash)
            .map_err(|_| "central settlement is unavailable")?
        {
            if record.resource != funding.expected_resource
                || record.received_msat != funding.amount_msat
            {
                funding.state = "central_conflict".into();
                self.save(&funding)?;
                return Err(
                    "the original payment hash belongs to another central obligation".into(),
                );
            }
            funding.state = "funded_liability".into();
            self.save(&funding)?;
            return Ok(funding);
        }
        let verified = match evaluate::verify(&self.config, &self.ledger, &wallet.node_id(), now) {
            Ok(v) => v,
            Err(error) => {
                funding.state = "admission_refused".into();
                self.save(&funding)?;
                return Err(error);
            }
        };
        if serde_json::to_vec(&verified.report).ok() != serde_json::to_vec(&funding.report).ok() {
            funding.state = "evidence_conflict".into();
            self.save(&funding)?;
            return Err("accepted contribution evidence changed".into());
        }
        let payment = match wallet.lookup_from_node(
            &funding.central_node,
            parse_hash32(&invoice.payment_hash).map_err(|_| "invalid original payment hash")?,
        ) {
            Ok(Some(p)) => p,
            Err(openagents_wallet::WalletError::NodeMismatch { .. }) => {
                funding.state = "receiver_refused".into();
                self.save(&funding)?;
                return Err(
                    "the approved central receiver changed; lookup was not dispatched".into(),
                );
            }
            Ok(None) | Err(_) => {
                funding.state = "funding_unknown".into();
                self.save(&funding)?;
                return Ok(funding);
            }
        };
        if payment.status != PaymentStatus::Succeeded {
            funding.state = if payment.status == PaymentStatus::Failed {
                "funding_failed"
            } else {
                "funding_unknown"
            }
            .into();
            self.save(&funding)?;
            return Ok(funding);
        }
        let now = clock()?;
        if now < started_at {
            funding.state = "admission_refused".into();
            self.save(&funding)?;
            return Err("the funding clock moved backward during lookup".into());
        }
        let paid_at = i64::try_from(payment.updated_at).map_err(|_| "funding time overflow")?;
        let preimage = match payment
            .preimage
            .as_deref()
            .and_then(|value| parse_hash32(value).ok())
        {
            Some(value) => value,
            None => {
                funding.state = "funding_conflict".into();
                self.save(&funding)?;
                return Err("funding preimage is absent or invalid".into());
            }
        };
        if payment.direction != PaymentDirection::Inbound
            || payment.payment_hash != invoice.payment_hash
            || payment.amount_msat != Some(invoice.amount_msat)
            || payment
                .bolt11
                .as_deref()
                .is_some_and(|returned| returned != invoice.bolt11)
            || hex(&Sha256::digest(preimage)) != invoice.payment_hash
            || paid_at < funding.created_at
            || paid_at < verified.accepted.accepted_at
            || paid_at >= funding.expires_at
            || paid_at > now
        {
            funding.state = "funding_conflict".into();
            self.save(&funding)?;
            return Err("receiver evidence differs from the exact funded obligation".into());
        }
        // Lookup can block. Re-read current permissions, acceptance, artifact
        // bytes, complete costs, and the central destination before accruing.
        let fresh = match evaluate::verify(&self.config, &self.ledger, &wallet.node_id(), now) {
            Ok(v) => v,
            Err(reason) => {
                funding.state = "admission_refused".into();
                self.save(&funding)?;
                return Err(reason);
            }
        };
        if fresh.accepted != verified.accepted
            || serde_json::to_vec(&fresh.report).ok() != serde_json::to_vec(&funding.report).ok()
        {
            funding.state = "evidence_conflict".into();
            self.save(&funding)?;
            return Err("contribution admission changed during funding lookup".into());
        }
        let admission_time = clock()?;
        if admission_time < now || admission_time >= fresh.admitted_until {
            funding.state = "admission_refused".into();
            self.save(&funding)?;
            return Err("current contribution admission expired before central accrual".into());
        }
        funding.reported_payment_fee_msat = payment.fee_msat;
        funding.receiver_evidence = Some(ReceiverEvidence {
            record_sha256: gym::sales_evidence::digest(
                &serde_json::to_vec(&payment)
                    .map_err(|_| "receiver observation cannot be encoded")?,
            ),
            payment_hash: payment.payment_hash.clone(),
            transferred_msat: invoice.amount_msat,
            observed_at: paid_at,
            central_node: wallet.node_id(),
        });
        funding.state = "funding_verified".into();
        self.save(&funding)?;

        let terms = &fresh.frozen.terms;
        let trust = Trust {
            frozen_terms_fingerprint: terms
                .fingerprint()
                .map_err(|_| "invalid frozen contribution")?,
            rights_verified: true,
            source_groups_verified: true,
            independently_controlled_evaluator: true,
            beneficiary_destination_verified: true,
            verified_acceptance: fresh.accepted.clone(),
            current_funding_authority: self.config.authority.clone(),
        };
        if let Err(error) = self.ledger.record_contribution_earned(
            terms,
            &fresh.accepted,
            &trust,
            &FundingReceipt {
                payment_hash: invoice.payment_hash.clone(),
                received_msat: funding.amount_msat,
                platform_fee_msat: fresh.frozen.platform_fee_msat,
                received_at: paid_at,
            },
        ) {
            funding.state = if matches!(error, pay_ledger::Error::Conflict(_)) {
                "central_conflict"
            } else {
                "central_refused"
            }
            .into();
            self.save(&funding)?;
            return Err("central contribution liability refused or conflicted".into());
        }
        // A crash here replays the same already sealed central settlement.
        funding.state = "funded_liability".into();
        self.save(&funding)?;
        Ok(funding)
    }
    /// Read central liability and payout states. Funding, an accrued reward,
    /// and a sent payout remain separate observations.
    pub fn statement(&self) -> Result<serde_json::Value> {
        let funding = self.retained()?;
        let record = funding
            .as_ref()
            .and_then(|f| f.invoice.as_ref())
            .map(|i| self.ledger.settlement(&i.payment_hash))
            .transpose()
            .map_err(|_| "central statement is unavailable")?
            .flatten();
        let liabilities = record
            .as_ref()
            .map(|r| self.ledger.settlement_liabilities(&r.key))
            .transpose()
            .map_err(|_| "central liability statement is unavailable")?;
        let payouts = record.as_ref().map(|r| self.ledger.settlement_payouts(&r.key)).transpose().map_err(|_| "central payout attempts are unavailable")?.unwrap_or_default().into_iter().map(|p| serde_json::json!({"id":p.id,"party":p.party,"amount_msat":p.amount_msat,"state":p.state.as_str(),"wallet_reference":p.wallet_reference,"sent_msat":p.sent_msat,"fee_msat":p.fee_msat,"lookup_required":matches!(p.state,pay_ledger::PayoutState::Sending|pay_ledger::PayoutState::Unknown),"wallet_attestation":false,"amount_scope":"whole payout batch; original settlement shares remain separate"})).collect::<Vec<_>>();
        let settlement = record.map(|r| serde_json::json!({"seq":r.seq,"key":r.key,"resource":r.resource,"received_msat":r.received_msat,"price_msat":r.price_msat,"settled_at":r.settled_at,"shares":r.shares.into_iter().map(|s| serde_json::json!({"party":s.party,"role":s.role,"amount_msat":s.amount_msat})).collect::<Vec<_>>()}));
        let liabilities = liabilities.map(|rows| rows.into_iter().map(|s|serde_json::json!({"party":s.party,"role":s.role,"amount_msat":s.amount_msat})).collect::<Vec<_>>());
        Ok(
            serde_json::json!({"schema":"openagents.contribution-statement.v1","funding":funding,"settlement":settlement,"liabilities":liabilities,"payouts":payouts,"serving_activated":false,"independent_commercial_qualification":"unverified","payment_fee_scope":"wallet-reported payment expense; absent is unknown, not zero"}),
        )
    }
}
fn validate_invoice(funding: &Funding, issued: &IssuedInvoice) -> Result<()> {
    let invoice = nostr::x402::decode_invoice(&issued.bolt11)
        .map_err(|_| "funding invoice signature or exact fields are invalid")?;
    if !matches!(invoice.currency(), "bc" | "tb")
        || invoice.amount_msat()
            != u64::try_from(funding.amount_msat).map_err(|_| "invalid funded amount")?
        || issued.amount_msat != invoice.amount_msat()
        || hex(&invoice.payment_hash()) != issued.payment_hash
        || hex(&invoice.description_hash()) != funding.request_hash
        || issued.description_hash != funding.request_hash
        || hex(&invoice.payee()) != funding.central_node
        || issued.pay_to != funding.central_node
        || funding.created_at < funding.intent_at
        || funding.created_at
            > funding
                .intent_at
                .checked_add(60)
                .ok_or("invoice issue window overflow")?
        || funding.expires_at > funding.admitted_expires_at
        || issued.expiry_secs != funding.invoice_expiry_secs
        || invoice.created_at() != funding.created_at as u64
        || invoice.expiry_seconds() != (funding.expires_at - funding.created_at) as u64
        || u64::from(issued.expiry_secs) != invoice.expiry_seconds()
    {
        return Err(
            "funding invoice changed original amount, obligation, receiver, or expiry".into(),
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests;

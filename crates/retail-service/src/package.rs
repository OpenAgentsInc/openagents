//! Runtime qualification, versioned configuration, and private operator reads.
//! An owner approval cannot replace native funding, cleanup, or accounting evidence.
use crate::{Backend, Error, Result, Service, store, types::Config};
use openagents_wallet::{LightningWallet, PaymentDirection, PaymentStatus, parse_hash32};
use pay_ledger::{
    Ledger,
    compute::{HoldState, PurchaseState},
};
use retail_cloud::{
    authority::{Current, ObserveGrant},
    contract,
    journal::Journal,
};
use retail_qualify::{
    launch,
    qualify::{DeploymentBinding, Mode as QualificationMode, Plan, QualificationReceipt},
};
use route_contract::{Digest, digest_of};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    net::SocketAddr,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub const HOST_SCHEMA: &str = "openagents.cloud.retail-host.v1";
pub const OPERATIONS_SCHEMA: &str = "openagents.cloud.retail-operations.v1";
pub const STORAGE_SCHEMA: &str = "openagents.cloud.retail-storage.v1";
pub const COMMIT: &str = env!("RETAIL_BUILD_COMMIT");
pub const TREE: &str = env!("RETAIL_BUILD_TREE");
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Closed,
    Development,
    Production,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Host {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial: Option<crate::commercial::Config>,
    #[serde(default)]
    pub schema: String,
    pub customer: Config,
    pub listen: SocketAddr,
    pub boat_api_base: String,
    pub boat_org: Option<String>,
    pub boat_key_file: PathBuf,
    pub wallet_home: PathBuf,
    pub poll_seconds: u64,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub operations: Option<Operations>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operations {
    pub schema: String,
    pub public_origin: String,
    /// Exact root-owned or service-owned TLS ingress configuration deployed separately.
    pub ingress_file: PathBuf,
    pub operator_key_file: PathBuf,
    /// Exact running identity reviewed by the owner; inspect prints it.
    pub approved_identity: Digest,
    pub plan_file: PathBuf,
    pub qualification_file: PathBuf,
    pub qualification_digest: Digest,
    pub qualification_state: PathBuf,
    pub qualification_ledger: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub schema: String,
    pub commit: String,
    pub tree: String,
    pub executable: Digest,
    pub configuration: Digest,
    pub service: String,
    pub customer_schema: String,
    pub storage_schema: String,
    pub price_book: String,
    pub price_digest: Digest,
    pub qualification_plan: String,
    pub template: String,
    pub mode: Mode,
}
impl Identity {
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }
}
pub(crate) struct Operating {
    pub identity: Identity,
    pub host: Host,
    pub config_path: PathBuf,
    pub config_digest: Digest,
    pub binding: DeploymentBinding,
    pub operator_digest: Option<Digest>,
    pub last_worker: Mutex<Option<(i64, bool)>>,
    pub ingress_digest: Option<Digest>,
    pub provider_source_digest: Digest,
}
/// Read private native input without adopting a shared file or following links.
pub fn private_read(path: &Path, max: u64) -> Result<Vec<u8>> {
    store::check_path(path)?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let m = file.metadata()?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.mode() & 0o077 != 0
        || m.len() > max
    {
        return Err(Error::Invalid(
            "a bounded private owned unshared file is required",
        ));
    }
    let mut bytes = Vec::new();
    file.by_ref().take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::Invalid("private input exceeds its bound"));
    }
    Ok(bytes)
}
/// Read public ingress configuration without accepting writable or shared input.
fn ingress_bytes(path: &Path) -> Result<Vec<u8>> {
    store::check_path(path)?;
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let m = f.metadata()?;
    if !m.is_file()
        || ![0, unsafe { libc::geteuid() }].contains(&m.uid())
        || m.nlink() != 1
        || m.mode() & 0o022 != 0
        || m.len() > 32768
    {
        return Err(Error::Invalid(
            "ingress configuration must be owned, unshared, non-writable, and bounded",
        ));
    }
    let mut bytes = Vec::new();
    f.by_ref().take(32769).read_to_end(&mut bytes)?;
    if bytes.len() > 32768 {
        return Err(Error::Invalid("ingress configuration exceeds its bound"));
    }
    Ok(bytes)
}
fn private_stamp(path: &Path, max: u64) -> Result<(u64, u64, u64, i64, i64)> {
    store::check_path(path)?;
    let m = fs::symlink_metadata(path)?;
    if !m.is_file()
        || m.uid() != unsafe { libc::geteuid() }
        || m.nlink() != 1
        || m.mode() & 0o077 != 0
        || m.len() > max
    {
        return Err(Error::Invalid(
            "qualification source is not private, owned, and bounded",
        ));
    }
    Ok((m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec()))
}
/// Read an executable with a bound before allocating its bytes.
pub fn executable_bytes(path: &Path) -> Result<Vec<u8>> {
    store::check_path(path)?;
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !f.metadata()?.is_file() || f.metadata()?.len() > 256 * 1024 * 1024 {
        return Err(Error::Invalid("retail executable exceeds package bound"));
    }
    let mut b = Vec::new();
    f.by_ref().take(256 * 1024 * 1024 + 1).read_to_end(&mut b)?;
    if b.len() > 256 * 1024 * 1024 {
        return Err(Error::Invalid("retail executable exceeds package bound"));
    }
    Ok(b)
}
pub fn load(path: &Path) -> Result<Host> {
    let host: Host = serde_json::from_slice(&private_read(path, 256 * 1024)?)?;
    host.check()?;
    Ok(host)
}
impl Host {
    pub fn check(&self) -> Result<()> {
        if !self.listen.ip().is_loopback()
            || !(1..=30).contains(&self.poll_seconds)
            || !self.wallet_home.is_absolute()
        {
            return Err(Error::Invalid(
                "use a loopback listener, explicit receiver path, and worker interval of 1–30 seconds",
            ));
        }
        if self.mode != Mode::Development && self.schema != HOST_SCHEMA {
            return Err(Error::Invalid("unsupported retail host schema"));
        }
        if self.mode == Mode::Production {
            let ops = self.operations.as_ref().ok_or(Error::Invalid(
                "production operations configuration is required",
            ))?;
            let date = self
                .customer
                .template
                .strip_prefix("oa-coder-main-")
                .unwrap_or("");
            if ops.schema != OPERATIONS_SCHEMA
                || self.boat_api_base != retail_qualify::bindings::BOAT_API
                || date.len() != 8
                || !date.bytes().all(|b| b.is_ascii_digit())
                || self.customer.plan_starts_left.is_none()
            {
                return Err(Error::Invalid(
                    "production requires the supported Boat binding, daily template, and explicit start allowance",
                ));
            }
            let origin = ops.public_origin.strip_prefix("https://").unwrap_or("");
            if origin.is_empty()
                || origin.contains(['/', '@', '?', '#'])
                || origin
                    .bytes()
                    .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
            {
                return Err(Error::Invalid(
                    "an explicit HTTPS ingress origin is required",
                ));
            }
        }
        store::check_path(&self.customer.state)?;
        store::check_path(&self.customer.ledger)?;
        store::check_path(&self.wallet_home)?;
        Ok(())
    }
    pub fn identity(
        &self,
        node: &str,
        executable: &Path,
        commit: &str,
        tree: &str,
    ) -> Result<(Identity, DeploymentBinding)> {
        let boat_key = private_read(&self.boat_key_file, 8192)?;
        self.identity_from_provider(node, executable, commit, tree, &boat_key)
    }
    fn identity_from_provider(
        &self,
        node: &str,
        executable: &Path,
        commit: &str,
        tree: &str,
        boat_key: &[u8],
    ) -> Result<(Identity, DeploymentBinding)> {
        let key_text = std::str::from_utf8(boat_key)
            .map_err(|_| Error::Invalid("invalid dedicated Boat key"))?;
        boat::ApiKey::new(key_text).map_err(|_| Error::Invalid("invalid dedicated Boat key"))?;
        let binding = DeploymentBinding {
            receiver: node.into(),
            boat_api_base: self.boat_api_base.clone(),
            boat_org: self.boat_org.clone(),
            boat_key_digest: retail_cloud::sha256_hex(key_text.trim().as_bytes()),
            template: self.customer.template.clone(),
            model_provider: "openai".into(),
            price_book: digest_of(&contract::price_book()),
        };
        let mut config = serde_json::to_value(self)?;
        // Approval names the identity computed from these original inputs.
        // Its own digest cannot be part of the identity it approves.
        if let Some(ops) = config.get_mut("operations").and_then(Value::as_object_mut) {
            ops.remove("approved_identity");
        }
        let ingress = self
            .operations
            .as_ref()
            .map(|o| ingress_bytes(&o.ingress_file).map(|b| Digest::of_bytes(&b)))
            .transpose()?;
        let bytes = executable_bytes(executable)?;
        Ok((
            Identity {
                schema: OPERATIONS_SCHEMA.into(),
                commit: commit.into(),
                tree: tree.into(),
                executable: Digest::of_bytes(&bytes),
                configuration: digest_of(&(config, &binding, &ingress, Digest::of_bytes(boat_key))),
                service: launch::SERVICE.into(),
                customer_schema: crate::types::SCHEMA.into(),
                storage_schema: STORAGE_SCHEMA.into(),
                price_book: contract::price_book().version,
                price_digest: digest_of(&contract::price_book()),
                qualification_plan: self.customer.supported_plan.clone(),
                template: self.customer.template.clone(),
                mode: self.mode,
            },
            binding,
        ))
    }
}
impl Operating {
    pub fn worker_healthy(&self, now: i64) -> bool {
        self.host.mode != Mode::Production
            || self.last_worker.lock().is_ok_and(|w| {
                w.is_some_and(|(at, ok)| {
                    ok && at <= now
                        && now.saturating_sub(at)
                            <= i64::try_from(self.host.poll_seconds * 3 + 30).unwrap_or(120)
                })
            })
    }
    pub fn new(
        host: Host,
        path: &Path,
        node: &str,
        provider_source_digest: Digest,
    ) -> Result<Self> {
        let config_bytes = private_read(path, 256 * 1024)?;
        let current: Host = serde_json::from_slice(&config_bytes)?;
        if serde_json::to_value(&current)? != serde_json::to_value(&host)? {
            return Err(Error::Conflict(
                "retail configuration changed during startup",
            ));
        }
        let provider_bytes = private_read(&host.boat_key_file, 8192)?;
        if Digest::of_bytes(&provider_bytes) != provider_source_digest {
            return Err(Error::Conflict("provider key changed during startup"));
        }
        let (identity, binding) = host.identity_from_provider(
            node,
            &std::env::current_exe()?,
            COMMIT,
            TREE,
            &provider_bytes,
        )?;
        let operator_digest = host
            .operations
            .as_ref()
            .map(|o| private_read(&o.operator_key_file, 4096).map(|b| Digest::of_bytes(&b)))
            .transpose()?;
        let ingress_digest = host
            .operations
            .as_ref()
            .map(|o| ingress_bytes(&o.ingress_file).map(|b| Digest::of_bytes(&b)))
            .transpose()?;
        Ok(Self {
            identity,
            host,
            config_path: path.into(),
            config_digest: Digest::of_bytes(&config_bytes),
            binding,
            operator_digest,
            last_worker: Mutex::new(None),
            ingress_digest,
            provider_source_digest,
        })
    }
    pub fn operator(&self, secret: &str) -> bool {
        self.operator_digest.as_ref().is_some_and(|d| {
            !secret.is_empty() && secret.len() <= 4096 && &Digest::of_bytes(secret.as_bytes()) == d
        }) && self.host.operations.as_ref().is_some_and(|o| {
            private_read(&o.operator_key_file, 4096)
                .is_ok_and(|b| Some(Digest::of_bytes(&b)) == self.operator_digest)
        })
    }
    pub fn gate(
        &self,
        wallet: &impl LightningWallet,
    ) -> std::result::Result<Option<QualificationReceipt>, &'static str> {
        if private_read(&self.config_path, 256 * 1024)
            .map(|b| Digest::of_bytes(&b))
            .ok()
            .as_ref()
            != Some(&self.config_digest)
        {
            return Err("configuration_changed");
        }
        if self.host.mode == Mode::Development {
            return Ok(self.host.customer.qualification.clone());
        }
        if self.host.mode == Mode::Closed {
            return Err("deployment_closed");
        }
        let ops = self.host.operations.as_ref().ok_or("operations_absent")?;
        if ingress_bytes(&ops.ingress_file)
            .map(|b| Digest::of_bytes(&b))
            .ok()
            != self.ingress_digest
        {
            return Err("ingress_configuration_changed");
        }
        if self.identity.tree != "clean"
            || self.identity.commit.len() != 40
            || ops.approved_identity != self.identity.digest()
        {
            return Err("running_identity_unapproved");
        }
        let key =
            private_read(&self.host.boat_key_file, 8192).map_err(|_| "provider_key_unavailable")?;
        if Digest::of_bytes(&key) != self.provider_source_digest {
            return Err("provider_key_changed");
        }
        let plan: Plan = serde_json::from_slice(
            &private_read(&ops.plan_file, 32768).map_err(|_| "plan_unavailable")?,
        )
        .map_err(|_| "plan_invalid")?;
        if plan.check().is_err() || plan.digest() != self.identity.qualification_plan {
            return Err("plan_mismatch");
        }
        let bytes = private_read(&ops.qualification_file, 256 * 1024)
            .map_err(|_| "qualification_unavailable")?;
        if Digest::of_bytes(&bytes) != ops.qualification_digest {
            return Err("qualification_changed");
        }
        let receipt: QualificationReceipt =
            serde_json::from_slice(&bytes).map_err(|_| "qualification_invalid")?;
        verify_qualification(
            &plan,
            &receipt,
            &self.binding,
            &ops.qualification_state,
            &ops.qualification_ledger,
            wallet,
        )?;
        if private_read(&self.config_path, 256 * 1024)
            .map(|b| Digest::of_bytes(&b))
            .ok()
            .as_ref()
            != Some(&self.config_digest)
            || private_read(&ops.qualification_file, 256 * 1024)
                .map(|b| Digest::of_bytes(&b))
                .ok()
                .as_ref()
                != Some(&ops.qualification_digest)
            || private_read(&ops.plan_file, 32768)
                .ok()
                .and_then(|b| serde_json::from_slice::<Plan>(&b).ok())
                .is_none_or(|p| p != plan)
            || private_read(&self.host.boat_key_file, 8192)
                .map(|b| Digest::of_bytes(&b))
                .ok()
                .as_ref()
                != Some(&self.provider_source_digest)
            || ingress_bytes(&ops.ingress_file)
                .map(|b| Digest::of_bytes(&b))
                .ok()
                != self.ingress_digest
        {
            return Err("deployment_sources_changed");
        }
        Ok(Some(receipt))
    }
}
/// Rebuild qualification from native read-only records and authenticated
/// receiver evidence. A renamed fake/simulation receipt cannot pass.
pub fn verify_qualification(
    plan: &Plan,
    r: &QualificationReceipt,
    binding: &DeploymentBinding,
    state: &Path,
    ledger_path: &Path,
    wallet: &impl LightningWallet,
) -> std::result::Result<(), &'static str> {
    if plan.check().is_err()
        || r.schema != retail_qualify::qualify::RECEIPT_SCHEMA
        || r.mode != QualificationMode::Funded
        || !r.qualified
        || r.failure.is_some()
        || r.simulation.is_some()
        || !r.preimage_verified
        || !r.teardown_acknowledged
        || !r.ledger_conserved
        || r.unknown_held_msat != 0
        || r.check.as_deref() != Some("verified")
        || r.plan != plan.digest()
        || r.account != plan.account
        || r.deployment.as_ref() != Some(binding)
    {
        return Err("qualification_not_valid");
    }
    let journal_path = state.join("journal.sqlite");
    // Validate private paths before native readers open; never create evidence.
    let ledger_stamp = private_stamp(ledger_path, 64 * 1024 * 1024)
        .map_err(|_| "qualification_ledger_unavailable")?;
    let journal_stamp = private_stamp(&journal_path, 64 * 1024 * 1024)
        .map_err(|_| "qualification_journal_unavailable")?;
    let ledger = Ledger::open_read_only(ledger_path).map_err(|_| "qualification_ledger_invalid")?;
    let journal =
        Journal::open_read_only(&journal_path).map_err(|_| "qualification_journal_invalid")?;
    let execution = r
        .execution
        .as_deref()
        .ok_or("qualification_execution_absent")?;
    let funded = journal
        .funded(execution)
        .map_err(|_| "qualification_execution_invalid")?
        .ok_or("qualification_execution_absent")?;
    let expected = contract::TaskRequest {
        source: plan.source.clone(),
        task: plan.task.clone(),
        checks: vec![plan.check.clone()],
        max_seconds: plan.max_seconds,
        ceiling_sats: Some(plan.ceiling_sats),
    };
    if funded.task != expected
        || funded.account != plan.account
        || funded.request != r.hold.clone().unwrap_or_default()
        || funded.admission
            != contract::admission(&plan.account, execution, &expected, &funded.quote, 1)
        || funded.quote.check(&contract::price_book()).is_err()
    {
        return Err("qualification_terms_mismatch");
    }
    let current = Current {
        observe: Some(ObserveGrant {
            account: plan.account.clone(),
            execution: execution.into(),
            revoked: false,
        }),
        ..Current::default()
    };
    let settlement = retail_cloud::settle::observe(&journal, &funded, &current)
        .map_err(|_| "qualification_settlement_invalid")?
        .ok_or("qualification_settlement_absent")?;
    let hold = ledger
        .hold(&funded.request)
        .map_err(|_| "qualification_hold_invalid")?
        .ok_or("qualification_hold_absent")?;
    let cleanup = journal
        .retention_receipt(execution, crate::http::now())
        .map_err(|_| "qualification_cleanup_invalid")?
        .ok_or("qualification_cleanup_absent")?;
    let provision = journal
        .provisioning(execution)
        .map_err(|_| "qualification_resource_invalid")?
        .ok_or("qualification_resource_absent")?;
    let dispatch = journal
        .dispatch(execution)
        .map_err(|_| "qualification_task_invalid")?
        .ok_or("qualification_task_absent")?;
    let usage = journal
        .usage(execution)
        .map_err(|_| "qualification_usage_invalid")?
        .ok_or("qualification_usage_absent")?;
    if hold.request != retail_cloud::reserve::hold_request(&funded, hold.request.at)
        || settlement.execution != funded.execution
        || settlement.request != funded.request
        || settlement.quote != digest_of(&funded.quote).to_string()
        || settlement.book != funded.quote.version
        || settlement.resource != pay_ledger::compute::hold::RETAIL_RESOURCE
        || settlement.usage_digest != Some(digest_of(&usage).to_string())
        || settlement.ending != route_contract::price_book::Ending::ExecutorEnded
        || settlement.charge != usage.settlement(&funded, settlement.ending)
        || settlement
            .charge
            .charge_sats
            .and_then(|s| s.checked_mul(1000))
            .and_then(|m| i64::try_from(m).ok())
            != settlement.charge_msat
        || hold.state != HoldState::Settled
        || hold.charge_msat != r.charge_msat
        || hold.released_msat() != r.released_msat
        || settlement.source != r.settlement_source
        || settlement.charge_msat != r.charge_msat
        || settlement.released_msat != r.released_msat.unwrap_or(-1)
        || settlement.held_msat != 0
        || settlement.checks != Some(retail_cloud::dispatch::Verdict::Verified)
        || !cleanup.deleted()
        || cleanup.resources.is_empty()
        || cleanup.resources.iter().any(|c| {
            c.acknowledged_at.is_none_or(|at| {
                at > i64::try_from(funded.confirmed_at)
                    .unwrap_or(i64::MAX)
                    .saturating_add(plan.cleanup_deadline_seconds)
            })
        })
        || dispatch.task != r.task.clone().unwrap_or_default()
        || match &provision.state {
            retail_cloud::provision::ProvisionState::Ready { resource, .. }
            | retail_cloud::provision::ProvisionState::Starting { resource } => {
                Some(resource.as_str())
            }
            _ => None,
        } != r.sandbox.as_deref()
        || usage.events.last().and_then(|e| e.seconds) != r.provider_seconds
    {
        return Err("qualification_delivery_mismatch");
    }
    if let Some(source) = &settlement.source {
        let debit = ledger
            .settlement(source)
            .map_err(|_| "qualification_debit_invalid")?
            .ok_or("qualification_debit_absent")?;
        if debit.resource != pay_ledger::compute::hold::RETAIL_RESOURCE
            || Some(debit.received_msat) != settlement.charge_msat
            || source != &format!("debit:{}", funded.request)
            || Some(debit.price_msat) != settlement.charge_msat
        {
            return Err("qualification_debit_mismatch");
        }
    } else {
        return Err("qualification_debit_absent");
    }
    let balance = ledger
        .compute_balance(&plan.account)
        .map_err(|_| "qualification_balance_invalid")?;
    if balance.available_msat < 0
        || balance.held_msat != 0
        || Some(balance.credited_msat)
            != balance
                .available_msat
                .checked_add(balance.held_msat)
                .and_then(|v| v.checked_add(balance.settled_msat))
    {
        return Err("qualification_ledger_drift");
    }
    let hash = r
        .invoice_payment_hash
        .as_deref()
        .ok_or("qualification_payment_absent")?;
    let purchase = ledger
        .top_ups(&plan.account)
        .map_err(|_| "qualification_payment_invalid")?
        .into_iter()
        .find(|p| p.top_up.payment_hash == hash)
        .ok_or("qualification_payment_absent")?;
    if purchase.state != PurchaseState::Paid
        || purchase.top_up.amount_msat
            != i64::try_from(plan.top_up_sats * 1000).map_err(|_| "qualification_amount_invalid")?
    {
        return Err("qualification_payment_not_paid");
    }
    let invoice = nostr::x402::decode_invoice(&purchase.top_up.invoice)
        .map_err(|_| "qualification_invoice_invalid")?;
    if invoice.currency() != "bc"
        || invoice.amount_msat() != purchase.top_up.amount_msat as u64
        || invoice.payment_hash() != parse_hash32(hash).map_err(|_| "qualification_hash_invalid")?
        || hex(&invoice.payee()) != binding.receiver
        || invoice.description_hash()
            != retail_cloud::topup::request_hash(
                &plan.account,
                &purchase.top_up.id,
                plan.top_up_sats,
            )
    {
        return Err("qualification_invoice_mismatch");
    }
    if invoice.expiry_seconds() != u64::from(retail_cloud::topup::INVOICE_EXPIRY_SECS)
        || i64::try_from(invoice.created_at())
            .unwrap_or(i64::MAX)
            .abs_diff(purchase.top_up.created_at)
            > 60
    {
        return Err("qualification_invoice_time_mismatch");
    }
    let payment = wallet
        .lookup_from_node(&binding.receiver, invoice.payment_hash())
        .map_err(|_| "qualification_receiver_unavailable")?
        .ok_or("qualification_receiver_unknown")?;
    if payment.updated_at
        > invoice
            .created_at()
            .saturating_add(invoice.expiry_seconds())
        || payment.updated_at > u64::try_from(crate::http::now().saturating_add(60)).unwrap_or(0)
        || payment.direction != PaymentDirection::Inbound
        || payment.status != PaymentStatus::Succeeded
        || payment.payment_hash != hash
        || payment.amount_msat != Some(invoice.amount_msat())
        || payment
            .bolt11
            .as_ref()
            .is_some_and(|s| s != &purchase.top_up.invoice)
        || payment.updated_at < invoice.created_at()
        || payment
            .preimage
            .as_ref()
            .is_none_or(|s| parse_hash32(s).map_or(true, |p| retail_cloud::sha256_hex(&p) != hash))
    {
        return Err("qualification_receiver_not_paid");
    }
    if private_stamp(ledger_path, 64 * 1024 * 1024).ok() != Some(ledger_stamp)
        || private_stamp(&journal_path, 64 * 1024 * 1024).ok() != Some(journal_stamp)
    {
        return Err("qualification_sources_changed");
    }
    Ok(())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl<B: Backend, W: LightningWallet + Send + Sync + 'static> Service<B, W> {
    pub fn with_operations(
        mut self,
        host: Host,
        path: &Path,
        provider_source_digest: Digest,
    ) -> Result<Self> {
        host.check()?;
        if serde_json::to_value(&host.customer)? != serde_json::to_value(&self.config)? {
            return Err(Error::Conflict(
                "operations name another customer configuration",
            ));
        }
        self.operations = Some(Arc::new(Operating::new(
            host,
            path,
            &self.wallet.node_id(),
            provider_source_digest,
        )?));
        Ok(self)
    }
    pub fn operator_status(&self, secret: &str, now: i64) -> Result<Value> {
        let ops = self
            .operations
            .as_ref()
            .filter(|o| o.operator(secret))
            .ok_or(Error::Denied)?;
        let store = self.lock()?;
        let alerts = launch::health(&store.journal, &store.ledger, now)?;
        let gate = ops.gate(&*self.wallet);
        let worker = ops
            .last_worker
            .lock()
            .map_err(|_| Error::Unavailable("worker status unavailable"))?
            .to_owned();
        Ok(
            json!({"schema":OPERATIONS_SCHEMA,"identity":ops.identity,"identity_digest":ops.identity.digest(),"paid":self.advertisement(&store,None)?,"deployment_closed":gate.err(),"alerts":alerts,"worker":{"last_tick":worker.map(|w|w.0),"last_ok":worker.map(|w|w.1),"ready":ops.worker_healthy(now)},"synthetic":ops.host.mode==Mode::Development}),
        )
    }
}

//! One explicitly admitted paid coding order. The labor journal retains the
//! commercial proof; the existing wallet and central ledger own money.

use crate::book::Book;
use crate::*;
use coder::task::sales::{self, partners};
use nostr::contracts::{ArtifactRef, DefinitionRef};
use nostr::market_contracts::{self as mkt, DomainProfile};
use nostr::private_artifact::OpenEnvelope;
use openagents_wallet::{LightningWallet, PaymentDirection, PaymentStatus};
use pay_ledger::markets::{bids, worker};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const NOTICE_SCHEMA: &str = "coder.paid-labor.notice.v1";
pub const GRANT_SCHEMA: &str = "coder.paid-labor.current-provider-grant.v1";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartnerPin {
    pub lead: String,
    pub assignment: String,
    pub proposal_sha256: String,
    pub account: String,
    pub owner_human: String,
    pub provider_human: String,
    pub support_human: String,
    pub obligation: receipts::service_sale::Fulfillment,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Setup {
    pub schema: String,
    pub worker: worker::WorkerTerms,
    pub rfq: bids::Request,
    pub bids: Vec<bids::Bid>,
    pub selection: bids::Selection,
    pub selected_at: u64,
    pub policy_file: std::path::PathBuf,
    pub partner: PartnerPin,
    /// A separately admitted host key, never the buyer or provider's own claim.
    pub admission_issuer: String,
    /// The current host policy epoch. A changed/revoked policy refuses reuse.
    pub policy_epoch: u64,
    pub central_node: String,
    pub destination_kind: String,
    pub destination_value: String,
    pub checker: Checker,
}
/// The independently admitted buyer checker stays outside provider writes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checker {
    pub labor_directory: std::path::PathBuf,
    pub task_directory: std::path::PathBuf,
    pub task_id: String,
    pub intent: coder::task::TaskIntent,
    pub program: std::path::PathBuf,
    pub program_sha256: String,
    pub expected_file: std::path::PathBuf,
    pub candidate_file: std::path::PathBuf,
    pub provider_file: std::path::PathBuf,
    pub wall_seconds: u64,
}
impl Setup {
    pub fn digest(&self) -> Result<String> {
        Ok(nostr::contracts::digest_bytes(
            &jcs(&json!(self)).map_err(|e| e.to_string())?,
        ))
    }
    pub(crate) fn check_market(&self, terms: &mkt::Terms) -> Result<()> {
        self.check_market_fields(terms)?;
        pay_ledger::markets::exact(&self.selection.admission).map_err(|e| e.to_string())?;
        for pin in [&self.admission_issuer, &self.partner.proposal_sha256] {
            pay_ledger::markets::exact(pin).map_err(|e| e.to_string())?;
        }
        if self.admission_issuer == terms.buyer
            || self.admission_issuer == terms.provider
            || self.central_node.len() != 66
            || secp256k1::PublicKey::from_str(&self.central_node).is_err()
            || self.destination_value.is_empty()
            || self.destination_value.len() > 1024
            || !["lud16", "spark", "node"].contains(&self.destination_kind.as_str())
            || self.partner.obligation.currency != "BTC"
            || self.partner.obligation.currency_scale != 100_000_000_000
            || self.partner.obligation.amount_minor != terms.price_msat
            || self.partner.obligation.trigger
                != receipts::service_sale::FulfillmentTrigger::AcceptedDelivery
            || self.partner.obligation.responsible_human != self.partner.provider_human
            || self.partner.obligation.bill.is_some()
            || self.partner.obligation.payment.is_some()
        {
            return Err("paid fulfillment needs distinct admission, exact BTC millisatoshis and qualified destinations".into());
        }
        Ok(())
    }
    fn check_market_fields(&self, terms: &mkt::Terms) -> Result<()> {
        if self.schema != "coder.paid-labor.setup.v1"
            || terms.payment_profile != mkt::LIGHTNING_PROFILE
            || terms.price_msat == 0
            || terms.price_msat > i64::MAX as u64
            || terms.fee_limit_msat > i64::MAX as u64
            || self.worker.price_msat != terms.price_msat as i64
            || self.worker.fee_limit_msat != terms.fee_limit_msat as i64
            || self.worker.buyer != terms.buyer
            || self.worker.provider != terms.provider
            || self.worker.labor_terms != hex_digest(&terms.profile_terms.digest)?
            || self.selected_at >= terms.quote_expires_at
            || !self.policy_file.is_absolute()
            || self.worker.max_rework != 0
            || self.policy_epoch == 0
            || self.worker.profile != worker::PROFILE
            || [
                terms.delivery_due_at,
                terms.review_due_at,
                terms.payment_due_at,
                terms.retain_until,
            ]
            .iter()
            .any(|t| *t > i64::MAX as u64)
            || !matches!(terms.network.as_deref(), Some("bitcoin" | "testnet"))
            || self.worker.deadlines.delivery != terms.delivery_due_at as i64
            || self.worker.deadlines.review != terms.review_due_at as i64
            || self.worker.deadlines.payment != terms.payment_due_at as i64
            || self.worker.deadlines.retain_until != terms.retain_until as i64
            || self.selection.order != self.worker.order
            || self.selection.payer != terms.buyer
        {
            return Err("paid setup changes the exact fixed postacceptance terms".into());
        }
        Ok(())
    }
    fn check_host_paths(&self, book: &Book) -> Result<()> {
        let input = book.blobs.resolve(&book.labor().execution.input)?;
        let provider = Path::new(
            input["intent"]["workspace"]["path"]
                .as_str()
                .ok_or("paid provider workspace unavailable")?,
        )
        .canonicalize()
        .map_err(|_| "paid provider workspace unavailable")?;
        let protected = Path::new(&self.checker.intent.workspace.path)
            .canonicalize()
            .map_err(|_| "protected buyer checker workspace unavailable")?;
        if protected == provider
            || protected.starts_with(&provider)
            || !self.checker.labor_directory.is_absolute()
            || !self.checker.task_directory.is_absolute()
            || self.checker.wall_seconds == 0
            || self.checker.wall_seconds > 60
            || self
                .checker
                .program
                .canonicalize()
                .map_err(|_| "buyer checker unavailable")?
                != Path::new("/usr/bin/cmp")
                    .canonicalize()
                    .map_err(|_| "buyer checker unavailable")?
            || hex_digest(&self.checker.program_sha256)?
                != hex_digest(&nostr::contracts::digest_bytes(
                    &std::fs::read(&self.checker.program)
                        .map_err(|_| "buyer checker unavailable")?,
                ))?
        {
            return Err(
                "paid checker must be a separately admitted bounded byte comparison".into(),
            );
        }
        for path in [
            &self.policy_file,
            &self.checker.labor_directory,
            &self.checker.task_directory,
        ] {
            if existing_path(path)?.starts_with(&provider) {
                return Err(
                    "provider writes cannot contain current policy or checker custody".into(),
                );
            }
        }
        for path in [
            &self.checker.expected_file,
            &self.checker.candidate_file,
            &self.checker.provider_file,
        ] {
            if path.as_os_str().is_empty()
                || path.components().count() != 1
                || path
                    .components()
                    .any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err("paid comparison needs confined relative file paths".into());
            }
        }
        if self.checker.expected_file == self.checker.candidate_file {
            return Err("expected and candidate checker inputs must differ".into());
        }
        Ok(())
    }
    pub(crate) fn check_record(&self, record: &mkt::Record) -> Result<()> {
        match record.body() {
            mkt::Body::Rfq { request, .. }
                if hex_digest(&record.artifact().digest)? != self.rfq.rfq
                    || hex_digest(&request.digest)? != self.worker.labor_terms =>
            {
                Err("RFQ differs from the separately retained bid comparison".into())
            }
            mkt::Body::Quote { rfq, .. } if hex_digest(&rfq.digest)? != self.rfq.rfq => {
                Err("quote changes the buyer's exact RFQ".into())
            }
            mkt::Body::Quote { quote_id, .. } if quote_id != &self.selection.quote => {
                Err("quote differs from the separately approved bid selection".into())
            }
            mkt::Body::Order { order_id, .. } if order_id != &self.selection.order => {
                Err("order differs from the separately approved bid selection".into())
            }
            _ => Ok(()),
        }
    }
}
fn existing_path(path: &Path) -> Result<std::path::PathBuf> {
    if path.exists() {
        return path
            .canonicalize()
            .map_err(|_| "protected path unavailable".into());
    }
    let parent = path
        .parent()
        .ok_or("protected path has no parent")?
        .canonicalize()
        .map_err(|_| "protected path parent unavailable")?;
    Ok(parent.join(path.file_name().ok_or("protected path name unavailable")?))
}
/// Keep monetary and authorization custody outside admitted provider writes.
pub fn check_private_host_path(book: &Book, path: &Path) -> Result<()> {
    let input = book.blobs.resolve(&book.labor().execution.input)?;
    let provider = Path::new(
        input["intent"]["workspace"]["path"]
            .as_str()
            .ok_or("provider workspace unavailable")?,
    )
    .canonicalize()
    .map_err(|_| "provider workspace unavailable")?;
    if existing_path(path)?.starts_with(provider) {
        return Err("provider writes cannot contain private host custody".into());
    }
    Ok(())
}
use std::str::FromStr;
fn hex_digest(value: &str) -> Result<String> {
    let value = value.strip_prefix("sha256:").unwrap_or(value);
    pay_ledger::markets::exact(value).map_err(|e| e.to_string())?;
    Ok(value.into())
}

pub(crate) struct Profile<'a> {
    pub labor: mkt::labor::LaborProfile<'a, crate::admission::Admission>,
    pub paid: Option<&'a Setup>,
}
impl DomainProfile for Profile<'_> {
    fn id(&self) -> &str {
        self.labor.id()
    }
    fn validate_request(&self, request: &ArtifactRef) -> std::result::Result<(), ContractError> {
        self.labor.validate_request(request)
    }
    fn validate_terms(
        &self,
        terms: &mkt::Terms,
        capability: &DefinitionRef,
    ) -> std::result::Result<(), ContractError> {
        self.labor.validate_terms(terms, capability)
    }
    fn validate_payment_profile(
        &self,
        terms: &mkt::Terms,
    ) -> std::result::Result<(), ContractError> {
        match self.paid {
            Some(paid) => paid.check_market(terms).map_err(contract),
            None => self.labor.validate_payment_profile(terms),
        }
    }
}

/// Signed current host policy, read outside provider-controlled task inputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrentGrant {
    pub schema: String,
    pub setup_digest: String,
    pub admission: String,
    pub epoch: u64,
    pub active: bool,
    pub observed_at: u64,
    pub expires_at: u64,
    pub independently_verified: bool,
    pub buyer_operator: String,
    pub provider_operator: String,
    pub source_rights_verified: bool,
    pub available_capacity: u32,
    pub destination_kind: String,
    pub destination_value: String,
    pub evidence: Vec<Value>,
}
/// The host independently checks current policy and canonical partner custody.
/// Implementations must not source this result from worker output or discovery.
pub trait Authority {
    fn check(&mut self, setup: &Setup, book: &Book, now: u64, dispatch: bool) -> Result<()>;
}

/// The actual local adapter: an owner-authenticated canonical private pipeline
/// plus a separately trusted, signed current provider policy document.
pub struct PipelineAuthority<'a> {
    pub root: &'a Path,
    pub credential: &'a Path,
    pub evidence: &'a Blobs,
}
impl Authority for PipelineAuthority<'_> {
    fn check(&mut self, setup: &Setup, book: &Book, now: u64, dispatch: bool) -> Result<()> {
        setup.check_host_paths(book)?;
        // Release canonical custody between checks so another authenticated
        // operator can revoke a grant while a bounded process is running.
        let mut store = sales::Store::open(self.root)?;
        let input = book.blobs.resolve(&book.labor().execution.input)?;
        let provider = Path::new(
            input["intent"]["workspace"]["path"]
                .as_str()
                .ok_or("provider source unavailable")?,
        )
        .canonicalize()
        .map_err(|_| "provider source unavailable")?;
        if existing_path(self.root)?.starts_with(&provider)
            || existing_path(self.credential)?.starts_with(&provider)
        {
            return Err("provider writes cannot contain canonical sales authority".into());
        }
        let access = store.authenticate(&sales::Store::read_credential(self.credential)?)?;
        store.authorize_funnel_snapshots(&access, &[])?;
        let lead = store.show(&access, &setup.partner.lead)?;
        let value = store.partner_show(&access, &setup.partner.lead, &setup.partner.assignment)?;
        let assignment: partners::Assignment = serde_json::from_value(value["assignment"].clone())
            .map_err(|_| "current accepted partner assignment is unavailable")?;
        if access.principal() != setup.partner.owner_human
            || lead.details.permission.state != sales::PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || assignment.proposal_sha256 != setup.partner.proposal_sha256
            || assignment.account != setup.partner.account
            || assignment.owner_human != setup.partner.owner_human
            || assignment.proposal.recipient_human != setup.partner.provider_human
            || assignment.proposal.expires_at <= now
            || !matches!(
                assignment.status,
                partners::Status::Accepted
                    | partners::Status::Delivered
                    | partners::Status::Completed
            )
            || dispatch && assignment.status != partners::Status::Accepted
            || !assignment
                .events
                .iter()
                .any(|e| e.outcome == "accepted" && e.actor == setup.partner.provider_human)
        {
            return Err(
                "paid fulfillment exceeds current accepted partner permission or responsibility"
                    .into(),
            );
        }
        let partners::Terms::Fulfillment {
            obligation, scope, ..
        } = &assignment.proposal.terms
        else {
            return Err("discovery cannot authorize paid fulfillment".into());
        };
        if obligation != &setup.partner.obligation {
            return Err("paid fulfillment changes the canonical partner obligation".into());
        }
        // The accepted scope names the independent support owner and exact
        // zero revision/rework policy. The authenticated host retained it.
        let scope_value = self
            .evidence
            .0
            .get(&format!("sha256:{}", scope.sha256))
            .ok_or("accepted partner scope is unavailable")?;
        if nostr::contracts::digest_bytes(&jcs(scope_value).map_err(|e| e.to_string())?)
            != format!("sha256:{}", scope.sha256)
            || scope_value["support_human"] != setup.partner.support_human
            || scope_value["revision_limit"] != 0
            || scope_value["rework_limit"] != 0
        {
            return Err("paid fulfillment changes its support or zero revision scope".into());
        }
        let grant_event: nostr::domain::Event =
            serde_json::from_slice(&private_document(&setup.policy_file, 64 * 1024)?)
                .map_err(|_| "malformed current signed provider policy")?;
        grant_event
            .validate_nip01_structure()
            .map_err(|e| e.to_string())?;
        grant_event.validate_crypto().map_err(|e| e.to_string())?;
        if grant_event.pubkey != setup.admission_issuer
            || grant_event.kind != 1
            || grant_event.created_at > now
            || grant_event.content.len() > 64 * 1024
        {
            return Err(
                "provider admission must come from the separately trusted current host".into(),
            );
        }
        let grant: CurrentGrant = serde_json::from_value(
            nostr::contracts::parse_strict(grant_event.content.as_bytes())
                .map_err(|e| e.to_string())?,
        )
        .map_err(|_| "malformed current provider grant")?;
        check_grant(setup, book, &grant, self.evidence, now, dispatch)
    }
}
fn check_grant(
    setup: &Setup,
    book: &Book,
    grant: &CurrentGrant,
    evidence: &Blobs,
    now: u64,
    dispatch: bool,
) -> Result<()> {
    let labor = book.blobs.resolve(&book.market().profile_terms)?;
    let roles = labor["role_relationships"]
        .as_array()
        .ok_or("admitted labor operator relationships are unavailable")?;
    for (party, operator) in [
        (&setup.worker.buyer, &setup.worker.buyer_operator),
        (&setup.worker.provider, &setup.worker.provider_operator),
    ] {
        if !roles
            .iter()
            .any(|r| r["pubkey"] == *party && r["operator"] == *operator)
        {
            return Err("current operator differs from the accepted labor relationship".into());
        }
    }
    if grant.schema != GRANT_SCHEMA
        || grant.setup_digest != setup.digest()?
        || grant.admission != setup.selection.admission
        || grant.epoch != setup.policy_epoch
        || !grant.active
        || grant.observed_at > now
        || grant.expires_at <= now
        || !grant.independently_verified
        || !grant.source_rights_verified
        || grant.buyer_operator != setup.worker.buyer_operator
        || grant.provider_operator != setup.worker.provider_operator
        || grant.destination_kind != setup.destination_kind
        || grant.destination_value != setup.destination_value
        || grant.evidence.is_empty()
        || grant.evidence.len() > 32
        || dispatch && grant.available_capacity < setup.worker.capacity_units
    {
        return Err("current independent provider, source rights, capacity or destination admission is unavailable".into());
    }
    for reference in &grant.evidence {
        evidence.get(reference)?;
    }
    if setup.worker.buyer_operator == setup.worker.provider_operator
        || setup.rfq.capability != hex_digest(&book.labor().execution.target.artifact.digest)?
        || setup.worker.source != hex_digest(&book.labor().execution.input.digest)?
        || setup.worker.checker != hex_digest(&book.policy().checker.artifact.digest)?
        || setup.worker.disclosure != hex_digest(&book.labor().execution.context.digest)?
        || setup.worker.execution_requirements
            != hex_digest(&book.labor().execution.requirements.digest)?
        || setup.worker.delivery_rights != hex_digest(&book.labor().rights.digest)?
        || setup.worker.market
            != book
                .order()
                .map(|o| o.market.as_str())
                .unwrap_or(&setup.worker.market)
        || setup.worker.deadlines.dispute != book.labor().dispute_due_at as i64
        || setup.worker.deadlines.resolution != book.labor().resolution_due_at as i64
        || setup.worker.cancellation_policy
            != hex_digest(&nostr::contracts::digest_bytes(
                &jcs(&labor["cancellation"]).map_err(|e| e.to_string())?,
            ))?
        || book.labor().max_reworks != 0
    {
        return Err(
            "worker qualification changes the exact admitted source/checker/rights closure".into(),
        );
    }
    bids::compare(&setup.rfq, &setup.bids, setup.selected_at as i64).map_err(|e| e.to_string())?;
    let bid = setup
        .bids
        .iter()
        .find(|b| b.quote == setup.selection.quote)
        .ok_or("selected bid is unavailable")?;
    setup
        .selection
        .validate(bid, setup.selected_at as i64)
        .map_err(|e| e.to_string())?;
    if bid.rfq != setup.rfq.rfq
        || bid.capability != setup.rfq.capability
        || bid.source != setup.rfq.source
        || bid.disclosure != setup.rfq.disclosure
        || bid.all_in().map_err(|e| e.to_string())? > setup.rfq.max_all_in_msat
        || bid.provider != setup.worker.provider
        || bid.labor_terms != setup.worker.labor_terms
        || bid.source != setup.worker.source
        || bid.disclosure != setup.worker.disclosure
        || bid.price_msat != setup.worker.price_msat
        || bid.fee_limit_msat != setup.worker.fee_limit_msat
    {
        return Err("paid fulfillment differs from the independently selected quote".into());
    }
    if dispatch {
        setup
            .worker
            .validate(
                now as i64,
                &worker::Admission {
                    buyer: setup.worker.buyer.clone(),
                    provider: setup.worker.provider.clone(),
                    buyer_operator: grant.buyer_operator.clone(),
                    provider_operator: grant.provider_operator.clone(),
                    independent_operators_verified: grant.independently_verified,
                    execution_requirements: setup.worker.execution_requirements.clone(),
                    disclosure: setup.worker.disclosure.clone(),
                    available_capacity: grant.available_capacity,
                    expires_at: grant.expires_at as i64,
                    payout_destination_verified: true,
                },
            )
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Costs {
    pub coordination_msat: Option<u64>,
    pub execution_msat: Option<u64>,
    pub checker_msat: Option<u64>,
    pub failed_attempts_msat: Option<u64>,
    pub payment_fees_msat: Option<u64>,
    pub evidence: Vec<Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notice {
    pub v: String,
    pub requires: Vec<String>,
    pub issuer: String,
    pub order: Value,
    pub kind: String,
    pub responsible_human: String,
    pub next_action: String,
    pub due_at: u64,
    pub evidence: Vec<Value>,
    pub costs: Option<Costs>,
}
pub(crate) fn notice(
    book: &mut Book,
    opened: &OpenEnvelope,
    blobs: &Blobs,
    now: u64,
) -> Result<&'static str> {
    let setup = book
        .paid()
        .ok_or("support profile requires a paid fulfillment setup")?;
    let body: Notice =
        serde_json::from_slice(opened.inline_bytes().ok_or("support notice unavailable")?)
            .map_err(|_| "malformed paid support notice")?;
    if body.v != NOTICE_SCHEMA
        || !body.requires.is_empty()
        || opened.artifact().schema.as_deref() != Some(NOTICE_SCHEMA)
        || body.issuer != opened.signer()
        || ![
            book.market().buyer.as_str(),
            book.market().provider.as_str(),
        ]
        .contains(&opened.signer())
        || Some(&mkt::parse_order_ref(&body.order).map_err(|e| e.to_string())?) != book.order()
        || !["cancel", "request_rework", "support", "costs"].contains(&body.kind.as_str())
        || body.responsible_human != setup.partner.support_human
        || body.next_action.is_empty()
        || body.next_action.len() > 2048
        || body.due_at < now
        || body.due_at >= book.market().retain_until
        || body.evidence.is_empty()
        || body.evidence.len() > 32
    {
        return Err(
            "paid notice changes its order, parties, support owner, deadline or evidence".into(),
        );
    }
    for reference in &body.evidence {
        blobs.get(reference)?;
    }
    if let Some(costs) = &body.costs {
        if costs.evidence.is_empty() || costs.evidence.len() > 32 {
            return Err("known and unknown costs require attributable source records".into());
        }
        for reference in &costs.evidence {
            blobs.get(reference)?;
        }
        if costs
            .payment_fees_msat
            .is_some_and(|fee| fee > book.market().fee_limit_msat)
        {
            return Err("observed routing cost exceeds the accepted fee ceiling".into());
        }
    }
    let value = json!({"record":artifact_value(opened.artifact()),"kind":body.kind,"notice":body});
    if book
        .paid_notices
        .iter()
        .any(|n| n["record"] == value["record"])
    {
        return Ok("duplicate");
    }
    if book.paid_notices.len() >= 64 {
        return Err("paid support history exceeds bound".into());
    }
    book.paid_notices.push(value);
    Ok(if body.kind == "request_rework" {
        "zero_revision_rework_refused"
    } else {
        "applied"
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub invoice_preparation_started: bool,
    pub invoice: Option<openagents_wallet::IssuedInvoice>,
    pub funding_state: Option<String>,
    pub funding_at: Option<u64>,
    pub settlement_key: Option<String>,
    pub interruption: Option<String>,
    pub checker_started: bool,
    pub checker: Option<Checked>,
    pub funding_observation: Option<openagents_wallet::PaymentRecord>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checked {
    pub submission: Value,
    pub candidate_sha256: String,
    pub expected_sha256: String,
    pub grant: coder::task::owner::Grant,
    pub observation: Option<coder::task::Task>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct QueuedNotice {
    pub event: nostr::domain::Event,
    pub attachments: Blobs,
}
impl crate::store::Store {
    /// Run the frozen independent buyer comparison through the existing owner.
    /// A durable attempt is never replaced after an interrupted dispatch.
    pub async fn verify_paid_delivery(
        &mut self,
        authority: &mut dyn Authority,
        now: u64,
    ) -> Result<Checked> {
        self.check_open()?;
        let setup = self
            .book
            .paid()
            .ok_or("not a paid fulfillment order")?
            .clone();
        authority.check(&setup, &self.book, now, false)?;
        if self.book.local_party() != self.book.market().buyer {
            return Err("only the admitted buyer can run the protected checker".into());
        }
        if now >= self.book.market().review_due_at || self.book.records.conflict {
            return Err("checker review deadline or order conflict requires reconciliation".into());
        }
        let submission = self
            .book
            .records
            .submission
            .clone()
            .ok_or("provider submission unavailable")?;
        if self.document.paid.checker_started {
            let mut retained = self
                .document
                .paid
                .checker
                .clone()
                .ok_or("checker preparation remains unknown")?;
            if retained.submission != submission {
                return Err("checker submission cannot change".into());
            }
            if let Ok(task) =
                coder::task::owner::recover(&setup.checker.task_directory, &setup.checker.task_id)
            {
                retained.observation = Some(task);
                self.document.paid.checker = Some(retained.clone());
                self.save()?;
            }
            return Ok(retained);
        }
        let output = self.book.records.resolve(&submission)?["deliverables"]
            .as_array()
            .and_then(|r| r.first())
            .and_then(|r| r.get("content"))
            .ok_or("provider deliverable unavailable")?;
        let patch = self.book.blobs.get(output)?;
        let bytes = patch["files"]
            .as_array()
            .and_then(|r| {
                r.iter()
                    .find(|f| f["path"] == setup.checker.provider_file.to_string_lossy().as_ref())
            })
            .and_then(|r| r["utf8"].as_str())
            .ok_or("selected provider file unavailable")?
            .as_bytes()
            .to_vec();
        if bytes.len() > 4096 {
            return Err("selected paid file exceeds delivery bound".into());
        }
        let workspace = Path::new(&setup.checker.intent.workspace.path);
        let expected = read_checker_file(&workspace.join(&setup.checker.expected_file))?;
        let expected_sha256 = nostr::contracts::digest_bytes(&expected);
        let input = self
            .book
            .blobs
            .resolve(&self.book.labor().execution.input)?;
        if input["expected_output_digest"] != expected_sha256 {
            return Err("protected expected bytes differ from accepted buyer criterion".into());
        }
        let path = workspace.join(&setup.checker.candidate_file);
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|_| "protected candidate already exists or unavailable")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        let grant = coder::task::owner::Grant {
            schema: coder::task::owner::GRANT_SCHEMA.into(),
            task_id: setup.checker.task_id.clone(),
            intent_digest: nostr::contracts::digest_bytes(
                &serde_json::to_vec(&setup.checker.intent).map_err(|e| e.to_string())?,
            ),
            expected_revision: 1,
            expected_source_snapshot: Some(coder_boundary::Snapshot::observe(workspace).digest()),
            program: setup.checker.program.clone(),
            arguments: vec![
                setup.checker.expected_file.to_string_lossy().into(),
                setup.checker.candidate_file.to_string_lossy().into(),
            ],
            write_workspace: false,
            wall_seconds: setup.checker.wall_seconds,
            stream_bytes: 4096,
            memory_bytes: 268435456,
            requirements: None,
            adapter_configuration: None,
        };
        let mut checked = Checked {
            submission,
            candidate_sha256: nostr::contracts::digest_bytes(&bytes),
            expected_sha256,
            grant,
            observation: None,
        };
        self.document.paid.checker_started = true;
        self.document.paid.checker = Some(checked.clone());
        self.save()?;
        {
            let mut tasks = coder::task::Store::open(&setup.checker.task_directory)
                .map_err(|e| e.to_string())?;
            let command = coder::task::Command {
                schema: coder::task::COMMAND_SCHEMA.into(),
                command_id: format!("paid-check-{}", setup.checker.task_id),
                task_id: setup.checker.task_id.clone(),
                expected_revision: None,
                action: coder::task::Action::Submit {
                    intent: setup.checker.intent.clone(),
                },
            };
            tasks
                .apply(&serde_json::to_vec(&command).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        authority.check(&setup, &self.book, now, false)?;
        self.check_open()?;
        checked.observation = Some(
            coder::task::owner::execute(
                &setup.checker.task_directory,
                &serde_json::to_vec(&checked.grant).map_err(|e| e.to_string())?,
            )
            .await
            .map_err(|e| e.to_string())?,
        );
        self.document.paid.checker = Some(checked.clone());
        self.save()?;
        Ok(checked)
    }
    /// Queue one authenticated narrowing notice without waiting for the running
    /// labor journal's lock. The runner validates it again before cancelling.
    pub fn queue_paid_notice(
        directory: &Path,
        setup: crate::book::Setup,
        secret: secp256k1::SecretKey,
        event: nostr::domain::Event,
        attachments: Blobs,
        now: u64,
    ) -> Result<()> {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let metadata =
            std::fs::symlink_metadata(directory).map_err(|_| "labor directory unavailable")?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err("queued notice needs the existing private labor directory".into());
        }
        let document: crate::store::Document = serde_json::from_slice(&private_document(
            &directory.join("labor.json"),
            16 * 1024 * 1024,
        )?)
        .map_err(|_| "current labor journal unavailable")?;
        if serde_json::to_value(&document.setup).map_err(|e| e.to_string())?
            != serde_json::to_value(&setup).map_err(|e| e.to_string())?
        {
            return Err("queued notice changes the frozen labor setup".into());
        }
        let opened = nostr::private_artifact::open(&event, &secret).map_err(|e| e.to_string())?;
        if opened.artifact().schema.as_deref() != Some(NOTICE_SCHEMA) {
            return Err("only a signed paid support notice may use the narrowing queue".into());
        }
        let mut book = Book::new(setup, secret)?;
        for observation in document.observations {
            let actual = crate::store::outcome(book.receive(
                &observation.event,
                observation.received_at,
                &observation.attachments,
            ));
            if actual != observation.outcome {
                return Err("queued notice cannot reconcile its retained labor history".into());
            }
        }
        book.receive(&event, now, &attachments)?;
        let bytes =
            serde_json::to_vec(&QueuedNotice { event, attachments }).map_err(|e| e.to_string())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("queued support notice exceeds bound".into());
        }
        let path = directory.join("paid-notice.json");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .map_err(|_| "a paid notice is already pending or unavailable")?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::File::open(directory)
            .and_then(|f| f.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub async fn dispatch_paid(
        &mut self,
        authority: &mut dyn Authority,
        event: nostr::domain::Event,
        grant: &[u8],
        tasks: &Path,
        now: u64,
    ) -> Result<crate::store::Dispatch> {
        let setup = self
            .book
            .paid()
            .ok_or("not a paid fulfillment order")?
            .clone();
        authority.check(&setup, &self.book, now, true)?;
        check_private_host_path(&self.book, &self.dir)?;
        check_private_host_path(&self.book, tasks)?;
        if self.book.paid_halted() {
            return Err("cancelled or zero-revision work requires manual reconciliation".into());
        }
        self.dispatch_admitted(event, grant, tasks, now, Some(authority))
            .await
    }
    fn earned(&self) -> Result<worker::Earned> {
        let setup = self.book.paid().ok_or("not a paid fulfillment order")?;
        let order = self
            .book
            .order()
            .ok_or("unconfirmed or conflicted paid order")?;
        if self.book.records.conflict || !self.book.records.disputes.is_empty() {
            return Err(
                "conflicted, disputed or cancelled labor cannot authorize settlement".into(),
            );
        }
        let dispatch = self
            .document
            .dispatch
            .as_ref()
            .ok_or("provider execution is unavailable")?;
        let actual = coder::task::owner::recover(&dispatch.task_directory, &dispatch.task_id)
            .map_err(|_| "provider execution evidence is unavailable")?;
        if self.document.paid.interruption.is_some()
            || actual.execution != coder::task::Execution::Finished
            || actual
                .run
                .as_ref()
                .and_then(|r| r.result.as_ref())
                .is_none_or(|r| r.exit_code != Some(0) || r.output_incomplete)
            || dispatch.observation.as_ref().is_none_or(|retained| {
                retained.run != actual.run || retained.intent_digest != actual.intent_digest
            })
            || matches!(
                actual.checks,
                coder::task::Checks::Disputed | coder::task::Checks::Failed
            )
        {
            return Err("paid execution is interrupted, failed, incomplete or changed".into());
        }
        let acceptance = self
            .book
            .records
            .acceptance
            .as_ref()
            .ok_or("buyer acceptance is unavailable")?;
        let body = self.book.records.resolve(acceptance)?;
        if body["amount_due_msat"] != self.book.market().price_msat || body["outcome"] != "accepted"
        {
            return Err("labor has no exact fully accepted payable amount".into());
        }
        self.check_actual_delivery(&actual, dispatch, setup)?;
        Ok(worker::Earned {
            obligation: setup.worker.obligation(),
            order: order.order_id.clone(),
            labor_terms: setup.worker.labor_terms.clone(),
            delivery: hex_digest(
                self.book
                    .records
                    .delivery
                    .as_ref()
                    .and_then(|r| r["digest"].as_str())
                    .ok_or("delivery is unavailable")?,
            )?,
            verification: hex_digest(
                self.book
                    .records
                    .verification
                    .as_ref()
                    .and_then(|r| r["digest"].as_str())
                    .ok_or("verification is unavailable")?,
            )?,
            acceptance: hex_digest(acceptance["digest"].as_str().ok_or("acceptance identity")?)?,
            buyer: order.buyer.clone(),
            provider: order.provider.clone(),
            accepted_msat: setup.worker.price_msat,
        })
    }
    fn check_actual_delivery(
        &self,
        actual: &coder::task::Task,
        dispatch: &crate::store::Dispatch,
        setup: &Setup,
    ) -> Result<()> {
        let submission_ref = self
            .book
            .records
            .submission
            .as_ref()
            .ok_or("accepted submission unavailable")?;
        let submission = self.book.records.resolve(submission_ref)?;
        let deliverables = submission["deliverables"]
            .as_array()
            .ok_or("paid deliverables unavailable")?;
        if deliverables.len() != 1 || deliverables[0]["id"] != "patch" {
            return Err("selected paid profile requires one exact patch".into());
        }
        let output = &deliverables[0]["content"];
        let patch = self.book.blobs.get(output)?;
        let run = actual.run.as_ref().ok_or("provider run unavailable")?;
        let result = run.result.as_ref().ok_or("provider result unavailable")?;
        if patch["v"] != "openagents.free-labor.patch.v1"
            || patch["base"]
                != actual
                    .intent
                    .workspace
                    .source_revision
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null)
            || patch["source_snapshot"] != run.admission.source_snapshot
            || patch["candidate_snapshot"]
                != result
                    .candidate_snapshot
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null)
        {
            return Err("paid patch changes the authoritative source or candidate snapshot".into());
        }
        let files = patch["files"]
            .as_array()
            .ok_or("paid patch files unavailable")?;
        if files.len() != 1
            || files[0]["path"] != setup.checker.provider_file.to_string_lossy().as_ref()
        {
            return Err("paid patch exceeds its selected output scope".into());
        }
        let bytes = coder::task::artifact::read(
            &dispatch.task_directory,
            &dispatch.task_id,
            &setup.checker.provider_file,
        )
        .map_err(|_| "authoritative provider artifact unavailable")?;
        if files[0]["utf8"].as_str().map(str::as_bytes) != Some(bytes.as_slice()) {
            return Err("paid patch differs from the retained provider bytes".into());
        }
        let candidate = nostr::contracts::digest_bytes(&bytes);
        let input = self
            .book
            .blobs
            .resolve(&self.book.labor().execution.input)?;
        if input["expected_output_digest"] != candidate {
            return Err("paid candidate fails the frozen byte criterion".into());
        }
        let actual_dispatch = serde_json::to_value(dispatch).map_err(|e| e.to_string())?;
        let mut bound = false;
        for r in submission["run_evidence"]
            .as_array()
            .ok_or("paid RUN evidence unavailable")?
        {
            let value = self.book.blobs.get(r)?;
            if value["record"]["type"] == "resolved" && value["record"]["data"]["output"] == *output
            {
                for receipt in value["record"]["data"]["receipts"]
                    .as_array()
                    .ok_or("paid RUN receipt unavailable")?
                {
                    bound |= self.book.blobs.get(receipt)? == &actual_dispatch;
                }
            }
        }
        if !bound {
            return Err("paid RUN lacks its exact authoritative dispatch receipt".into());
        }
        let document: crate::store::Document = serde_json::from_slice(&private_document(
            &setup.checker.labor_directory.join("labor.json"),
            16 * 1024 * 1024,
        )?)
        .map_err(|_| "protected checker custody unavailable")?;
        if document
            .setup
            .paid
            .as_ref()
            .ok_or("protected checker setup unavailable")?
            .digest()?
            != setup.digest()?
        {
            return Err("protected checker has another accepted setup".into());
        }
        let checked = document
            .paid
            .checker
            .ok_or("protected checker attempt unavailable")?;
        let check =
            coder::task::owner::recover(&setup.checker.task_directory, &setup.checker.task_id)
                .map_err(|_| "protected checker execution unavailable")?;
        let check_run = check
            .run
            .as_ref()
            .ok_or("protected checker run unavailable")?;
        if checked.submission != *submission_ref
            || checked.candidate_sha256 != candidate
            || checked.expected_sha256 != candidate
            || checked.observation.as_ref() != Some(&check)
            || check.intent != setup.checker.intent
            || check_run.admission.grant != checked.grant
            || check_run.admission.program_digest
                != format!("sha256:{}", hex_digest(&setup.checker.program_sha256)?)
            || checked.grant.program != setup.checker.program
            || checked.grant.write_workspace
            || checked.grant.arguments
                != vec![
                    setup.checker.expected_file.to_string_lossy().into_owned(),
                    setup.checker.candidate_file.to_string_lossy().into_owned(),
                ]
            || check.execution != coder::task::Execution::Finished
            || check_run.result.as_ref().is_none_or(|r| {
                r.exit_code != Some(0) || r.output_incomplete || r.stop_requested || !r.group_clear
            })
        {
            return Err(
                "paid checker result changes its exact protected input, grant or outcome".into(),
            );
        }
        let verification = self.book.records.resolve(
            self.book
                .records
                .verification
                .as_ref()
                .ok_or("paid verification unavailable")?,
        )?;
        let receipt = self.book.blobs.get(
            verification["checker_receipts"]
                .as_array()
                .and_then(|r| r.first())
                .ok_or("paid checker receipt unavailable")?,
        )?;
        let check_value = serde_json::to_value(&check).map_err(|e| e.to_string())?;
        if !receipt["evidence"]
            .as_array()
            .ok_or("paid checker observation unavailable")?
            .iter()
            .any(|r| self.book.blobs.get(r).is_ok_and(|v| v == &check_value))
        {
            return Err(
                "signed LAB verification lacks its actual protected checker observation".into(),
            );
        }
        Ok(())
    }
    /// Prepare one exact central receive invoice after acceptance. Interrupted
    /// preparation stays unknown; this method never silently issues another.
    pub fn prepare_worker_invoice(
        &mut self,
        authority: &mut dyn Authority,
        wallet: &dyn LightningWallet,
        now: u64,
    ) -> Result<openagents_wallet::IssuedInvoice> {
        self.prepare_worker_invoice_with_clock(authority, wallet, now, || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|time| time.as_secs())
                .map_err(|_| "current invoice observation time is unavailable".into())
        })
    }
    pub(crate) fn prepare_worker_invoice_with_clock(
        &mut self,
        authority: &mut dyn Authority,
        wallet: &dyn LightningWallet,
        now: u64,
        observe: impl FnOnce() -> Result<u64>,
    ) -> Result<openagents_wallet::IssuedInvoice> {
        self.check_open()?;
        let setup = self
            .book
            .paid()
            .ok_or("not a paid fulfillment order")?
            .clone();
        authority.check(&setup, &self.book, now, false)?;
        if self.book.local_party() != setup.worker.provider {
            return Err("only the admitted provider journal prepares the central invoice".into());
        }
        let earned = self.earned()?;
        if wallet.node_id() != setup.central_node {
            return Err("central receive wallet differs from the admitted node".into());
        }
        if let Some(invoice) = &self.document.paid.invoice {
            check_invoice(&self.book, &setup, invoice, &earned, now)?;
            return Ok(invoice.clone());
        }
        if self.document.paid.invoice_preparation_started
            || now >= self.book.market().payment_due_at
        {
            return Err(
                "invoice preparation or payment deadline requires manual reconciliation".into(),
            );
        }
        // Reserve the resident's actual reply timeout and one second of
        // timestamp rounding within the original absolute payment deadline.
        let expiry: u32 = self
            .book
            .market()
            .payment_due_at
            .checked_sub(now)
            .and_then(|remaining| {
                remaining.checked_sub(openagents_wallet::resident::REPLY_WAIT.as_secs() + 1)
            })
            .filter(|remaining| *remaining > 0)
            .and_then(|remaining| remaining.try_into().ok())
            .ok_or("payment deadline lacks the bounded resident receive reply window")?;
        self.document.paid.invoice_preparation_started = true;
        self.document.paid.funding_state = Some("unknown".into());
        self.save()?;
        let request = receive_hash(&setup, &earned)?;
        let invoice = wallet
            .receive_exact_from_node(
                &setup.central_node,
                setup.worker.price_msat as u64,
                request.into(),
                expiry,
            )
            .map_err(|e| e.to_string())?;
        let observed_at = observe()?;
        if observed_at < now || observed_at >= self.book.market().payment_due_at {
            return Err("invoice observation time requires manual reconciliation".into());
        }
        authority.check(&setup, &self.book, observed_at, false)?;
        self.check_open()?;
        check_invoice(&self.book, &setup, &invoice, &earned, observed_at)?;
        self.document.paid.invoice = Some(invoice.clone());
        self.save()?;
        Ok(invoice)
    }
    /// Observe authenticated inbound funding and accrue the exact obligation
    /// through the existing central ledger. It dispatches no payment or payout.
    pub fn reconcile_worker_funding(
        &mut self,
        authority: &mut dyn Authority,
        wallet: &dyn LightningWallet,
        ledger: &mut pay_ledger::Ledger,
        now: u64,
    ) -> Result<State> {
        self.check_open()?;
        let setup = self
            .book
            .paid()
            .ok_or("not a paid fulfillment order")?
            .clone();
        authority.check(&setup, &self.book, now, false)?;
        let earned = self.earned()?;
        earned.validate(&setup.worker).map_err(|e| e.to_string())?;
        check_destination(ledger, &setup, now)?;
        let invoice = self
            .document
            .paid
            .invoice
            .clone()
            .ok_or("exact receive invoice is unavailable")?;
        check_invoice(&self.book, &setup, &invoice, &earned, now)?;
        if wallet.node_id() != setup.central_node {
            return Err("central funding lookup has another wallet identity".into());
        }
        let hash =
            openagents_wallet::parse_hash32(&invoice.payment_hash).map_err(|e| e.to_string())?;
        let began = std::time::Instant::now();
        let outcome = wallet.lookup_from_node(&setup.central_node, hash).map_err(
            |_| "authenticated funding lookup is unavailable; liability remains unknown",
        )?;
        let mut state = self.document.paid.clone();
        state.funding_state = Some("unknown".into());
        if let Some(payment) = outcome {
            if payment.payment_hash != invoice.payment_hash
                || payment.direction != PaymentDirection::Inbound
                || payment.updated_at > now
            {
                return Err("funding lookup changes invoice identity, direction or time".into());
            }
            if payment.status == PaymentStatus::Failed {
                state.funding_state = Some("failed".into());
            }
            if payment.status == PaymentStatus::Succeeded {
                if self.document.paid.funding_state.as_deref() == Some("failed") {
                    return Err(
                        "known failed funding cannot be rewritten by conflicting lookup evidence"
                            .into(),
                    );
                }
                if payment.amount_msat != Some(self.book.market().price_msat)
                    || payment
                        .bolt11
                        .as_deref()
                        .is_some_and(|value| value != invoice.bolt11)
                    || payment.updated_at > self.book.market().payment_due_at
                    || payment.updated_at
                        < nostr::x402::decode_invoice(&invoice.bolt11)
                            .map_err(|_| "retained funding invoice invalid")?
                            .created_at()
                {
                    return Err(
                        "funding lookup changes accepted amount, invoice or deadline".into(),
                    );
                }
                if let Some(preimage) = &payment.preimage {
                    let bytes =
                        openagents_wallet::parse_hash32(preimage).map_err(|e| e.to_string())?;
                    if <[u8; 32]>::from(Sha256::digest(bytes)) != hash {
                        return Err("funding preimage differs from invoice hash".into());
                    }
                }
                let receipt = worker::FundingReceipt {
                    payment_hash: invoice.payment_hash.clone(),
                    received_msat: setup.worker.price_msat,
                    platform_fee_msat: 0,
                    received_at: payment.updated_at as i64,
                };
                let current = now.saturating_add(began.elapsed().as_secs());
                authority.check(&setup, &self.book, current, false)?;
                self.check_open()?;
                check_destination(ledger, &setup, current)?;
                if self
                    .document
                    .paid
                    .funding_at
                    .is_some_and(|at| at != payment.updated_at)
                {
                    return Err("confirmed funding time cannot change".into());
                }
                let mut observed = payment.clone();
                observed.preimage = None;
                if let Some(prior) = &self.document.paid.funding_observation {
                    // LDK's Bolt11 receive records omit the invoice. Any
                    // present invoice already matched the retained signed
                    // invoice above; optional rehydration changes no payment
                    // identity and must not rewrite the first observation.
                    let mut comparable = observed.clone();
                    comparable.bolt11 = prior.bolt11.clone();
                    if prior != &comparable {
                        return Err("confirmed funding observation cannot change".into());
                    }
                    observed = prior.clone();
                }
                ledger
                    .record_worker_earned(&setup.worker, &earned, &receipt)
                    .map_err(|e| e.to_string())?;
                state.funding_state = Some("funded".into());
                state.funding_at = Some(payment.updated_at);
                state.settlement_key = Some(invoice.payment_hash.clone());
                state.funding_observation = Some(observed);
            }
        }
        if self.document.paid.funding_state.as_deref() == Some("failed")
            && state.funding_state.as_deref() != Some("failed")
        {
            return Err(
                "known failed funding cannot be rewritten by conflicting lookup evidence".into(),
            );
        }
        if self.document.paid.funding_state.as_deref() == Some("funded")
            && state.funding_state.as_deref() != Some("funded")
        {
            return Err(
                "confirmed funding cannot be rewritten by weaker or conflicting lookup evidence"
                    .into(),
            );
        }
        self.document.paid = state;
        self.save()?;
        Ok(self.document.paid.clone())
    }
    pub fn paid_state(&self) -> &State {
        &self.document.paid
    }
    pub fn paid_report(&self) -> Result<Value> {
        let setup = self.book.paid().ok_or("not a paid fulfillment order")?;
        let costs = self
            .book
            .paid_notices
            .iter()
            .rev()
            .find_map(|n| {
                n["notice"]["costs"]
                    .as_object()
                    .map(|_| n["notice"]["costs"].clone())
            })
            .unwrap_or(json!(Costs::default()));
        let categories = [
            "coordination_msat",
            "execution_msat",
            "checker_msat",
            "failed_attempts_msat",
            "payment_fees_msat",
        ];
        let known = categories
            .iter()
            .try_fold(setup.worker.price_msat as u64, |sum, key| {
                costs[*key].as_u64().and_then(|n| sum.checked_add(n))
            });
        Ok(
            json!({"schema":"coder.paid-labor.report.v1","market":setup.worker.market,"order":setup.worker.order,"partner_assignment":setup.partner.assignment,"accepted_price_msat":setup.worker.price_msat,"routing_fee_ceiling_msat":setup.worker.fee_limit_msat,"platform_fee_msat":0,"escrow":false,"worker_credit_risk":true,"executable_revisions":0,"executable_reworks":0,"signed_buyer_acceptance":self.book.records.acceptance.is_some(),"authoritative_earned_validation":self.earned().err(),"funding":self.document.paid,"support":self.book.paid_notices,"observed_costs":costs,"all_in_known_msat":known,"commercial_qualification":"owner_required","limitations":["Signed records establish attribution, and synthetic fixtures do not qualify an independently operated commercial service.","Unresolved execution, checker, funding, failed attempts, or costs remain unknown. The platform adds no inferred FX, coordination fee, or escrow.","Later cancellation stops new work and does not erase an already accepted obligation. Zero executable revisions require a separately accepted new order for additional work."]}),
        )
    }
}
fn check_destination(ledger: &pay_ledger::Ledger, setup: &Setup, now: u64) -> Result<()> {
    let payee = ledger
        .payee(&setup.worker.provider)
        .map_err(|e| e.to_string())?
        .ok_or("provider payout destination is not registered")?;
    if payee.destination_kind != setup.destination_kind
        || payee.destination_value != setup.destination_value
        || u64::try_from(payee.verified_at)
            .ok()
            .is_none_or(|at| at > now)
    {
        return Err("current provider payout destination differs from admitted terms".into());
    }
    Ok(())
}
fn receive_hash(setup: &Setup, earned: &worker::Earned) -> Result<[u8; 32]> {
    Ok(Sha256::digest(jcs(&json!({"v":"coder.paid-labor.receive.v1","setup":setup.digest()?,"earned":earned,"central_node":setup.central_node})).map_err(|e| e.to_string())?).into())
}
fn read_checker_file(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "protected expected file unavailable")?;
    let metadata = file
        .metadata()
        .map_err(|_| "protected expected file unavailable")?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() > 4096 {
        return Err("protected expected file must be bounded and ordinary".into());
    }
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| "protected expected bytes unavailable")?;
    if bytes.len() > 4096 {
        return Err("protected expected file exceeds bound".into());
    }
    Ok(bytes)
}
fn check_invoice(
    book: &Book,
    setup: &Setup,
    invoice: &openagents_wallet::IssuedInvoice,
    earned: &worker::Earned,
    now: u64,
) -> Result<()> {
    let decoded = nostr::x402::decode_invoice(&invoice.bolt11)
        .map_err(|_| "exact signed central invoice is invalid")?;
    let currency = match book.market().network.as_deref() {
        Some("bitcoin") => "bc",
        Some("testnet") => "tb",
        Some("regtest") => "bcrt",
        _ => return Err("unsupported paid worker network".into()),
    };
    let node =
        secp256k1::PublicKey::from_str(&setup.central_node).map_err(|_| "invalid central node")?;
    if decoded.created_at() > now
        || decoded.currency() != currency
        || decoded.amount_msat() != book.market().price_msat
        || decoded.payee() != node.serialize()
        || invoice.pay_to != setup.central_node
        || invoice.amount_msat != decoded.amount_msat()
        || openagents_wallet::parse_hash32(&invoice.payment_hash).map_err(|e| e.to_string())?
            != decoded.payment_hash()
        || decoded
            .created_at()
            .checked_add(decoded.expiry_seconds())
            .is_none_or(|at| at > book.market().payment_due_at)
        || decoded.description_hash() != receive_hash(setup, earned)?
        || invoice.description_hash
            != decoded
                .description_hash()
                .iter()
                .map(|n| format!("{n:02x}"))
                .collect::<String>()
        || invoice.expiry_secs as u64 != decoded.expiry_seconds()
    {
        return Err(
            "receive invoice differs from exact amount, node, network, hash or deadline".into(),
        );
    }
    Ok(())
}

/// Read one explicit private host document without following links or FIFOs.
pub fn private_document(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "private paid fulfillment input is unavailable")?;
    let meta = file
        .metadata()
        .map_err(|_| "paid fulfillment metadata is unavailable")?;
    if !meta.is_file()
        || meta.nlink() != 1
        || meta.permissions().mode() & 0o077 != 0
        || meta.len() > maximum as u64
    {
        return Err("paid fulfillment input must be a bounded private regular file".into());
    }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "paid fulfillment input read failed")?;
    if bytes.len() > maximum {
        return Err("paid fulfillment input exceeds bound".into());
    }
    Ok(bytes)
}

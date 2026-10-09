//! Same-origin browser delegation to the retail service.
//!
//! The retail service rejects browser `Origin` requests and authenticates a
//! native principal with a bearer. This adapter keeps that guard: the
//! operator provisions one native retail client configuration per account,
//! workspace, and membership epoch, and the site calls the service from the
//! server with that principal. The bearer never reaches the page.
//!
//! Effects (funding, quotes, confirmations, and cancellation) carry a
//! browser request identity that is journaled before the native call. An
//! exact retry recovers the original operation; different bytes conflict.
//! The native client's own retained review and confirmation intent make a
//! lost reply or a site restart return the same funded execution. Balances,
//! holds, and executions stay with the retail service.
//!
//! The customer's own OpenAI key is held in the shared [`super::custody`]
//! vault with explicit consent and is released only to the exact reviewed
//! quote or confirmation.

use super::custody::{self, CustodyError, Key, Material, Scope, Vault};
use super::private::ProtectedFile;
use super::session::{SessionError, Viewer, now};
use super::ui::{self, Tone};
use super::{failure, protect, refused, service, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use compute_workbench::retail::{self as native, Client, ProviderKey};
use maud::{Markup, Render, html};
use openagents_ui::forms::{Checkbox, Field, FieldAria, Input, InputType, Textarea};
use route_contract::Digest as RouteDigest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

mod purchases;
pub(crate) use purchases::href as purchase_href;

pub const SCHEMA: &str = "openagents.cloud.retail-delegations.v1";
const JOURNAL_SCHEMA: &str = "openagents.cloud.retail-web-requests.v1";
const UNAVAILABLE: &str = "The retail delegation configuration is unavailable or changed.";
const RECORDS_MAX: usize = 256;
/// A key in custody for retail lasts at most one day unless stored again.
const CUSTODY_SECONDS: u64 = 24 * 60 * 60;
const TERMS: &str = "OpenAgents keeps your own OpenAI API key in private server custody for this account, workspace, and retail delegation. It is used only to review a quote and, after you confirm that exact quote with separate consent, is handed to the authenticated retail service for that one funded task. Model usage bills to your OpenAI account. It is never shown, exported, logged, or shared, and you can remove it at any time; removal here does not recall a key already handed to a confirmed task, which the retail service removes after cleanup.";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    /// Private directory owned by this site for custody and request journals.
    directory: PathBuf,
    delegations: Vec<Declared>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declared {
    id: String,
    account: String,
    workspace: String,
    members_epoch: u64,
    /// A native `openagents.compute-retail-client.v1` configuration.
    client: PathBuf,
    /// Narrow the native principal to observation for this browser scope.
    #[serde(default)]
    read_only: bool,
}

/// One explicitly provisioned account/workspace/epoch delegation.
pub(crate) struct Delegation {
    id: String,
    account: String,
    workspace: String,
    members_epoch: u64,
    read_only: bool,
    client: native::Config,
    identity: String,
    files: [ProtectedFile; 2],
    lock: Arc<tokio::sync::Mutex<()>>,
}

/// Operator-provisioned retail delegations; sign-in never creates one.
pub struct Delegations {
    config: ProtectedFile,
    vault: Arc<Vault>,
    journals: PathBuf,
    delegations: Vec<Arc<Delegation>>,
}

impl Delegations {
    pub fn load(path: &FsPath) -> Result<Self, String> {
        let (config, bytes) = ProtectedFile::open(path, 64 * 1024)?;
        let declared: Configuration = serde_json::from_slice(&bytes).map_err(|_| UNAVAILABLE)?;
        if declared.schema != SCHEMA || declared.delegations.len() > 64 {
            return Err(UNAVAILABLE.into());
        }
        custody::checked_root(&declared.directory).map_err(|_| UNAVAILABLE)?;
        let vault_root = declared.directory.join("custody");
        let journals = declared.directory.join("requests");
        for directory in [&vault_root, &journals] {
            if std::fs::symlink_metadata(directory).is_err() {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .mode(0o700)
                    .create(directory)
                    .map_err(|_| UNAVAILABLE)?;
            }
            custody::checked_root(directory).map_err(|_| UNAVAILABLE)?;
        }
        let vault = Arc::new(Vault::open(&vault_root)?);
        let mut delegations: Vec<Arc<Delegation>> = Vec::new();
        for declared in declared.delegations {
            if delegations.iter().any(|d| d.id == declared.id) {
                return Err(UNAVAILABLE.into());
            }
            delegations.push(Arc::new(Delegation::load(declared)?));
        }
        Ok(Self {
            config,
            vault,
            journals,
            delegations,
        })
    }

    /// Resolve only the current native account, workspace, and membership epoch.
    pub(crate) fn get(&self, viewer: &Viewer, id: &str) -> Result<&Arc<Delegation>, SessionError> {
        self.config.check().map_err(|_| SessionError::Unavailable)?;
        let delegation = self
            .delegations
            .iter()
            .find(|d| d.id == id)
            .ok_or(SessionError::Forbidden)?;
        delegation.admit(viewer)?;
        Ok(delegation)
    }

    pub(crate) fn current<'a>(&'a self, viewer: &Viewer) -> Vec<&'a Arc<Delegation>> {
        if self.config.check().is_err() {
            return Vec::new();
        }
        self.delegations
            .iter()
            .filter(|d| d.admit(viewer).is_ok())
            .collect()
    }
}

impl Delegation {
    fn load(declared: Declared) -> Result<Self, String> {
        if !valid_id(&declared.id)
            || !valid_id(&declared.account)
            || !valid_id(&declared.workspace)
            || declared.members_epoch > 9_007_199_254_740_991
        {
            return Err(UNAVAILABLE.into());
        }
        let (client_file, bytes) = ProtectedFile::open(&declared.client, 32 * 1024)?;
        let client: native::Config = serde_json::from_slice(&bytes).map_err(|_| UNAVAILABLE)?;
        if client.schema != native::SCHEMA {
            return Err(UNAVAILABLE.into());
        }
        let (bearer_file, mut bearer) = ProtectedFile::open(&client.bearer_file, 4096)?;
        let bearer_digest = hex(&Sha256::digest(&bearer));
        bearer.fill(0);
        let read_only = declared.read_only || client.read_only;
        let identity = digest(&json!({
            "schema":SCHEMA,"delegation":declared.id,"account":declared.account,
            "workspace":declared.workspace,"members_epoch":declared.members_epoch,
            "endpoint":client.endpoint,"principal":client.principal,
            "state":client.state,"read_only":read_only,"bearer":bearer_digest,
        }));
        Ok(Self {
            id: declared.id,
            account: declared.account,
            workspace: declared.workspace,
            members_epoch: declared.members_epoch,
            read_only,
            client,
            identity,
            files: [client_file, bearer_file],
            lock: Arc::new(tokio::sync::Mutex::new(())),
        })
    }

    fn admit(&self, viewer: &Viewer) -> Result<(), SessionError> {
        let Some(workspace) = &viewer.workspace else {
            return Err(SessionError::Forbidden);
        };
        if viewer.account_id != self.account
            || workspace.id != self.workspace
            || workspace.members_epoch != self.members_epoch
            || viewer.expires_at <= now()
        {
            return Err(SessionError::Forbidden);
        }
        for file in &self.files {
            file.check().map_err(|_| SessionError::Unavailable)?;
        }
        Ok(())
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn read_only(&self) -> bool {
        self.read_only
    }

    fn scope(&self) -> Scope {
        Scope {
            account: self.account.clone(),
            workspace: self.workspace.clone(),
            members_epoch: self.members_epoch,
            subject: format!("retail:{}", self.id),
            material: Material::OpenAiApiKey,
        }
    }
}

/// Why an adapter call did not produce a result. Nothing here carries a
/// credential or a raw service response.
#[derive(Debug)]
pub(crate) enum Failure {
    Session(SessionError),
    /// The browser scope is observation-only.
    ReadOnly,
    /// The outcome is unknown; retry the same request identity.
    Unknown,
    Refused(&'static str),
    Service(String),
    Custody(CustodyError),
    /// The native owner answered with a purchase part that differs from the
    /// one this site first retained.
    Changed(&'static str),
}

impl From<SessionError> for Failure {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl From<native::Error> for Failure {
    fn from(error: native::Error) -> Self {
        match error {
            native::Error::Refused(message) => Self::Refused(message),
            native::Error::Service(code) => Self::Service(code),
            native::Error::Private(_) => Self::Session(SessionError::Unavailable),
            native::Error::Transport | native::Error::Json(_) | native::Error::Io(_) => {
                Self::Unknown
            }
        }
    }
}

impl From<CustodyError> for Failure {
    fn from(error: CustodyError) -> Self {
        Self::Custody(error)
    }
}

pub(crate) fn answer(error: Failure) -> Response {
    match error {
        Failure::Session(error) => refused(error),
        Failure::ReadOnly => failure(
            StatusCode::FORBIDDEN,
            "Observation only",
            "This retail delegation is observation-only. It cannot fund, quote, confirm, or cancel.",
        ),
        Failure::Unknown => failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Outcome unknown",
            "The retail service did not answer. Retry the same request from the Retail page; it recovers the original operation and never repeats a purchase or dispatch.",
        ),
        Failure::Refused(message) => {
            failure(StatusCode::CONFLICT, "Retail request refused", message)
        }
        Failure::Service(code) => {
            let (status, message) = match code.as_str() {
                "access_denied" => (
                    StatusCode::FORBIDDEN,
                    "The retail service refused this principal's current rights.",
                ),
                "insufficient_balance" => (
                    StatusCode::PAYMENT_REQUIRED,
                    "The purchased balance cannot cover this quote's maximum.",
                ),
                "terms_conflict" => (
                    StatusCode::CONFLICT,
                    "The retail terms changed or conflict with an earlier request. Review a new offer.",
                ),
                "invalid_request" => (
                    StatusCode::BAD_REQUEST,
                    "The retail service refused this request.",
                ),
                _ => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "The retail service is busy or unavailable. Retry the same request.",
                ),
            };
            failure(status, "Retail request refused", message)
        }
        Failure::Custody(error) => {
            let status = match error {
                CustodyError::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
                CustodyError::Changed => StatusCode::CONFLICT,
                _ => StatusCode::BAD_REQUEST,
            };
            failure(status, "Key custody", &error.to_string())
        }
        Failure::Changed(part) => failure(
            StatusCode::CONFLICT,
            "Purchase record changed",
            &format!(
                "The retail service answered with a different {part} than the one retained for this purchase. Nothing is shown as current; the original record stays retained for reconciliation."
            ),
        ),
    }
}

/// One journaled browser request: its operation, exact parameter digest, and
/// the recorded outcome once the native owner answered.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    op: String,
    digest: String,
    params: Value,
    at: u64,
    outcome: Option<Value>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    schema: String,
    delegation: String,
    records: BTreeMap<String, Record>,
}

impl Journal {
    fn name(delegation: &Delegation) -> String {
        format!("{}.json", &delegation.identity["sha256:".len()..])
    }

    fn load(root: &FsPath, delegation: &Delegation) -> Result<Self, Failure> {
        custody::checked_root(root)?;
        let bytes = custody::read_private(&root.join(Self::name(delegation)), 4 * 1024 * 1024)?;
        let Some(bytes) = bytes else {
            return Ok(Self {
                schema: JOURNAL_SCHEMA.into(),
                delegation: delegation.identity.clone(),
                records: BTreeMap::new(),
            });
        };
        let journal: Self = serde_json::from_slice(&bytes)
            .map_err(|_| Failure::Session(SessionError::Unavailable))?;
        if journal.schema != JOURNAL_SCHEMA || journal.delegation != delegation.identity {
            return Err(Failure::Session(SessionError::Unavailable));
        }
        Ok(journal)
    }

    fn save(&self, root: &FsPath, delegation: &Delegation) -> Result<(), Failure> {
        custody::checked_root(root)?;
        let bytes =
            serde_json::to_vec(self).map_err(|_| Failure::Session(SessionError::Unavailable))?;
        custody::write_private(root, &Self::name(delegation), &bytes)?;
        Ok(())
    }
}

/// A retail effect's typed operation. Its parameters are what the journal binds.
#[derive(Clone)]
enum Effect {
    TopUp {
        amount_sats: u64,
    },
    Quote {
        task: retail_cloud::contract::TaskRequest,
    },
    Confirm {
        review: String,
    },
    Cancel {
        execution: String,
    },
}

impl Effect {
    fn op(&self) -> &'static str {
        match self {
            Self::TopUp { .. } => "top_up",
            Self::Quote { .. } => "quote",
            Self::Confirm { .. } => "confirm",
            Self::Cancel { .. } => "cancel",
        }
    }
    fn params(&self) -> Value {
        match self {
            Self::TopUp { amount_sats } => json!({"amount_sats":amount_sats}),
            Self::Quote { task } => json!({"task":task}),
            Self::Confirm { review } => json!({"review":review}),
            Self::Cancel { execution } => json!({"execution":execution}),
        }
    }
}

impl Delegations {
    /// Run `work` on a freshly opened native client in a blocking thread.
    /// One request per delegation runs at a time; the client's own exclusive
    /// state lock refuses a second process.
    async fn native<T: Send + 'static>(
        &self,
        viewer: &Viewer,
        id: &str,
        work: impl FnOnce(&Delegation, &mut Client, &FsPath, &Vault) -> Result<T, Failure>
        + Send
        + 'static,
    ) -> Result<T, Failure>
    where
        T: Send,
    {
        let delegation = self.get(viewer, id)?.clone();
        let _turn = delegation.lock.clone().lock_owned().await;
        let journals = self.journals.clone();
        let vault = self.vault.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut client = Client::open(delegation.client.clone())?;
            let result = work(&delegation, &mut client, &journals, &vault);
            for file in &delegation.files {
                file.check().map_err(|_| SessionError::Unavailable)?;
            }
            result
        })
        .await
        .map_err(|_| Failure::Unknown)?;
        // Membership or configuration changes after the call refuse its result.
        self.get(viewer, id)?;
        result
    }

    pub(crate) async fn account(
        &self,
        viewer: &Viewer,
        id: &str,
    ) -> Result<native::Account, Failure> {
        self.native(viewer, id, |_, client, _, _| Ok(client.account()?))
            .await
    }

    pub(crate) fn custody(
        &self,
        viewer: &Viewer,
        id: &str,
    ) -> Result<Option<custody::Status>, Failure> {
        let delegation = self.get(viewer, id)?;
        Ok(self.vault.status(&delegation.scope(), now())?)
    }

    pub(crate) fn store_key(
        &self,
        viewer: &Viewer,
        id: &str,
        key: Key,
        consent: bool,
    ) -> Result<custody::Status, Failure> {
        let delegation = self.get(viewer, id)?;
        if delegation.read_only {
            return Err(Failure::ReadOnly);
        }
        let at = now();
        Ok(self.vault.store(
            &delegation.scope(),
            key,
            consent,
            &terms_digest(),
            at,
            at + CUSTODY_SECONDS,
        )?)
    }

    /// Removal works for every admitted viewer, including observation-only.
    pub(crate) fn revoke_key(&self, viewer: &Viewer, id: &str) -> Result<bool, Failure> {
        let delegation = self.get(viewer, id)?;
        Ok(self.vault.revoke(&delegation.scope())?)
    }

    /// Recent journaled requests, newest first, without parameters that
    /// could carry private task text beyond what the reviewer saw.
    pub(crate) async fn requests(
        &self,
        viewer: &Viewer,
        id: &str,
    ) -> Result<Vec<(String, Value)>, Failure> {
        let delegation = self.get(viewer, id)?;
        let journal = Journal::load(&self.journals, delegation)?;
        let mut records: Vec<(String, Record)> = journal.records.into_iter().collect();
        records.sort_by(|a, b| b.1.at.cmp(&a.1.at).then(a.0.cmp(&b.0)));
        Ok(records
            .into_iter()
            .take(16)
            .map(|(request, record)| {
                (
                    request,
                    json!({"op":record.op,"params":record.params,"outcome":record.outcome}),
                )
            })
            .collect())
    }

    /// Journal the request before the native effect, then perform it once.
    async fn effect(
        &self,
        viewer: &Viewer,
        id: &str,
        request: String,
        effect: Effect,
    ) -> Result<Value, Failure> {
        if !request_id(&request) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        {
            let delegation = self.get(viewer, id)?;
            if delegation.read_only {
                return Err(Failure::ReadOnly);
            }
        }
        self.native(viewer, id, move |delegation, client, journals, vault| {
            let params = effect.params();
            let exact = digest(&json!({"op":effect.op(),"params":params}));
            let mut journal = Journal::load(journals, delegation)?;
            if let Effect::Confirm { review } = &effect
                && !journal.records.values().any(|record| {
                    record.op == "quote"
                        && record
                            .outcome
                            .as_ref()
                            .is_some_and(|o| o["digest"] == json!(review))
                })
            {
                return Err(Failure::Refused(
                    "Confirm only a quote reviewed on this page.",
                ));
            }
            match journal.records.get(&request) {
                Some(record) if record.digest != exact => {
                    return Err(Failure::Refused(
                        "This request identity was used for another operation. Reload and review it again.",
                    ));
                }
                Some(Record {
                    outcome: Some(value),
                    ..
                }) => return Ok(value.clone()),
                Some(_) => {}
                None => {
                    if journal.records.len() >= RECORDS_MAX {
                        let oldest = journal
                            .records
                            .iter()
                            .filter(|(_, record)| record.outcome.is_some())
                            .min_by_key(|(_, record)| record.at)
                            .map(|(key, _)| key.clone())
                            .ok_or(Failure::Refused(
                                "Too many retail requests have unknown outcomes. Reconcile them first.",
                            ))?;
                        journal.records.remove(&oldest);
                    }
                    journal.records.insert(
                        request.clone(),
                        Record {
                            op: effect.op().into(),
                            digest: exact,
                            params,
                            at: now(),
                            outcome: None,
                        },
                    );
                    journal.save(journals, delegation)?;
                }
            }
            let value = match perform(delegation, client, vault, &journal, &request, effect) {
                Ok(value) => value,
                Err(error) => {
                    // A definitive refusal performed nothing, so it leaves no
                    // unknown outcome behind; an unknown one stays journaled.
                    // A client-side refusal may follow a native answer, so it
                    // stays journaled too.
                    let definitive = match &error {
                        Failure::Custody(_) => true,
                        Failure::Service(code) => !matches!(code.as_str(), "busy" | "unavailable"),
                        _ => false,
                    };
                    if definitive
                        && journal
                            .records
                            .get(&request)
                            .is_some_and(|r| r.outcome.is_none())
                    {
                        journal.records.remove(&request);
                        journal.save(journals, delegation)?;
                    }
                    return Err(error);
                }
            };
            if let Some(record) = journal.records.get_mut(&request) {
                record.outcome = Some(value.clone());
            }
            journal.save(journals, delegation)?;
            Ok(value)
        })
        .await
    }
}

/// One journaled funding request and the native owner's current record of it.
pub(crate) struct Funding {
    pub request: String,
    pub amount_sats: Option<u64>,
    /// The first recorded native answer; `None` when the outcome is unknown.
    pub recorded: Option<native::Purchase>,
    /// The owner's current record, or `None` when it did not answer now.
    pub current: Option<native::Purchase>,
}

/// A retail statement built from the native account, this site's journaled
/// funding answers, and each purchase's retained first-observed record.
pub(crate) struct Statement {
    pub account: native::Account,
    pub funding: Vec<Funding>,
    pub purchases: Vec<(String, BTreeMap<String, Value>)>,
    pub more: bool,
}

impl Delegations {
    /// Read the retail statement for one delegation. Nothing here creates,
    /// retries, or settles a purchase; changed original records refuse.
    pub(crate) async fn statement(&self, viewer: &Viewer, id: &str) -> Result<Statement, Failure> {
        self.native(viewer, id, |delegation, client, journals, _| {
            let account = client.account()?;
            let journal = Journal::load(journals, delegation)?;
            let mut records: Vec<(&String, &Record)> = journal
                .records
                .iter()
                .filter(|(_, r)| r.op == "top_up")
                .collect();
            records.sort_by(|a, b| b.1.at.cmp(&a.1.at).then(a.0.cmp(b.0)));
            let mut funding = Vec::new();
            for (request, record) in records.into_iter().take(32) {
                let recorded: Option<native::Purchase> = match &record.outcome {
                    Some(value) => Some(
                        serde_json::from_value(value.clone())
                            .map_err(|_| Failure::Changed("invoice"))?,
                    ),
                    None => None,
                };
                let current = match &recorded {
                    Some(original) => match client.top_up_status(&original.purchase) {
                        Ok(current) => {
                            if current.account != original.account
                                || current.amount_msat != original.amount_msat
                                || current.payment_hash != original.payment_hash
                            {
                                return Err(Failure::Changed("invoice"));
                            }
                            Some(current)
                        }
                        Err(native::Error::Refused(message)) => {
                            return Err(Failure::Refused(message));
                        }
                        Err(_) => None,
                    },
                    None => None,
                };
                funding.push(Funding {
                    request: request.clone(),
                    amount_sats: record.params["amount_sats"].as_u64(),
                    recorded,
                    current,
                });
            }
            let page = client.executions(None)?;
            let listed = page["executions"].as_array().cloned().unwrap_or_default();
            let mut purchases = Vec::new();
            for entry in &listed {
                let Some(execution) = entry["execution"].as_str().filter(|e| valid_id(e)) else {
                    continue;
                };
                purchases.push((
                    execution.to_owned(),
                    purchases::retained(delegation, journals, execution)?,
                ));
            }
            Ok(Statement {
                account,
                funding,
                purchases,
                more: page["next"].is_string() && listed.len() >= 32,
            })
        })
        .await
    }
}

fn perform(
    delegation: &Delegation,
    client: &mut Client,
    vault: &Vault,
    journal: &Journal,
    request: &str,
    effect: Effect,
) -> Result<Value, Failure> {
    let scope = delegation.scope();
    Ok(match effect {
        Effect::TopUp { amount_sats } => {
            // The browser request identity is the service idempotency key.
            serde_json::to_value(client.top_up(request, amount_sats)?)
                .map_err(|_| Failure::Unknown)?
        }
        Effect::Quote { task } => {
            let status = vault.status(&scope, now())?.ok_or(CustodyError::Absent)?;
            let key = provider_key(vault.release(&scope, &status.digest, now())?)?;
            let review = client.quote_key(request, task, &key)?;
            json!({"digest":review.digest,"key_digest":review.provider_key_digest,"execution":review.offer.admission.execution,"expires_at":review.offer.offer.expires_at,"maximum_sats":review.offer.quote.max_sats,"lines":review.lines()})
        }
        Effect::Confirm { review } => {
            let quoted = journal
                .records
                .values()
                .filter(|record| record.op == "quote")
                .filter_map(|record| record.outcome.as_ref())
                .find(|outcome| outcome["digest"] == json!(review))
                .ok_or(Failure::Refused(
                    "Confirm only a quote reviewed on this page.",
                ))?;
            let key_digest = quoted["key_digest"]
                .as_str()
                .ok_or(Failure::Unknown)?
                .to_owned();
            let digest: RouteDigest = serde_json::from_value(json!(review))
                .map_err(|_| Failure::Session(SessionError::InvalidRequest))?;
            let key = provider_key(vault.release(&scope, &key_digest, now())?)?;
            serde_json::to_value(client.confirm_key(&digest, &key, true)?)
                .map_err(|_| Failure::Unknown)?
        }
        Effect::Cancel { execution } => client.cancel(&execution)?,
    })
}

fn provider_key(key: Key) -> Result<ProviderKey, Failure> {
    ProviderKey::new(key.into_delivery()).map_err(|_| Failure::Custody(CustodyError::Unavailable))
}

#[cfg(test)]
pub(crate) fn tests_identity(delegation: &Delegation) -> String {
    delegation.identity.clone()
}

fn terms_digest() -> String {
    digest(&json!({"schema":custody::SCHEMA,"terms":TERMS}))
}

fn request_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value, "." | "..")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn digest(value: &Value) -> String {
    format!(
        "sha256:{}",
        hex(&Sha256::digest(value.to_string().as_bytes()))
    )
}

fn fresh_request() -> String {
    hex(&secp256k1::rand::random::<[u8; 16]>())
}

// ---- Routes ---------------------------------------------------------------

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/billing/retail", get(index))
        .route("/cloud/app/billing/retail/{id}/key", post(store_key))
        .route(
            "/cloud/app/billing/retail/{id}/key/remove",
            post(remove_key),
        )
        .route("/cloud/app/billing/retail/{id}/top-up", post(top_up))
        .route("/cloud/app/billing/retail/{id}/quote", post(quote))
        .route("/cloud/app/billing/retail/{id}/confirm", post(confirm))
        .route("/cloud/app/billing/retail/{id}/cancel", post(cancel))
        .merge(purchases::routes())
}

pub(crate) fn available(app: &App, viewer: &Viewer) -> bool {
    app.config
        .cloud_retail
        .as_ref()
        .is_some_and(|retail| !retail.current(viewer).is_empty())
}

const PAGE: &str = "/cloud/app/billing/retail";

struct Context<'a> {
    app: &'a App,
    service: &'a super::session::CloudSession,
    viewer: Viewer,
    retail: &'a Delegations,
}

async fn context<'a>(app: &'a App, headers: &HeaderMap) -> Result<Context<'a>, Response> {
    let service = service(app)?;
    let viewer = match service.authenticate(headers).await {
        Ok(value) => value,
        Err(SessionError::Unauthenticated) => {
            return Err(protect(Redirect::to("/cloud/sign-in").into_response()));
        }
        Err(error) => return Err(refused(error)),
    };
    let retail = app
        .config
        .cloud_retail
        .as_deref()
        .ok_or_else(|| refused(SessionError::Forbidden))?;
    Ok(Context {
        app,
        service,
        viewer,
        retail,
    })
}

impl Context<'_> {
    fn csrf(&self, headers: &HeaderMap, scope: &str, target: &str) -> Result<String, Response> {
        self.service
            .csrf(headers, &self.viewer, scope, target)
            .map_err(refused)
    }

    fn verify(
        &self,
        headers: &HeaderMap,
        token: &str,
        scope: &str,
        target: &str,
    ) -> Result<(), Response> {
        self.service
            .verify_csrf(headers, Some(&self.viewer), scope, target, token)
            .map_err(refused)
    }
}

fn target(delegation: &Delegation, request: &str) -> String {
    format!("{}:{}", delegation.identity, request)
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegations = context.retail.current(&context.viewer);
    let mut sections = Vec::with_capacity(delegations.len());
    for delegation in &delegations {
        match section(&context, &headers, delegation).await {
            Ok(value) => sections.push(value),
            Err(response) => return response,
        }
    }
    let content = html! {
        h2 { "Retail delegation" }
        p { "Retail purchases go through an operator-provisioned delegation for this account, workspace, and membership. The server calls the retail service with that native principal; no service credential reaches this page. Signing in grants no funding, quote, confirmation, or cancellation right." }
        @if delegations.is_empty() {
            (ui::unavailable(
                "Unavailable",
                "Unavailable: no retail delegation is provisioned for this account, workspace, and membership.",
            ))
        }
        @for section in &sections { (section) }
    };
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(content),
        None,
    )
}

/// A required, labelled form control: `control` receives the field's ARIA
/// wiring; `description` is the note under it.
fn field<C: Render>(
    id: &str,
    label: &str,
    description: Option<&str>,
    control: impl FnOnce(FieldAria) -> C,
) -> Markup {
    let mut field = Field::new(id, label).required(true);
    if let Some(description) = description {
        field = field.description(description);
    }
    let aria = field.aria();
    field.control(control(aria)).render()
}

async fn section(
    context: &Context<'_>,
    headers: &HeaderMap,
    delegation: &Delegation,
) -> Result<Markup, Response> {
    let id = delegation.id();
    let base = format!("{PAGE}/{id}");
    let account = context
        .retail
        .account(&context.viewer, delegation.id())
        .await;
    let rights = match &account {
        Ok(account) => Some(account.capabilities.clone()),
        Err(Failure::Session(error)) => return Err(refused(*error)),
        Err(_) => None,
    };
    let can_spend =
        !delegation.read_only() && rights.as_ref().is_some_and(|r| r.observe && r.spend);
    let can_dispatch = can_spend && rights.as_ref().is_some_and(|r| r.execute && r.disclose);
    let can_cancel =
        !delegation.read_only() && rights.as_ref().is_some_and(|r| r.observe && r.execute);

    // Key custody: masked status only.
    let custody = context
        .retail
        .custody(&context.viewer, delegation.id())
        .map_err(answer)?;
    let remove = match &custody {
        Some(_) => {
            let request = fresh_request();
            let csrf = context.csrf(headers, "retail-key-remove", &target(delegation, &request))?;
            Some(
                ui::BoundForm::new(format!("{base}/key/remove"))
                    .csrf(&csrf)
                    .bind("request", &request)
                    .submit_with(ui::submit("Remove key from custody", false)),
            )
        }
        None => None,
    };
    let place = if delegation.read_only() {
        None
    } else {
        let request = fresh_request();
        let csrf = context.csrf(headers, "retail-key", &target(delegation, &request))?;
        // Built by hand for `autocomplete="off"` on the form itself.
        Some(html! {
            form class="cloud-form" method="post" action=(format!("{base}/key")) autocomplete="off" {
                (ui::csrf(&csrf))
                (ui::hidden("request", &request))
                (field(&format!("retail-{id}-key"), "OpenAI API key", Some(TERMS), |aria| {
                    Input::new("key")
                        .input_type(InputType::Password)
                        .autocomplete("off")
                        .spellcheck(false)
                        .required(true)
                        .aria(aria)
                }))
                (Checkbox::new("consent", "I consent to this custody")
                    .id(format!("retail-{id}-consent"))
                    .value("custody")
                    .required(true))
                div class="cloud-form-actions" { (ui::submit("Place key in custody", true)) }
            }
        })
    };
    let fund = if can_spend {
        let request = fresh_request();
        let csrf = context.csrf(headers, "retail-top-up", &target(delegation, &request))?;
        Some(
            ui::BoundForm::new(format!("{base}/top-up"))
                .csrf(&csrf)
                .bind("request", &request)
                .body(field(
                    &format!("retail-{id}-amount"),
                    "Credits (sats)",
                    Some("Pay the invoice with your own wallet. Credits are non-withdrawable and grant no execution authority."),
                    |aria| {
                        Input::new("amount_sats")
                            .input_type(InputType::Number)
                            .min("1")
                            .max("1000000")
                            .required(true)
                            .aria(aria)
                    },
                ))
                .submit("Request an exact invoice"),
        )
    } else {
        None
    };
    let quote = if can_dispatch && custody.is_some() {
        let request = fresh_request();
        let csrf = context.csrf(headers, "retail-quote", &target(delegation, &request))?;
        Some(
            ui::BoundForm::new(format!("{base}/quote"))
                .csrf(&csrf)
                .bind("request", &request)
                .body(html! {
                    (field(&format!("retail-{id}-repository"), "Public GitHub repository", None, |aria| {
                        Input::new("repository").required(true).aria(aria)
                    }))
                    (field(&format!("retail-{id}-commit"), "Commit (40 hex)", None, |aria| {
                        Input::new("commit").required(true).aria(aria)
                    }))
                    (field(&format!("retail-{id}-task"), "Task", None, |aria| {
                        Textarea::new("task").required(true).aria(aria)
                    }))
                    (field(&format!("retail-{id}-checks"), "Checks, one per line", None, |aria| {
                        Textarea::new("checks").required(true).aria(aria)
                    }))
                    (field(&format!("retail-{id}-max-seconds"), "Wall limit (seconds)", None, |aria| {
                        Input::new("max_seconds")
                            .input_type(InputType::Number)
                            .min("1")
                            .max("3600")
                            .required(true)
                            .aria(aria)
                    }))
                })
                .submit("Review quote"),
        )
    } else {
        None
    };
    let cancel = if can_cancel {
        let request = fresh_request();
        let csrf = context.csrf(headers, "retail-cancel", &target(delegation, &request))?;
        Some(
            ui::BoundForm::new(format!("{base}/cancel"))
                .csrf(&csrf)
                .bind("request", &request)
                .body(field(
                    &format!("retail-{id}-execution"),
                    "Execution",
                    Some("A stop request is not a stopped meter; acknowledgment, deletion, and settlement are recorded separately."),
                    |aria| Input::new("execution").required(true).aria(aria),
                ))
                .submit_with(ui::submit("Request stop", false)),
        )
    } else {
        None
    };

    // Journaled requests with retry and confirmation controls.
    let requests = context
        .retail
        .requests(&context.viewer, delegation.id())
        .await
        .map_err(answer)?;
    let mut items = Vec::with_capacity(requests.len());
    for (request, record) in &requests {
        let op = record["op"].as_str().unwrap_or("");
        let unknown = record["outcome"].is_null();
        let retry = if unknown && !delegation.read_only() {
            retry_form(context, headers, delegation, request, record)?
        } else {
            None
        };
        let detail = if op == "quote" {
            let confirm = match (record["outcome"]["digest"].as_str(), can_dispatch) {
                (Some(review), true) => {
                    let confirm = fresh_request();
                    let csrf =
                        context.csrf(headers, "retail-confirm", &target(delegation, &confirm))?;
                    Some(
                        ui::BoundForm::new(format!("{base}/confirm"))
                            .csrf(&csrf)
                            .bind("request", &confirm)
                            .bind("review", review)
                            .body(
                                Checkbox::new(
                                    "custody",
                                    "Hand my key to the authenticated retail service for this exact task",
                                )
                                .id(format!("retail-{id}-confirm-{confirm}"))
                                .value("service")
                                .required(true),
                            )
                            .submit("Confirm and reserve the maximum"),
                    )
                }
                _ => None,
            };
            html! {
                @if let Some(lines) = record["outcome"]["lines"].as_str() {
                    pre { (lines) }
                }
                @if let Some(confirm) = confirm { (confirm) }
            }
        } else if let Some(value) = record["outcome"].as_object() {
            let execution = value
                .get("execution")
                .and_then(Value::as_str)
                .filter(|e| valid_id(e));
            html! {
                @if let Some(execution) = execution {
                    a href=(purchases::href(delegation.id(), execution)) { "Open purchase " (execution) }
                }
                pre { (serde_json::to_string_pretty(value).unwrap_or_default()) }
            }
        } else {
            html! {}
        };
        let short = &request[..8];
        items.push(html! {
            li {
                span {
                    (op) " \u{b7} request " (short) " \u{b7} "
                    @if unknown {
                        (ui::status("Outcome unknown", Tone::Warning))
                    } @else {
                        (ui::status("Recorded", Tone::Neutral))
                    }
                }
                @if let Some(retry) = retry {
                    (ui::outcome_unknown(
                        html! { "Request " (short) " has no recorded outcome. Retry it with its original identity and parameters; it never repeats a purchase or dispatch." },
                        Some(retry),
                    ))
                }
                (detail)
            }
        });
    }

    Ok(html! {
        section class="cloud-card" id=(format!("retail-{id}")) {
            h3 { "Delegation " (id) }
            p {
                "Principal " (delegation.client.principal) " \u{b7} "
                (if delegation.read_only() {
                    "Observation only"
                } else {
                    "Funding, quotes, confirmation, and cancellation delegated"
                })
                " \u{b7} identity " code { (&delegation.identity[..19]) }
            }
            @match &account {
                Ok(account) => {
                    pre class="retail-account" { (account.lines()) }
                    p { a href=(format!("{base}/purchases")) { "Purchases: progress, artifacts, receipts, and recovery" } }
                }
                Err(_) => {
                    (ui::unavailable(
                        "Retail service: Unavailable",
                        "Current rights are unknown, so no effect is offered.",
                    ))
                }
            }
            h4 { "Your OpenAI API key" }
            @match &custody {
                Some(status) => {
                    p { "In custody: " (status.material.label()) " \u{b7} " (status.masked()) " \u{b7} expires at " (status.expires_at) }
                    @if let Some(remove) = remove { (remove) }
                }
                None => { p { "No key in custody." } }
            }
            @if let Some(place) = place { (place) }
            @if let Some(fund) = fund {
                h4 { "Fund" }
                (fund)
            }
            @if let Some(quote) = quote {
                h4 { "Review a quote" }
                (quote)
            }
            @if let Some(cancel) = cancel {
                h4 { "Request a stop" }
                (cancel)
            }
            @if !items.is_empty() {
                h4 { "Requests" }
                ul class="retail-requests" {
                    @for item in &items { (item) }
                }
            }
        }
    })
}

/// A request whose outcome is unknown is retried with its original identity
/// and parameters only.
fn retry_form(
    context: &Context<'_>,
    headers: &HeaderMap,
    delegation: &Delegation,
    request: &str,
    record: &Value,
) -> Result<Option<ui::BoundForm>, Response> {
    let base = format!("{PAGE}/{}", delegation.id());
    let params = &record["params"];
    let (action, fields) = match record["op"].as_str().unwrap_or("") {
        "top_up" => (
            "top-up",
            vec![("amount_sats", params["amount_sats"].to_string())],
        ),
        "confirm" => (
            "confirm",
            vec![
                ("review", params["review"].as_str().unwrap_or("").into()),
                ("custody", "service".into()),
            ],
        ),
        "cancel" => (
            "cancel",
            vec![(
                "execution",
                params["execution"].as_str().unwrap_or("").into(),
            )],
        ),
        "quote" => {
            let task = &params["task"];
            let checks = task["checks"]
                .as_array()
                .map(|v| {
                    v.iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_default();
            (
                "quote",
                vec![
                    (
                        "repository",
                        task["source"]["repository"].as_str().unwrap_or("").into(),
                    ),
                    (
                        "commit",
                        task["source"]["commit"].as_str().unwrap_or("").into(),
                    ),
                    ("task", task["task"].as_str().unwrap_or("").into()),
                    ("checks", checks),
                    ("max_seconds", task["max_seconds"].to_string()),
                ],
            )
        }
        _ => return Ok(None),
    };
    let scope = format!("retail-{action}");
    let csrf = context.csrf(headers, &scope, &target(delegation, request))?;
    let mut form = ui::retry(format!("{base}/{action}"), &csrf).bind("request", request);
    for (name, value) in fields {
        form = form.bind(name, &value);
    }
    Ok(Some(form))
}

fn done(fragment: &str) -> Response {
    protect(Redirect::to(&format!("{PAGE}#retail-{}", escape(fragment))).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KeyForm {
    csrf: String,
    request: String,
    key: String,
    #[serde(default)]
    consent: Option<String>,
}

impl Drop for KeyForm {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.key).into_bytes();
        bytes.fill(0);
    }
}

async fn store_key(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<KeyForm>, FormRejection>,
) -> Response {
    let Ok(Form(mut form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegation = match context.retail.get(&context.viewer, &id) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if let Err(response) = context.verify(
        &headers,
        &form.csrf,
        "retail-key",
        &target(delegation, &form.request),
    ) {
        return response;
    }
    let key = match Key::new(std::mem::take(&mut form.key)) {
        Ok(value) => value,
        Err(error) => return answer(Failure::Custody(error)),
    };
    let consent = form.consent.as_deref() == Some("custody");
    match context.retail.store_key(&context.viewer, &id, key, consent) {
        Ok(_) => done(&id),
        Err(error) => answer(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoveForm {
    csrf: String,
    request: String,
}

async fn remove_key(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<RemoveForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegation = match context.retail.get(&context.viewer, &id) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if let Err(response) = context.verify(
        &headers,
        &form.csrf,
        "retail-key-remove",
        &target(delegation, &form.request),
    ) {
        return response;
    }
    match context.retail.revoke_key(&context.viewer, &id) {
        Ok(_) => done(&id),
        Err(error) => answer(error),
    }
}

async fn run_effect(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    scope: &str,
    csrf: &str,
    request: String,
    effect: Result<Effect, SessionError>,
) -> Response {
    // A stop returns to the purchase it concerns.
    let next = match &effect {
        Ok(Effect::Cancel { execution }) => Some(purchases::href(id, execution)),
        _ => None,
    };
    let context = match context(app, headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegation = match context.retail.get(&context.viewer, id) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if let Err(response) = context.verify(headers, csrf, scope, &target(delegation, &request)) {
        return response;
    }
    let effect = match effect {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    match context
        .retail
        .effect(&context.viewer, id, request, effect)
        .await
    {
        Ok(_) => match next {
            Some(next) => protect(Redirect::to(&next).into_response()),
            None => done(id),
        },
        Err(error) => answer(error),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TopUpForm {
    csrf: String,
    request: String,
    amount_sats: u64,
}

async fn top_up(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<TopUpForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let effect = if (1..=1_000_000).contains(&form.amount_sats) {
        Ok(Effect::TopUp {
            amount_sats: form.amount_sats,
        })
    } else {
        Err(SessionError::InvalidRequest)
    };
    run_effect(
        &app,
        &headers,
        &id,
        "retail-top-up",
        &form.csrf,
        form.request,
        effect,
    )
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuoteForm {
    csrf: String,
    request: String,
    repository: String,
    commit: String,
    task: String,
    checks: String,
    max_seconds: u64,
}

async fn quote(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<QuoteForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let checks: Vec<String> = form
        .checks
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect();
    let effect = Ok(Effect::Quote {
        task: retail_cloud::contract::TaskRequest {
            source: retail_cloud::authority::Source {
                repository: form.repository.trim().into(),
                commit: form.commit.trim().into(),
            },
            task: form.task.replace("\r\n", "\n"),
            checks,
            max_seconds: form.max_seconds,
            ceiling_sats: None,
        },
    });
    run_effect(
        &app,
        &headers,
        &id,
        "retail-quote",
        &form.csrf,
        form.request,
        effect,
    )
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmForm {
    csrf: String,
    request: String,
    review: String,
    #[serde(default)]
    custody: Option<String>,
}

async fn confirm(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<ConfirmForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let effect = if form.custody.as_deref() != Some("service") {
        Err(SessionError::InvalidRequest)
    } else if form.review.len() > 128 {
        Err(SessionError::InvalidRequest)
    } else {
        Ok(Effect::Confirm {
            review: form.review,
        })
    };
    run_effect(
        &app,
        &headers,
        &id,
        "retail-confirm",
        &form.csrf,
        form.request,
        effect,
    )
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CancelForm {
    csrf: String,
    request: String,
    execution: String,
}

async fn cancel(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<CancelForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let effect = if valid_id(&form.execution) {
        Ok(Effect::Cancel {
            execution: form.execution,
        })
    } else {
        Err(SessionError::InvalidRequest)
    };
    run_effect(
        &app,
        &headers,
        &id,
        "retail-cancel",
        &form.csrf,
        form.request,
        effect,
    )
    .await
}

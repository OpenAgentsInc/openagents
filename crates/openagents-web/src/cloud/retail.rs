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
use super::{failure, protect, refused, service, ticket, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use compute_workbench::retail::{self as native, Client, ProviderKey};
use route_contract::Digest as RouteDigest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;

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

fn answer(error: Failure) -> Response {
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
            let value = perform(delegation, client, vault, &journal, &request, effect)?;
            if let Some(record) = journal.records.get_mut(&request) {
                record.outcome = Some(value.clone());
            }
            journal.save(journals, delegation)?;
            Ok(value)
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
    let mut content = String::from(
        "<h2>Retail delegation</h2><p>Retail purchases go through an operator-provisioned delegation for this account, workspace, and membership. The server calls the retail service with that native principal; no service credential reaches this page. Signing in grants no funding, quote, confirmation, or cancellation right.</p>",
    );
    let delegations = context.retail.current(&context.viewer);
    if delegations.is_empty() {
        content.push_str("<p>Unavailable: no retail delegation is provisioned for this account, workspace, and membership.</p>");
    }
    for delegation in delegations {
        match section(&context, &headers, delegation).await {
            Ok(value) => content.push_str(&value),
            Err(response) => return response,
        }
    }
    workspace_shell(
        context.app,
        &headers,
        context.service,
        &context.viewer,
        "billing",
        Some(&content),
        None,
    )
}

async fn section(
    context: &Context<'_>,
    headers: &HeaderMap,
    delegation: &Delegation,
) -> Result<String, Response> {
    let id = escape(delegation.id());
    let base = format!("{PAGE}/{id}");
    let mut out = format!(
        "<section class=\"cloud-card\" id=\"retail-{id}\"><h3>Delegation {id}</h3><p>Principal {} · {} · identity <code>{}</code></p>",
        escape(&delegation.client.principal),
        if delegation.read_only() {
            "Observation only"
        } else {
            "Funding, quotes, confirmation, and cancellation delegated"
        },
        escape(&delegation.identity[..19]),
    );
    let account = context
        .retail
        .account(&context.viewer, delegation.id())
        .await;
    let rights = match &account {
        Ok(account) => {
            out.push_str(&format!(
                "<pre class=\"retail-account\">{}</pre>",
                escape(&account.lines())
            ));
            Some(account.capabilities.clone())
        }
        Err(Failure::Session(error)) => return Err(refused(*error)),
        Err(_) => {
            out.push_str("<p>Retail service: Unavailable. Current rights are unknown, so no effect is offered.</p>");
            None
        }
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
    out.push_str("<h4>Your OpenAI API key</h4>");
    match &custody {
        Some(status) => {
            let request = fresh_request();
            out.push_str(&format!(
                "<p>In custody: {} · {} · expires at {}</p><form method=\"post\" action=\"{base}/key/remove\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><button type=\"submit\">Remove key from custody</button></form>",
                escape(status.material.label()),
                escape(&status.masked()),
                status.expires_at,
                ticket(&context.csrf(headers, "retail-key-remove", &target(delegation, &request))?),
            ));
        }
        None => out.push_str("<p>No key in custody.</p>"),
    }
    if !delegation.read_only() {
        let request = fresh_request();
        out.push_str(&format!(
            "<form method=\"post\" action=\"{base}/key\" autocomplete=\"off\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><p><label>OpenAI API key <input type=\"password\" name=\"key\" autocomplete=\"off\" spellcheck=\"false\" required></label></p><p class=\"dim\">{}</p><p><label><input type=\"checkbox\" name=\"consent\" value=\"custody\" required> I consent to this custody</label></p><p><button type=\"submit\">Place key in custody</button></p></form>",
            ticket(&context.csrf(headers, "retail-key", &target(delegation, &request))?),
            escape(TERMS),
        ));
    }

    if can_spend {
        let request = fresh_request();
        out.push_str(&format!(
            "<h4>Fund</h4><form method=\"post\" action=\"{base}/top-up\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><label>Credits (sats) <input type=\"number\" name=\"amount_sats\" min=\"1\" max=\"1000000\" required></label> <button type=\"submit\">Request an exact invoice</button></form><p class=\"dim\">Pay the invoice with your own wallet. Credits are non-withdrawable and grant no execution authority.</p>",
            ticket(&context.csrf(headers, "retail-top-up", &target(delegation, &request))?),
        ));
    }
    if can_dispatch && custody.is_some() {
        let request = fresh_request();
        out.push_str(&format!(
            "<h4>Review a quote</h4><form method=\"post\" action=\"{base}/quote\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><p><label>Public GitHub repository <input name=\"repository\" required></label></p><p><label>Commit (40 hex) <input name=\"commit\" required></label></p><p><label>Task <textarea name=\"task\" required></textarea></label></p><p><label>Checks, one per line <textarea name=\"checks\" required></textarea></label></p><p><label>Wall limit (seconds) <input type=\"number\" name=\"max_seconds\" min=\"1\" max=\"3600\" required></label></p><p><button type=\"submit\">Review quote</button></p></form>",
            ticket(&context.csrf(headers, "retail-quote", &target(delegation, &request))?),
        ));
    }
    if can_cancel {
        let request = fresh_request();
        out.push_str(&format!(
            "<h4>Request a stop</h4><form method=\"post\" action=\"{base}/cancel\">{}<input type=\"hidden\" name=\"request\" value=\"{request}\"><label>Execution <input name=\"execution\" required></label> <button type=\"submit\">Request stop</button></form><p class=\"dim\">A stop request is not a stopped meter; acknowledgment, deletion, and settlement are recorded separately.</p>",
            ticket(&context.csrf(headers, "retail-cancel", &target(delegation, &request))?),
        ));
    }

    // Journaled requests with retry and confirmation controls.
    let requests = context
        .retail
        .requests(&context.viewer, delegation.id())
        .await
        .map_err(answer)?;
    if !requests.is_empty() {
        out.push_str("<h4>Requests</h4><ul class=\"retail-requests\">");
    }
    for (request, record) in &requests {
        let op = record["op"].as_str().unwrap_or("");
        let state = if record["outcome"].is_null() {
            "Outcome unknown"
        } else {
            "Recorded"
        };
        out.push_str(&format!(
            "<li><span>{} · request {} · {state}</span>",
            escape(op),
            escape(&request[..8])
        ));
        if record["outcome"].is_null() && !delegation.read_only() {
            out.push_str(&retry_form(context, headers, delegation, request, record)?);
        }
        if op == "quote" {
            if let Some(lines) = record["outcome"]["lines"].as_str() {
                out.push_str(&format!("<pre>{}</pre>", escape(lines)));
            }
            if let (Some(review), true) = (record["outcome"]["digest"].as_str(), can_dispatch) {
                let confirm = fresh_request();
                out.push_str(&format!(
                    "<form method=\"post\" action=\"{base}/confirm\">{}<input type=\"hidden\" name=\"request\" value=\"{confirm}\"><input type=\"hidden\" name=\"review\" value=\"{}\"><p><label><input type=\"checkbox\" name=\"custody\" value=\"service\" required> Hand my key to the authenticated retail service for this exact task</label></p><button type=\"submit\">Confirm and reserve the maximum</button></form>",
                    ticket(&context.csrf(headers, "retail-confirm", &target(delegation, &confirm))?),
                    escape(review),
                ));
            }
        } else if let Some(value) = record["outcome"].as_object() {
            out.push_str(&format!(
                "<pre>{}</pre>",
                escape(&serde_json::to_string_pretty(value).unwrap_or_default())
            ));
        }
        out.push_str("</li>");
    }
    if !requests.is_empty() {
        out.push_str("</ul>");
    }
    out.push_str("</section>");
    Ok(out)
}

/// A request whose outcome is unknown is retried with its original identity
/// and parameters only.
fn retry_form(
    context: &Context<'_>,
    headers: &HeaderMap,
    delegation: &Delegation,
    request: &str,
    record: &Value,
) -> Result<String, Response> {
    let base = format!("{PAGE}/{}", escape(delegation.id()));
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
        _ => return Ok(String::new()),
    };
    let scope = format!("retail-{}", action);
    let mut form = format!(
        "<form method=\"post\" action=\"{base}/{action}\">{}<input type=\"hidden\" name=\"request\" value=\"{}\">",
        ticket(&context.csrf(headers, &scope, &target(delegation, request))?),
        escape(request)
    );
    for (name, value) in fields {
        form.push_str(&format!(
            "<input type=\"hidden\" name=\"{name}\" value=\"{}\">",
            escape(&value)
        ));
    }
    form.push_str("<button type=\"submit\">Retry the same request</button></form>");
    Ok(form)
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
        Ok(_) => done(id),
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

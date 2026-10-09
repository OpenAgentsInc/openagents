//! Same-origin browser delegation to the separate sales-owner remote adapter.
//!
//! This site never opens the private pipeline and keeps no contact. The
//! operator provisions one delegation per account, workspace, and membership
//! epoch; each names the owner adapter's endpoint, one binding, and that
//! binding's bearer, which stays on the server. Account, host, Studio, world,
//! or billing membership creates none. The owner adapter rechecks the bound
//! sales credential, its revocation, and record revisions on every call.
//!
//! An effect's request identity, parameters, and exact command digest are
//! journaled before dispatch. An exact retry recovers the original receipt;
//! changed parameters conflict; a lost reply shows Outcome unknown and is
//! reconciled with the owner by identity and digest, never by resending
//! different bytes. Only receipts and digests are retained here.

use super::custody::{self, CustodyError};
use super::private::ProtectedFile;
use super::session::{SessionError, Viewer, now};
use super::ui;
use super::{failure, protect, refused, service, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use coder::task::sales::remote::{
    self as owner, Actor, Code, Effect, Op, Settled, Standing, Summary, record_id,
};
use coder::task::sales::{self as pipeline, Lead, Receipt, Role, Stage};
use maud::{Markup, Render, html};
use openagents_ui::forms::{Field, Select};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[path = "sales_views.rs"]
mod views;

pub const SCHEMA: &str = "openagents.cloud.sales-delegations.v1";
const JOURNAL_SCHEMA: &str = "openagents.cloud.sales-web-requests.v1";
const UNAVAILABLE: &str = "The sales delegation configuration is unavailable or changed.";
const RECORDS_MAX: usize = 256;
const PAGE: &str = "/cloud/app/sales";
const STAGES: [(Stage, &str); 5] = [
    (Stage::New, "New"),
    (Stage::Qualified, "Qualified"),
    (Stage::Pilot, "Pilot"),
    (Stage::Active, "Active"),
    (Stage::Closed, "Closed"),
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    schema: String,
    /// Private directory owned by this site for request journals.
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
    /// The owner adapter's `/v1/sales` endpoint.
    endpoint: String,
    /// The owner-provisioned binding this delegation presents.
    binding: String,
    /// That binding's bearer, readable only by this site.
    bearer_file: PathBuf,
    /// Permit plain HTTP to a numeric loopback adapter in development.
    #[serde(default)]
    development_loopback: bool,
}

struct Bearer(String);

impl Drop for Bearer {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}

/// One explicitly provisioned account/workspace/epoch delegation.
pub(crate) struct Delegation {
    id: String,
    account: String,
    workspace: String,
    members_epoch: u64,
    endpoint: reqwest::Url,
    binding: String,
    bearer: Bearer,
    identity: String,
    file: ProtectedFile,
    lock: Arc<tokio::sync::Mutex<()>>,
}

/// Operator-provisioned sales delegations; sign-in never creates one.
pub struct Delegations {
    config: ProtectedFile,
    journals: PathBuf,
    delegations: Vec<Arc<Delegation>>,
    http: reqwest::Client,
}

impl Delegations {
    pub fn load(path: &FsPath) -> Result<Self, String> {
        let (config, bytes) = ProtectedFile::open(path, 64 * 1024)?;
        let declared: Configuration = serde_json::from_slice(&bytes).map_err(|_| UNAVAILABLE)?;
        if declared.schema != SCHEMA || declared.delegations.len() > 64 {
            return Err(UNAVAILABLE.into());
        }
        custody::checked_root(&declared.directory).map_err(|_| UNAVAILABLE)?;
        let journals = declared.directory.join("sales-requests");
        if std::fs::symlink_metadata(&journals).is_err() {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&journals)
                .map_err(|_| UNAVAILABLE)?;
        }
        custody::checked_root(&journals).map_err(|_| UNAVAILABLE)?;
        let mut delegations: Vec<Arc<Delegation>> = Vec::new();
        for declared in declared.delegations {
            if delegations.iter().any(|d| d.id == declared.id) {
                return Err(UNAVAILABLE.into());
            }
            delegations.push(Arc::new(Delegation::load(declared)?));
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| UNAVAILABLE)?;
        Ok(Self {
            config,
            journals,
            delegations,
            http,
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
            || !valid_id(&declared.binding)
            || declared.members_epoch > 9_007_199_254_740_991
        {
            return Err(UNAVAILABLE.into());
        }
        let endpoint = reqwest::Url::parse(&declared.endpoint).map_err(|_| UNAVAILABLE)?;
        let loopback = endpoint
            .host_str()
            .and_then(|h| h.trim_matches(['[', ']']).parse::<std::net::IpAddr>().ok())
            .is_some_and(|a| a.is_loopback());
        if !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || endpoint.path() != crate::sales_remote::PATH
            || !(endpoint.scheme() == "https"
                || (declared.development_loopback && loopback && endpoint.scheme() == "http"))
        {
            return Err(UNAVAILABLE.into());
        }
        let (file, mut bytes) = ProtectedFile::open(&declared.bearer_file, 4096)?;
        let bearer = std::str::from_utf8(&bytes)
            .map(|s| s.trim().to_owned())
            .map_err(|_| UNAVAILABLE);
        let bearer_digest = hex(&Sha256::digest(&bytes));
        bytes.fill(0);
        let bearer = Bearer(bearer?);
        if bearer.0.is_empty() || bearer.0.bytes().any(|b| !b.is_ascii_graphic()) {
            return Err(UNAVAILABLE.into());
        }
        let identity = digest(&json!({
            "schema":SCHEMA,"delegation":declared.id,"account":declared.account,
            "workspace":declared.workspace,"members_epoch":declared.members_epoch,
            "endpoint":endpoint.as_str(),"binding":declared.binding,"bearer":bearer_digest,
        }));
        Ok(Self {
            id: declared.id,
            account: declared.account,
            workspace: declared.workspace,
            members_epoch: declared.members_epoch,
            endpoint,
            binding: declared.binding,
            bearer,
            identity,
            file,
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
        self.file.check().map_err(|_| SessionError::Unavailable)
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    fn actor(&self) -> Actor {
        Actor {
            account: self.account.clone(),
            workspace: self.workspace.clone(),
            members_epoch: self.members_epoch,
        }
    }
}

/// Why an adapter call did not produce a result. Nothing here carries a
/// credential, a contact, or a raw owner response.
#[derive(Debug)]
pub(crate) enum Failure {
    Session(SessionError),
    /// No answer: the outcome is unknown; retry the same request.
    Unknown,
    /// The owner adapter's fixed refusal code.
    Owner(Code),
    /// The record changed before this request reached the owner.
    Changed,
    /// The request identity was used for different parameters.
    Reused,
}

impl From<SessionError> for Failure {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl From<CustodyError> for Failure {
    fn from(_: CustodyError) -> Self {
        Self::Session(SessionError::Unavailable)
    }
}

pub(crate) fn answer(error: Failure) -> Response {
    let (status, title, message) = match error {
        Failure::Session(error) => return refused(error),
        Failure::Unknown => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Outcome unknown",
            "The sales owner did not answer. Retry the same request from the record page; it reconciles the original change and never applies a different one.",
        ),
        Failure::Changed => (
            StatusCode::CONFLICT,
            "Record changed",
            "The record changed before this request reached the sales owner. Nothing was applied; review the current record.",
        ),
        Failure::Reused => (
            StatusCode::CONFLICT,
            "Request conflict",
            "This request identity was used for another change. Reload and review it again.",
        ),
        Failure::Owner(code) => match code {
            Code::AccessDenied => (
                StatusCode::FORBIDDEN,
                "Sales access refused",
                "The sales owner refused this binding's current credential, rights, or record access.",
            ),
            Code::Stale => (
                StatusCode::CONFLICT,
                "Record changed",
                "The record revision moved. Nothing was applied; review the current record.",
            ),
            Code::Conflict => (
                StatusCode::CONFLICT,
                "Request conflict",
                "The sales owner recorded this request identity with different bytes.",
            ),
            Code::Refused => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "Change refused",
                "The sales owner refused this change; nothing was applied.",
            ),
            Code::InvalidRequest => (
                StatusCode::BAD_REQUEST,
                "Sales request refused",
                "The sales owner refused this request.",
            ),
            Code::Busy | Code::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Sales owner unavailable",
                "The sales owner is busy or unavailable. Retry the same request.",
            ),
        },
    };
    failure(status, title, message)
}

/// One journaled browser request: exact parameters, the exact command
/// digest, and the owner's receipt once known. No record body is retained.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    lead: String,
    revision: u64,
    stage: Stage,
    params: String,
    command: String,
    at: u64,
    receipt: Option<Receipt>,
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
        let Some(bytes) =
            custody::read_private(&root.join(Self::name(delegation)), 4 * 1024 * 1024)?
        else {
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

/// The exact command a stage change sends, and its digest.
fn stage_command(lead: &Lead, request: &str, stage: Stage) -> Result<(String, String), Failure> {
    let mut details = lead.details.clone();
    details.stage = stage;
    let command = serde_json::to_string(&pipeline::Command {
        schema: pipeline::COMMAND_SCHEMA.into(),
        id: request.into(),
        lead: Some(lead.id.clone()),
        expected_revision: lead.revision,
        operation: pipeline::Operation::Update { details },
    })
    .map_err(|_| Failure::Session(SessionError::Unavailable))?;
    let exact = hex(&Sha256::digest(command.as_bytes()));
    Ok((command, exact))
}

impl Delegations {
    /// One owner call from the server with the binding's bearer and no
    /// browser Origin. Membership is rechecked after the answer.
    async fn call<T: serde::de::DeserializeOwned>(
        &self,
        viewer: &Viewer,
        delegation: &Delegation,
        op: Op,
    ) -> Result<T, Failure> {
        let body = serde_json::to_vec(&owner::Request {
            schema: owner::REQUEST_SCHEMA.into(),
            actor: delegation.actor(),
            op,
        })
        .map_err(|_| Failure::Session(SessionError::Unavailable))?;
        let response = self
            .http
            .post(delegation.endpoint.clone())
            .bearer_auth(&delegation.bearer.0)
            .header("x-sales-binding", &delegation.binding)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| Failure::Unknown)?;
        let bytes = response.bytes().await.map_err(|_| Failure::Unknown)?;
        self.get(viewer, &delegation.id)?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(Failure::Unknown);
        }
        let mut value: Value = serde_json::from_slice(&bytes).map_err(|_| Failure::Unknown)?;
        if value["schema"] != owner::RESPONSE_SCHEMA {
            return Err(Failure::Unknown);
        }
        if let Some(code) = value["error"].as_str() {
            return Err(match Code::parse(code) {
                Code::Unavailable => Failure::Unknown,
                code => Failure::Owner(code),
            });
        }
        serde_json::from_value(value["result"].take()).map_err(|_| Failure::Unknown)
    }

    /// One read-only owner call for another projection (partners, WEB-16).
    /// Effects and reconciliation stay with this module's journal.
    pub(crate) async fn read<T: serde::de::DeserializeOwned>(
        &self,
        viewer: &Viewer,
        id: &str,
        op: Op,
    ) -> Result<T, Failure> {
        if matches!(op, Op::Apply { .. } | Op::Reconcile { .. }) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        let delegation = self.get(viewer, id)?.clone();
        self.call(viewer, &delegation, op).await
    }

    pub(crate) async fn standing(&self, viewer: &Viewer, id: &str) -> Result<Standing, Failure> {
        let delegation = self.get(viewer, id)?.clone();
        self.call(viewer, &delegation, Op::Standing).await
    }

    pub(crate) async fn list(&self, viewer: &Viewer, id: &str) -> Result<Vec<Summary>, Failure> {
        let delegation = self.get(viewer, id)?.clone();
        self.call(
            viewer,
            &delegation,
            Op::List {
                after: None,
                limit: 50,
            },
        )
        .await
    }

    pub(crate) async fn show(
        &self,
        viewer: &Viewer,
        id: &str,
        lead: &str,
    ) -> Result<Lead, Failure> {
        if !record_id(lead) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        let delegation = self.get(viewer, id)?.clone();
        self.call(
            viewer,
            &delegation,
            Op::Show {
                lead: lead.to_owned(),
            },
        )
        .await
    }

    /// Journaled requests for one record, newest first.
    fn requests(
        &self,
        viewer: &Viewer,
        id: &str,
        lead: &str,
    ) -> Result<Vec<(String, Record)>, Failure> {
        let delegation = self.get(viewer, id)?;
        let journal = Journal::load(&self.journals, delegation)?;
        let mut records: Vec<(String, Record)> = journal
            .records
            .into_iter()
            .filter(|(_, r)| r.lead == lead)
            .collect();
        records.sort_by(|a, b| b.1.at.cmp(&a.1.at).then(a.0.cmp(&b.0)));
        records.truncate(16);
        Ok(records)
    }

    /// Change one record's stage. The request is journaled with its exact
    /// command digest before dispatch; retries reconcile by identity.
    pub(crate) async fn stage(
        &self,
        viewer: &Viewer,
        id: &str,
        lead: &str,
        request: &str,
        revision: u64,
        stage: Stage,
    ) -> Result<Receipt, Failure> {
        if !request_id(request) || !record_id(lead) {
            return Err(Failure::Session(SessionError::InvalidRequest));
        }
        let delegation = self.get(viewer, id)?.clone();
        let _turn = delegation.lock.clone().lock_owned().await;
        let params = digest(&json!({"lead":lead,"revision":revision,"stage":stage}));
        let mut journal = Journal::load(&self.journals, &delegation)?;
        if let Some(record) = journal.records.get(request).cloned() {
            if record.params != params {
                return Err(Failure::Reused);
            }
            if let Some(receipt) = record.receipt {
                return Ok(receipt);
            }
            return self
                .reconcile(viewer, &delegation, &mut journal, request, record)
                .await;
        }
        // Recheck the current record before admitting a new change.
        let current: Lead = self
            .call(
                viewer,
                &delegation,
                Op::Show {
                    lead: lead.to_owned(),
                },
            )
            .await?;
        if current.revision != revision {
            return Err(Failure::Changed);
        }
        let (command, exact) = stage_command(&current, request, stage)?;
        if journal.records.len() >= RECORDS_MAX {
            let oldest = journal
                .records
                .iter()
                .filter(|(_, r)| r.receipt.is_some())
                .min_by_key(|(_, r)| r.at)
                .map(|(k, _)| k.clone())
                .ok_or(Failure::Owner(Code::Busy))?;
            journal.records.remove(&oldest);
        }
        journal.records.insert(
            request.to_owned(),
            Record {
                lead: lead.to_owned(),
                revision,
                stage,
                params,
                command: exact,
                at: now(),
                receipt: None,
            },
        );
        journal.save(&self.journals, &delegation)?;
        let result = self
            .call(
                viewer,
                &delegation,
                Op::Apply {
                    request: request.to_owned(),
                    command,
                },
            )
            .await;
        self.settle(&delegation, &mut journal, request, result)
    }

    async fn reconcile(
        &self,
        viewer: &Viewer,
        delegation: &Delegation,
        journal: &mut Journal,
        request: &str,
        record: Record,
    ) -> Result<Receipt, Failure> {
        let settled: Result<Settled, Failure> = self
            .call(
                viewer,
                delegation,
                Op::Reconcile {
                    request: request.to_owned(),
                    digest: record.command.clone(),
                },
            )
            .await;
        let result = match settled {
            Ok(Settled::Recorded { receipt }) => Ok(receipt),
            Ok(Settled::Absent) => {
                // The owner never journaled it, so nothing was applied. Send
                // it only if the same exact command can still be formed.
                let current: Lead = self
                    .call(
                        viewer,
                        delegation,
                        Op::Show {
                            lead: record.lead.clone(),
                        },
                    )
                    .await?;
                let (command, exact) = stage_command(&current, request, record.stage)?;
                if current.revision != record.revision || exact != record.command {
                    journal.records.remove(request);
                    journal.save(&self.journals, delegation)?;
                    return Err(Failure::Changed);
                }
                self.call(
                    viewer,
                    delegation,
                    Op::Apply {
                        request: request.to_owned(),
                        command,
                    },
                )
                .await
            }
            Err(error) => Err(error),
        };
        self.settle(delegation, journal, request, result)
    }

    fn settle(
        &self,
        delegation: &Delegation,
        journal: &mut Journal,
        request: &str,
        result: Result<Receipt, Failure>,
    ) -> Result<Receipt, Failure> {
        match result {
            Ok(receipt) => {
                if let Some(record) = journal.records.get_mut(request) {
                    record.receipt = Some(receipt.clone());
                }
                journal.save(&self.journals, delegation)?;
                Ok(receipt)
            }
            Err(Failure::Owner(code)) if code.definitive() => {
                // A definitive refusal applied nothing; no unknown outcome remains.
                if journal
                    .records
                    .get(request)
                    .is_some_and(|r| r.receipt.is_none())
                {
                    journal.records.remove(request);
                    journal.save(&self.journals, delegation)?;
                }
                Err(Failure::Owner(code))
            }
            Err(error) => Err(error),
        }
    }
}

fn request_id(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
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

fn stage_label(stage: Stage) -> &'static str {
    STAGES
        .iter()
        .find(|(s, _)| *s == stage)
        .map_or("Unknown", |(_, label)| label)
}

fn stage_value(stage: Stage) -> String {
    serde_json::to_value(stage)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

// ---- Routes ---------------------------------------------------------------

#[path = "sales_floor.rs"]
mod floor;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .merge(floor::routes())
        .route(PAGE, get(index))
        .route("/cloud/app/sales/{id}/leads/{lead}", get(record))
        .route(
            "/cloud/app/sales/{id}/leads/{lead}/stage",
            post(change_stage),
        )
        .merge(views::routes())
}

pub(crate) fn available(app: &App, viewer: &Viewer) -> bool {
    app.config
        .cloud_sales
        .as_ref()
        .is_some_and(|sales| !sales.current(viewer).is_empty())
}

struct Context<'a> {
    app: &'a App,
    service: &'a super::session::CloudSession,
    viewer: Viewer,
    sales: &'a Delegations,
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
    let sales = app
        .config
        .cloud_sales
        .as_deref()
        .ok_or_else(|| refused(SessionError::Forbidden))?;
    Ok(Context {
        app,
        service,
        viewer,
        sales,
    })
}

fn target(delegation: &Delegation, lead: &str, request: &str) -> String {
    format!("{}:{lead}:{request}", delegation.identity)
}

fn shell(context: &Context<'_>, headers: &HeaderMap, content: Markup) -> Response {
    workspace_shell(
        context.app,
        headers,
        context.service,
        &context.viewer,
        "sales",
        Some(content),
        None,
    )
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    if app.config.cloud_sales.is_none() {
        return super::workspace_page(&app, &headers, "sales").await;
    }
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut cards = Vec::new();
    let delegations = context.sales.current(&context.viewer);
    for delegation in &delegations {
        let id = delegation.id();
        let standing = match context.sales.standing(&context.viewer, id).await {
            Ok(standing) => standing,
            Err(Failure::Session(error)) => return refused(error),
            Err(error) => {
                cards.push(html! {
                    section class="cloud-card" id=(format!("sales-{id}")) {
                        h3 { "Sales delegation " (id) }
                        p { "Sales owner: " (unavailable_label(&error)) ". No record or change is offered." }
                    }
                });
                continue;
            }
        };
        let records = match context.sales.list(&context.viewer, id).await {
            Ok(leads) if leads.is_empty() => {
                html! { p { "No records are visible to this principal." } }
            }
            Ok(leads) => {
                let mut table = ui::table("Sales pipeline").header([
                    "Record",
                    "Stage",
                    "Responsible human",
                    "Permission",
                    "Next due",
                    "Revision",
                ]);
                for lead in leads {
                    table = table.row([
                        html! {
                            a href=(format!("{PAGE}/{id}/leads/{}", lead.id)) {
                                (&lead.id[..lead.id.len().min(16)])
                            }
                        },
                        html! { (stage_label(lead.stage)) },
                        html! { (lead.responsible_human) },
                        html! { (permission_label(lead.permission)) },
                        html! {
                            @match lead.next_due_at {
                                Some(due) => { (due) }
                                None => { "None" }
                            }
                        },
                        html! { (lead.revision) },
                    ]);
                }
                html! { div class="sales-pipeline" { (table) } }
            }
            Err(Failure::Session(error)) => return refused(error),
            Err(error) => html! { p { "Records: " (unavailable_label(&error)) "." } },
        };
        cards.push(html! {
            section class="cloud-card" id=(format!("sales-{id}")) {
                h3 { "Sales delegation " (id) }
                p {
                    "Principal " (standing.principal) " \u{b7} " (role_label(standing.role))
                    " \u{b7} " (effects_label(&standing.effects))
                }
                (views::nav(id, ""))
                @if standing.supervise {
                    (floor::floor_link(id))
                }
                (records)
            }
        });
    }
    let content = html! {
        h2 { "Private sales" }
        p { "The private pipeline stays with its sales owner. This page reaches it only through a delegation provisioned for this account, workspace, and membership; the owner rechecks its separate sales credential on every read and change. Account, host, Studio, world, and billing membership grant no pipeline access." }
        @if delegations.is_empty() {
            (ui::unavailable("No sales delegation", "Unavailable: no sales delegation is provisioned for this account, workspace, and membership."))
        }
        @for card in &cards { (card) }
    };
    shell(&context, &headers, content)
}

fn role_label(role: Role) -> &'static str {
    match role {
        Role::Owner => "Pipeline owner",
        Role::Writer => "Writer",
        Role::Reader => "Reader",
    }
}

fn effects_label(effects: &[Effect]) -> String {
    if effects.is_empty() {
        return "Observation only".into();
    }
    let names: Vec<String> = effects
        .iter()
        .map(|e| {
            serde_json::to_value(e)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.replace('_', " ")))
                .unwrap_or_default()
        })
        .collect();
    format!("Admitted changes: {}", names.join(", "))
}

fn permission_label(state: pipeline::PermissionState) -> &'static str {
    match state {
        pipeline::PermissionState::Granted => "Granted",
        pipeline::PermissionState::Revoked => "Revoked",
        pipeline::PermissionState::Unknown => "Unknown",
    }
}

fn unavailable_label(error: &Failure) -> &'static str {
    match error {
        Failure::Owner(Code::AccessDenied) => {
            "Access refused for this binding's current credential"
        }
        _ => "Unavailable",
    }
}

async fn record(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, lead)): Path<(String, String)>,
) -> Response {
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegation = match context.sales.get(&context.viewer, &id) {
        Ok(value) => value.clone(),
        Err(error) => return refused(error),
    };
    let standing = match context.sales.standing(&context.viewer, &id).await {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let found = match context.sales.show(&context.viewer, &id, &lead).await {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let base = format!("{PAGE}/{id}/leads/{}", found.id);
    let writable = standing.effects.contains(&Effect::Update)
        && match standing.role {
            Role::Owner => true,
            Role::Writer => found.responsible_human == standing.principal,
            Role::Reader => false,
        };
    let change = if writable {
        let request = fresh_request();
        let csrf = match context.service.csrf(
            &headers,
            &context.viewer,
            "sales-stage",
            &target(&delegation, &found.id, &request),
        ) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        let field = Field::new("sales-stage", "Stage");
        let mut select = Select::new("stage").aria(field.aria());
        for (stage, label) in STAGES {
            if stage != found.details.stage {
                select = select.option(stage_value(stage), label);
            }
        }
        html! {
            h3 { "Change stage" }
            (ui::BoundForm::new(format!("{base}/stage"))
                .csrf(&csrf)
                .bind("request", &request)
                .bind("revision", &found.revision.to_string())
                .body(field.control(select))
                .submit(&format!("Change stage at revision {}", found.revision)))
        }
    } else {
        html! { p { "No change is admitted for this principal and record." } }
    };
    let requests = match context.sales.requests(&context.viewer, &id, &found.id) {
        Ok(value) => value,
        Err(error) => return answer(error),
    };
    let mut entries = Vec::with_capacity(requests.len());
    for (request, entry) in &requests {
        entries.push(match &entry.receipt {
            Some(receipt) => html! {
                li {
                    "Stage " (stage_label(entry.stage)) " \u{b7} request " (&request[..8])
                    " \u{b7} Recorded at revision " (receipt.revision) " (" (receipt.outcome) ")"
                }
            },
            None => {
                let csrf = match context.service.csrf(
                    &headers,
                    &context.viewer,
                    "sales-stage",
                    &target(&delegation, &entry.lead, request),
                ) {
                    Ok(value) => value,
                    Err(error) => return refused(error),
                };
                html! {
                    li {
                        "Stage " (stage_label(entry.stage)) " \u{b7} request " (&request[..8])
                        (ui::outcome_unknown(
                            "The sales owner did not answer. Retry the same request; it reconciles the original change and never applies a different one.",
                            Some(
                                ui::retry(format!("{base}/stage"), &csrf)
                                    .bind("request", request)
                                    .bind("revision", &entry.revision.to_string())
                                    .bind("stage", &stage_value(entry.stage)),
                            ),
                        ))
                    }
                }
            }
        });
    }
    let content = html! {
        p { a href=(PAGE) { "Private sales" } }
        h2 { "Record " (&found.id[..found.id.len().min(16)]) }
        div class="sales-record" {
            (ui::Details::new()
                .row("Contact", found.contact.as_str())
                .row("Account", found.details.account.as_str())
                .row("Stage", stage_label(found.details.stage))
                .row("Responsible human", found.responsible_human.as_str())
                .row(
                    "Permission",
                    format!(
                        "{} \u{b7} expires at {}",
                        permission_label(found.details.permission.state),
                        found.details.permission.expires_at
                    ),
                )
                .row(
                    "Next action",
                    found.details.next.as_ref().map_or_else(
                        || "None".to_owned(),
                        |n| format!("{} \u{b7} due {}", n.description, n.due_at),
                    ),
                )
                .row("Workflow", found.details.workflow.as_str())
                .row("Revision", found.revision))
        }
        (views::nav(&id, ""))
        @if !found.service_sales.is_empty() {
            h3 { "Service records" }
            ul {
                @for sale in found.service_sales.keys() {
                    li {
                        a href=(format!("{base}/services/{sale}")) { (sale) }
                        " \u{b7} see Pilots and delivery and Invoices and fulfillment"
                    }
                }
            }
        }
        @if standing.role == Role::Owner {
            p { a href=(format!("{base}/audit")) { "Audit for this record" } }
        }
        (change)
        @if !entries.is_empty() {
            h3 { "Requests" }
            ul class="sales-requests" {
                @for entry in &entries { (entry) }
            }
        }
    };
    shell(&context, &headers, content)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StageForm {
    csrf: String,
    request: String,
    revision: u64,
    stage: String,
}

async fn change_stage(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, lead)): Path<(String, String)>,
    form: Result<Form<StageForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let delegation = match context.sales.get(&context.viewer, &id) {
        Ok(value) => value.clone(),
        Err(error) => return refused(error),
    };
    if let Err(error) = context.service.verify_csrf(
        &headers,
        Some(&context.viewer),
        "sales-stage",
        &target(&delegation, &lead, &form.request),
        &form.csrf,
    ) {
        return refused(error);
    }
    let Ok(stage) = serde_json::from_value::<Stage>(json!(form.stage)) else {
        return refused(SessionError::InvalidRequest);
    };
    match context
        .sales
        .stage(
            &context.viewer,
            &id,
            &lead,
            &form.request,
            form.revision,
            stage,
        )
        .await
    {
        Ok(_) => protect(
            Redirect::to(&format!("{PAGE}/{}/leads/{}", escape(&id), escape(&lead)))
                .into_response(),
        ),
        Err(error) => answer(error),
    }
}

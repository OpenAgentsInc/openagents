//! Team membership, scoped policies, limits, and reports (WEB-12).
//!
//! Every page here reads or changes the existing native owners through the
//! viewer's own current session and the one selected workspace: the account
//! owner's membership, invitation, role, removal, and recovery routes; the
//! gateway's exact team policy; its cumulative monetary caps and holds; and
//! its private team work report and access history. Nothing is re-derived
//! or stored here, and no workspace comes from the URL — a request can only
//! act on the selected workspace, so another workspace is never reachable.
//!
//! Each form is a CSRF ticket bound to the account, session, selected
//! workspace, and its membership epoch, plus the exact subject it changes.
//! Any accepted invitation, role change, or removal starts a new epoch, so a
//! form reviewed before it refuses instead of acting on a changed team.
//!
//! The lane is enabled only by an explicit browser qualification document;
//! a native, desktop, or mobile qualification cannot enable it. A page here
//! grants no spending, execution, publication, or host right.

use super::private::ProtectedFile;
use super::session::{CloudSession, SessionError, Viewer, native_error};
use super::{failure, protect, refused, service, ticket, workspace_shell};
use crate::App;
use crate::layout::escape;
use axum::Router;
use axum::body::Bytes;
use axum::extract::Form;
use axum::extract::State;
use axum::extract::rejection::FormRejection;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use futures_util::stream;
use receipts::team_policy::{Change, Terms};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::path::Path as FsPath;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SCHEMA: &str = "openagents.cloud.team-browser-qualification.v1";
const PAGE: &str = "/cloud/app/team";
const UNQUALIFIED: &str = "Browser team controls require their own browser qualification. A native, desktop, or mobile qualification does not enable them.";
const EPOCH_NOTE: &str = "Accepting an invitation, changing a role, removing a member, or transferring ownership starts a new membership epoch for this workspace. Every member's stored Claude credential belongs to the epoch it was added under, so it stays hidden after the change until that member adds it again under Settings. Open views, task observations, and forms from the earlier epoch stop and must be reopened.";
const MAX_INVITE_DAYS: u64 = 30;
#[cfg(not(test))]
const WATCH_INTERVAL: Duration = Duration::from_secs(5);
#[cfg(test)]
const WATCH_INTERVAL: Duration = Duration::from_millis(20);
const WATCH_READS: u32 = 120;

/// The separately qualified browser lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Lane {
    Membership,
    Recovery,
    Policy,
    Budgets,
    Reports,
}

impl Lane {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "membership" => Self::Membership,
            "recovery" => Self::Recovery,
            "policy" => Self::Policy,
            "budgets" => Self::Budgets,
            "reports" => Self::Reports,
            _ => return None,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declared {
    schema: String,
    /// Must be exactly `browser`; other surfaces qualify separately.
    surface: String,
    /// The exact public origin the browser qualification covers.
    origin: String,
    lanes: Vec<String>,
    /// The owner's qualification evidence reference.
    evidence: String,
}

/// The owner's explicit browser qualification for the team lanes.
pub struct Qualification {
    file: ProtectedFile,
    origin: String,
    lanes: BTreeSet<Lane>,
}

impl Qualification {
    pub fn load(path: &FsPath) -> Result<Self, String> {
        let (file, bytes) = ProtectedFile::open(path, 16 * 1024)?;
        let declared: Declared = serde_json::from_slice(&bytes)
            .map_err(|_| "The team browser qualification is malformed.")?;
        if declared.schema != SCHEMA {
            return Err("The team browser qualification has an unsupported schema.".into());
        }
        if declared.surface != "browser" {
            return Err("Only a browser qualification enables browser team controls; native, desktop, and mobile qualification cannot.".into());
        }
        if declared.evidence.is_empty()
            || declared.evidence.len() > 512
            || declared.evidence.chars().any(char::is_control)
            || declared.origin.is_empty()
            || declared.origin.len() > 512
        {
            return Err("The team browser qualification needs an origin and evidence.".into());
        }
        let mut lanes = BTreeSet::new();
        for lane in &declared.lanes {
            if !lanes.insert(Lane::parse(lane).ok_or("Unknown team browser lane.")?) {
                return Err("A team browser lane is listed twice.".into());
            }
        }
        if lanes.is_empty() {
            return Err("The team browser qualification enables no lane.".into());
        }
        Ok(Self {
            file,
            origin: declared.origin.trim_end_matches('/').into(),
            lanes,
        })
    }

    fn admits(&self, service: &CloudSession, lane: Lane) -> bool {
        self.file.check().is_ok() && self.origin == service.origin() && self.lanes.contains(&lane)
    }
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(index))
        .route("/cloud/app/team/watch", get(watch))
        .route("/cloud/app/team/invite", post(invite))
        .route("/cloud/app/team/withdraw", post(withdraw))
        .route("/cloud/app/team/role", post(role))
        .route("/cloud/app/team/remove", post(remove))
        .route("/cloud/app/team/recovery", post(recovery))
        .route("/cloud/app/team/policy", post(policy))
        .route("/cloud/app/team/budgets", post(budgets))
        .route("/cloud/app/team/reports", get(reports))
        .route("/cloud/app/team/export", get(export))
        .route(
            "/cloud/app/team/accept",
            get(accept_page).post(accept_submit),
        )
        .route("/cloud/recover", get(recover_page).post(recover_submit))
}

/// Whether the browser lane is qualified at all; each section checks its own lane.
pub(crate) fn available(app: &App) -> bool {
    lane(app, Lane::Membership)
}

pub(crate) fn lane(app: &App, lane: Lane) -> bool {
    match (&app.config.cloud_team, &app.config.cloud) {
        (Some(qualification), Some(service)) => qualification.admits(service, lane),
        _ => false,
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn admin(role: &str) -> bool {
    matches!(role, "owner" | "admin")
}

struct Context<'a> {
    app: &'a App,
    service: &'a CloudSession,
    viewer: Viewer,
    workspace: String,
    epoch: u64,
    role: String,
}

async fn context<'a>(
    app: &'a App,
    headers: &HeaderMap,
    needed: Lane,
) -> Result<Context<'a>, Response> {
    let service = service(app)?;
    let viewer = match service.authenticate(headers).await {
        Ok(value) => value,
        Err(SessionError::Unauthenticated) => {
            return Err(protect(Redirect::to("/cloud/sign-in").into_response()));
        }
        Err(error) => return Err(refused(error)),
    };
    if !lane(app, needed) {
        return Err(failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Team controls unavailable",
            UNQUALIFIED,
        ));
    }
    let Some(selected) = viewer.workspace.as_ref() else {
        return Err(failure(
            StatusCode::FORBIDDEN,
            "Select a workspace",
            "Team pages bind one selected workspace and its current membership.",
        ));
    };
    let (workspace, epoch, role) = (
        selected.id.clone(),
        selected.members_epoch,
        selected.role.clone(),
    );
    Ok(Context {
        app,
        service,
        viewer,
        workspace,
        epoch,
        role,
    })
}

impl Context<'_> {
    fn team(&self) -> jev::Team<'_> {
        self.viewer.client().account().team(&self.viewer.account_id)
    }

    fn csrf(&self, headers: &HeaderMap, scope: &str, target: &str) -> Result<String, Response> {
        self.service
            .csrf(headers, &self.viewer, scope, target)
            .map_err(refused)
    }

    fn verify(
        &self,
        headers: &HeaderMap,
        scope: &str,
        target: &str,
        token: &str,
    ) -> Result<(), Response> {
        self.service
            .verify_csrf(headers, Some(&self.viewer), scope, target, token)
            .map_err(refused)
    }

    /// A read finished under the membership it started with; otherwise
    /// its result is refused rather than shown as current.
    async fn still_current(&self, headers: &HeaderMap) -> Result<(), Response> {
        let now = self.service.authenticate(headers).await.map_err(refused)?;
        let same = now.account_id == self.viewer.account_id
            && now.workspace.as_ref().is_some_and(|w| {
                w.id == self.workspace && w.members_epoch == self.epoch && w.role == self.role
            });
        if !same {
            return Err(refused(SessionError::Conflict));
        }
        Ok(())
    }

    fn shell(&self, headers: &HeaderMap, content: &str) -> Response {
        workspace_shell(
            self.app,
            headers,
            self.service,
            &self.viewer,
            "team",
            Some(content),
            None,
        )
    }

    fn require_admin(&self) -> Result<(), Response> {
        if admin(&self.role) {
            Ok(())
        } else {
            Err(failure(
                StatusCode::FORBIDDEN,
                "Read-only membership",
                "Your current role in this workspace can read the team but cannot change it.",
            ))
        }
    }
}

fn hidden(name: &str, value: &str) -> String {
    format!(
        "<input type=\"hidden\" name=\"{name}\" value=\"{}\">",
        escape(value)
    )
}

fn subject(value: &str) -> Result<(), Response> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(refused(SessionError::InvalidRequest));
    }
    Ok(())
}

fn date(unix: u64) -> String {
    format!("{unix} (Unix seconds)")
}

// ---- Overview ----------------------------------------------------------------

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let context = match context(&app, &headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let roster = match context.team().members(&context.workspace).await {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    if roster.workspace.members_epoch != context.epoch || roster.role != context.role {
        return refused(SessionError::Conflict);
    }
    let mut content = format!(
        "<h2>Team</h2><p>Workspace <code>{}</code> · {} · your role <strong>{}</strong> · membership epoch <code>{}</code>{}</p><p class=\"dim\">Read under your current session for the selected workspace only. Membership grants no host, computer, execution, spending, or publication right; each of those keeps its own grant.</p>",
        escape(&context.workspace),
        escape(&roster.workspace.kind),
        escape(&context.role),
        context.epoch,
        roster
            .workspace
            .seats
            .map_or_else(String::new, |s| format!(" · {s} seats")),
    );
    content.push_str(&format!(
        "<section id=\"team-observation\" hx-ext=\"sse\" sse-connect=\"/cloud/app/team/watch\" sse-close=\"retire\"><div id=\"team-live\" sse-swap=\"refresh\" hx-swap=\"innerHTML\" aria-live=\"polite\"><p class=\"dim\">Watching current membership.</p></div><div id=\"team-roster\" sse-swap=\"retire\" hx-swap=\"innerHTML\">"
    ));
    let manage = admin(&context.role);
    content.push_str("<h3>Members</h3><table><tr><th>Account</th><th>Role</th><th>Status</th>");
    if manage {
        content.push_str("<th>Change</th>");
    }
    content.push_str("</tr>");
    for member in &roster.members {
        content.push_str(&format!(
            "<tr><td><code>{}</code>{}</td><td>{}</td><td>{}</td>",
            escape(&member.account),
            if member.account == context.viewer.account_id {
                " (you)"
            } else {
                ""
            },
            escape(&member.role),
            escape(&member.status)
        ));
        if manage {
            content.push_str("<td>");
            if member.status == "active"
                && member.role != "owner"
                && member.account != context.viewer.account_id
            {
                let next = if member.role == "admin" {
                    "member"
                } else {
                    "admin"
                };
                let target = format!("{}:{next}", member.account);
                let (role_csrf, remove_csrf) = match (
                    context.csrf(&headers, "team-role", &target),
                    context.csrf(&headers, "team-remove", &member.account),
                ) {
                    (Ok(a), Ok(b)) => (a, b),
                    (Err(r), _) | (_, Err(r)) => return r,
                };
                content.push_str(&format!(
                    "<form method=\"post\" action=\"/cloud/app/team/role\">{}{}{}<button type=\"submit\">Make {next}</button></form><form method=\"post\" action=\"/cloud/app/team/remove\">{}{}<button type=\"submit\">Remove</button></form>",
                    ticket(&role_csrf),
                    hidden("account", &member.account),
                    hidden("role", next),
                    ticket(&remove_csrf),
                    hidden("account", &member.account),
                ));
                if lane(context.app, Lane::Recovery) {
                    let csrf = match context.csrf(&headers, "team-recovery", &member.account) {
                        Ok(value) => value,
                        Err(response) => return response,
                    };
                    content.push_str(&format!(
                        "<form method=\"post\" action=\"/cloud/app/team/recovery\">{}{}<button type=\"submit\">Issue recovery token</button></form>",
                        ticket(&csrf),
                        hidden("account", &member.account),
                    ));
                }
            } else {
                content.push_str("<span class=\"dim\">No browser change</span>");
            }
            content.push_str("</td>");
        }
        content.push_str("</tr>");
    }
    content.push_str("</table>");
    content.push_str("<h3>Invitations</h3>");
    if roster.invitations.is_empty() {
        content.push_str("<p>No invitations are retained for this workspace.</p>");
    } else {
        content.push_str("<table><tr><th>Invitation</th><th>Role</th><th>Status</th><th>Invited by</th><th>Expires</th><th>Accepted by</th>");
        if manage {
            content.push_str("<th>Change</th>");
        }
        content.push_str("</tr>");
        let observed = now();
        for invitation in &roster.invitations {
            let role = match invitation.role {
                jev::TeamRole::Admin => "admin",
                jev::TeamRole::Member => "member",
            };
            let status = if invitation.status == "pending" && invitation.expires_unix <= observed {
                "expired"
            } else {
                invitation.status.as_str()
            };
            content.push_str(&format!(
                "<tr><td><code>{}</code></td><td>{role}</td><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td>",
                escape(&invitation.id),
                escape(status),
                escape(&invitation.invited_by),
                date(invitation.expires_unix),
                invitation
                    .accepted_by
                    .as_deref()
                    .map_or_else(|| "none".into(), |a| format!("<code>{}</code>", escape(a))),
            ));
            if manage {
                if status == "pending" {
                    let csrf = match context.csrf(&headers, "team-withdraw", &invitation.id) {
                        Ok(value) => value,
                        Err(response) => return response,
                    };
                    content.push_str(&format!(
                        "<td><form method=\"post\" action=\"/cloud/app/team/withdraw\">{}{}<button type=\"submit\">Withdraw</button></form></td>",
                        ticket(&csrf),
                        hidden("invitation", &invitation.id)
                    ));
                } else {
                    content.push_str("<td></td>");
                }
            }
            content.push_str("</tr>");
        }
        content.push_str("</table>");
    }
    if manage {
        let csrf = match context.csrf(&headers, "team-invite", "") {
            Ok(value) => value,
            Err(response) => return response,
        };
        content.push_str(&format!(
            "<section class=\"cloud-card\"><h3>Invite someone</h3><p>An invitation is a single-use, expiring token. It is shown once, after you create it; deliver it yourself. It joins only this workspace, at the reviewed role, and holds a seat until it is accepted, withdrawn, or expires.</p><form method=\"post\" action=\"/cloud/app/team/invite\">{}<label>Role <select name=\"role\"><option value=\"member\">member</option><option value=\"admin\">admin</option></select></label> <label>Expires after days <input name=\"days\" type=\"number\" min=\"1\" max=\"{MAX_INVITE_DAYS}\" value=\"7\" required></label> <button type=\"submit\">Create invitation</button></form></section>",
            ticket(&csrf)
        ));
    } else {
        content.push_str("<p class=\"dim\">Your role is read-only here: invitations, roles, removal, recovery, and policy changes need a current admin or owner.</p>");
    }
    content.push_str(&format!(
        "<p class=\"dim\">{EPOCH_NOTE}</p></div></section><p><a href=\"/cloud/app/team/accept\">Accept an invitation to another workspace</a></p>"
    ));

    if lane(context.app, Lane::Policy) {
        match policy_section(&context, &headers).await {
            Ok(section) => content.push_str(&section),
            Err(response) => return response,
        }
    }
    if lane(context.app, Lane::Budgets) {
        match budget_section(&context, &headers).await {
            Ok(section) => content.push_str(&section),
            Err(response) => return response,
        }
    }
    if lane(context.app, Lane::Reports) {
        content.push_str("<section class=\"cloud-card\"><h3>Reports</h3><p>Work, cost, wait, and outcome rows from the native team report, under your current read rights: an admin reads every member's work, a member reads their own. Acceptance is a pinned attributed review, not remote attestation.</p><p><a href=\"/cloud/app/team/reports\">Open team report</a> · <a href=\"/cloud/app/team/export\">Export report and access history</a></p></section>");
    }
    content.push_str("<section class=\"cloud-card\"><h3>Department knowledge</h3><p>A team shares only admitted documents, workflows, and evaluations through their own exact grants. Sharing them is not model training and not an enterprise certification; training is a separate service with its own terms.</p></section>");
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    context.shell(&headers, &content)
}

// ---- Roster watch -------------------------------------------------------------

struct Watch {
    app: App,
    headers: HeaderMap,
    account: String,
    workspace: String,
    epoch: u64,
    role: String,
    digest: Option<String>,
    reads: u32,
    first: bool,
    ended: bool,
}

/// The roster digest under unchanged standing, or `None` once the viewer's
/// account, workspace, role, or membership epoch changed or became unknown.
async fn watch_read(state: &Watch) -> Option<String> {
    let service = service(&state.app).ok()?;
    if !lane(&state.app, Lane::Membership) {
        return None;
    }
    let viewer = service.authenticate(&state.headers).await.ok()?;
    let selected = viewer.workspace.as_ref()?;
    if viewer.account_id != state.account
        || selected.id != state.workspace
        || selected.members_epoch != state.epoch
        || selected.role != state.role
    {
        return None;
    }
    let roster = viewer
        .client()
        .account()
        .team(&viewer.account_id)
        .members(&state.workspace)
        .await
        .ok()?;
    if roster.workspace.members_epoch != state.epoch || roster.role != state.role {
        return None;
    }
    let bytes = serde_json::to_vec(&roster).ok()?;
    Some(
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// Connected observers recheck native standing on every read and retire as
/// soon as the membership they started under no longer holds.
async fn watch(State(app): State<App>, headers: HeaderMap) -> Response {
    let context = match context(&app, &headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let state = Watch {
        account: context.viewer.account_id.clone(),
        workspace: context.workspace.clone(),
        epoch: context.epoch,
        role: context.role.clone(),
        app: app.clone(),
        headers,
        digest: None,
        reads: 0,
        first: true,
        ended: false,
    };
    let events = stream::unfold(state, |mut state| async move {
        if state.ended {
            return None;
        }
        if !state.first {
            tokio::time::sleep(WATCH_INTERVAL).await;
        }
        state.first = false;
        state.reads += 1;
        let event = match watch_read(&state).await {
            Some(digest) if state.reads <= WATCH_READS => {
                if state.digest.as_ref().is_some_and(|d| d != &digest) {
                    state.digest = Some(digest);
                    Event::default().event("refresh").data(
                        "<p>Invitations changed. Reopen this page to review the current list.</p>",
                    )
                } else {
                    state.digest = Some(digest);
                    Event::default().comment("membership standing checked")
                }
            }
            _ => {
                state.ended = true;
                Event::default().event("retire").data(
                    "<p>Observation stopped: your membership, role, or this workspace's membership epoch changed, or standing could not be checked. Reopen the team page to read the current membership.</p>",
                )
            }
        };
        Some((Ok::<_, Infallible>(event), state))
    });
    let mut response = protect(Sse::new(events).into_response());
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response
}

// ---- Membership changes -------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invite {
    role: String,
    days: u64,
    csrf: String,
}

async fn invite(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Invite>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = context.verify(&headers, "team-invite", "", &form.csrf) {
        return response;
    }
    if let Err(response) = context.require_admin() {
        return response;
    }
    let role = match form.role.as_str() {
        "member" => jev::TeamRole::Member,
        "admin" => jev::TeamRole::Admin,
        _ => return refused(SessionError::InvalidRequest),
    };
    if form.days == 0 || form.days > MAX_INVITE_DAYS {
        return refused(SessionError::InvalidRequest);
    }
    let grant = match context
        .team()
        .invite(&context.workspace, role, form.days * 86_400)
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    let content = format!(
        "<h2>Invitation created</h2><p>Invitation <code>{}</code> joins <code>{}</code> as <strong>{}</strong> and expires at {}.</p><p>Copy this single-use token now and deliver it yourself. It is not shown again and is not stored by this site.</p><p><code class=\"secret\">{}</code></p><p>The person signs in with their own account, opens Team, chooses <em>Accept an invitation</em>, and reviews this workspace and role before accepting. {EPOCH_NOTE}</p><p><a href=\"{PAGE}\">Back to team</a></p>",
        escape(&grant.invitation.id),
        escape(&context.workspace),
        escape(&form.role),
        date(grant.invitation.expires_unix),
        escape(grant.token.expose()),
    );
    context.shell(&headers, &content)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Withdraw {
    invitation: String,
    csrf: String,
}

async fn withdraw(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Withdraw>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = subject(&form.invitation)
        .and_then(|()| context.verify(&headers, "team-withdraw", &form.invitation, &form.csrf))
        .and_then(|()| context.require_admin())
    {
        return response;
    }
    if let Err(error) = context
        .team()
        .withdraw(&context.workspace, &form.invitation)
        .await
    {
        return refused(native_error(error));
    }
    context.shell(
        &headers,
        &format!(
            "<h2>Invitation withdrawn</h2><p>Invitation <code>{}</code> can no longer be accepted.</p><p><a href=\"{PAGE}\">Back to team</a></p>",
            escape(&form.invitation)
        ),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleChange {
    account: String,
    role: String,
    csrf: String,
}

async fn role(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RoleChange>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let target = format!("{}:{}", form.account, form.role);
    if let Err(response) = subject(&form.account)
        .and_then(|()| context.verify(&headers, "team-role", &target, &form.csrf))
        .and_then(|()| context.require_admin())
    {
        return response;
    }
    let role = match form.role.as_str() {
        "member" => jev::TeamRole::Member,
        "admin" => jev::TeamRole::Admin,
        _ => return refused(SessionError::InvalidRequest),
    };
    let member = match context
        .team()
        .role(&context.workspace, &form.account, role)
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    changed(
        &app,
        &headers,
        &format!(
            "<h2>Role changed</h2><p><code>{}</code> is now <strong>{}</strong> in <code>{}</code>.</p>",
            escape(&member.account),
            escape(&member.role),
            escape(&context.workspace)
        ),
    )
    .await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Removal {
    account: String,
    csrf: String,
}

async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Removal>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = subject(&form.account)
        .and_then(|()| context.verify(&headers, "team-remove", &form.account, &form.csrf))
        .and_then(|()| context.require_admin())
    {
        return response;
    }
    let member = match context
        .team()
        .remove(&context.workspace, &form.account)
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    changed(
        &app,
        &headers,
        &format!(
            "<h2>Member removed</h2><p><code>{}</code> no longer belongs to <code>{}</code>. Their sessions in this workspace, task observations, and team reads stop on their next standing check, including views already open. Their retained records and original receipts are unchanged.</p>",
            escape(&member.account),
            escape(&context.workspace)
        ),
    )
    .await
}

/// After a membership change the selected epoch is new: re-read standing
/// so the shell shows the current epoch, and explain what that changes.
async fn changed(app: &App, headers: &HeaderMap, message: &str) -> Response {
    let context = match context(app, headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    context.shell(
        headers,
        &format!(
            "{message}<p>The workspace is now at membership epoch <code>{}</code>.</p><p>{EPOCH_NOTE}</p><p><a href=\"{PAGE}\">Back to team</a></p>",
            context.epoch
        ),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryForm {
    account: String,
    csrf: String,
}

async fn recovery(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RecoveryForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match context(&app, &headers, Lane::Recovery).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(response) = subject(&form.account)
        .and_then(|()| context.verify(&headers, "team-recovery", &form.account, &form.csrf))
        .and_then(|()| context.require_admin())
    {
        return response;
    }
    let grant = match context
        .team()
        .recovery(&context.workspace, &form.account)
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    context.shell(
        &headers,
        &format!(
            "<h2>Recovery token issued</h2><p>This single-use token lets <code>{}</code> replace their account key. It expires at {}. Copy it now and deliver it yourself; it is not shown again and is not stored by this site.</p><p><code class=\"secret\">{}</code></p><p>They redeem it at <a href=\"/cloud/recover\">Recover account access</a>, which ends every session of the old key and shows the replacement key once. Recovery restores account access only: it never restores a removed membership, and it changes no task, policy, budget, or receipt.</p><p><a href=\"{PAGE}\">Back to team</a></p>",
            escape(&grant.account),
            date(grant.expires_at),
            escape(grant.token.expose())
        ),
    )
}

// ---- Accept an invitation ---------------------------------------------------

async fn accept_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(SessionError::Unauthenticated) => {
            return protect(Redirect::to("/cloud/sign-in").into_response());
        }
        Err(error) => return refused(error),
    };
    if !lane(&app, Lane::Membership) {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Team controls unavailable",
            UNQUALIFIED,
        );
    }
    let csrf = match service.csrf(&headers, &viewer, "team-accept", "") {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let content = format!(
        "<h2>Accept an invitation</h2><p>Review the exact workspace and role the inviter gave you. The invitation is consumed once, only by your current account, and only for that workspace and role; a mismatch refuses without using it.</p><form method=\"post\" action=\"/cloud/app/team/accept\">{}<label>Workspace <input name=\"workspace\" required maxlength=\"128\" autocomplete=\"off\"></label> <label>Role <select name=\"role\"><option value=\"member\">member</option><option value=\"admin\">admin</option></select></label> <label>Invitation token <input type=\"password\" name=\"token\" required maxlength=\"256\" autocomplete=\"off\"></label> <button type=\"submit\">Accept</button></form><p class=\"dim\">{EPOCH_NOTE}</p>",
        ticket(&csrf)
    );
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "team",
        Some(&content),
        None,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Accept {
    workspace: String,
    role: String,
    token: String,
    csrf: String,
}

async fn accept_submit(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Accept>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if !lane(&app, Lane::Membership) {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Team controls unavailable",
            UNQUALIFIED,
        );
    }
    if let Err(error) = service.verify_csrf(&headers, Some(&viewer), "team-accept", "", &form.csrf)
    {
        return refused(error);
    }
    if let Err(response) = subject(&form.workspace) {
        return response;
    }
    let role = match form.role.as_str() {
        "member" => jev::TeamRole::Member,
        "admin" => jev::TeamRole::Admin,
        _ => return refused(SessionError::InvalidRequest),
    };
    let token = jev::ApiKey::new(form.token);
    let member = match viewer
        .client()
        .account()
        .team(&viewer.account_id)
        .accept_reviewed(&form.workspace, role, &token)
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    let content = format!(
        "<h2>Invitation accepted</h2><p>You joined <code>{}</code> as <strong>{}</strong>. Choose it under <em>Choose workspace</em> to open it. {EPOCH_NOTE}</p>",
        escape(&form.workspace),
        escape(&member.role)
    );
    // Re-read standing so the new workspace appears in the switcher.
    let viewer = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "team",
        Some(&content),
        None,
    )
}

// ---- Account recovery (signed out) ----------------------------------------------

async fn recover_page(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !lane(&app, Lane::Recovery) {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Recovery unavailable",
            UNQUALIFIED,
        );
    }
    let csrf = match service.login_csrf(&headers, "recover", "") {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let mut response = super::page(&format!(
        "<h1>Recover account access</h1><p>Enter the single-use recovery token a workspace admin gave you. Redeeming it replaces your account key and ends every session of the old one. The replacement key is shown once. Recovery restores account access only; it does not restore a removed membership.</p><form method=\"post\" action=\"/cloud/recover\">{}<label>Recovery token <input type=\"password\" name=\"token\" required maxlength=\"256\" autocomplete=\"off\"></label> <button type=\"submit\">Recover</button></form><p><a href=\"/cloud/sign-in\">Sign in</a></p>",
        ticket(&csrf.token)
    ));
    for cookie in csrf.legacy_cookies {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    if let Some(cookie) = csrf.cookie {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recover {
    token: String,
    csrf: String,
}

async fn recover_submit(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Recover>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    if !lane(&app, Lane::Recovery) {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Recovery unavailable",
            UNQUALIFIED,
        );
    }
    if let Err(error) = service.verify_csrf(&headers, None, "recover", "", &form.csrf) {
        return refused(error);
    }
    let recovered = match service.recover(&form.token).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let mut response = super::page(&format!(
        "<h1>Account key replaced</h1><p>{}Copy this replacement account key now. It is shown once and is not stored by this site; every session of the old key has ended.</p><p><code class=\"secret\">{}</code></p><p><a href=\"/cloud/sign-in\">Sign in with the new key</a></p>",
        recovered
            .account
            .as_deref()
            .map_or_else(String::new, |a| format!(
                "Account <code>{}</code>. ",
                escape(a)
            )),
        escape(recovered.key.expose())
    ));
    for cookie in service.clear_cookies() {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

// ---- Policy -------------------------------------------------------------------

async fn policy_section(context: &Context<'_>, headers: &HeaderMap) -> Result<String, Response> {
    let view = match context
        .viewer
        .client()
        .account()
        .team_policy(&context.viewer.account_id, &context.workspace)
        .await
    {
        Ok(value) => value,
        Err(error) => {
            return Ok(match native_error(error) {
                SessionError::Unauthenticated => return Err(refused(SessionError::Unauthenticated)),
                _ => "<section class=\"cloud-card\"><h3>Policy</h3><p>No current team policy is readable for this workspace and membership. Without one, nothing is narrowed here and nothing is enabled by this page.</p></section>".into(),
            });
        }
    };
    let reference = &view.reference;
    let mut out = format!(
        "<section class=\"cloud-card\"><h3>Policy</h3><p>Exact team policy version <code>{}</code>, digest <code>{}</code>, expires at {}, owner <code>{}</code>, reviewer <code>{}</code>. Rules narrow native grants; they never add one.</p><p>Enabled here: {}. Unsupported: {}.</p>",
        reference.version,
        escape(&reference.digest),
        date(reference.expires_unix),
        escape(&reference.owner),
        escape(&reference.reviewer),
        list(&view.enabled),
        list(&view.unsupported),
    );
    match &view.reviewed {
        None => {
            out.push_str("<p class=\"dim\">Rule details are visible to admins and the owner.</p>")
        }
        Some(reviewed) => {
            let narrow = admin(&context.role);
            let csrf = if narrow {
                Some(context.csrf(headers, "team-policy", &reviewed.digest)?)
            } else {
                None
            };
            if let Some(csrf) = &csrf {
                out.push_str(&format!(
                    "<form method=\"post\" action=\"/cloud/app/team/policy\">{}{}",
                    ticket(csrf),
                    hidden("expected", &reviewed.digest)
                ));
            }
            out.push_str("<table><tr><th>Keep</th><th>Capability</th><th>Model or plugin</th><th>Placement</th><th>Data classes</th><th>Recipients</th></tr>");
            for (index, rule) in reviewed.terms.rules.iter().enumerate() {
                let effect = &rule.effect;
                let scope = effect.model.clone().unwrap_or_else(|| {
                    effect
                        .plugin
                        .as_ref()
                        .map_or_else(String::new, |p| format!("{} {}", p.release, p.module))
                });
                out.push_str(&format!(
                    "<tr><td>{}</td><td>{}</td><td><code>{}</code></td><td>{} <code>{}</code></td><td>{}</td><td>{}</td></tr>",
                    if narrow {
                        format!("<input type=\"checkbox\" name=\"keep\" value=\"{index}\" checked>")
                    } else {
                        String::new()
                    },
                    escape(&serde_json::to_string(&effect.capability).unwrap_or_default()),
                    escape(&scope),
                    escape(&serde_json::to_string(&effect.placement.kind).unwrap_or_default()),
                    escape(&effect.placement.identity),
                    escape(&rule.data_classes.join(", ")),
                    effect.recipients.len()
                ));
            }
            out.push_str("</table>");
            if narrow {
                out.push_str(&format!(
                    "<p>Narrow this policy: clear rules to drop them, or bring the expiry earlier. A browser change can only narrow; it cannot add a rule or extend the expiry.</p><label>Expires at (Unix seconds) <input name=\"expires\" type=\"number\" min=\"1\" max=\"{}\" value=\"{}\" required></label> <button type=\"submit\">Review narrowed policy</button></form>",
                    reviewed.terms.expires_unix, reviewed.terms.expires_unix
                ));
            }
        }
    }
    out.push_str("</section>");
    Ok(out)
}

fn list(values: &[String]) -> String {
    if values.is_empty() {
        "none".into()
    } else {
        values
            .iter()
            .map(|v| format!("<code>{}</code>", escape(v)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Bounded `application/x-www-form-urlencoded` pairs with a fixed key set.
fn pairs(body: &[u8], allowed: impl Fn(&str) -> bool) -> Result<Vec<(String, String)>, Response> {
    if body.len() > 8192 {
        return Err(refused(SessionError::InvalidRequest));
    }
    let pairs: Vec<(String, String)> = url::form_urlencoded::parse(body)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    if pairs.len() > 512 || pairs.iter().any(|(k, _)| !allowed(k)) {
        return Err(refused(SessionError::InvalidRequest));
    }
    Ok(pairs)
}

fn single<'a>(pairs: &'a [(String, String)], key: &str) -> Result<&'a str, Response> {
    let mut found = pairs.iter().filter(|(k, _)| k == key);
    match (found.next(), found.next()) {
        (Some((_, v)), None) => Ok(v),
        _ => Err(refused(SessionError::InvalidRequest)),
    }
}

/// The narrowed terms: a subset of the reviewed rules, an expiry no later
/// than the reviewed one, and the next version.
pub(crate) fn narrowed(current: &Terms, keep: &BTreeSet<usize>, expires: u64) -> Option<Terms> {
    if keep.iter().any(|i| *i >= current.rules.len())
        || expires == 0
        || expires > current.expires_unix
        || expires <= now()
    {
        return None;
    }
    let terms = Terms {
        version: current.version.checked_add(1)?,
        expires_unix: expires,
        rules: current
            .rules
            .iter()
            .enumerate()
            .filter(|(i, _)| keep.contains(i))
            .map(|(_, r)| r.clone())
            .collect(),
    };
    (terms.narrows(current) && terms.validate().is_ok()).then_some(terms)
}

async fn policy(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let pairs = match pairs(&body, |k| {
        matches!(k, "csrf" | "expected" | "expires" | "keep")
    }) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let context = match context(&app, &headers, Lane::Policy).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (csrf, expected, expires) = match (
        single(&pairs, "csrf"),
        single(&pairs, "expected"),
        single(&pairs, "expires"),
    ) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        _ => return refused(SessionError::InvalidRequest),
    };
    if let Err(response) = context
        .verify(&headers, "team-policy", expected, csrf)
        .and_then(|()| context.require_admin())
    {
        return response;
    }
    let Ok(expires) = expires.parse::<u64>() else {
        return refused(SessionError::InvalidRequest);
    };
    let mut keep = BTreeSet::new();
    for (key, value) in &pairs {
        if key == "keep" {
            match value.parse::<usize>() {
                Ok(index) if keep.insert(index) => {}
                _ => return refused(SessionError::InvalidRequest),
            }
        }
    }
    let account = context.viewer.account_id.clone();
    let current = match context
        .viewer
        .client()
        .account()
        .team_policy(&account, &context.workspace)
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    let Some(reviewed) = current.reviewed.filter(|r| r.digest == expected) else {
        return refused(SessionError::Conflict);
    };
    let Some(terms) = narrowed(&reviewed.terms, &keep, expires) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "Policy change refused",
            "A browser change can only drop rules or bring the expiry earlier, and the expiry must be in the future.",
        );
    };
    let change = Change {
        expected_digest: Some(reviewed.digest.clone()),
        terms,
    };
    let view = match context
        .viewer
        .client()
        .account()
        .review_team_policy(&account, &context.workspace, &change)
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    context.shell(
        &headers,
        &format!(
            "<h2>Policy narrowed</h2><p>Version <code>{}</code>, digest <code>{}</code>, expires at {}. Requests admitted earlier keep the policy they were admitted under; new requests use this one.</p><p><a href=\"{PAGE}\">Back to team</a></p>",
            view.reference.version,
            escape(&view.reference.digest),
            date(view.reference.expires_unix)
        ),
    )
}

// ---- Budgets ------------------------------------------------------------------

fn position(label: &str, value: &Value) -> String {
    let n = |k: &str| {
        value[k]
            .as_u64()
            .map_or("unknown".into(), |v| v.to_string())
    };
    format!(
        "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
        label,
        n("cap"),
        n("alert_at"),
        n("reserved"),
        n("unknown"),
        n("settled_net"),
        n("used"),
        n("remaining"),
        escape(value["alert"].as_str().unwrap_or("unknown"))
    )
}

async fn budget_section(context: &Context<'_>, headers: &HeaderMap) -> Result<String, Response> {
    let answer = match context.team().budgets(&context.workspace).await {
        Ok(value) => value,
        Err(error) => {
            return match native_error(error) {
                SessionError::Unauthenticated => Err(refused(SessionError::Unauthenticated)),
                _ => Ok("<section class=\"cloud-card\"><h3>Limits</h3><p>No reviewed budget roster is readable for this workspace and membership. Without one, this page shows no limit and enables no spending.</p></section>".into()),
            };
        }
    };
    let budget = &answer["budget"];
    let mut out = format!(
        "<section class=\"cloud-card\"><h3>Limits</h3><p>Budget version <code>{}</code>, policy <code>{}</code>, scope {}, in {} millionths. Caps are cumulative; reserved amounts are holds still in flight, and unknown amounts are holds whose outcome is not yet settled. Both count against every cap until resolved, so concurrent holds cannot exceed a cap together. Unattributed earlier use: {}.</p><table><tr><th>Scope</th><th>Cap</th><th>Alert at</th><th>Reserved (holds)</th><th>Unknown</th><th>Settled</th><th>Used</th><th>Remaining</th><th>Alert</th></tr>",
        budget["version"].as_u64().unwrap_or(0),
        escape(budget["policy"].as_str().unwrap_or("")),
        escape(budget["scope"].as_str().unwrap_or("")),
        escape(budget["currency"].as_str().unwrap_or("")),
        budget["unattributed_used"]
            .as_u64()
            .map_or("unknown".into(), |v| v.to_string()),
    );
    out.push_str(&position("Workspace", &budget["workspace"]));
    for (key, label) in [("teams", "Team"), ("people", "Person")] {
        if let Some(map) = budget[key].as_object() {
            for (name, value) in map {
                out.push_str(&position(
                    &format!("{label} <code>{}</code>", escape(name)),
                    value,
                ));
            }
        }
    }
    out.push_str("</table>");
    if let Some(limits) = answer["limitations"].as_array() {
        out.push_str("<ul class=\"dim\">");
        for limit in limits.iter().filter_map(Value::as_str) {
            out.push_str(&format!("<li>{}</li>", escape(limit)));
        }
        out.push_str("</ul>");
    }
    let document = &answer["policy_document"];
    if context.role == "owner" && document.is_object() {
        let digest = budget["policy"].as_str().unwrap_or("");
        let csrf = context.csrf(headers, "team-budgets", digest)?;
        out.push_str(&format!(
            "<form method=\"post\" action=\"/cloud/app/team/budgets\">{}{}<p>Lower caps or alert thresholds. A browser change can only narrow limits for the same roster; it never resets earlier use.</p>",
            ticket(&csrf),
            hidden("expected", digest)
        ));
        let field = |name: &str, label: &str, limit: &Value| {
            format!(
                "<p>{label}: <label>cap <input type=\"number\" name=\"{name}.cap\" min=\"0\" max=\"{0}\" value=\"{0}\"></label> <label>alert at <input type=\"number\" name=\"{name}.alert\" min=\"0\" max=\"{1}\" value=\"{1}\"></label></p>",
                limit["cap"].as_u64().unwrap_or(0),
                limit["alert_at"].as_u64().unwrap_or(0),
            )
        };
        out.push_str(&field("workspace", "Workspace", &document["workspace"]));
        if let Some(teams) = document["teams"].as_object() {
            for (name, limit) in teams {
                out.push_str(&field(
                    &format!("team.{}", escape(name)),
                    &format!("Team <code>{}</code>", escape(name)),
                    limit,
                ));
            }
        }
        if let Some(people) = document["people"].as_object() {
            for (name, person) in people {
                out.push_str(&field(
                    &format!("person.{}", escape(name)),
                    &format!("Person <code>{}</code>", escape(name)),
                    &person["limit"],
                ));
            }
        }
        out.push_str("<button type=\"submit\">Review narrowed limits</button></form>");
    }
    out.push_str("</section>");
    Ok(out)
}

/// Lower each cap and threshold in place. Anything larger than the
/// reviewed value, an alert above its cap, or an unknown name refuses.
pub(crate) fn narrowed_budget(
    current: &Value,
    inputs: &BTreeMap<String, u64>,
) -> Result<Value, &'static str> {
    let mut next = current.clone();
    let mut seen = 0;
    fn apply(
        limit: &mut Value,
        name: &str,
        inputs: &BTreeMap<String, u64>,
        seen: &mut usize,
    ) -> Result<(), &'static str> {
        let cap = limit["cap"].as_u64().ok_or("malformed")?;
        let alert = limit["alert_at"].as_u64().ok_or("malformed")?;
        let new_cap = inputs.get(&format!("{name}.cap")).copied().unwrap_or(cap);
        let new_alert = inputs
            .get(&format!("{name}.alert"))
            .copied()
            .unwrap_or(alert);
        *seen += usize::from(inputs.contains_key(&format!("{name}.cap")))
            + usize::from(inputs.contains_key(&format!("{name}.alert")));
        if new_cap > cap || new_alert > alert || new_alert > new_cap {
            return Err(
                "A browser change can only lower caps and thresholds, and an alert cannot exceed its cap.",
            );
        }
        limit["cap"] = json!(new_cap);
        limit["alert_at"] = json!(new_alert);
        Ok(())
    }
    apply(&mut next["workspace"], "workspace", inputs, &mut seen)?;
    if let Some(teams) = next["teams"].as_object_mut() {
        for (name, limit) in teams.iter_mut() {
            apply(limit, &format!("team.{name}"), inputs, &mut seen)?;
        }
    }
    if let Some(people) = next["people"].as_object_mut() {
        for (name, person) in people.iter_mut() {
            apply(
                &mut person["limit"],
                &format!("person.{name}"),
                inputs,
                &mut seen,
            )?;
        }
    }
    if seen != inputs.len() {
        return Err("The limits named a scope outside the reviewed roster.");
    }
    let version = current["version"].as_u64().ok_or("malformed")?;
    next["version"] = json!(version.checked_add(1).ok_or("malformed")?);
    Ok(next)
}

async fn budgets(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let pairs = match pairs(&body, |k| {
        matches!(k, "csrf" | "expected") || k.ends_with(".cap") || k.ends_with(".alert")
    }) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let context = match context(&app, &headers, Lane::Budgets).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let (csrf, expected) = match (single(&pairs, "csrf"), single(&pairs, "expected")) {
        (Ok(a), Ok(b)) => (a.to_string(), b.to_string()),
        _ => return refused(SessionError::InvalidRequest),
    };
    if let Err(response) = context.verify(&headers, "team-budgets", &expected, &csrf) {
        return response;
    }
    if context.role != "owner" {
        return failure(
            StatusCode::FORBIDDEN,
            "Read-only limits",
            "Only the current workspace owner can change limits.",
        );
    }
    let mut inputs = BTreeMap::new();
    for (key, value) in &pairs {
        if key.ends_with(".cap") || key.ends_with(".alert") {
            let Ok(amount) = value.parse::<u64>() else {
                return refused(SessionError::InvalidRequest);
            };
            if inputs.insert(key.clone(), amount).is_some() {
                return refused(SessionError::InvalidRequest);
            }
        }
    }
    let team = context.team();
    let current = match team.budgets(&context.workspace).await {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    if current["budget"]["policy"].as_str() != Some(expected.as_str())
        || !current["policy_document"].is_object()
    {
        return refused(SessionError::Conflict);
    }
    let next = match narrowed_budget(&current["policy_document"], &inputs) {
        Ok(value) => value,
        Err(message) => return failure(StatusCode::BAD_REQUEST, "Limits refused", message),
    };
    // One request identity per reviewed policy and narrowed document, so a
    // repeated submission names the same native change.
    let request: String = Sha256::digest(format!("{expected}\0{}", next).as_bytes())
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect();
    let answer = match team
        .change_budgets(
            &context.workspace,
            &format!("web-{request}"),
            Some(&expected),
            &next,
        )
        .await
    {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    context.shell(
        &headers,
        &format!(
            "<h2>Limits narrowed</h2><p>Budget version <code>{}</code>, policy <code>{}</code>. Earlier use and open holds still count; nothing was reset.</p><p><a href=\"{PAGE}\">Back to team</a></p>",
            answer["budget"]["version"].as_u64().unwrap_or(0),
            escape(answer["budget"]["policy"].as_str().unwrap_or(""))
        ),
    )
}

// ---- Reports and export -----------------------------------------------------------

fn amount(value: &Value) -> String {
    value
        .as_u64()
        .map_or_else(|| "unknown".into(), |v| v.to_string())
}

async fn reports(State(app): State<App>, headers: HeaderMap) -> Response {
    let context = match context(&app, &headers, Lane::Reports).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let report = match context.team().report(&context.workspace).await {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let mut content = format!(
        "<h2>Team report</h2><p>Scope <strong>{}</strong> for <code>{}</code>, account revision <code>{}</code>, statement <code>{}</code>. Amounts use <code>{}</code>. A missing charge, wait, or cost stays unknown. Acceptance is a pinned attributed review, not remote attestation.</p><table><tr><th>Task</th><th>Member</th><th>State</th><th>Hold</th><th>Reserved</th><th>Charged</th><th>Refunded</th><th>Wait (ms)</th><th>Provider cost</th><th>Hosting cost</th><th>Evidence</th><th>Policy</th><th>Budget</th><th>Receipt</th></tr>",
        escape(report["scope"].as_str().unwrap_or("")),
        escape(&context.workspace),
        escape(report["account_revision"].as_str().unwrap_or("")),
        escape(report["statement_reference"].as_str().unwrap_or("")),
        escape(&report["unit"].to_string()),
    );
    for row in report["rows"].as_array().into_iter().flatten() {
        let text = |k: &str| escape(row[k].as_str().unwrap_or("none"));
        content.push_str(&format!(
            "<tr><td><code>{}</code></td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}{}</td><td><code>{}</code></td><td><code>{}</code></td><td><code>{}</code></td></tr>",
            text("task"),
            escape(row["original_member"]["account"].as_str().unwrap_or("unattributed")),
            text("state"),
            escape(&row["hold_phase"].to_string().trim_matches('"').to_string()),
            amount(&row["reserved"]),
            amount(&row["charged"]),
            amount(&row["refunded"]),
            amount(&row["wait_ms"]),
            amount(&row["provider_cost"]),
            amount(&row["hosting_cost"]),
            escape(row["evidence"]["status"].as_str().unwrap_or("unavailable")),
            if row["evidence"]["accepted"] == json!(true) { " · accepted" } else { "" },
            text("team_policy_reference"),
            text("budget_policy_reference"),
            text("receipt"),
        ));
    }
    let totals = &report["totals"];
    content.push_str(&format!(
        "</table><p>{} tasks · {} accepted · {} failed · {} delivered · {} known and {} unknown charges · {} refunded.{}</p><p>Production qualification: {}.</p><p><a href=\"/cloud/app/team/export\">Export report and access history</a> · <a href=\"{PAGE}\">Back to team</a></p>",
        amount(&totals["tasks"]),
        amount(&totals["accepted"]),
        amount(&totals["failed"]),
        amount(&totals["delivered"]),
        amount(&totals["known_charges"]),
        amount(&totals["unknown_charges"]),
        amount(&totals["refunded"]),
        if report["more"] == json!(true) {
            format!(
                " More rows exist beyond the bounded {} the native report returns.",
                amount(&report["maximum_rows"])
            )
        } else {
            String::new()
        },
        if report["production_qualification"] == json!(true) { "yes" } else { "no" },
    ));
    context.shell(&headers, &content)
}

/// One bounded private export: the native report and access history read
/// under the same current membership, refused if it changed meanwhile.
async fn export(
    State(app): State<App>,
    headers: HeaderMap,
    query: axum::extract::RawQuery,
) -> Response {
    if query.0.is_some_and(|q| !q.is_empty()) {
        return refused(SessionError::InvalidRequest);
    }
    let context = match context(&app, &headers, Lane::Reports).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let team = context.team();
    let report = match team.report(&context.workspace).await {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    let access = match team.access(&context.workspace).await {
        Ok(value) => value,
        Err(error) => return refused(native_error(error)),
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    let document = json!({
        "schema": "openagents.cloud.team-export.v1",
        "workspace": context.workspace,
        "account": context.viewer.account_id,
        "role": context.role,
        "members_epoch": context.epoch,
        "exported_at": now(),
        "report": report,
        "access": access,
        "disclosure": "Bounded to your current read rights: an admin reads every member's rows and access events, a member reads their own. Grants nothing.",
    });
    let mut response = protect(
        (
            [(header::CONTENT_TYPE, "application/json")],
            document.to_string(),
        )
            .into_response(),
    );
    if let Ok(value) = HeaderValue::from_str(&format!(
        "attachment; filename=\"team-{}.json\"",
        context.workspace
    )) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

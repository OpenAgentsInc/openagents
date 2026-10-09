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
use super::ui;
use super::{failure, protect, refused, service, workspace_shell};
use crate::App;
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
use maud::{Markup, Render, html};
use openagents_ui::forms::{Checkbox, Field, Input, InputType, Select};
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
const RETIRED: &str = "Observation stopped: your membership, role, or this workspace's membership epoch changed, or standing could not be checked. Reopen the team page to read the current membership.";

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

    fn shell(&self, headers: &HeaderMap, content: impl Render) -> Response {
        workspace_shell(
            self.app,
            headers,
            self.service,
            &self.viewer,
            "team",
            Some(content.render()),
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
    let manage = admin(&context.role);
    let mut members = ui::table("Members");
    members = if manage {
        members.header(["Account", "Role", "Status", "Change"])
    } else {
        members.header(["Account", "Role", "Status"])
    };
    for member in &roster.members {
        let mut cells = vec![
            html! {
                code { (member.account) }
                @if member.account == context.viewer.account_id { " (you)" }
            },
            html! { (member.role) },
            html! { (member.status) },
        ];
        if manage {
            let change = if member.status == "active"
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
                let recovery = if lane(context.app, Lane::Recovery) {
                    let csrf = match context.csrf(&headers, "team-recovery", &member.account) {
                        Ok(value) => value,
                        Err(response) => return response,
                    };
                    Some(
                        ui::BoundForm::new("/cloud/app/team/recovery")
                            .csrf(&csrf)
                            .bind("account", &member.account)
                            .submit_with(ui::submit("Issue recovery token", false)),
                    )
                } else {
                    None
                };
                html! {
                    (ui::BoundForm::new("/cloud/app/team/role")
                        .csrf(&role_csrf)
                        .bind("account", &member.account)
                        .bind("role", next)
                        .submit_with(ui::submit(&format!("Make {next}"), false)))
                    (ui::BoundForm::new("/cloud/app/team/remove")
                        .csrf(&remove_csrf)
                        .bind("account", &member.account)
                        .submit_with(ui::submit("Remove", false)))
                    @if let Some(form) = &recovery { (form) }
                }
            } else {
                html! { span class="cloud-note" { "No browser change" } }
            };
            cells.push(change);
        }
        members = members.row(cells);
    }

    let mut invitations = None;
    if !roster.invitations.is_empty() {
        let mut table = ui::table("Invitations");
        table = if manage {
            table.header([
                "Invitation",
                "Role",
                "Status",
                "Invited by",
                "Expires",
                "Accepted by",
                "Change",
            ])
        } else {
            table.header([
                "Invitation",
                "Role",
                "Status",
                "Invited by",
                "Expires",
                "Accepted by",
            ])
        };
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
            let mut cells = vec![
                html! { code { (invitation.id) } },
                html! { (role) },
                html! { (status) },
                html! { code { (invitation.invited_by) } },
                html! { (date(invitation.expires_unix)) },
                html! {
                    @match invitation.accepted_by.as_deref() {
                        Some(account) => { code { (account) } }
                        None => { "none" }
                    }
                },
            ];
            if manage {
                if status == "pending" {
                    let csrf = match context.csrf(&headers, "team-withdraw", &invitation.id) {
                        Ok(value) => value,
                        Err(response) => return response,
                    };
                    cells.push(
                        ui::BoundForm::new("/cloud/app/team/withdraw")
                            .csrf(&csrf)
                            .bind("invitation", &invitation.id)
                            .submit_with(ui::submit("Withdraw", false))
                            .render(),
                    );
                } else {
                    cells.push(html! {});
                }
            }
            table = table.row(cells);
        }
        invitations = Some(table);
    }

    let invite = if manage {
        let csrf = match context.csrf(&headers, "team-invite", "") {
            Ok(value) => value,
            Err(response) => return response,
        };
        let role = Field::new("team-invite-role", "Role");
        let role_aria = role.aria();
        let days = Field::new("team-invite-days", "Expires after days").required(true);
        let days_aria = days.aria();
        Some(ui::card(html! {
            h3 { "Invite someone" }
            p { "An invitation is a single-use, expiring token. It is shown once, after you create it; deliver it yourself. It joins only this workspace, at the reviewed role, and holds a seat until it is accepted, withdrawn, or expires." }
            (ui::BoundForm::new("/cloud/app/team/invite")
                .csrf(&csrf)
                .body(html! {
                    (role.control(
                        Select::new("role")
                            .aria(role_aria)
                            .option("member", "member")
                            .option("admin", "admin"),
                    ))
                    (days.control(
                        Input::new("days")
                            .aria(days_aria)
                            .input_type(InputType::Number)
                            .min("1")
                            .max(MAX_INVITE_DAYS.to_string())
                            .value("7"),
                    ))
                })
                .submit("Create invitation"))
        }))
    } else {
        None
    };

    let policy = if lane(context.app, Lane::Policy) {
        match policy_section(&context, &headers).await {
            Ok(section) => Some(section),
            Err(response) => return response,
        }
    } else {
        None
    };
    let budgets = if lane(context.app, Lane::Budgets) {
        match budget_section(&context, &headers).await {
            Ok(section) => Some(section),
            Err(response) => return response,
        }
    } else {
        None
    };
    let content = html! {
        h2 { "Team" }
        p {
            "Workspace " code { (context.workspace) } " \u{b7} " (roster.workspace.kind)
            " \u{b7} your role " strong { (context.role) }
            " \u{b7} membership epoch " code { (context.epoch) }
            @if let Some(seats) = roster.workspace.seats { " \u{b7} " (seats) " seats" }
        }
        p class="cloud-note" { "Read under your current session for the selected workspace only. Membership grants no host, computer, execution, spending, or publication right; each of those keeps its own grant." }
        section id="team-observation" hx-ext="sse" sse-connect="/cloud/app/team/watch" sse-close="retire" {
            div id="team-live" sse-swap="refresh" hx-swap="innerHTML" aria-live="polite" {
                p class="cloud-note" { "Watching current membership." }
            }
            div id="team-roster" sse-swap="retire" hx-swap="innerHTML" {
                h3 { "Members" }
                (members)
                h3 { "Invitations" }
                @match &invitations {
                    Some(table) => { (table) }
                    None => { p { "No invitations are retained for this workspace." } }
                }
                @match &invite {
                    Some(card) => { (card) }
                    None => {
                        p class="cloud-note" { "Your role is read-only here: invitations, roles, removal, recovery, and policy changes need a current admin or owner." }
                    }
                }
                p class="cloud-note" { (EPOCH_NOTE) }
            }
        }
        p { a href="/cloud/app/team/accept" { "Accept an invitation to another workspace" } }
        @if let Some(section) = &policy { (section) }
        @if let Some(section) = &budgets { (section) }
        @if lane(context.app, Lane::Reports) {
            (ui::card(html! {
                h3 { "Reports" }
                p { "Work, cost, wait, and outcome rows from the native team report, under your current read rights: an admin reads every member's work, a member reads their own. Acceptance is a pinned attributed review, not remote attestation." }
                (ui::links([
                    ("/cloud/app/team/reports", "Open team report"),
                    ("/cloud/app/team/export", "Export report and access history"),
                ]))
            }))
        }
        (ui::card(html! {
            h3 { "Department knowledge" }
            p { "A team shares only admitted documents, workflows, and evaluations through their own exact grants. Sharing them is not model training and not an enterprise certification; training is a separate service with its own terms." }
        }))
    };
    if let Err(response) = context.still_current(&headers).await {
        return response;
    }
    context.shell(&headers, content)
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
///
/// Each event carries the roster digest as its id. A browser reconnect after
/// a dropped connection or a suspended tab sends that id back, so a roster
/// change made while detached is announced instead of becoming the new
/// baseline unseen; a reconnect that can no longer be admitted retires.
async fn watch(State(app): State<App>, headers: HeaderMap) -> Response {
    let previous = match headers.get("last-event-id") {
        None => None,
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|value| value.strip_prefix("team-v1:"))
            .filter(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }) {
            Some(digest) => Some(digest.to_owned()),
            None => return super::retired_stream(RETIRED),
        },
    };
    let context = match context(&app, &headers, Lane::Membership).await {
        Ok(value) => value,
        Err(_) if super::reconnect(&headers) => return super::retired_stream(RETIRED),
        Err(response) => return response,
    };
    let state = Watch {
        account: context.viewer.account_id.clone(),
        workspace: context.workspace.clone(),
        epoch: context.epoch,
        role: context.role.clone(),
        app: app.clone(),
        headers,
        digest: previous,
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
                let id = format!("team-v1:{digest}");
                if state.digest.as_ref().is_some_and(|d| d != &digest) {
                    state.digest = Some(digest);
                    Event::default().event("refresh").id(id).data(
                        html! { p { "Invitations changed. Reopen this page to review the current list." } }
                            .into_string(),
                    )
                } else if state.digest.is_none() {
                    // The first read names the baseline a reconnect resumes from.
                    state.digest = Some(digest);
                    Event::default().event("standing").id(id).data("")
                } else {
                    Event::default().comment("membership standing checked")
                }
            }
            _ => {
                state.ended = true;
                Event::default()
                    .event("retire")
                    .data(html! { p { (RETIRED) } }.into_string())
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
    let content = html! {
        h2 { "Invitation created" }
        p {
            "Invitation " code { (grant.invitation.id) } " joins " code { (context.workspace) }
            " as " strong { (form.role) } " and expires at " (date(grant.invitation.expires_unix)) "."
        }
        p { "Copy this single-use token now and deliver it yourself. It is not shown again and is not stored by this site." }
        p { code class="secret" { (grant.token.expose()) } }
        p {
            "The person signs in with their own account, opens Team, chooses " em { "Accept an invitation" }
            ", and reviews this workspace and role before accepting. " (EPOCH_NOTE)
        }
        (back())
    };
    context.shell(&headers, content)
}

/// The "Back to team" link every result page ends with.
fn back() -> Markup {
    html! { p { a href=(PAGE) { "Back to team" } } }
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
        html! {
            h2 { "Invitation withdrawn" }
            p { "Invitation " code { (form.invitation) } " can no longer be accepted." }
            (back())
        },
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
        html! {
            h2 { "Role changed" }
            p {
                code { (member.account) } " is now " strong { (member.role) }
                " in " code { (context.workspace) } "."
            }
        },
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
        html! {
            h2 { "Member removed" }
            p {
                code { (member.account) } " no longer belongs to " code { (context.workspace) }
                ". Their sessions in this workspace, task observations, and team reads stop on their next standing check, including views already open. Their retained records and original receipts are unchanged."
            }
        },
    )
    .await
}

/// After a membership change the selected epoch is new: re-read standing
/// so the shell shows the current epoch, and explain what that changes.
async fn changed(app: &App, headers: &HeaderMap, message: Markup) -> Response {
    let context = match context(app, headers, Lane::Membership).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    context.shell(
        headers,
        html! {
            (message)
            p { "The workspace is now at membership epoch " code { (context.epoch) } "." }
            p { (EPOCH_NOTE) }
            (back())
        },
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
        html! {
            h2 { "Recovery token issued" }
            p {
                "This single-use token lets " code { (grant.account) }
                " replace their account key. It expires at " (date(grant.expires_at))
                ". Copy it now and deliver it yourself; it is not shown again and is not stored by this site."
            }
            p { code class="secret" { (grant.token.expose()) } }
            p {
                "They redeem it at " a href="/cloud/recover" { "Recover account access" }
                ", which ends every session of the old key and shows the replacement key once. Recovery restores account access only: it never restores a removed membership, and it changes no task, policy, budget, or receipt."
            }
            (back())
        },
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
    let workspace = Field::new("team-accept-workspace", "Workspace").required(true);
    let workspace_aria = workspace.aria();
    let role = Field::new("team-accept-role", "Role");
    let role_aria = role.aria();
    let token = Field::new("team-accept-token", "Invitation token").required(true);
    let token_aria = token.aria();
    let content = html! {
        h2 { "Accept an invitation" }
        p { "Review the exact workspace and role the inviter gave you. The invitation is consumed once, only by your current account, and only for that workspace and role; a mismatch refuses without using it." }
        (ui::BoundForm::new("/cloud/app/team/accept")
            .csrf(&csrf)
            .body(html! {
                (workspace.control(
                    Input::new("workspace")
                        .aria(workspace_aria)
                        .maxlength(128)
                        .autocomplete("off"),
                ))
                (role.control(
                    Select::new("role")
                        .aria(role_aria)
                        .option("member", "member")
                        .option("admin", "admin"),
                ))
                (token.control(
                    Input::new("token")
                        .aria(token_aria)
                        .input_type(InputType::Password)
                        .maxlength(256)
                        .autocomplete("off"),
                ))
            })
            .submit("Accept"))
        p class="cloud-note" { (EPOCH_NOTE) }
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "team",
        Some(content),
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
    let content = html! {
        h2 { "Invitation accepted" }
        p {
            "You joined " code { (form.workspace) } " as " strong { (member.role) }
            ". Choose it under " em { "Choose workspace" } " to open it. " (EPOCH_NOTE)
        }
    };
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
        Some(content),
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
    let token = Field::new("recover-token", "Recovery token").required(true);
    let token_aria = token.aria();
    let mut response = ui::document(
        &headers,
        html! {
            h1 { "Recover account access" }
            p { "Enter the single-use recovery token a workspace admin gave you. Redeeming it replaces your account key and ends every session of the old one. The replacement key is shown once. Recovery restores account access only; it does not restore a removed membership." }
            (ui::BoundForm::new("/cloud/recover")
                .csrf(&csrf.token)
                .body(token.control(
                    Input::new("token")
                        .aria(token_aria)
                        .input_type(InputType::Password)
                        .maxlength(256)
                        .autocomplete("off"),
                ))
                .submit("Recover"))
            p { a href="/cloud/sign-in" { "Sign in" } }
        },
    );
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
    let mut response = ui::document(
        &headers,
        html! {
            h1 { "Account key replaced" }
            p {
                @if let Some(account) = recovered.account.as_deref() {
                    "Account " code { (account) } ". "
                }
                "Copy this replacement account key now. It is shown once and is not stored by this site; every session of the old key has ended."
            }
            p { code class="secret" { (recovered.key.expose()) } }
            p { a href="/cloud/sign-in" { "Sign in with the new key" } }
        },
    );
    for cookie in service.clear_cookies() {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

// ---- Policy -------------------------------------------------------------------

async fn policy_section(context: &Context<'_>, headers: &HeaderMap) -> Result<Markup, Response> {
    let view = match context
        .viewer
        .client()
        .account()
        .team_policy(&context.viewer.account_id, &context.workspace)
        .await
    {
        Ok(value) => value,
        Err(error) => {
            return match native_error(error) {
                SessionError::Unauthenticated => Err(refused(SessionError::Unauthenticated)),
                _ => Ok(ui::card(html! {
                    h3 { "Policy" }
                    p { "No current team policy is readable for this workspace and membership. Without one, nothing is narrowed here and nothing is enabled by this page." }
                })),
            };
        }
    };
    let reference = &view.reference;
    let rules = match &view.reviewed {
        None => {
            html! { p class="cloud-note" { "Rule details are visible to admins and the owner." } }
        }
        Some(reviewed) => {
            let narrow = admin(&context.role);
            let mut table = ui::table("Policy rules").header([
                "Keep",
                "Capability",
                "Model or plugin",
                "Placement",
                "Data classes",
                "Recipients",
            ]);
            for (index, rule) in reviewed.terms.rules.iter().enumerate() {
                let effect = &rule.effect;
                let scope = effect.model.clone().unwrap_or_else(|| {
                    effect
                        .plugin
                        .as_ref()
                        .map_or_else(String::new, |p| format!("{} {}", p.release, p.module))
                });
                table = table.row([
                    html! {
                        @if narrow {
                            (Checkbox::new("keep", format!("Keep rule {}", index + 1))
                                .id(format!("team-policy-keep-{index}"))
                                .value(index.to_string())
                                .checked(true))
                        }
                    },
                    html! { (serde_json::to_string(&effect.capability).unwrap_or_default()) },
                    html! { code { (scope) } },
                    html! {
                        (serde_json::to_string(&effect.placement.kind).unwrap_or_default())
                        " " code { (effect.placement.identity) }
                    },
                    html! { (rule.data_classes.join(", ")) },
                    html! { (effect.recipients.len()) },
                ]);
            }
            if narrow {
                let csrf = context.csrf(headers, "team-policy", &reviewed.digest)?;
                let expires =
                    Field::new("team-policy-expires", "Expires at (Unix seconds)").required(true);
                let expires_aria = expires.aria();
                let limit = reviewed.terms.expires_unix.to_string();
                ui::BoundForm::new("/cloud/app/team/policy")
                    .csrf(&csrf)
                    .bind("expected", &reviewed.digest)
                    .body(html! {
                        (table)
                        p { "Narrow this policy: clear rules to drop them, or bring the expiry earlier. A browser change can only narrow; it cannot add a rule or extend the expiry." }
                        (expires.control(
                            Input::new("expires")
                                .aria(expires_aria)
                                .input_type(InputType::Number)
                                .min("1")
                                .max(limit.clone())
                                .value(limit),
                        ))
                    })
                    .submit("Review narrowed policy")
                    .render()
            } else {
                table.render()
            }
        }
    };
    Ok(ui::card(html! {
        h3 { "Policy" }
        p {
            "Exact team policy version " code { (reference.version) } ", digest " code { (reference.digest) }
            ", expires at " (date(reference.expires_unix)) ", owner " code { (reference.owner) }
            ", reviewer " code { (reference.reviewer) } ". Rules narrow native grants; they never add one."
        }
        p { "Enabled here: " (list(&view.enabled)) ". Unsupported: " (list(&view.unsupported)) "." }
        (rules)
    }))
}

fn list(values: &[String]) -> Markup {
    html! {
        @if values.is_empty() {
            "none"
        } @else {
            @for (index, value) in values.iter().enumerate() {
                @if index > 0 { ", " }
                code { (value) }
            }
        }
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
        html! {
            h2 { "Policy narrowed" }
            p {
                "Version " code { (view.reference.version) } ", digest " code { (view.reference.digest) }
                ", expires at " (date(view.reference.expires_unix))
                ". Requests admitted earlier keep the policy they were admitted under; new requests use this one."
            }
            (back())
        },
    )
}

// ---- Budgets ------------------------------------------------------------------

fn position(label: Markup, value: &Value) -> Vec<Markup> {
    let n = |k: &str| {
        let text = value[k]
            .as_u64()
            .map_or("unknown".into(), |v| v.to_string());
        html! { (text) }
    };
    vec![
        label,
        n("cap"),
        n("alert_at"),
        n("reserved"),
        n("unknown"),
        n("settled_net"),
        n("used"),
        n("remaining"),
        html! { (value["alert"].as_str().unwrap_or("unknown")) },
    ]
}

/// One scope's cap and alert inputs for the owner's narrowing form.
fn limit_inputs(name: &str, label: Markup, limit: &Value) -> Markup {
    let cap = limit["cap"].as_u64().unwrap_or(0).to_string();
    let alert = limit["alert_at"].as_u64().unwrap_or(0).to_string();
    let cap_field = Field::new(format!("team-budget-{name}.cap"), "Cap");
    let cap_aria = cap_field.aria();
    let alert_field = Field::new(format!("team-budget-{name}.alert"), "Alert at");
    let alert_aria = alert_field.aria();
    html! {
        fieldset {
            legend { (label) }
            (cap_field.control(
                Input::new(format!("{name}.cap"))
                    .aria(cap_aria)
                    .input_type(InputType::Number)
                    .min("0")
                    .max(cap.clone())
                    .value(cap),
            ))
            (alert_field.control(
                Input::new(format!("{name}.alert"))
                    .aria(alert_aria)
                    .input_type(InputType::Number)
                    .min("0")
                    .max(alert.clone())
                    .value(alert),
            ))
        }
    }
}

async fn budget_section(context: &Context<'_>, headers: &HeaderMap) -> Result<Markup, Response> {
    let answer = match context.team().budgets(&context.workspace).await {
        Ok(value) => value,
        Err(error) => {
            return match native_error(error) {
                SessionError::Unauthenticated => Err(refused(SessionError::Unauthenticated)),
                _ => Ok(ui::card(html! {
                    h3 { "Limits" }
                    p { "No reviewed budget roster is readable for this workspace and membership. Without one, this page shows no limit and enables no spending." }
                })),
            };
        }
    };
    let budget = &answer["budget"];
    let mut table = ui::table("Limits").header([
        "Scope",
        "Cap",
        "Alert at",
        "Reserved (holds)",
        "Unknown",
        "Settled",
        "Used",
        "Remaining",
        "Alert",
    ]);
    table = table.row(position(html! { "Workspace" }, &budget["workspace"]));
    for (key, label) in [("teams", "Team"), ("people", "Person")] {
        if let Some(map) = budget[key].as_object() {
            for (name, value) in map {
                table = table.row(position(html! { (label) " " code { (name) } }, value));
            }
        }
    }
    let document = &answer["policy_document"];
    let form = if context.role == "owner" && document.is_object() {
        let digest = budget["policy"].as_str().unwrap_or("");
        let csrf = context.csrf(headers, "team-budgets", digest)?;
        let mut inputs = vec![limit_inputs(
            "workspace",
            html! { "Workspace" },
            &document["workspace"],
        )];
        if let Some(teams) = document["teams"].as_object() {
            for (name, limit) in teams {
                inputs.push(limit_inputs(
                    &format!("team.{name}"),
                    html! { "Team " code { (name) } },
                    limit,
                ));
            }
        }
        if let Some(people) = document["people"].as_object() {
            for (name, person) in people {
                inputs.push(limit_inputs(
                    &format!("person.{name}"),
                    html! { "Person " code { (name) } },
                    &person["limit"],
                ));
            }
        }
        Some(
            ui::BoundForm::new("/cloud/app/team/budgets")
                .csrf(&csrf)
                .bind("expected", digest)
                .body(html! {
                    p { "Lower caps or alert thresholds. A browser change can only narrow limits for the same roster; it never resets earlier use." }
                    @for input in &inputs { (input) }
                })
                .submit("Review narrowed limits"),
        )
    } else {
        None
    };
    Ok(ui::card(html! {
        h3 { "Limits" }
        p {
            "Budget version " code { (budget["version"].as_u64().unwrap_or(0)) }
            ", policy " code { (budget["policy"].as_str().unwrap_or("")) }
            ", scope " (budget["scope"].as_str().unwrap_or(""))
            ", in " (budget["currency"].as_str().unwrap_or("")) " millionths. Caps are cumulative; reserved amounts are holds still in flight, and unknown amounts are holds whose outcome is not yet settled. Both count against every cap until resolved, so concurrent holds cannot exceed a cap together. Unattributed earlier use: "
            (budget["unattributed_used"].as_u64().map_or("unknown".into(), |v| v.to_string()))
            "."
        }
        (table)
        @if let Some(limits) = answer["limitations"].as_array() {
            ul class="cloud-note" {
                @for limit in limits.iter().filter_map(Value::as_str) {
                    li { (limit) }
                }
            }
        }
        @if let Some(form) = &form { (form) }
    }))
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
        html! {
            h2 { "Limits narrowed" }
            p {
                "Budget version " code { (answer["budget"]["version"].as_u64().unwrap_or(0)) }
                ", policy " code { (answer["budget"]["policy"].as_str().unwrap_or("")) }
                ". Earlier use and open holds still count; nothing was reset."
            }
            (back())
        },
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
    let mut table = ui::table("Team report").header([
        "Task",
        "Member",
        "State",
        "Hold",
        "Reserved",
        "Charged",
        "Refunded",
        "Wait (ms)",
        "Provider cost",
        "Hosting cost",
        "Evidence",
        "Policy",
        "Budget",
        "Receipt",
    ]);
    for row in report["rows"].as_array().into_iter().flatten() {
        let text = |k: &str| row[k].as_str().unwrap_or("none").to_owned();
        table = table.row([
            html! { code { (text("task")) } },
            html! { code { (row["original_member"]["account"].as_str().unwrap_or("unattributed")) } },
            html! { (text("state")) },
            html! { (row["hold_phase"].to_string().trim_matches('"')) },
            html! { (amount(&row["reserved"])) },
            html! { (amount(&row["charged"])) },
            html! { (amount(&row["refunded"])) },
            html! { (amount(&row["wait_ms"])) },
            html! { (amount(&row["provider_cost"])) },
            html! { (amount(&row["hosting_cost"])) },
            html! {
                (row["evidence"]["status"].as_str().unwrap_or("unavailable"))
                @if row["evidence"]["accepted"] == json!(true) { " \u{b7} accepted" }
            },
            html! { code { (text("team_policy_reference")) } },
            html! { code { (text("budget_policy_reference")) } },
            html! { code { (text("receipt")) } },
        ]);
    }
    let totals = &report["totals"];
    let content = html! {
        h2 { "Team report" }
        p {
            "Scope " strong { (report["scope"].as_str().unwrap_or("")) }
            " for " code { (context.workspace) }
            ", account revision " code { (report["account_revision"].as_str().unwrap_or("")) }
            ", statement " code { (report["statement_reference"].as_str().unwrap_or("")) }
            ". Amounts use " code { (report["unit"].to_string()) }
            ". A missing charge, wait, or cost stays unknown. Acceptance is a pinned attributed review, not remote attestation."
        }
        (table)
        p {
            (amount(&totals["tasks"])) " tasks \u{b7} "
            (amount(&totals["accepted"])) " accepted \u{b7} "
            (amount(&totals["failed"])) " failed \u{b7} "
            (amount(&totals["delivered"])) " delivered \u{b7} "
            (amount(&totals["known_charges"])) " known and "
            (amount(&totals["unknown_charges"])) " unknown charges \u{b7} "
            (amount(&totals["refunded"])) " refunded."
            @if report["more"] == json!(true) {
                " More rows exist beyond the bounded " (amount(&report["maximum_rows"]))
                " the native report returns."
            }
        }
        p {
            "Production qualification: "
            (if report["production_qualification"] == json!(true) { "yes" } else { "no" })
            "."
        }
        (ui::links([
            ("/cloud/app/team/export", "Export report and access history"),
            (PAGE, "Back to team"),
        ]))
    };
    context.shell(&headers, content)
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

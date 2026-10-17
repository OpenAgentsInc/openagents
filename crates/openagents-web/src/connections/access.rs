//! Who may see and use Connections while Google hasn't approved the app
//! (#11574).
//!
//! Google Drive's read-only scope is restricted: until Google verifies
//! the app, only the consent screen's test users can connect. So the whole
//! feature is shown only to accounts the owner allows: Settings,
//! Connections; a project's Sources; the connections, sources, and Drive
//! API; Coder's Drive tools (they ask `GET /v1/connections`); and the
//! chat's Drive answers. Everyone else gets the site's not-found page (or
//! `404 not_found` from the API), as for an address that doesn't exist.
//!
//! Site admins always may. Anyone else is on the access list
//! ([`crate::cloud::connections::Access`]) by account id or email, kept
//! sealed beside the connections, and changed by an admin at
//! [`PAGE`] or `/v1/admin/access/drive-connections`, so a change needs no
//! redeploy. Until an admin first changes it the list is
//! [`crate::cloud::connections::ACCESS_SEED`].
//!
//! A chat answered for an app's message has no sign-in at hand, only the
//! account; it is let in when the list names the account, or the account
//! passed a signed-in check by email or as an admin ([`knows`]).

use axum::Router;
use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get};
use maud::html;
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color, ControlSize};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::Input;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::account_owner;
use crate::cloud::connections::{Access, AccessEntry, Seen};
use crate::cloud::session::{CloudSession, SessionError, Viewer, now};
use crate::cloud::{protect, refused};
use crate::coder_sync::{answer, refused as api_refused};

/// The admin page.
pub(crate) const PAGE: &str = "/admin/access/drive-connections";
/// The admin API.
pub(crate) const API: &str = "/v1/admin/access/drive-connections";
const CSRF_SCOPE: &str = "drive-access";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(page).post(change))
        .route(API, get(api_list).post(api_add))
        .route(&format!("{API}/{{who}}"), delete(api_remove))
}

/// Whether `path` is one of these.
pub(crate) fn owns(path: &str) -> bool {
    path == PAGE || path == API || path.starts_with(&format!("{API}/"))
}

/// Whether `viewer` may use Connections. Keeps the list's record of who
/// passed by email or as an admin current.
pub(crate) fn permitted(app: &App, viewer: &Viewer) -> bool {
    let Some(store) = app.config.connections.as_deref() else {
        return false;
    };
    let Ok(access) = store.access() else {
        return viewer.admin;
    };
    let via = if viewer.admin {
        Some("admin".to_owned())
    } else {
        access.entry_for_account(&viewer.account_id, viewer.email.as_deref())
    };
    let recorded = access
        .seen
        .iter()
        .find(|s| s.account == viewer.account_id)
        .map(|s| s.via.clone());
    let wanted = via.clone().filter(|v| v != "account");
    if recorded != wanted {
        let account = viewer.account_id.clone();
        let _ = store.update_access(|access| {
            access.seen.retain(|s| s.account != account);
            if let Some(via) = wanted
                && access.seen.len() < crate::cloud::connections::MAX_ACCESS
            {
                access.seen.push(Seen { account, via });
            }
        });
    }
    via.is_some()
}

/// Whether the account behind the chat owner value `owner` may use
/// Connections, without its sign-in at hand.
pub(crate) fn knows(app: &App, owner: &str) -> bool {
    let Some(access) = app
        .config
        .connections
        .as_deref()
        .and_then(|store| store.access().ok())
    else {
        return false;
    };
    access
        .entries
        .iter()
        .filter_map(|e| e.account.as_deref())
        .chain(access.seen.iter().map(|s| s.account.as_str()))
        .any(|account| account_owner(account) == owner)
}

/// A page's signed-in viewer who may use Connections; anyone else gets
/// the not-found page (a signed-out visitor, the sign-in page first).
pub(crate) async fn page_viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
    path: &str,
) -> Result<(&'a CloudSession, Viewer), Response> {
    if app.config.connections.is_none() {
        return Err(crate::not_found_page());
    }
    let (service, viewer) = crate::settings::viewer(app, headers, path).await?;
    if permitted(app, &viewer) {
        Ok((service, viewer))
    } else {
        Err(crate::not_found_page())
    }
}

/// The app API's `404`, the same as for an address that doesn't exist.
pub(crate) fn api_not_found() -> Response {
    api_refused(StatusCode::NOT_FOUND, "not_found", "Not found.")
}

/// An app API caller (a sign-in or sign-in key) as a viewer: the account
/// service and the viewer, or the app API's refusal.
pub(crate) async fn api_signed_in<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, Viewer), Response> {
    let (_, token, _) = crate::projects::api::caller(app, headers).await?;
    crate::account_export::app_viewer(app, &token).await
}

/// An app API caller who may use Connections; anyone else gets `404`.
pub(crate) async fn api_viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, Viewer), Response> {
    if app.config.connections.is_none() {
        return Err(api_not_found());
    }
    let (service, viewer) = api_signed_in(app, headers).await?;
    if permitted(app, &viewer) {
        Ok((service, viewer))
    } else {
        Err(api_not_found())
    }
}

// The admin's list --------------------------------------------------------

fn entry_object(entry: &AccessEntry) -> Value {
    json!({
        "account": entry.account,
        "email": entry.email,
        "added_at": (entry.added_at > 0).then(|| crate::projects::api::rfc3339(entry.added_at)),
    })
}

fn list_object(access: &Access) -> Value {
    json!({
        "object": "access_list",
        "feature": "drive-connections",
        "admins": "always allowed",
        "data": access.entries.iter().map(entry_object).collect::<Vec<_>>(),
        "seen": access.seen.iter().map(|s| json!({"account": s.account, "via": s.via})).collect::<Vec<_>>(),
    })
}

/// What an admin adds: an email, or an account id.
fn parse_who(who: &str) -> Result<AccessEntry, &'static str> {
    let who = who.trim();
    if who.contains('@') {
        let valid = who.len() <= 254
            && who.split('@').count() == 2
            && who.split('@').all(|part| !part.is_empty())
            && !who.chars().any(|c| c.is_whitespace() || c.is_control());
        return if valid {
            Ok(AccessEntry {
                email: Some(who.to_lowercase()),
                added_at: now(),
                ..AccessEntry::default()
            })
        } else {
            Err("That isn't an email address.")
        };
    }
    if crate::cloud::session::identifier(who) {
        Ok(AccessEntry {
            account: Some(who.to_owned()),
            added_at: now(),
            ..AccessEntry::default()
        })
    } else {
        Err("Give an email address or an account id.")
    }
}

/// An admin viewer of the API; anyone else gets `404`.
async fn api_admin<'a>(app: &'a App, headers: &HeaderMap) -> Result<Viewer, Response> {
    if app.config.connections.is_none() {
        return Err(api_not_found());
    }
    let (_, viewer) = api_signed_in(app, headers).await?;
    if viewer.admin {
        Ok(viewer)
    } else {
        Err(api_not_found())
    }
}

fn unavailable() -> Response {
    api_refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "The list couldn't be read or changed. Try again later.",
    )
}

/// `GET /v1/admin/access/drive-connections`.
async fn api_list(State(app): State<App>, headers: HeaderMap) -> Response {
    if let Err(response) = api_admin(&app, &headers).await {
        return response;
    }
    match app.config.connections.as_deref().map(|s| s.access()) {
        Some(Ok(access)) => answer(StatusCode::OK, list_object(&access)),
        _ => unavailable(),
    }
}

#[derive(Deserialize)]
struct AddBody {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    account: Option<String>,
}

/// `POST /v1/admin/access/drive-connections` `{email}` or `{account}`.
async fn api_add(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<axum::Json<AddBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    if let Err(response) = api_admin(&app, &headers).await {
        return response;
    }
    let who = match body {
        Ok(axum::Json(AddBody {
            email: Some(who), ..
        }))
        | Ok(axum::Json(AddBody {
            account: Some(who), ..
        })) => who,
        _ => {
            return api_refused(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Send {\"email\": \"…\"} or {\"account\": \"…\"} as JSON.",
            );
        }
    };
    let entry = match parse_who(&who) {
        Ok(entry) => entry,
        Err(why) => return api_refused(StatusCode::BAD_REQUEST, "invalid_request", why),
    };
    let Some(store) = app.config.connections.as_deref() else {
        return unavailable();
    };
    match store.update_access(|access| access.add(entry).map(|added| (added, list_object(access))))
    {
        Ok(Ok((added, mut list))) => {
            list["added"] = json!(added);
            answer(
                if added {
                    StatusCode::CREATED
                } else {
                    StatusCode::OK
                },
                list,
            )
        }
        Ok(Err(why)) => api_refused(StatusCode::CONFLICT, "full", why),
        Err(_) => unavailable(),
    }
}

/// `DELETE /v1/admin/access/drive-connections/{email or account}`.
async fn api_remove(
    State(app): State<App>,
    headers: HeaderMap,
    Path(who): Path<String>,
) -> Response {
    if let Err(response) = api_admin(&app, &headers).await {
        return response;
    }
    let Some(store) = app.config.connections.as_deref() else {
        return unavailable();
    };
    match store.update_access(|access| (access.remove(&who), list_object(access))) {
        Ok((true, mut list)) => {
            list["removed"] = json!(true);
            answer(StatusCode::OK, list)
        }
        Ok((false, _)) => api_refused(
            StatusCode::NOT_FOUND,
            "not_found",
            "No one on the list matches that.",
        ),
        Err(_) => unavailable(),
    }
}

// The admin page ----------------------------------------------------------

async fn admin_page_viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
) -> Result<(&'a CloudSession, Viewer), Response> {
    if app.config.connections.is_none() {
        return Err(crate::not_found_page());
    }
    let (service, viewer) = crate::settings::viewer(app, headers, PAGE).await?;
    if viewer.admin {
        Ok((service, viewer))
    } else {
        Err(crate::not_found_page())
    }
}

async fn page(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match admin_page_viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let csrf = service
        .csrf(&headers, &viewer, CSRF_SCOPE, &viewer.account_id)
        .unwrap_or_default();
    let access = app
        .config
        .connections
        .as_deref()
        .and_then(|s| s.access().ok())
        .unwrap_or_default();
    let body = view(&access, &csrf);
    crate::settings::page(
        &headers,
        service,
        &viewer,
        "Google Drive access",
        PAGE,
        body,
    )
}

/// The admin page's body (separate from I/O for the screens gallery).
pub(crate) fn view(access: &Access, csrf: &str) -> maud::Markup {
    html! {
        div class="oa-settings" {
            h1 class="oa-heading" data-level="1" { "Google Drive access" }
            (MarkdownRoot::new(html! {
                p { "Until Google approves the app, only the people here and site admins see Connections and Drive sources. Add each person to the test users on " a href="https://console.cloud.google.com/auth/audience?project=openagentsgemini" rel="noopener" target="_blank" { "Google's Audience page" } " too." }
            }))
            section class="oa-settings-group" aria-labelledby="access-list" {
                h2 #access-list { "Allowed" }
                @if access.entries.is_empty() {
                    (MarkdownRoot::new(html! { p { "No one yet besides admins." } }))
                }
                @for entry in &access.entries {
                    @let who = entry.email.clone().or_else(|| entry.account.clone()).unwrap_or_default();
                    div class="oa-settings-row" {
                        div class="oa-settings-text" {
                            span class="oa-settings-label" { (who) }
                        }
                        div class="oa-settings-control" {
                            form method="post" action=(PAGE) {
                                input type="hidden" name="csrf" value=(csrf);
                                input type="hidden" name="remove" value=(who);
                                (Button::new("Remove")
                                    .kind(ButtonType::Submit)
                                    .size(ControlSize::Sm)
                                    .variant(ButtonVariant::Ghost)
                                    .color(Color::Secondary))
                            }
                        }
                    }
                }
            }
            form method="post" action=(PAGE) class="oa-page-actions" {
                input type="hidden" name="csrf" value=(csrf);
                (Input::new("add").placeholder("Email or account id").aria_label("Email or account id"))
                (Button::new("Add").kind(ButtonType::Submit))
            }
        }
    }
}

#[derive(Deserialize)]
struct ChangeForm {
    csrf: String,
    #[serde(default)]
    add: Option<String>,
    #[serde(default)]
    remove: Option<String>,
}

async fn change(
    State(app): State<App>,
    headers: HeaderMap,
    Form(form): Form<ChangeForm>,
) -> Response {
    let (service, viewer) = match admin_page_viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        CSRF_SCOPE,
        &viewer.account_id,
        &form.csrf,
    ) {
        return refused(error);
    }
    let Some(store) = app.config.connections.as_deref() else {
        return refused(SessionError::Unavailable);
    };
    let changed = match (form.add.as_deref(), form.remove.as_deref()) {
        (Some(add), _) if !add.trim().is_empty() => match parse_who(add) {
            Ok(entry) => store
                .update_access(|a| a.add(entry).map(|_| ()))
                .map_err(|_| "x"),
            Err(_) => Ok(Ok(())),
        },
        (_, Some(remove)) => store
            .update_access(|a| {
                a.remove(remove);
                Ok(())
            })
            .map_err(|_| "x"),
        _ => Ok(Ok(())),
    };
    match changed {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(_) => refused(SessionError::Unavailable),
    }
}

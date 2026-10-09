//! Visible Verse connections. World, computer, and private work are separate
//! connections: joining a world supplies no task, terminal, review, or sales
//! right, and the 3D view is optional for every other workspace page.

use super::hosts::Binding;
use super::private::ProtectedFile;
use super::session::{SessionError, Viewer, now};
use super::ui;
use super::{protect, refused, render, service, workspace_shell};
use crate::App;
use axum::extract::rejection::QueryRejection;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use coder_access::Right;
use coder_ui::workspace;
use maud::{Markup, html};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::{Path as FilePath, PathBuf};

const CHAMBER: &str = "chamber.json";
const UNAVAILABLE: &str = "The admitted world configuration is unavailable or changed.";
/// The browser chamber client's own per-file bound.
const FILE_MAX: usize = 32 * 1024 * 1024;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/verse", get(index))
        .route("/cloud/app/verse/open", get(open))
        .route("/cloud/app/hosts/{binding}/verse", get(join))
        .route("/cloud/world/{binding}/{ticket}/", get(chamber_page))
        .route("/cloud/world/{binding}/{ticket}/{*path}", get(content))
}

/// The browser chamber configuration the `everglade-web` client reads
/// (`docs/verse/platform-clients.md`), checked against its host binding.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Chamber {
    websocket: String,
    host: String,
    grant: String,
    epoch: u64,
    generation: u64,
    instance: u64,
    content: String,
    pack: String,
    scene: String,
    assets: String,
    #[serde(default)]
    bindings: Vec<Value>,
    #[serde(default)]
    mips: bool,
}

/// One host's admitted browser world, pinned to its original bytes.
pub(crate) struct World {
    directory: PathBuf,
    file: ProtectedFile,
    bytes: Vec<u8>,
    chamber: Chamber,
}

impl World {
    pub(crate) fn load(
        directory: &FilePath,
        host: &str,
        generation: u64,
        loopback: bool,
    ) -> Result<Self, String> {
        if !directory.is_absolute() {
            return Err(UNAVAILABLE.into());
        }
        let (file, bytes) = ProtectedFile::open(&directory.join(CHAMBER), 64 * 1024)?;
        let chamber: Chamber = serde_json::from_slice(&bytes).map_err(|_| UNAVAILABLE)?;
        if chamber.host != host
            || chamber.generation != generation
            || chamber.instance == 0
            || chamber.epoch > 9_007_199_254_740_991
            || chamber.bindings.len() > 64
            || !hex64(&chamber.grant)
            || !hex64(&chamber.content)
            || ![&chamber.pack, &chamber.scene, &chamber.assets]
                .iter()
                .all(|value| relative(value))
        {
            return Err(UNAVAILABLE.into());
        }
        websocket(&chamber.websocket, loopback)?;
        Ok(Self {
            directory: directory.into(),
            file,
            bytes,
            chamber,
        })
    }

    /// The world's exact identity, part of its binding's identity.
    pub(crate) fn identity(&self) -> Value {
        json!({
            "instance":self.chamber.instance,"content":self.chamber.content,
            "websocket":self.chamber.websocket,"grant":self.chamber.grant,
            "epoch":self.chamber.epoch,
            "config":format!("sha256:{:x}", Sha256::digest(&self.bytes)),
        })
    }

    pub(crate) fn check(&self) -> Result<(), SessionError> {
        self.file.check().map_err(|_| SessionError::Unavailable)
    }

    fn origin(&self) -> String {
        url::Url::parse(&self.chamber.websocket)
            .map(|url| url.origin().ascii_serialization())
            .unwrap_or_default()
    }

    /// Read the configuration or one file of the admitted content closure.
    fn read(&self, path: &str) -> Result<(&'static str, Vec<u8>), SessionError> {
        self.check()?;
        if path == CHAMBER {
            return Ok(("application/json", self.bytes.clone()));
        }
        let admitted = path == self.chamber.pack
            || path == self.chamber.scene
            || path
                .strip_prefix(self.chamber.assets.as_str())
                .is_some_and(|rest| rest.starts_with('/'));
        if !relative(path) || !admitted {
            return Err(SessionError::Forbidden);
        }
        let (_, bytes) = ProtectedFile::open(&self.directory.join(path), FILE_MAX)
            .map_err(|_| SessionError::Unavailable)?;
        let mime = if path.ends_with(".json") {
            "application/json"
        } else {
            "application/octet-stream"
        };
        Ok((mime, bytes))
    }
}

fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The same bounded same-origin relative paths the browser client accepts.
fn relative(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.is_ascii()
        && !value.starts_with('/')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
}

fn websocket(value: &str, loopback: bool) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|_| UNAVAILABLE)?;
    let local = url.host_str().is_some_and(|host| {
        host.trim_matches(['[', ']'])
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
    });
    if value.len() > 2048
        || url.host_str().is_none()
        || !(url.scheme() == "wss" || loopback && local && url.scheme() == "ws")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value
            .bytes()
            .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
    {
        return Err(UNAVAILABLE.into());
    }
    Ok(())
}

/// Rights a binding supplies for private work. `world` is never one of them.
fn private_rights(binding: &Binding) -> Vec<&'static str> {
    let rights = &binding.access().grant.rights;
    [
        (Right::Observe, "Observe"),
        (Right::Operate, "Operate"),
        (Right::Review, "Review"),
        (Right::Terminal, "Terminal"),
    ]
    .into_iter()
    .filter(|(right, _)| rights.contains(*right))
    .map(|(_, label)| label)
    .collect()
}

fn card(key: &str, label: &str, state: &str, reason: &str) -> Result<Markup, Response> {
    render(&workspace::connection_state(
        key,
        label,
        state,
        reason,
        super::colors(),
    ))
    .map(|value| ui::card(ui::native(&value)))
}

/// The world state for one binding, independent of the other connections.
fn world_state(binding: &Binding, viewer: &Viewer) -> (&'static str, String, bool) {
    if binding.declared_world().is_none() {
        return (
            "Unavailable",
            "This host offers no admitted browser world. The public worlds below remain open."
                .into(),
            false,
        );
    }
    if !binding.access().grant.rights.contains(Right::World) {
        return (
            "Unavailable",
            "This binding's native grant lacks the world right.".into(),
            false,
        );
    }
    match binding.world(viewer) {
        Ok(world) => (
            "Ready to join",
            format!(
                "Chamber instance {} · content {} · route {} · grant epoch {}. The host admits only an enrolled character bound to this instance and content, and checks it on every join. A joined world reports no other player's presence here.",
                world.chamber.instance,
                &world.chamber.content[..16],
                world.origin(),
                world.chamber.epoch
            ),
            true,
        ),
        Err(_) => ("Unavailable", UNAVAILABLE.into(), false),
    }
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
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
    let mut content = vec![html! {
        h2 { "Verse connections" }
        p { "World, computer, and private work are separate connections. Joining a world grants no task, terminal, review, typist, or sales right, and the 3D view is never required to submit or supervise work." }
    }];
    let bindings = app
        .config
        .cloud_hosts
        .as_ref()
        .map_or_else(Vec::new, |hosts| hosts.current(&viewer));
    if bindings.is_empty() {
        for (key, label, reason) in [
            (
                "world",
                "World",
                "No admitted host world. You can explore the public worlds below.",
            ),
            (
                "computer",
                "Computer",
                "No explicitly enrolled host connection for this account and workspace.",
            ),
            (
                "private-work",
                "Private work",
                "No current observe, operate, review, typist, or sales grant. Account sign-in grants none of these rights.",
            ),
        ] {
            match card(key, label, "Unavailable", reason) {
                Ok(value) => content.push(value),
                Err(response) => return response,
            }
        }
    }
    for binding in bindings {
        content.push(html! { h3 { "Host connection " (binding.id()) } });
        let (state, reason, joinable) = world_state(binding, &viewer);
        let checked = super::workbench::qualify(binding, &viewer).await;
        let capabilities = binding.capabilities();
        let operations = if capabilities.is_empty() {
            "task observation".to_string()
        } else {
            format!("task observation and {}", capabilities.join(", "))
        };
        let computer = match &checked {
            Ok(()) => (
                "Connected",
                format!(
                    "Host {} · generation {} · {} route · supports {} · checked at {}.",
                    binding.host(),
                    binding.generation(),
                    if binding.direct() { "direct" } else { "relay" },
                    operations,
                    now()
                ),
            ),
            Err(_) => (
                "Unavailable",
                format!(
                    "Host {} · generation {} did not answer a current observation check. Private views stay closed until it does.",
                    binding.host(),
                    binding.generation()
                ),
            ),
        };
        let rights = private_rights(binding);
        let private = if checked.is_ok() && !rights.is_empty() {
            (
                "Granted",
                format!(
                    "{} through this binding until {}. Typist control applies only to an attached terminal under its current typist. This binding carries no agent-controller or sales right. Joining a world adds none of these.",
                    rights.join(", "),
                    binding.access().grant.expires_at
                ),
            )
        } else {
            (
                "Unavailable",
                "No current private-work right is verified. Joining a world supplies none.".into(),
            )
        };
        for (key, label, (state, reason)) in [
            ("world", "World", (state, reason)),
            ("computer", "Computer", computer),
            ("private-work", "Private work", private),
        ] {
            match card(key, label, state, &reason) {
                Ok(value) => content.push(value),
                Err(response) => return response,
            }
        }
        let id = binding.id();
        let mut actions = Vec::new();
        if joinable {
            actions.push((format!("/cloud/app/hosts/{id}/verse"), "Join"));
        }
        if checked.is_ok() {
            actions.push((
                format!("/cloud/app/hosts/{id}/tasks"),
                "Open associated work",
            ));
            if super::workbench::assets_ready(&app) && binding.browser_config(&viewer).is_ok() {
                actions.push((format!("/cloud/app/hosts/{id}/workbench"), "Open workbench"));
            }
        }
        if !actions.is_empty() {
            content.push(ui::links(
                actions.iter().map(|(href, label)| (href.as_str(), *label)),
            ));
        }
    }
    content.push(html! {
        h2 { "Public worlds" }
        p { "These worlds are open without an account. They supply no host, Studio, or private-work connection, and nothing you do there reaches your work." }
        (ui::links([
            ("/grid", "Open Verse"),
            ("/everglade", "Everglade"),
            ("/druid", "The Grove"),
        ]))
        p { "A world station opens the same canonical work reference as this app. Reaching a desk never authorizes or starts a task." }
    });
    let content = html! { @for part in &content { (part) } };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "verse",
        Some(content),
        None,
    )
}

/// Join: issue a short world ticket and open the chamber renderer.
async fn join(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let Some(hosts) = &app.config.cloud_hosts else {
        return refused(SessionError::Unavailable);
    };
    let binding = match hosts.get(&viewer, &id) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if let Err(error) = binding.world(&viewer) {
        return refused(error);
    }
    if renderer(&app).is_none() {
        return refused(SessionError::Unavailable);
    }
    let ticket = match service.world_ticket(
        &viewer,
        binding.identity(),
        binding.access().grant.expires_at,
    ) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    protect(
        Redirect::to(&format!(
            "/cloud/world/{}/{ticket}/?zone=chamber",
            binding.id()
        ))
        .into_response(),
    )
}

fn renderer(app: &App) -> Option<&FilePath> {
    let directory = app.config.everglade.as_deref()?;
    (directory.join(crate::pages::GLUE).is_file() && directory.join(crate::pages::WASM).is_file())
        .then_some(directory)
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Zone {
    zone: Option<String>,
}

async fn chamber_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, ticket)): Path<(String, String)>,
    query: Result<Query<Zone>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
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
    let Some(hosts) = &app.config.cloud_hosts else {
        return refused(SessionError::Unavailable);
    };
    let binding = match hosts.get(&viewer, &id) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if let Err(error) = verify(service, binding, &ticket) {
        return refused(error);
    }
    let world = match binding.world(&viewer) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let Some(directory) = renderer(&app) else {
        return refused(SessionError::Unavailable);
    };
    if query.zone.as_deref() != Some("chamber") {
        return protect(
            Redirect::to(&format!(
                "/cloud/world/{}/{}/?zone=chamber",
                binding.id(),
                ticket
            ))
            .into_response(),
        );
    }
    let bytes = tokio::fs::metadata(directory.join(crate::pages::WASM))
        .await
        .map_or(0, |metadata| metadata.len());
    let body = format!(
        "<h1 class=\"unseen\">Verse</h1>\
<div class=\"glade\" id=\"everglade\" data-module=\"/everglade/{glue}\" \
data-wasm=\"/everglade/{wasm}\" data-wasm-bytes=\"{bytes}\" data-pack=\"/everglade/pack/\">\
<canvas id=\"{canvas}\" tabindex=\"0\" aria-label=\"Admitted host world\"></canvas>\
<nav class=\"glade-leave\" aria-label=\"World\"><a href=\"/cloud/app/verse\">Leave</a></nav>\
<p class=\"glade-status\" id=\"everglade-status\" aria-live=\"polite\">Joining the admitted world…</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to join this world.</p></noscript></div>\
<script type=\"module\" src=\"/static/everglade.js\"></script>",
        glue = crate::pages::GLUE,
        wasm = crate::pages::WASM,
        canvas = crate::pages::CANVAS_ID,
    );
    let mut response = protect(crate::layout::fullscreen("Verse", &body));
    let policy = crate::pages::EVERGLADE_POLICY.replace(
        "connect-src 'self'",
        &format!("connect-src 'self' {}", world.origin()),
    );
    if let Ok(value) = HeaderValue::from_str(&policy) {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, value);
    }
    response
}

fn verify(
    service: &super::session::CloudSession,
    binding: &Binding,
    ticket: &str,
) -> Result<(), SessionError> {
    service.verify_world_ticket(
        ticket,
        binding.account(),
        binding.account_workspace(),
        binding.members_epoch(),
        binding.identity(),
    )
}

/// Credential-free reads by the world client; the ticket carries the scope.
async fn content(
    State(app): State<App>,
    Path((id, ticket, path)): Path<(String, String, String)>,
) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let Some(hosts) = &app.config.cloud_hosts else {
        return refused(SessionError::Unavailable);
    };
    let answer = hosts.find(&id).and_then(|binding| {
        verify(service, binding, &ticket)?;
        binding.current_world()?.read(&path)
    });
    let (mime, bytes) = match answer {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let mut response = protect((StatusCode::OK, bytes).into_response());
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    response
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Open {
    host: String,
    resource: String,
}

/// Resolve a station's canonical work reference to the app's own view.
async fn open(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<Open>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
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
    let Some(hosts) = &app.config.cloud_hosts else {
        return refused(SessionError::Unavailable);
    };
    let binding = match hosts.get(&viewer, &query.host) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let resource = match super::workbench::reference(&query.resource) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if resource.host
        != (::workbench::Host::Paired {
            key: binding.host().into(),
        })
    {
        return refused(SessionError::Conflict);
    }
    let id = binding.id();
    match resource.kind {
        ::workbench::Kind::Terminal => protect(
            Redirect::to(&format!(
                "/cloud/app/hosts/{id}/workbench?resource={}",
                query.resource
            ))
            .into_response(),
        ),
        ::workbench::Kind::Run if super::work::task_id(&resource.id) => protect(
            Redirect::to(&format!("/cloud/app/hosts/{id}/tasks/{}", resource.id)).into_response(),
        ),
        _ => {
            let tasks = format!("/cloud/app/hosts/{id}/tasks");
            let content = html! {
                h2 { "Associated work" }
                pre { (serde_json::to_string_pretty(&resource).expect("reference serializes")) }
                p { "This reference needs its own admitted owner viewer. This page offers no action for it." }
                (ui::links([
                    (tasks.as_str(), "Open associated work"),
                    ("/cloud/app/verse", "Verse connections"),
                ]))
            };
            workspace_shell(
                &app,
                &headers,
                service,
                &viewer,
                "verse",
                Some(content),
                None,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_paths_stay_relative_and_bounded() {
        for value in ["chamber/pack.json", "chamber/assets/a.ktx2", "x"] {
            assert!(relative(value));
        }
        for value in [
            "",
            "/etc/passwd",
            "chamber/../x",
            "chamber//x",
            "./x",
            "a%2f..",
            "chamber/assets/ä",
        ] {
            assert!(!relative(value), "{value}");
        }
    }

    #[test]
    fn world_routes_require_tls_except_loopback_fixtures() {
        assert!(websocket("wss://game.example/chamber", false).is_ok());
        assert!(websocket("ws://127.0.0.1:9000/chamber", true).is_ok());
        for (value, loopback) in [
            ("ws://127.0.0.1:9000/chamber", false),
            ("ws://game.example/chamber", true),
            ("wss://user:pw@game.example/", false),
            ("wss://game.example/?grant=x", false),
            ("https://game.example/", false),
        ] {
            assert!(websocket(value, loopback).is_err(), "{value}");
        }
    }
}

//! Account-bound navigation to separately enrolled native host terminals.

use super::hosts::Binding;
use super::session::{SessionError, Viewer};
use super::{POLICY, refused, service, workspace_shell};
use crate::App;
use axum::extract::rejection::QueryRejection;
use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, header},
    response::Response,
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use coder_access::{Operation, Outcome, task_read::ListQuery};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/workbench", get(index))
        .route("/cloud/app/hosts/{binding}/workbench", get(host))
}

pub(super) fn assets_ready(app: &App) -> bool {
    app.config.cloud_build.as_ref().is_some_and(|dir| {
        dir.join("coder_browser_web.js").is_file()
            && dir.join("coder_browser_web_bg.wasm").is_file()
    })
}

pub(super) fn available(app: &App, viewer: &Viewer) -> bool {
    assets_ready(app)
        && app.config.cloud_hosts.as_ref().is_some_and(|hosts| {
            hosts
                .current(viewer)
                .iter()
                .any(|binding| binding.browser_config(viewer).is_ok())
        })
}

async fn index(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let bindings = app
        .config
        .cloud_hosts
        .as_ref()
        .map_or_else(Vec::new, |hosts| hosts.current(&viewer));
    let ready: Vec<&str> = if assets_ready(&app) {
        bindings
            .iter()
            .filter(|binding| binding.browser_config(&viewer).is_ok())
            .map(|binding| binding.id())
            .collect()
    } else {
        Vec::new()
    };
    let content = html! {
        h2 { "Native workbench" }
        p { "Enroll this page as a separate host device, then open an existing native session. Account sign-in grants no terminal access." }
        ul {
            @for id in &ready {
                li {
                    a href=(format!("/cloud/app/hosts/{id}/workbench")) { "Open workbench on " (id) }
                    " \u{b7} "
                    a href=(format!("/cloud/app/hosts/{id}/workbench?sign_in=claude")) { "Sign in to Claude" }
                }
            }
            @if ready.is_empty() {
                li { "No qualified browser terminal connection is configured for this workspace." }
            }
        }
        p { "Retail Cloud tasks offer no customer shell." }
        @if !ready.is_empty() { (claude_sign_in()) }
    }
    .into_string();
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "workbench",
        Some(&content),
        None,
    )
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Input {
    resource: Option<String>,
    session: Option<String>,
    sign_in: Option<String>,
}

/// Plain-text engine copy: no logos, and no OpenAgents Claude login form.
fn claude_sign_in() -> Markup {
    html! {
        p { "A computer with the Claude Code engine runs Claude Code. Sign in to Claude opens a terminal on your computer and runs " code { "claude" } "; you finish Anthropic's own sign-in there, with your own plan or API key. OpenAgents never asks for, receives, or stores your Claude login, and it stays only in your computer." }
        p { "Each workbench shows whether that computer's Claude Code is signed in, its plan or key type, when the login expires, and any usage-limit reset Claude Code reports, read by running " code { "claude auth status" } " inside the computer. Only that status leaves it, and OpenAgents keeps no usage ledger for your plan. When the login is expiring or expired, Renew Claude sign-in opens the same terminal sign-in." }
    }
}

/// The engine sign-in a workbench page may run: only Claude Code's own
/// program, from the runtime image, with no arguments.
fn sign_in(engine: &str, binding: &Binding) -> Result<Value, SessionError> {
    if engine != coder_cloud::claude::ENGINE {
        return Err(SessionError::InvalidRequest);
    }
    Ok(serde_json::json!({
        "program": coder_cloud::claude::PROGRAM,
        "workspace": coder_host::mailbox::workspace_id(binding.workspace()),
    }))
}

pub(super) fn reference(input: &str) -> Result<workbench::ResourceRef, SessionError> {
    if input.len() > 8192 {
        return Err(SessionError::InvalidRequest);
    }
    let bytes = URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|_| SessionError::InvalidRequest)?;
    if bytes.len() > 4096 {
        return Err(SessionError::InvalidRequest);
    }
    let resource: workbench::ResourceRef =
        serde_json::from_slice(&bytes).map_err(|_| SessionError::InvalidRequest)?;
    resource.check().map_err(|_| SessionError::InvalidRequest)?;
    Ok(resource)
}

pub(super) fn configuration_digest(
    binding: &Binding,
    viewer: &Viewer,
) -> Result<String, SessionError> {
    let config = binding.browser_config(viewer)?;
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(config.to_string().as_bytes())
    ))
}

/// Prove the resident observation route independently of the page's own grant.
pub(super) async fn qualify(binding: &Binding, viewer: &Viewer) -> Result<(), SessionError> {
    let query = ListQuery {
        workspace: binding.workspace().into(),
        cursor: None,
        limit: 1,
    };
    let outcome = binding
        .read(
            viewer,
            Operation::ListTasks {
                query: query.clone(),
            },
        )
        .await?;
    match outcome {
        Outcome::Tasks { tasks } if tasks.answers(&query) => Ok(()),
        _ => Err(SessionError::Conflict),
    }
}

async fn host(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Query<Input>, QueryRejection>,
) -> Response {
    let Ok(Query(input)) = input else {
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
    if !assets_ready(&app) {
        return refused(SessionError::Unavailable);
    }
    let mut config = match binding.browser_config(&viewer) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if config["loopback"] == true && !loopback_origin(service.origin()) {
        return refused(SessionError::Forbidden);
    }
    if let Err(error) = qualify(binding, &viewer).await {
        return refused(error);
    }
    let current = match service.authenticate(&headers).await {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    if super::work::authority_value(&current) != super::work::authority_value(&viewer) {
        return refused(SessionError::Conflict);
    }
    let mut identity: Vec<Markup> = Vec::new();
    if let Some(encoded) = &input.resource {
        let resource = match reference(encoded) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        identity.push(html! {
            h3 { "Original work reference" }
            pre { (serde_json::to_string_pretty(&resource).expect("reference serializes")) }
        });
        if resource.kind == workbench::Kind::Terminal {
            if resource.host
                != (workbench::Host::Paired {
                    key: binding.host().into(),
                })
                || resource.generation.as_deref()
                    != Some(&coder_host::mailbox::terminal_generation(
                        binding.host(),
                        binding.generation(),
                    ))
                || resource.workspace.as_ref().is_some_and(|workspace| {
                    *workspace != coder_host::mailbox::workspace_id(binding.workspace())
                })
            {
                return refused(SessionError::Conflict);
            }
            config["terminal"] =
                serde_json::json!({"generation":resource.generation,"terminal":resource.id});
        } else {
            identity.push(html! { p { "This reference needs its own admitted owner viewer. This terminal page offers no action for it." } });
        }
    }
    if let Some(engine) = &input.sign_in {
        if input.resource.is_some() || input.session.is_some() {
            return refused(SessionError::InvalidRequest);
        }
        config["sign_in"] = match sign_in(engine, binding) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        identity.push(claude_sign_in());
    }
    if let Some(session) = input.session {
        if session.len() != 64
            || !session
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return refused(SessionError::InvalidRequest);
        }
        config["session"] = Value::String(session);
    }
    let digest = match configuration_digest(binding, &viewer) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let resource = match super::work::workbench_resource(binding, &viewer, digest) {
        Ok(value) => value,
        Err(error) => return refused(error),
    };
    let content = html! {
        h2 { "Native workbench" }
        p {
            "Host " code { (binding.host()) }
            " \u{b7} Generation " (binding.generation())
            " \u{b7} Workspace context " code { (binding.workspace()) }
        }
        p { "A host invitation grants host-wide terminal access under its native rights and expiry. The selected account workspace is navigation context; it does not narrow that grant. This page holds its own device key in memory." }
        @for part in &identity { (part) }
        pre id="cloud-workbench-config" hidden { (config.to_string()) }
        // The terminal renderer mounts here; it stays a dark panel in both themes.
        section id="cloud-workbench" aria-label="Granted native workbench" data-theme="dark" {
            p { "Starting the shared terminal renderer. Enrollment is required before reading native sessions." }
        }
        p { "Closing this page detaches the viewer and leaves the host terminal alive. Reconnect requires fresh enrollment and a retained snapshot. Clipboard controls require a gesture. Retail Cloud tasks offer no customer shell." }
    }
    .into_string();
    let mut response = workspace_shell(
        &app,
        &headers,
        service,
        &current,
        "workbench",
        Some(&content),
        Some(resource),
    );
    let origins = binding.browser_origins().join(" ");
    let policy = POLICY.replace(
        "connect-src 'self'",
        &format!("connect-src 'self' {origins}"),
    );
    if let Ok(value) = HeaderValue::from_str(&policy) {
        response
            .headers_mut()
            .insert(header::CONTENT_SECURITY_POLICY, value);
    }
    response
}

fn loopback_origin(value: &str) -> bool {
    url::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "http"
            && match url.host() {
                Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
                Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
                _ => false,
            }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn loopback_fixture_accepts_literal_ipv4_and_ipv6_only() {
        for origin in ["http://127.0.0.1:8080", "http://[::1]:8080"] {
            assert!(super::loopback_origin(origin));
        }
        for origin in [
            "https://127.0.0.1:8080",
            "http://localhost:8080",
            "http://192.0.2.1:8080",
            "http://[2001:db8::1]:8080",
        ] {
            assert!(!super::loopback_origin(origin));
        }
    }
}

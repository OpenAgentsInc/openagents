//! Account-bound navigation to separately enrolled native host terminals.

use super::hosts::Binding;
use super::session::{SessionError, Viewer};
use super::{POLICY, refused, service, workspace_shell};
use crate::{App, layout::escape};
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
    let mut content = String::from(
        "<h2>Native workbench</h2><p>Enroll this page as a separate host device, then open an existing native session. Account sign-in grants no terminal access.</p><ul>",
    );
    let bindings = app
        .config
        .cloud_hosts
        .as_ref()
        .map_or_else(Vec::new, |hosts| hosts.current(&viewer));
    let mut count = 0;
    if assets_ready(&app) {
        for binding in bindings {
            if binding.browser_config(&viewer).is_ok() {
                count += 1;
                content.push_str(&format!(
                    "<li><a href=\"/cloud/app/hosts/{}/workbench\">Open workbench on {}</a></li>",
                    escape(binding.id()),
                    escape(binding.id())
                ));
            }
        }
    }
    if count == 0 {
        content.push_str(
            "<li>No qualified browser terminal connection is configured for this workspace.</li>",
        );
    }
    content.push_str("</ul><p>Retail Cloud tasks offer no customer shell.</p>");
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
}

fn reference(input: &str) -> Result<workbench::ResourceRef, SessionError> {
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
    let mut identity = String::new();
    if let Some(encoded) = &input.resource {
        let resource = match reference(encoded) {
            Ok(value) => value,
            Err(error) => return refused(error),
        };
        identity = format!(
            "<h3>Original work reference</h3><pre>{}</pre>",
            escape(&serde_json::to_string_pretty(&resource).expect("reference serializes"))
        );
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
            identity.push_str("<p>This reference needs its own admitted owner viewer. This terminal page offers no action for it.</p>");
        }
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
    let content = format!(
        "<h2>Native workbench</h2><p>Host <code>{}</code> · Generation {} · Workspace context <code>{}</code></p><p>A host invitation grants host-wide terminal access under its native rights and expiry. The selected account workspace is navigation context; it does not narrow that grant. This page holds its own device key in memory.</p>{identity}<pre id=\"cloud-workbench-config\" hidden>{}</pre><section id=\"cloud-workbench\" aria-label=\"Granted native workbench\"><p>Starting the shared terminal renderer. Enrollment is required before reading native sessions.</p></section><p>Closing this page detaches the viewer and leaves the host terminal alive. Reconnect requires fresh enrollment and a retained snapshot. Clipboard controls require a gesture. Retail Cloud tasks offer no customer shell.</p>",
        escape(binding.host()),
        binding.generation(),
        escape(binding.workspace()),
        escape(&config.to_string())
    );
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

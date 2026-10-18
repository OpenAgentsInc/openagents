//! `/labs/agent/{id}/card`: one saved agent's autonomy card on the website
//! (#12045), the same Rust Native view the desktop and the phone mount. It
//! reads the definition and its grants from the account; the agent's chat
//! and decision log stay on the computer that runs it, and the card says so.
use crate::App;
use agent_lab::autonomy::{Card, Intent};
use axum::{
    Router,
    extract::{Form, Path, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use openagents_sdk::{CancellationToken, Client, Session, agents::valid_id};
use serde_json::json;
use std::collections::BTreeMap;

const CSRF: &str = "agent-card";

pub(crate) fn routes() -> Router<App> {
    Router::new().route("/labs/agent/{id}/card", get(page).post(submit))
}

fn instance(id: &str) -> String {
    format!("agent-card-{}", &id[..id.len().min(24)])
}

async fn load(client: &Client, session: &Session, id: &str) -> Result<Card, openagents_sdk::Error> {
    let agents = client.agents(session)?;
    let cancel = CancellationToken::new();
    let definition = agents.get(id, None, &cancel).await?;
    let grants = agents.grants(id, &cancel).await?;
    Ok(Card {
        agent: id.to_owned(),
        name: definition.spec.name,
        autonomy: definition.spec.autonomy,
        grants,
        here: None,
    })
}

async fn page(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if !valid_id(&id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (client, session) = match crate::agent_lab_page::access(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match load(&client, &session, &id).await {
        Ok(card) => render(&app, &headers, &card, None).await,
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn submit(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(mut fields): Form<BTreeMap<String, String>>,
) -> Response {
    if !valid_id(&id) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let (client, session) = match crate::agent_lab_page::access(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(cloud) = app.config.cloud.as_deref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Some(ticket) = fields.remove("_csrf") else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Ok(viewer) = cloud.authenticate(&headers).await else {
        return StatusCode::FORBIDDEN.into_response();
    };
    if cloud
        .verify_csrf(
            &headers,
            Some(&viewer),
            CSRF,
            &lab_kit::digest(&json!(id)),
            &ticket,
        )
        .is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(card) = load(&client, &session, &id).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let node = fields.remove("_action").unwrap_or_default();
    let notice = match act(&client, &session, &card, &node).await {
        Ok(()) => return Redirect::to(&format!("/labs/agent/{id}/card")).into_response(),
        Err(notice) => notice,
    };
    render(&app, &headers, &card, Some(notice)).await
}

/// Resolve the activation against the card as it is now, then change the
/// grants through the account.
async fn act(client: &Client, session: &Session, card: &Card, node: &str) -> Result<(), String> {
    let view = card
        .view(&instance(&card.agent), 1)
        .map_err(|_| "Reload this page.".to_owned())?;
    let intent = view
        .activate(&rust_native::Activation {
            instance: instance(&card.agent),
            revision: 1,
            node: node.into(),
        })
        .map_err(|_| "Reload this page.".to_owned())?
        .clone();
    let agents = client.agents(session).map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    match intent {
        Intent::Log { on: true } => agents
            .grant(
                &card.agent,
                vec![agent_contract::GrantScope::LogWrite.word().to_owned()],
                agent_contract::GrantOver::Account,
                &cancel,
            )
            .await
            .map(|_| ())
            .map_err(|e| e.to_string()),
        Intent::Log { on: false } => {
            for grant in card.log_grants() {
                agents
                    .revoke_grant(&card.agent, &grant, &cancel)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        Intent::Revoke { grant } => agents
            .revoke_grant(&card.agent, &grant, &cancel)
            .await
            .map_err(|e| e.to_string()),
    }
}

async fn render(app: &App, headers: &HeaderMap, card: &Card, notice: Option<String>) -> Response {
    let Some(cloud) = app.config.cloud.as_deref() else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let Ok(viewer) = cloud.authenticate(headers).await else {
        return Redirect::to("/login").into_response();
    };
    let Ok(ticket) = cloud.csrf(headers, &viewer, CSRF, &lab_kit::digest(&json!(card.agent)))
    else {
        return StatusCode::FORBIDDEN.into_response();
    };
    let Ok(view) = card.view(&instance(&card.agent), 1) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let Ok(body) = rust_native_web::render(&view) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let escape = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let notice = notice
        .map(|n| format!(r#"<p role="status">{}</p>"#, escape(&n)))
        .unwrap_or_default();
    let html = format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>{} · OpenAgents</title><style>{}body{{background:#12151b;color:#ddd;max-width:780px;margin:24px auto;padding:16px;font:16px system-ui}}a{{color:#a8c8ff}}button{{font:inherit}}.rn-view .rn-stack{{gap:12px}}</style></head><body><a href="/labs/agent">Agent Lab</a>{notice}<form method="post"><input type="hidden" name="_csrf" value="{}"><input type="hidden" name="_action" id="action">{}</form><script src="/static/agent-lab.js" defer></script></body></html>"#,
        escape(&card.name),
        rust_native_web::CSS,
        escape(&ticket),
        body
    );
    (
        [
            ("cache-control", "no-store, private"),
            ("referrer-policy", "same-origin"),
            ("content-security-policy", "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'"),
        ],
        Html(html),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_contract::{AgentGrant, GrantOver, GrantScope};

    /// #12045: the website mounts the shared card for any agent, in plain
    /// words, and its actions resolve only against the card as drawn.
    #[test]
    fn the_website_mounts_the_shared_card() {
        let card = Card {
            agent: "c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3c3".into(),
            name: "Release reviewer".into(),
            autonomy: None,
            grants: vec![AgentGrant {
                id: "d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1d1".into(),
                scopes: vec![GrantScope::ChatStop],
                over: GrantOver::Account,
                granted_at: 0,
            }],
            here: None,
        };
        let view = card.view(&instance(&card.agent), 1).unwrap();
        let html = rust_native_web::render(&view).unwrap();
        assert!(html.contains("Release reviewer"));
        assert!(html.contains("Decision log: off"));
        assert!(html.contains("Its chat and decision log are on the computer that runs it."));
        let visible = oa_copy::visible_text(&html);
        assert!(oa_copy::violations(&visible, &[]).is_empty(), "{visible}");
        assert!(
            view.activate(&rust_native::Activation {
                instance: instance(&card.agent),
                revision: 1,
                node: "not-a-control".into(),
            })
            .is_err()
        );
    }
}

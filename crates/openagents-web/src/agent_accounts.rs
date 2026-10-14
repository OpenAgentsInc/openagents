//! `/settings/agent-accounts` (#11630): agents that registered themselves
//! (`POST /v1/agents/register` on the gateway, docs/agents/self-registration.md).
//!
//! - A signed-in person enters the claim code an agent got when it
//!   registered, which links the agent to their account.
//! - The person sees their agents and can revoke one; its key stops at once.
//! - A site admin also sees every agent account and can revoke any.
//!
//! Each action is the account service's own route under the person's
//! session (`jev` `claim_agent`, `claimed_agents`, `revoke_agent`,
//! `admin_agents`).

use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use openagents_ui::actions::{Button, ButtonType, ButtonVariant, Color};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::{Field, Input, InputType};
use serde::Deserialize;
use serde_json::Value;

use crate::App;
use crate::cloud::byo::fresh_request;
use crate::cloud::protect;
use crate::cloud::session::{SessionError, Viewer};
use crate::settings::{PAGE as SETTINGS, page, viewer};
use crate::ui_page::action_link;

pub(crate) const PAGE: &str = "/settings/agent-accounts";
pub(crate) const REVOKE: &str = "/settings/agent-accounts/revoke";
const CLAIM_SCOPE: &str = "agent-claim";
const REVOKE_SCOPE: &str = "agent-revoke";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PAGE, get(show).post(claim))
        .route(REVOKE, post(revoke))
}

fn target(viewer: &Viewer, request: &str) -> String {
    format!("{}:{request}", viewer.account_id)
}

fn problem(status: StatusCode, text: &str) -> Response {
    protect(crate::layout::problem(
        status,
        "Agents",
        text,
        (PAGE, "Agents"),
    ))
}

const UNREACHABLE: &str = "Your agents can't be reached right now. Try again in a minute.";

/// One agent, as the lists show it.
struct Row {
    id: String,
    name: String,
    hint: String,
    revoked: bool,
}

fn rows(document: &Value) -> Vec<Row> {
    document["agents"]
        .as_array()
        .map(|agents| {
            agents
                .iter()
                .map(|agent| {
                    let day = |field: &str| {
                        agent[field]
                            .as_str()
                            .map(|at| at.get(..10).unwrap_or(at).to_owned())
                    };
                    let mut hint = format!(
                        "Joined {}",
                        day("registered").unwrap_or_else(|| "earlier".into())
                    );
                    if let Some(at) = day("revoked") {
                        hint.push_str(&format!(" · Turned off {at}"));
                    } else if day("claimed").is_none() {
                        hint.push_str(" · Not claimed");
                    }
                    if let Some(balance) = agent["balance_usd"].as_str() {
                        hint.push_str(&format!(" · Credit ${balance}"));
                    }
                    Row {
                        id: agent["id"].as_str().unwrap_or_default().to_owned(),
                        name: agent["name"].as_str().unwrap_or_default().to_owned(),
                        hint,
                        revoked: !agent["revoked"].is_null(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn list(title: &str, id: &str, empty: &str, rows: &[Row], revoke: (&str, &str)) -> Markup {
    html! {
        section class="oa-settings-group" aria-labelledby=(id) {
            h2 id=(id) { (title) }
            @if rows.is_empty() {
                p { (empty) }
            }
            @for row in rows {
                div class="oa-settings-row" {
                    div class="oa-settings-text" {
                        span class="oa-settings-label" { (row.name) }
                        span class="oa-settings-hint" { (row.hint) }
                    }
                    @if !row.revoked {
                        div class="oa-settings-control" {
                            form method="post" action=(REVOKE) {
                                input type="hidden" name="csrf" value=(revoke.0);
                                input type="hidden" name="request" value=(revoke.1);
                                input type="hidden" name="agent" value=(row.id);
                                (Button::new("Turn off")
                                    .kind(ButtonType::Submit)
                                    .variant(ButtonVariant::Soft)
                                    .color(Color::Secondary))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The page: what agents are, the claim form, the person's agents, and
/// for a site admin every agent.
fn content(mine: &[Row], all: Option<&[Row]>, claim: (&str, &str), revoke: (&str, &str)) -> Markup {
    let code = Field::new("agent-claim-code", "Claim code")
        .description("The code your agent got when it signed up. It starts with clm_.");
    html! {
        p { (action_link("Settings", SETTINGS)) }
        (MarkdownRoot::new(html! {
            h1 { "Agents" }
            p {
                "Agents can sign up for OpenAgents on their own and use a small free amount each day. "
                "Claim one to link it to your account, so you can see it and turn it off."
            }
        }))
        section class="oa-settings-group" aria-labelledby="agent-claim" {
            h2 #agent-claim { "Claim an agent" }
            form method="post" action=(PAGE) autocomplete="off" {
                input type="hidden" name="csrf" value=(claim.0);
                input type="hidden" name="request" value=(claim.1);
                (code.clone().control(
                    Input::new("code")
                        .input_type(InputType::Text)
                        .aria(code.aria()),
                ))
                p { (Button::new("Claim").kind(ButtonType::Submit)) }
            }
        }
        (list("Your agents", "agent-mine", "You haven't claimed any agents.", mine, revoke))
        @if let Some(all) = all {
            (list("Every agent (admin)", "agent-all", "No agent has signed up yet.", all, revoke))
        }
    }
}

async fn show(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let account = viewer.client().account();
    let Ok(mine) = account.claimed_agents().await else {
        return problem(StatusCode::BAD_GATEWAY, UNREACHABLE);
    };
    let all = if viewer.admin {
        account.admin_agents().await.ok()
    } else {
        None
    };
    let mut tickets = Vec::new();
    for scope in [CLAIM_SCOPE, REVOKE_SCOPE] {
        let request = fresh_request();
        match service.csrf(&headers, &viewer, scope, &target(&viewer, &request)) {
            Ok(csrf) => tickets.push((csrf, request)),
            Err(error) => return crate::cloud::refused(error),
        }
    }
    let all_rows = all.as_ref().map(rows);
    let body = content(
        &rows(&mine),
        all_rows.as_deref(),
        (&tickets[0].0, &tickets[0].1),
        (&tickets[1].0, &tickets[1].1),
    );
    page(&headers, service, &viewer, "Agents", PAGE, body)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimForm {
    csrf: String,
    request: String,
    #[serde(default)]
    code: String,
}

async fn claim(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<ClaimForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        CLAIM_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    let code = form.code.trim();
    if !code.starts_with("clm_") || code.len() > 200 {
        return problem(
            StatusCode::BAD_REQUEST,
            "That isn't a claim code. It starts with clm_.",
        );
    }
    match viewer.client().account().claim_agent(code).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(jev::Error::Api(error)) => problem(
            StatusCode::BAD_REQUEST,
            match error.status {
                409 => "That agent was already claimed or turned off.",
                _ => "That claim code didn't work. Check it, or ask the agent for its code again.",
            },
        ),
        Err(_) => problem(StatusCode::BAD_GATEWAY, UNREACHABLE),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeForm {
    csrf: String,
    request: String,
    agent: String,
}

async fn revoke(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<RevokeForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return crate::cloud::refused(SessionError::InvalidRequest);
    };
    let (service, viewer) = match viewer(&app, &headers, PAGE).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        REVOKE_SCOPE,
        &target(&viewer, &form.request),
        &form.csrf,
    ) {
        return crate::cloud::refused(error);
    }
    match viewer.client().account().revoke_agent(&form.agent).await {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(jev::Error::Api(_)) => problem(
            StatusCode::FORBIDDEN,
            "You can turn off only agents you claimed.",
        ),
        Err(_) => problem(StatusCode::BAD_GATEWAY, UNREACHABLE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_reads_plainly() {
        let agents = serde_json::json!({"agents": [
            {"id": "acct_1", "name": "scout", "registered": "2026-10-14T20:00:00Z", "claimed": "2026-10-14T21:00:00Z", "revoked": null},
            {"id": "acct_2", "name": "old", "registered": "2026-10-13T20:00:00Z", "revoked": "2026-10-14T01:00:00Z", "balance_usd": "0.5"}
        ]});
        let mine = rows(&agents);
        assert_eq!(mine[0].hint, "Joined 2026-10-14");
        assert!(mine[1].revoked);
        assert!(mine[1].hint.contains("Credit $0.5"));
        let html = content(&mine, Some(&mine), ("c", "r"), ("c2", "r2")).into_string();
        assert!(html.contains("Claim an agent"));
        assert!(html.contains("Every agent (admin)"));
        crate::copy_guard::assert_plain(PAGE, &html);
    }
}

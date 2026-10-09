//! The agent discovery documents (`openagents discover` reads them): the
//! A2A agent card, the agent-skills index, and the skill it names. They
//! come from the `discovery` crate, the same source `discover` previews,
//! rendered for this site's public origin.

use axum::Router;
use axum::extract::State;
use axum::http::header;
use axum::response::{IntoResponse, Json};
use axum::routing::get;

use crate::App;

/// The skill's path under the skills index.
pub(crate) const SKILL_PATH: &str = "/.well-known/agent-skills/openagents-decision-api/SKILL.md";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/.well-known/agent-card.json", get(agent_card))
        .route("/.well-known/agent-skills/index.json", get(skills_index))
        .route(SKILL_PATH, get(skill))
        .route("/.well-known/security.txt", get(security_txt))
        .route("/security.txt", get(security_txt))
}

/// Where to report a security problem (RFC 9116).
pub(crate) const SECURITY_CONTACT: &str = "mailto:chris+security@openagents.com";
/// RFC 9116 requires an expiry under a year away; a test fails a month
/// before it lapses so it gets renewed.
pub(crate) const SECURITY_EXPIRES: &str = "2027-10-01T00:00:00Z";

/// The origin the documents name: the first public host, else this
/// machine's loopback address.
pub(crate) fn origin(app: &App) -> String {
    match app.config.public_hosts.first() {
        Some(host) => format!("https://{host}"),
        None => format!("http://localhost:{}", app.config.port),
    }
}

/// The agent card's extension naming the ways the API takes payment per
/// request now; absent when it takes none.
pub(crate) const PAYMENTS_EXTENSION: &str = "https://openagents.com/ext/api-payments/v1";

async fn agent_card(State(app): State<App>) -> impl IntoResponse {
    let methods = crate::payments::live(&app).await;
    Json(agent_card_with(&origin(&app), &methods))
}

/// The A2A agent card, with the API's ways to pay per request.
pub(crate) fn agent_card_with(
    origin: &str,
    methods: &[crate::payments::Method],
) -> serde_json::Value {
    let mut card = discovery::site::agent_card(origin);
    if methods.is_empty() {
        return card;
    }
    let extension = serde_json::json!({
        "uri": PAYMENTS_EXTENSION,
        "description": "The OpenAgents API (https://api.openagents.com/v1) takes payment per request with no key, by these methods.",
        "required": false,
        "params": {
            "api": "https://api.openagents.com/v1",
            "docs": format!("{origin}/docs/api/for-agents"),
            "methods": methods,
        }
    });
    if let Some(capabilities) = card
        .get_mut("capabilities")
        .and_then(serde_json::Value::as_object_mut)
    {
        let extensions = capabilities
            .entry("extensions")
            .or_insert_with(|| serde_json::json!([]));
        if let Some(list) = extensions.as_array_mut() {
            list.push(extension);
        }
    } else if let Some(object) = card.as_object_mut() {
        object.insert(
            "capabilities".into(),
            serde_json::json!({"extensions": [extension]}),
        );
    }
    card
}

async fn skills_index(State(app): State<App>) -> impl IntoResponse {
    Json(crate::agent_ready::skills_index(&origin(&app)))
}

async fn security_txt() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!(
            "Contact: {SECURITY_CONTACT}\nExpires: {SECURITY_EXPIRES}\nPreferred-Languages: en\n"
        ),
    )
}

async fn skill() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        discovery::plugins::CANONICAL_SKILL,
    )
}

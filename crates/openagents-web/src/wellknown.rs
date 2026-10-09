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
}

/// The origin the documents name: the first public host, else this
/// machine's loopback address.
pub(crate) fn origin(app: &App) -> String {
    match app.config.public_hosts.first() {
        Some(host) => format!("https://{host}"),
        None => format!("http://localhost:{}", app.config.port),
    }
}

async fn agent_card(State(app): State<App>) -> impl IntoResponse {
    Json(discovery::site::agent_card(&origin(&app)))
}

async fn skills_index(State(app): State<App>) -> impl IntoResponse {
    Json(crate::agent_ready::skills_index(&origin(&app)))
}

async fn skill() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        discovery::plugins::CANONICAL_SKILL,
    )
}

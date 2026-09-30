//! The live, read-only boards: `/earn` (the mesh's book, tape, and
//! devices), `/weights` (where a model's weights are across the fleet),
//! and `/qa` (the claim registry against its receipts).
//!
//! Each reads a snapshot from the backend, which production projects from
//! the fleet coordinator's status and the service's receipts. A board is
//! public and draws no prompt, no answer, no login, and no full computer
//! id. The private service also streamed updates over a WebSocket and
//! replayed recorded fixtures; this site draws a snapshot per request and
//! runs no script, so reload to read newer state.

use axum::Router;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;

use crate::App;
use crate::backend::{Board, Dashboard, NOT_CONNECTED};
use crate::layout::{escape, page};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/earn", get(earn))
        .route("/weights", get(weights))
        .route("/qa", get(qa))
}

async fn earn(State(app): State<App>) -> Response {
    draw(
        &app,
        Board::Earn,
        "Earn",
        "<h1>Earn</h1><p>Machines offer their spare capacity to the mesh, within the limits \
their owners set, and earn credit for the work they serve. This board shows the mesh's book, \
its tape of receipts, and its devices. Run <code>coder earn on</code> to join.</p>",
    )
    .await
}

async fn weights(State(app): State<App>) -> Response {
    draw(
        &app,
        Board::Weights,
        "Weight map",
        "<h1>Weight map</h1><p>Where a model's weights are across the fleet: every session, \
every host, and the layers each holds.</p>",
    )
    .await
}

async fn qa(State(app): State<App>) -> Response {
    draw(
        &app,
        Board::Qa,
        "QA ledger",
        "<h1>QA ledger</h1><p>Every claim the product makes, the obligations that prove it, \
and the receipts recorded against them, by surface and evidence lane.</p>",
    )
    .await
}

async fn draw(app: &App, board: Board, title: &str, lead: &str) -> Response {
    let backend = &app.config.backend;
    let body = if !backend.connected() {
        format!("{lead}<p class=\"notice\">{}</p>", escape(NOT_CONNECTED))
    } else {
        match backend.dashboard(board).await {
            Some(dashboard) => format!("{lead}{}", tables(&dashboard)),
            None => format!("{lead}<p class=\"dim\">Nothing is reporting to this board.</p>"),
        }
    };
    page(title, None, &body)
}

fn tables(dashboard: &Dashboard) -> String {
    let mut out = format!("<p class=\"hint\">{}</p>", escape(&dashboard.summary));
    for table in &dashboard.tables {
        out.push_str(&format!("<h2>{}</h2>", escape(&table.title)));
        if table.rows.is_empty() {
            out.push_str("<p class=\"dim\">None.</p>");
            continue;
        }
        out.push_str("<table><thead><tr>");
        for column in &table.columns {
            out.push_str(&format!("<th>{}</th>", escape(column)));
        }
        out.push_str("</tr></thead><tbody>");
        for row in &table.rows {
            out.push_str("<tr>");
            for cell in row {
                out.push_str(&format!("<td>{}</td>", escape(cell)));
            }
            out.push_str("</tr>");
        }
        out.push_str("</tbody></table>");
    }
    out
}

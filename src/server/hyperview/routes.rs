use super::{handlers, ws};
use crate::server::config::AppState;
use axum::{routing::get, Router};
use axum::http::StatusCode;
use axum::body::Body;
use axum::response::{Response, IntoResponse};

pub fn hyperview_routes() -> Router<AppState> {
    Router::new()
        .route("/hyperview/main", get(handlers::main_page))
        .route("/hyperview/login", get(handlers::login_page))
        .route("/hyperview/mobile/logout", get(handlers::mobile_logout))
        .route("/hyperview/user", get(handlers::user_info))
        .route("/hyperview/append", get(handlers::append_content))
        .route("/hyperview/status/connected", get(handlers::connected_status))
        .route("/hyperview/status/disconnected", get(handlers::disconnected_status))
        .route("/hyperview/repo/{owner}/{repo}/repomap", get(handlers::generate_repomap))
        .route("/hyperview/repo/{owner}/{repo}/content/{*path}", get(handlers::content))
        .route("/hyperview/repos", get(handlers::github_repos))
        .route("/hyperview/repo/{owner}/{repo}/issues", get(handlers::github_issues))
        .route("/hyperview/repo/{owner}/{repo}/issues/{number}/analyze", get(handlers::analyze_issue))
        .route("/hyperview/solver/{solver_id}/status", get(handlers::solver_status))
        .route("/hyperview/solver/{solver_id}/files", get(handlers::solver_files))
        .route("/hyperview/solver/{solver_id}/diffs", get(handlers::solver_diffs))
        .route("/hyperview/solver/{solver_id}/approve/{change_id}", get(handlers::approve_change))
        .route("/hyperview/solver/{solver_id}/reject/{change_id}", get(handlers::reject_change))
        .route("/hyperview/ws", get(ws::hyperview_ws_handler))
        .route("/hyperview/fragments/user-info", get(handlers::user_info))
        .route("/hyperview/fragments/github-repos", get(handlers::github_repos))
        .route("/hyperview/fragments/content", get(handlers::content))
        .route("/templates/pages/auth/login.xml", get(handlers::login_page))
        .route("/templates/pages/main.xml", get(handlers::main_page))
        .route("/hyperview/solve-demo-modal", get(solve_demo_modal))
}

pub async fn solve_demo_modal() -> impl IntoResponse {
    tracing::info!("solve_demo_modal handler called");
    let modal_xml = r###"<?xml version="1.0" encoding="UTF-8" ?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="modalHeader"
        backgroundColor="#111111"
        flexDirection="row"
        justifyContent="space-between"
        alignItems="center"
        padding="16"
        borderBottomWidth="1"
        borderBottomColor="#333333" />

      <style id="modalTitle"
        color="#ffffff"
        fontSize="20"
        fontWeight="600" />

      <style id="closeButton"
        color="#0A84FF"
        fontSize="16" />

      <style id="modalBody"
        backgroundColor="#000000"
        flex="1" />

      <style id="content"
        color="#ffffff"
        fontSize="16"
        padding="16" />
    </styles>

    <body style="modalBody" safe-area="true">
      <header style="modalHeader">
        <text style="modalTitle">Demo Modal</text>
        <text style="closeButton" action="close" href="#">Close</text>
      </header>

      <text style="content">This is a demo modal screen.</text>
    </body>
  </screen>
</doc>"###;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/vnd.hyperview+xml")
        .body(Body::from(modal_xml))
        .unwrap()
}

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
        .route("/hyperview/screen2", get(demo_screen2))
        .route("/hyperview/screen2-redirect", get(screen2_redirect))
        .route("/hyperview/modal-redirect", get(modal_redirect))
}

pub async fn demo_home() -> impl IntoResponse {
    let xml = r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="container" flex="1" backgroundColor="black" paddingTop="48" />
      <style id="userInfoText" color="white" fontSize="16" />
      <style fontSize="16" id="Description" margin="24" marginBottom="0" />
      <style fontFamily="HKGrotesk-Medium" color="white" fontSize="24" id="Basic" margin="24" />
      <style fontFamily="HKGrotesk-Bold" color="white" fontSize="16" id="Bold" margin="24" />
      <style backgroundColor="#63CB76" color="white" fontSize="32" id="Color" margin="24" padding="16" />
    </styles>

    <body scroll="true" style="container">
      <text style="Basic">Home Screen</text>
      <text style="Color">Welcome to Demo!</text>

      <view>
        <behavior trigger="press" action="push" href="/screen2" />
        <text style="Basic">Go to Screen 2</text>
      </view>
    </body>
  </screen>
</doc>"###;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/vnd.hyperview+xml")
        .body(Body::from(xml))
        .unwrap()
}

pub async fn demo_screen2() -> impl IntoResponse {
    let xml = r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="container" flex="1" backgroundColor="#2C3E50" paddingTop="48" />
      <style fontFamily="HKGrotesk-Medium" color="white" fontSize="24" id="Basic" margin="24" />
      <style backgroundColor="#E74C3C" color="white" fontSize="32" id="Color" margin="24" padding="16" />
      <style id="button" backgroundColor="#444" padding="16" marginBottom="16" borderRadius="8" alignItems="center" />
      <style id="buttonText" color="white" fontSize="16" />
    </styles>

    <body scroll="true" style="container">
      <text style="Basic">Screen 2</text>
      <text style="Color">Second Demo Screen!</text>

      <view style="button">
        <behavior trigger="press" action="back" />
        <text style="buttonText">Go Back</text>
      </view>
    </body>
  </screen>
</doc>"###;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/vnd.hyperview+xml")
        .body(Body::from(xml))
        .unwrap()
}

pub async fn demo_screen3() -> impl IntoResponse {
    let xml = r###"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="container" flex="1" backgroundColor="#8E44AD" paddingTop="48" />
      <style fontFamily="HKGrotesk-Medium" color="white" fontSize="24" id="Basic" margin="24" />
      <style backgroundColor="#2ECC71" color="white" fontSize="32" id="Color" margin="24" padding="16" />
    </styles>

    <body scroll="true" style="container">
      <text style="Basic">Screen 3 (Modal)</text>
      <text style="Color">Modal Demo Screen!</text>

      <view>
        <behavior trigger="press" action="back" />
        <text style="Basic">Go Back</text>
      </view>
    </body>
  </screen>
</doc>"###;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/vnd.hyperview+xml")
        .body(Body::from(xml))
        .unwrap()
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

pub async fn screen2_redirect() -> impl IntoResponse {
    let xml = r###"<behavior xmlns="https://hyperview.org/hyperview" trigger="load" action="push" href="/hyperview/screen2" />"###;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/vnd.hyperview+xml")
        .body(Body::from(xml))
        .unwrap()
}

pub async fn modal_redirect() -> impl IntoResponse {
    let xml = r###"<behavior xmlns="https://hyperview.org/hyperview" trigger="load" action="new" href="/hyperview/solve-demo-modal" />"###;

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/vnd.hyperview+xml")
        .body(Body::from(xml))
        .unwrap()
}

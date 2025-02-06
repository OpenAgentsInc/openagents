use crate::server::config::AppState;
use axum::{
    extract::State,
    http::{header, StatusCode},
    response::Response,
};

pub async fn hello_world(State(_state): State<AppState>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="text" alignItems="center" justifyContent="center" />
    </styles>
    <body>
      <view style="text">
        <text>Hello from OpenAgents!</text>
      </view>
    </body>
  </screen>
</doc>"#
            )
            .into(),
        )
        .unwrap()
}

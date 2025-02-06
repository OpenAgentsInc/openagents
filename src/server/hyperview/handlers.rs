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
            r#"<?xml version="1.0" encoding="UTF-8"?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <styles>
      <style id="text" alignItems="center" justifyContent="center" height="800" backgroundColor="black" />
      <style id="textstyle" color="white" fontWeight="bold" fontSize="24" />
    </styles>
    <body>
      <view style="text">
        <text style="textstyle">Onyx</text>
      </view>
    </body>
  </screen>
</doc>"#
                .to_string()
                .into(),
        )
        .unwrap()
}

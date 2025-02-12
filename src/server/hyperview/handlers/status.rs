use axum::{
    http::{header, StatusCode},
    response::Response,
};

pub async fn connected_status() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<text xmlns="https://hyperview.org/hyperview" style="statusText,statusConnected">
  Connected
</text>"###
                .into(),
        )
        .unwrap()
}

pub async fn disconnected_status() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(
            r###"<?xml version="1.0" encoding="UTF-8"?>
<text xmlns="https://hyperview.org/hyperview" style="statusText,statusDisconnected">
  Disconnected - Reconnecting...
</text>"###
                .into(),
        )
        .unwrap()
}
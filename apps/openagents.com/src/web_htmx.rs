use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::web_maud::render_notice_fragment as render_maud_notice_fragment;

const CACHE_NO_STORE: &str = "no-store";
const HX_REDIRECT_HEADER: &str = "HX-Redirect";
const HX_PUSH_URL_HEADER: &str = "HX-Push-Url";
const HX_TRIGGER_HEADER: &str = "HX-Trigger";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmxRequest {
    pub is_hx_request: bool,
    pub target: Option<String>,
    pub trigger: Option<String>,
    pub boosted: bool,
    pub history_restore_request: bool,
    pub current_url: Option<String>,
}

pub fn classify_request(headers: &HeaderMap) -> HtmxRequest {
    HtmxRequest {
        is_hx_request: header_is_true(headers, "hx-request"),
        target: header_text(headers, "hx-target"),
        trigger: header_text(headers, "hx-trigger"),
        boosted: header_is_true(headers, "hx-boosted"),
        history_restore_request: header_is_true(headers, "hx-history-restore-request"),
        current_url: header_text(headers, "hx-current-url"),
    }
}

pub fn is_hx_request(headers: &HeaderMap) -> bool {
    classify_request(headers).is_hx_request
}

pub fn notice_response(
    target_id: &str,
    status: &str,
    is_error: bool,
    http_status: StatusCode,
) -> Response {
    fragment_response(
        render_maud_notice_fragment(target_id, status, is_error),
        http_status,
    )
}

pub fn fragment_response(fragment_html: String, http_status: StatusCode) -> Response {
    (
        http_status,
        [
            (CONTENT_TYPE, "text/html; charset=utf-8"),
            (CACHE_CONTROL, CACHE_NO_STORE),
        ],
        fragment_html,
    )
        .into_response()
}

pub fn redirect_response(location: &str) -> Response {
    let mut response = StatusCode::OK.into_response();
    if let Ok(value) = HeaderValue::from_str(location) {
        response.headers_mut().insert(HX_REDIRECT_HEADER, value);
    }
    response
}

pub fn set_push_url_header(response: &mut Response, url: &str) {
    if let Ok(value) = HeaderValue::from_str(url) {
        response.headers_mut().insert(HX_PUSH_URL_HEADER, value);
    }
}

pub fn set_trigger_header(response: &mut Response, event: &str) {
    if let Ok(value) = HeaderValue::from_str(event) {
        response.headers_mut().insert(HX_TRIGGER_HEADER, value);
    }
}

fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn header_is_true(headers: &HeaderMap, name: &str) -> bool {
    header_text(headers, name)
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        classify_request, fragment_response, redirect_response, set_push_url_header,
        set_trigger_header,
    };
    use axum::body::to_bytes;
    use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
    use axum::http::{HeaderMap, StatusCode};

    #[test]
    fn classify_request_reads_htmx_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("hx-request", "true".parse().expect("valid value"));
        headers.insert("hx-target", "chat-status".parse().expect("valid value"));
        headers.insert("hx-trigger", "send-form".parse().expect("valid value"));
        headers.insert("hx-boosted", "true".parse().expect("valid value"));
        headers.insert(
            "hx-history-restore-request",
            "true".parse().expect("valid value"),
        );
        headers.insert(
            "hx-current-url",
            "https://openagents.test/chat/t_123"
                .parse()
                .expect("valid value"),
        );

        let classified = classify_request(&headers);
        assert!(classified.is_hx_request);
        assert_eq!(classified.target.as_deref(), Some("chat-status"));
        assert_eq!(classified.trigger.as_deref(), Some("send-form"));
        assert!(classified.boosted);
        assert!(classified.history_restore_request);
        assert_eq!(
            classified.current_url.as_deref(),
            Some("https://openagents.test/chat/t_123")
        );
    }

    #[tokio::test]
    async fn fragment_response_sets_html_content_type_and_no_store_cache() {
        let response = fragment_response("<div id=\"status\">ok</div>".to_string(), StatusCode::OK);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            response
                .headers()
                .get(CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        assert_eq!(body.as_ref(), b"<div id=\"status\">ok</div>");
    }

    #[test]
    fn redirect_response_sets_hx_redirect_header() {
        let response = redirect_response("/chat/thread-123");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("HX-Redirect")
                .and_then(|value| value.to_str().ok()),
            Some("/chat/thread-123")
        );
    }

    #[test]
    fn set_push_url_header_sets_hx_push_url() {
        let mut response = fragment_response("<div>ok</div>".to_string(), StatusCode::OK);
        set_push_url_header(&mut response, "/chat/thread-xyz");
        assert_eq!(
            response
                .headers()
                .get("HX-Push-Url")
                .and_then(|value| value.to_str().ok()),
            Some("/chat/thread-xyz")
        );
    }

    #[test]
    fn set_trigger_header_sets_hx_trigger() {
        let mut response = fragment_response("<div>ok</div>".to_string(), StatusCode::OK);
        set_trigger_header(&mut response, "chat-message-sent");
        assert_eq!(
            response
                .headers()
                .get("HX-Trigger")
                .and_then(|value| value.to_str().ok()),
            Some("chat-message-sent")
        );
    }
}

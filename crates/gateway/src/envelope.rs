//! One error shape and one request id on every answer (#11156,
//! docs/api/design.md section 2.5).
//!
//! Every route here answers a refusal as
//! `{"error": {"type", "code", "message", "param", "request_id"}}`:
//! `type` is the class (`invalid_request`, `authentication`, `permission`,
//! `payment_required`, `limit_reached`, `not_found`, `conflict`,
//! `rate_limited`, `server_error`, `unavailable`), `code` the specific
//! reason. The inference routes already answer with `type`, `code`, and
//! `param`; the account, decision, and skills routes answered only `code`
//! and `message`. Rather than touch every refusal, [`layer`] fills in what
//! a JSON error body lacks on the way out, keeping every field it already
//! had, so a client that reads `error.code` keeps working.
//!
//! Every answer also carries `x-request-id`: the one the route set (the
//! inference routes set their own), or a new `req_...` id.
//!
//! Lists answer `{"data": [...], "next": "<cursor>" or null}` and take
//! `?after=<cursor>`; [`list`] adds those beside a route's older fields
//! (`items`, `entries`, `next_cursor`, `cursor`) for one release.

use std::sync::atomic::{AtomicU64, Ordering};

use axum::body::Body;
use axum::extract::Request;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use serde_json::{Value, json};

/// The response header every answer carries.
pub const REQUEST_ID: &str = "x-request-id";

/// The largest error body [`layer`] rewrites; anything larger passes as is.
const MAX_ERROR_BODY: usize = 256 * 1024;

/// The error class for an HTTP status (section 2.5).
#[must_use]
pub fn error_type(status: u16) -> &'static str {
    match status {
        401 => "authentication",
        402 => "payment_required",
        403 => "permission",
        404 => "not_found",
        409 => "conflict",
        429 => "rate_limited",
        503 => "unavailable",
        400..=499 => "invalid_request",
        _ => "server_error",
    }
}

/// A new request id: `req_` and 24 hex digits, unique within the process
/// and unlikely to repeat across processes.
#[must_use]
pub fn mint_request_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!(
        "req_{nanos:016x}{:08x}",
        (count as u32) ^ std::process::id()
    )
}

/// Fill in the shared error object on a JSON body, keeping every field it
/// already has. `None` when the body is not an error body.
#[must_use]
pub fn normalize(status: u16, mut body: Value, request_id: &str) -> Option<Value> {
    let kind = error_type(status);
    let error = body.get_mut("error")?;
    if let Some(text) = error.as_str() {
        // `{"error": "text"}`: the text is the code when it reads like one
        // (`invalid_request`), otherwise the message.
        {
            let text = text.to_owned();
            let is_code = !text.is_empty()
                && text
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            let (code, message) = if is_code {
                (text, default_message(kind).to_owned())
            } else {
                (kind.to_owned(), text)
            };
            *error = json!({
                "type": kind,
                "code": code,
                "message": message,
                "param": Value::Null,
                "request_id": request_id,
            });
        }
    } else if let Some(fields) = error.as_object_mut() {
        {
            let code_limit = fields.get("code").and_then(Value::as_str) == Some("limit_reached");
            fields.entry("type").or_insert_with(|| {
                json!(if status == 403 && code_limit {
                    "limit_reached"
                } else {
                    kind
                })
            });
            let class = fields
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or(kind)
                .to_owned();
            fields.entry("code").or_insert_with(|| json!(class));
            fields
                .entry("message")
                .or_insert_with(|| json!(default_message(kind)));
            fields.entry("param").or_insert(Value::Null);
            fields
                .entry("request_id")
                .or_insert_with(|| json!(request_id));
        }
    } else {
        return None;
    }
    Some(body)
}

fn default_message(kind: &str) -> &'static str {
    match kind {
        "authentication" => "Sign in or send an API key.",
        "permission" => "You can't do that here.",
        "payment_required" => "This needs payment first.",
        "not_found" => "Nothing is here.",
        "conflict" => "That conflicts with what is already here.",
        "rate_limited" => "Too many requests. Try again shortly.",
        "unavailable" => "This is unavailable right now. Try again shortly.",
        "server_error" => "Something went wrong on our side. Try again.",
        _ => "That request isn't valid.",
    }
}

/// Add the shared list fields beside a route's own: `data` (the page's
/// items) and `next` (the cursor for `?after=`, or null).
pub fn list(body: &mut Value, items: &Value, next: &Value) {
    body["data"] = items.clone();
    body["next"] = next.clone();
}

fn is_json(response: &Response) -> bool {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            let value = value.to_ascii_lowercase();
            value.starts_with("application/json") || value.starts_with("application/problem+json")
        })
}

/// A decision call's forwarded answer or refusal is the backend's own
/// bytes, and its receipt commits to those bytes (`x-outcome: answered` or
/// `refused`), so it passes unchanged.
fn carries_backend_bytes(response: &Response) -> bool {
    response
        .headers()
        .get("x-outcome")
        .is_some_and(|value| value == "answered" || value == "refused")
}

/// The middleware: a request id on every answer, and the shared error
/// object on every JSON refusal.
pub async fn layer(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let request_id = match response
        .headers()
        .get(REQUEST_ID)
        .and_then(|value| value.to_str().ok())
    {
        Some(id) => id.to_owned(),
        None => {
            let id = mint_request_id();
            if let Ok(value) = HeaderValue::from_str(&id) {
                response.headers_mut().insert(REQUEST_ID, value);
            }
            id
        }
    };
    let status = response.status();
    if status.as_u16() < 400 || !is_json(&response) || carries_backend_bytes(&response) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, MAX_ERROR_BODY).await {
        Ok(bytes) => bytes,
        // Too large or broken: the body is gone, so answer the bare class.
        Err(_) => {
            let body =
                normalize(status.as_u16(), json!({"error": {}}), &request_id).unwrap_or_default();
            parts.headers.remove(CONTENT_LENGTH);
            return Response::from_parts(parts, Body::from(body.to_string()));
        }
    };
    let rewritten = serde_json::from_slice::<Value>(&bytes)
        .ok()
        .and_then(|value| normalize(status.as_u16(), value, &request_id))
        .and_then(|value| serde_json::to_vec(&value).ok());
    match rewritten {
        Some(body) => {
            parts.headers.remove(CONTENT_LENGTH);
            Response::from_parts(parts, Body::from(body))
        }
        None => Response::from_parts(parts, Body::from(bytes)),
    }
}

/// A refusal in the shared shape, for code that builds one directly.
#[must_use]
pub fn refusal(status: StatusCode, code: &str, message: &str) -> Response {
    let body = json!({"error": {
        "type": error_type(status.as_u16()),
        "code": code,
        "message": message,
        "param": Value::Null,
    }});
    let mut response = Response::new(Body::from(body.to_string()));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

/// When the older paths were marked deprecated (2026-10-09), as the
/// `Deprecation` header's `@<unix time>` (RFC 9745).
pub const DEPRECATED_SINCE: &str = "@1791504000";

/// Mark an answer from an older path (docs/api/design.md section 2.4):
/// `Deprecation` and `Link: <successor>; rel="successor-version"`. A
/// FIRST-PARTY older path stays until the two newest client releases stop
/// calling it, so it names no `Sunset`.
pub fn mark_deprecated(response: &mut Response, successor: &str) {
    let headers = response.headers_mut();
    headers.insert("deprecation", HeaderValue::from_static(DEPRECATED_SINCE));
    if let Ok(link) = HeaderValue::from_str(&format!("<{successor}>; rel=\"successor-version\"")) {
        headers.insert(axum::http::header::LINK, link);
    }
}

/// `route` answering at an older path: the same handler as at its
/// successor, which is the request's path with the first `older` replaced
/// by `newer`, marked with [`mark_deprecated`].
#[must_use]
pub fn deprecated<S>(
    route: axum::routing::MethodRouter<S>,
    older: &'static str,
    newer: &'static str,
) -> axum::routing::MethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    route.layer(axum::middleware::map_response(
        move |uri: axum::http::Uri, mut response: Response| async move {
            let successor = uri.path().replacen(older, newer, 1);
            mark_deprecated(&mut response, &successor);
            response
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_older_path_names_its_successor() {
        let mut response = refusal(StatusCode::NOT_FOUND, "not_found", "No.");
        mark_deprecated(&mut response, "/v1/projects");
        assert_eq!(response.headers()["deprecation"], DEPRECATED_SINCE);
        assert_eq!(
            response.headers()[axum::http::header::LINK],
            "</v1/projects>; rel=\"successor-version\""
        );
    }

    #[test]
    fn an_account_refusal_gains_type_param_and_request_id_and_keeps_its_fields() {
        let body =
            json!({"error": {"code": "workspace_not_found", "message": "No such workspace."}});
        let out = normalize(404, body, "req_1").unwrap();
        assert_eq!(
            out["error"],
            json!({
                "type": "not_found",
                "code": "workspace_not_found",
                "message": "No such workspace.",
                "param": null,
                "request_id": "req_1",
            })
        );
    }

    #[test]
    fn an_inference_refusal_keeps_its_own_type() {
        let body = json!({"error": {"type": "unauthorized", "code": null, "param": null, "message": "No key."}});
        let out = normalize(401, body, "req_2").unwrap();
        assert_eq!(out["error"]["type"], "unauthorized");
        assert_eq!(out["error"]["code"], Value::Null);
        assert_eq!(out["error"]["request_id"], "req_2");
    }

    #[test]
    fn a_text_error_becomes_the_object() {
        let out = normalize(401, json!({"error": "unauthorized"}), "req_3").unwrap();
        assert_eq!(out["error"]["type"], "authentication");
        assert_eq!(out["error"]["code"], "unauthorized");
        assert!(out["error"]["message"].is_string());
        let out = normalize(503, json!({"error": "meter unavailable"}), "req_4").unwrap();
        assert_eq!(out["error"]["code"], "unavailable");
        assert_eq!(out["error"]["message"], "meter unavailable");
    }

    #[test]
    fn a_message_only_error_gains_a_code() {
        let out = normalize(
            403,
            json!({"error": {"message": "An admin token is required."}}),
            "r",
        )
        .unwrap();
        assert_eq!(out["error"]["type"], "permission");
        assert_eq!(out["error"]["code"], "permission");
    }

    #[test]
    fn a_limit_refusal_is_its_own_class() {
        let out = normalize(
            403,
            json!({"error": {"code": "limit_reached", "message": "Cap."}}),
            "r",
        )
        .unwrap();
        assert_eq!(out["error"]["type"], "limit_reached");
    }

    #[test]
    fn a_body_without_an_error_is_left_alone() {
        assert!(normalize(400, json!({"status": "nope"}), "r").is_none());
    }

    #[test]
    fn every_status_has_a_class() {
        assert_eq!(error_type(400), "invalid_request");
        assert_eq!(error_type(410), "invalid_request");
        assert_eq!(error_type(500), "server_error");
        assert_eq!(error_type(502), "server_error");
        assert_eq!(error_type(503), "unavailable");
    }

    #[test]
    fn request_ids_differ() {
        assert_ne!(mint_request_id(), mint_request_id());
        assert!(mint_request_id().starts_with("req_"));
    }

    #[tokio::test]
    async fn the_layer_fills_in_a_refusal_and_stamps_a_request_id() {
        use axum::routing::get;
        let app = axum::Router::new()
            .route(
                "/refuse",
                get(|| async {
                    crate::accounts::refused(
                        StatusCode::NOT_FOUND,
                        "workspace_not_found",
                        "No such workspace.",
                    )
                }),
            )
            .route("/ok", get(|| async { axum::Json(json!({"ok": true})) }))
            .route(
                "/forwarded",
                get(|| async {
                    let mut response = crate::accounts::refused(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "too_many_options",
                        "The door's own words.",
                    );
                    response
                        .headers_mut()
                        .insert("x-outcome", HeaderValue::from_static("refused"));
                    response
                }),
            )
            .layer(axum::middleware::from_fn(layer));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let answer = reqwest::get(format!("http://{address}/refuse"))
            .await
            .unwrap();
        assert_eq!(answer.status(), StatusCode::NOT_FOUND);
        let id = answer.headers()[REQUEST_ID].to_str().unwrap().to_owned();
        assert!(id.starts_with("req_"));
        let body: Value = answer.json().await.unwrap();
        assert_eq!(body["error"]["type"], "not_found");
        assert_eq!(body["error"]["code"], "workspace_not_found");
        assert_eq!(body["error"]["message"], "No such workspace.");
        assert_eq!(body["error"]["param"], Value::Null);
        assert_eq!(body["error"]["request_id"], id.as_str());

        let answer = reqwest::get(format!("http://{address}/ok")).await.unwrap();
        assert!(answer.headers().contains_key(REQUEST_ID));
        assert_eq!(answer.json::<Value>().await.unwrap(), json!({"ok": true}));

        // A forwarded refusal is the door's bytes, which its receipt seals.
        let answer = reqwest::get(format!("http://{address}/forwarded"))
            .await
            .unwrap();
        let body: Value = answer.json().await.unwrap();
        assert!(body["error"].get("type").is_none());
    }

    #[test]
    fn lists_gain_data_and_next() {
        let mut body = json!({"items": [1, 2], "next_cursor": "c"});
        let items = body["items"].clone();
        let next = body["next_cursor"].clone();
        list(&mut body, &items, &next);
        assert_eq!(body["data"], json!([1, 2]));
        assert_eq!(body["next"], "c");
        assert_eq!(body["next_cursor"], "c");
    }
}

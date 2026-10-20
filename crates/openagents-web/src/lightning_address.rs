//! Same-origin Lightning address paths, forwarded to the account authority.
use crate::App;
use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::any,
};
use serde_json::json;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/.well-known/lnurlp/{name}", any(public))
        .route("/lnurlp/{name}", any(public))
        .route("/lnurlp/{name}/invoice", any(public))
        .route("/lnurlp/verify/{hash}", any(public))
        .route("/lnurlpay/{*path}", any(public))
        .route("/v1/device/lightning-address", any(management))
        .route("/v1/device/lightning-address/challenge", any(management))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
}
pub(crate) fn owns(path: &str) -> bool {
    path.starts_with("/.well-known/lnurlp/")
        || path.starts_with("/lnurlp/")
        || path.starts_with("/lnurlpay/")
        || path == "/v1/device/lightning-address"
        || path == "/v1/device/lightning-address/challenge"
}
fn reply(status: StatusCode, value: serde_json::Value, public: bool) -> Response {
    let mut response = (status, axum::Json(value)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    if public {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            HeaderValue::from_static("*"),
        );
    }
    response
}
fn unavailable(public: bool) -> Response {
    reply(
        StatusCode::SERVICE_UNAVAILABLE,
        json!({"status":"ERROR","reason":"Receiving payments isn't available right now. Try again later."}),
        public,
    )
}
async fn public(
    State(app): State<App>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !matches!(method, Method::GET | Method::POST)
        || body.len() > 16 * 1024
        || uri
            .path_and_query()
            .map_or("/", |p| p.as_str())
            .to_owned()
            .len()
            > 16 * 1024
    {
        return reply(
            StatusCode::BAD_REQUEST,
            json!({"status":"ERROR","reason":"That payment request isn't supported."}),
            true,
        );
    }
    forward(
        &app,
        method,
        &uri.path_and_query().map_or("/", |p| p.as_str()).to_owned(),
        &headers,
        None,
        body,
        true,
    )
    .await
}
async fn management(
    State(app): State<App>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !matches!(
        method,
        Method::GET | Method::POST | Method::PUT | Method::DELETE
    ) || body.len() > 16 * 1024
    {
        return unavailable(false);
    }
    // Device calls always carry their own account session, never a cookie-only write.
    let Some(authorization) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.starts_with("Bearer ") && v.len() < 8192)
    else {
        return reply(
            StatusCode::UNAUTHORIZED,
            json!({"error":{"code":"unauthenticated","message":"Log in again to continue."}}),
            false,
        );
    };
    let path = uri.path().replacen("/v1/device/", "/v1/account/", 1);
    forward(
        &app,
        method,
        &path,
        &headers,
        Some(authorization),
        body,
        false,
    )
    .await
}
async fn forward(
    app: &App,
    method: Method,
    path: &str,
    headers: &HeaderMap,
    authorization: Option<&str>,
    body: Bytes,
    public: bool,
) -> Response {
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable(public);
    };
    match service
        .lightning_call(method, path, authorization, &body, headers)
        .await
    {
        Ok(answer) => reply(
            StatusCode::from_u16(answer.status).unwrap_or(StatusCode::BAD_GATEWAY),
            answer.body,
            public,
        ),
        Err(_) => unavailable(public),
    }
}

/// Device-session routes used by the native wallets and CLI.
pub(crate) fn extend_contract(document: &mut serde_json::Value) {
    for (method, path, id, summary, request) in [
        (
            "get",
            "/v1/device/lightning-address",
            "getDeviceLightningAddresses",
            "Read your Lightning addresses",
            serde_json::Value::Null,
        ),
        (
            "post",
            "/v1/device/lightning-address/challenge",
            "challengeDeviceLightningAddress",
            "Request a challenge to link your wallet",
            json!({"expected_revision":{"type":"integer","minimum":0}}),
        ),
        (
            "put",
            "/v1/device/lightning-address",
            "enrollDeviceLightningAddress",
            "Prove your wallet controls this address",
            json!({"challenge":{"type":"string","minLength":64,"maxLength":64},"pubkey":{"type":"string","minLength":66,"maxLength":66},"signature":{"type":"string","maxLength":144}}),
        ),
        (
            "delete",
            "/v1/device/lightning-address",
            "disableDeviceLightningAddress",
            "Stop receiving at your address",
            json!({"name":{"type":"string","maxLength":30},"expected_revision":{"type":"integer","minimum":1}}),
        ),
    ] {
        let mut operation = json!({
            "operationId":id,"tags":["Wallet"],"summary":summary,
            "description":"Uses your current account session. Wallet signatures are made locally; never send recovery words or private keys. Responses are not cached.",
            "security":[{"appBearer":[]}],
            "responses":{
                "200":{"description":"The current address state or a challenge to link your wallet.","content":{"application/json":{"schema":{"type":"object"}}}},
                "400":{"description":"Check the request and wallet signature."},
                "401":{"description":"Log in again to continue."},
                "409":{"description":"Refresh the address revision and try again."},
                "503":{"description":"Receiving is unavailable or has not been configured."}
            }
        });
        if let Some(fields) = request.as_object() {
            let required: Vec<_> = fields.keys().collect();
            operation["requestBody"] = json!({"required":true,"content":{"application/json":{"schema":{"type":"object","additionalProperties":false,"required":required,"properties":request}}}});
        }
        document["paths"][path][method] = operation;
    }
}

use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::compat::{WalletCompatHttpError, WalletCompatService};
use crate::service::{InvoicePaymentRequest, WalletExecutorError, WalletExecutorService};

#[derive(Clone)]
struct AppState {
    service: Arc<WalletExecutorService>,
    compat: Arc<WalletCompatService>,
}

pub struct WalletExecutorHttpServer {
    pub address: String,
    shutdown: Option<oneshot::Sender<()>>,
    join: tokio::task::JoinHandle<()>,
}

impl WalletExecutorHttpServer {
    pub async fn close(mut self) -> Result<(), String> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.join
            .await
            .map_err(|error| format!("server join failed: {error}"))
    }
}

pub async fn make_wallet_executor_http_server(
    service: Arc<WalletExecutorService>,
    compat: Arc<WalletCompatService>,
) -> Result<WalletExecutorHttpServer, String> {
    let state = AppState { service, compat };
    let app = build_router(state.clone());

    let bind_host = state
        .service
        .config()
        .host
        .parse::<std::net::IpAddr>()
        .map_err(|error| format!("invalid bind host: {error}"))?;
    let listener = TcpListener::bind((bind_host, state.service.config().port))
        .await
        .map_err(|error| format!("failed to bind listener: {error}"))?;

    let address = format!(
        "http://{}",
        listener.local_addr().map_err(|error| error.to_string())?
    );
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let join = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    Ok(WalletExecutorHttpServer {
        address,
        shutdown: Some(shutdown_tx),
        join,
    })
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/status", get(status))
        .route("/pay-bolt11", post(pay_bolt11))
        .route("/create-invoice", post(create_invoice))
        .route("/receive-address", get(receive_address))
        .route("/wallets/create", post(wallets_create))
        .route("/wallets/status", post(wallets_status))
        .route("/wallets/create-invoice", post(wallets_create_invoice))
        .route("/wallets/pay-bolt11", post(wallets_pay_bolt11))
        .route("/wallets/send-spark", post(wallets_send_spark))
        .with_state(state)
}

async fn healthz(headers: HeaderMap) -> Response {
    let request_id = request_id_from_headers(&headers);
    json_response(
        StatusCode::OK,
        &request_id,
        json!({ "ok": true, "requestId": request_id }),
        false,
    )
}

async fn status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from_headers(&headers);

    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let status = state.service.status().await;
    json_response(
        StatusCode::OK,
        &request_id,
        json!({ "ok": true, "requestId": request_id, "status": status }),
        false,
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayBolt11Body {
    request_id: Option<String>,
    payment: PayBolt11PaymentBody,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PayBolt11PaymentBody {
    invoice: String,
    max_amount_msats: u64,
    host: String,
}

async fn pay_bolt11(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let request_id = request_id_from_headers(&headers);

    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let parsed: PayBolt11Body = match parse_json_body(&body) {
        Ok(value) => value,
        Err(message) => {
            return invalid_request_response(&request_id, &message);
        }
    };

    let payment = InvoicePaymentRequest {
        invoice: parsed.payment.invoice,
        max_amount_msats: parsed.payment.max_amount_msats,
        host: parsed.payment.host,
    };

    match state
        .service
        .pay_bolt11(payment, parsed.request_id.or(Some(request_id.clone())))
        .await
    {
        Ok(result) => json_response(
            StatusCode::OK,
            &request_id,
            json!({ "ok": true, "requestId": request_id, "result": result }),
            false,
        ),
        Err(error) => wallet_executor_error_response(error, &request_id),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateInvoiceBody {
    amount_sats: u64,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    expiry_secs: Option<u64>,
}

async fn create_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = request_id_from_headers(&headers);

    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let parsed: CreateInvoiceBody = match parse_json_body(&body) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&request_id, &message),
    };
    if parsed.amount_sats == 0 {
        return invalid_request_response(&request_id, "amount_sats must be > 0");
    }

    match state
        .service
        .create_invoice(
            parsed.amount_sats,
            parsed.description,
            parsed.expiry_secs,
            Some(request_id.clone()),
        )
        .await
    {
        Ok(result) => json_response(
            StatusCode::OK,
            &request_id,
            json!({ "ok": true, "requestId": request_id, "result": result }),
            false,
        ),
        Err(error) => wallet_executor_error_response(error, &request_id),
    }
}

async fn receive_address(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let request_id = request_id_from_headers(&headers);

    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let network = match state.service.config().network {
        crate::config::SparkNetwork::Mainnet => "mainnet",
        crate::config::SparkNetwork::Regtest => "regtest",
    };

    match state.service.receive_addresses().await {
        Ok(addresses) => json_response(
            StatusCode::OK,
            &request_id,
            json!({
                "ok": true,
                "requestId": request_id,
                "result": {
                    "sparkAddress": addresses.spark_address,
                    "bitcoinAddress": addresses.bitcoin_address,
                    "network": network,
                }
            }),
            false,
        ),
        Err(error) => wallet_executor_error_response(error, &request_id),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletCreateBody {
    wallet_id: String,
    mnemonic: Option<String>,
}

async fn wallets_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = request_id_from_headers(&headers);
    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let parsed: WalletCreateBody = match parse_json_body(&body) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&request_id, &message),
    };

    match state
        .compat
        .wallets_create(parsed.wallet_id, parsed.mnemonic)
        .await
    {
        Ok(result) => json_response(
            StatusCode::OK,
            &request_id,
            json!({ "ok": result.ok, "result": result.result }),
            false,
        ),
        Err(error) => compat_error_response(error, &request_id),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletStatusBody {
    wallet_id: String,
    mnemonic: String,
}

async fn wallets_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = request_id_from_headers(&headers);
    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let parsed: WalletStatusBody = match parse_json_body(&body) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&request_id, &message),
    };

    match state
        .compat
        .wallets_status(parsed.wallet_id, parsed.mnemonic)
        .await
    {
        Ok(result) => json_response(
            StatusCode::OK,
            &request_id,
            json!({ "ok": result.ok, "result": result.result }),
            false,
        ),
        Err(error) => compat_error_response(error, &request_id),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletCreateInvoiceBody {
    wallet_id: String,
    mnemonic: String,
    amount_sats: u64,
    description: Option<String>,
}

async fn wallets_create_invoice(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = request_id_from_headers(&headers);
    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let parsed: WalletCreateInvoiceBody = match parse_json_body(&body) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&request_id, &message),
    };

    match state
        .compat
        .wallets_create_invoice(
            parsed.wallet_id,
            parsed.mnemonic,
            parsed.amount_sats,
            parsed.description,
        )
        .await
    {
        Ok(result) => json_response(
            StatusCode::OK,
            &request_id,
            json!({ "ok": result.ok, "result": result.result }),
            false,
        ),
        Err(error) => compat_error_response(error, &request_id),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletPayBolt11Body {
    wallet_id: String,
    mnemonic: String,
    invoice: String,
    max_amount_msats: u64,
    timeout_ms: Option<u64>,
    host: Option<String>,
}

async fn wallets_pay_bolt11(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = request_id_from_headers(&headers);
    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let parsed: WalletPayBolt11Body = match parse_json_body(&body) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&request_id, &message),
    };

    match state
        .compat
        .wallets_pay_bolt11(
            request_id.clone(),
            parsed.wallet_id,
            parsed.mnemonic,
            parsed.invoice,
            parsed.max_amount_msats,
            parsed.timeout_ms,
            parsed.host,
        )
        .await
    {
        Ok(result) => json_response(
            StatusCode::OK,
            &request_id,
            json!({ "ok": result.ok, "result": result.result }),
            false,
        ),
        Err(error) => compat_error_response(error, &request_id),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WalletSendSparkBody {
    wallet_id: String,
    mnemonic: String,
    spark_address: String,
    amount_sats: u64,
    timeout_ms: Option<u64>,
}

async fn wallets_send_spark(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let request_id = request_id_from_headers(&headers);
    if let Some(response) = authorize(&state, &headers, &request_id) {
        return response;
    }

    let parsed: WalletSendSparkBody = match parse_json_body(&body) {
        Ok(value) => value,
        Err(message) => return invalid_request_response(&request_id, &message),
    };

    match state
        .compat
        .wallets_send_spark(
            parsed.wallet_id,
            parsed.mnemonic,
            parsed.spark_address,
            parsed.amount_sats,
            parsed.timeout_ms,
        )
        .await
    {
        Ok(result) => json_response(
            StatusCode::OK,
            &request_id,
            json!({ "ok": result.ok, "result": result.result }),
            false,
        ),
        Err(error) => compat_error_response(error, &request_id),
    }
}

fn parse_json_body<T: serde::de::DeserializeOwned>(body: &Bytes) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|error| format!("invalid json body: {error}"))
}

fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

fn bearer_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("bearer ") {
        return None;
    }
    let token = trimmed[7..].trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

fn constant_time_eq(left: &str, right: &str) -> bool {
    let left_bytes = left.as_bytes();
    let right_bytes = right.as_bytes();
    if left_bytes.len() != right_bytes.len() {
        return false;
    }

    let mut diff = 0_u8;
    for (a, b) in left_bytes.iter().zip(right_bytes.iter()) {
        diff |= *a ^ *b;
    }
    diff == 0
}

fn authorize(state: &AppState, headers: &HeaderMap, request_id: &str) -> Option<Response> {
    let expected = state.service.config().auth_token.clone()?;
    let provided = bearer_token_from_headers(headers);

    if provided
        .as_deref()
        .map(|token| constant_time_eq(token, &expected))
        .unwrap_or(false)
    {
        return None;
    }

    Some(json_response(
        StatusCode::UNAUTHORIZED,
        request_id,
        json!({
            "ok": false,
            "error": {
                "requestId": request_id,
                "code": "unauthorized",
                "message": "missing or invalid bearer token"
            }
        }),
        true,
    ))
}

fn invalid_request_response(request_id: &str, message: &str) -> Response {
    json_response(
        StatusCode::BAD_REQUEST,
        request_id,
        json!({
            "ok": false,
            "error": {
                "requestId": request_id,
                "code": "invalid_request",
                "message": message
            }
        }),
        false,
    )
}

fn wallet_executor_error_response(error: WalletExecutorError, request_id: &str) -> Response {
    match error {
        WalletExecutorError::Policy(error) => {
            let mut details = serde_json::Map::new();
            if let Some(host) = error.host {
                let _ = details.insert("host".to_string(), Value::String(host));
            }
            if let Some(value) = error.max_allowed_msats {
                let _ = details.insert("maxAllowedMsats".to_string(), Value::from(value));
            }
            if let Some(value) = error.quoted_amount_msats {
                let _ = details.insert("quotedAmountMsats".to_string(), Value::from(value));
            }
            if let Some(value) = error.window_spend_msats {
                let _ = details.insert("windowSpendMsats".to_string(), Value::from(value));
            }
            if let Some(value) = error.window_cap_msats {
                let _ = details.insert("windowCapMsats".to_string(), Value::from(value));
            }

            json_response(
                StatusCode::FORBIDDEN,
                request_id,
                json!({
                    "ok": false,
                    "error": {
                        "requestId": request_id,
                        "code": error.code.as_str(),
                        "message": error.message,
                        "details": Value::Object(details),
                    }
                }),
                false,
            )
        }
        WalletExecutorError::Idempotency(error) => json_response(
            StatusCode::CONFLICT,
            request_id,
            json!({
                "ok": false,
                "error": {
                    "requestId": request_id,
                    "code": error.code.as_str(),
                    "message": error.message,
                }
            }),
            false,
        ),
        WalletExecutorError::Spark(error) => {
            let status = if matches!(
                error.code,
                crate::error::SparkGatewayErrorCode::PaymentPending
            ) {
                StatusCode::GATEWAY_TIMEOUT
            } else {
                StatusCode::BAD_GATEWAY
            };

            json_response(
                status,
                request_id,
                json!({
                    "ok": false,
                    "error": {
                        "requestId": request_id,
                        "code": error.code.as_str(),
                        "message": error.message,
                    }
                }),
                false,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use serde_json::Value;

    use crate::compat::WalletCompatService;
    use crate::config::WalletExecutorConfig;
    use crate::gateway::MockPaymentGateway;
    use crate::service::WalletExecutorService;

    use super::make_wallet_executor_http_server;

    async fn start_server() -> Result<(String, super::WalletExecutorHttpServer)> {
        let mut config = WalletExecutorConfig::default_mock();
        config.host = "127.0.0.1".to_string();
        config.port = 0;
        config.wallet_id = "wallet-test".to_string();
        config.auth_token = Some("test-token".to_string());

        let service = Arc::new(WalletExecutorService::new(
            config.clone(),
            Arc::new(MockPaymentGateway::new(None)),
        ));
        let compat = Arc::new(WalletCompatService::new(config));
        let server = make_wallet_executor_http_server(service, compat)
            .await
            .map_err(anyhow::Error::msg)?;
        Ok((server.address.clone(), server))
    }

    #[tokio::test]
    async fn create_invoice_requires_auth() -> Result<()> {
        let (base, server) = start_server().await?;
        let client = reqwest::Client::new();

        let res = client
            .post(format!("{base}/create-invoice"))
            .header("content-type", "application/json")
            .body(serde_json::json!({"amountSats": 1}).to_string())
            .send()
            .await?;
        assert_eq!(res.status(), reqwest::StatusCode::UNAUTHORIZED);

        server.close().await.map_err(anyhow::Error::msg)?;
        Ok(())
    }

    #[tokio::test]
    async fn create_invoice_is_idempotent_and_conflicts_on_mismatch() -> Result<()> {
        let (base, server) = start_server().await?;
        let client = reqwest::Client::new();

        let first = client
            .post(format!("{base}/create-invoice"))
            .header("authorization", "Bearer test-token")
            .header("x-request-id", "inv-test-1")
            .json(&serde_json::json!({
                "amountSats": 21,
                "description": "deposit",
                "expirySecs": 60,
            }))
            .send()
            .await?;
        assert!(first.status().is_success());
        let first_json: Value = first.json().await?;
        let invoice_first = first_json
            .pointer("/result/invoice")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let receipt_hash_first = first_json
            .pointer("/result/receipt/canonicalJsonSha256")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let second = client
            .post(format!("{base}/create-invoice"))
            .header("authorization", "Bearer test-token")
            .header("x-request-id", "inv-test-1")
            .json(&serde_json::json!({
                "amountSats": 21,
                "description": "deposit",
                "expirySecs": 60,
            }))
            .send()
            .await?;
        assert!(second.status().is_success());
        let second_json: Value = second.json().await?;
        let invoice_second = second_json
            .pointer("/result/invoice")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let receipt_hash_second = second_json
            .pointer("/result/receipt/canonicalJsonSha256")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        assert_eq!(invoice_first, invoice_second);
        assert_eq!(receipt_hash_first, receipt_hash_second);

        let conflict = client
            .post(format!("{base}/create-invoice"))
            .header("authorization", "Bearer test-token")
            .header("x-request-id", "inv-test-1")
            .json(&serde_json::json!({
                "amountSats": 22,
            }))
            .send()
            .await?;
        assert_eq!(conflict.status(), reqwest::StatusCode::CONFLICT);

        server.close().await.map_err(anyhow::Error::msg)?;
        Ok(())
    }

    #[tokio::test]
    async fn receive_address_is_stable_and_non_empty_in_mock_mode() -> Result<()> {
        let (base, server) = start_server().await?;
        let client = reqwest::Client::new();

        let first = client
            .get(format!("{base}/receive-address"))
            .header("authorization", "Bearer test-token")
            .send()
            .await?;
        assert!(first.status().is_success());
        let first_json: Value = first.json().await?;
        let spark_first = first_json
            .pointer("/result/sparkAddress")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let btc_first = first_json
            .pointer("/result/bitcoinAddress")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        assert!(!spark_first.is_empty());
        assert!(!btc_first.is_empty());
        assert_eq!(
            first_json
                .pointer("/result/network")
                .and_then(Value::as_str),
            Some("regtest")
        );

        let second = client
            .get(format!("{base}/receive-address"))
            .header("authorization", "Bearer test-token")
            .send()
            .await?;
        assert!(second.status().is_success());
        let second_json: Value = second.json().await?;
        let spark_second = second_json
            .pointer("/result/sparkAddress")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let btc_second = second_json
            .pointer("/result/bitcoinAddress")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        assert_eq!(spark_first, spark_second);
        assert_eq!(btc_first, btc_second);

        server.close().await.map_err(anyhow::Error::msg)?;
        Ok(())
    }
}

fn compat_error_response(error: WalletCompatHttpError, request_id: &str) -> Response {
    let mut body = json!({
        "ok": false,
        "error": {
            "requestId": request_id,
            "code": error.code,
            "message": error.message,
        }
    });

    if let Some(details) = error.details {
        body["error"]["details"] = details;
    }

    let status = StatusCode::from_u16(error.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    json_response(status, request_id, body, false)
}

fn json_response(
    status: StatusCode,
    request_id: &str,
    body: Value,
    include_www_authenticate: bool,
) -> Response {
    let mut response = Response::new(axum::body::Body::from(body.to_string()));
    *response.status_mut() = status;

    let headers = response.headers_mut();
    let _ = headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );

    if let Ok(value) = HeaderValue::from_str(request_id) {
        let _ = headers.insert("x-request-id", value);
    }

    if include_www_authenticate {
        let _ = headers.insert(
            "www-authenticate",
            HeaderValue::from_static("Bearer realm=\"wallet-executor\""),
        );
    }

    response
}

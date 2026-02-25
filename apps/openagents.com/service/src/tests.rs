use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::header::{
    ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, ETAG, IF_NONE_MATCH, SET_COOKIE,
};
use axum::http::{HeaderValue, Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use chrono::{Duration, Utc};
use hmac::Mac;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::tempdir;
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tower::ServiceExt;

use crate::build_router;
use crate::build_router_with_observability;
use crate::config::Config;
use crate::domain_store::{
    AutopilotAggregate, AutopilotPolicyRecord, AutopilotProfileRecord, AutopilotRecord,
    AutopilotRuntimeBindingRecord, CreateAutopilotInput, CreateL402PaywallInput, DomainStore,
    RecordL402GatewayEventInput, RecordL402ReceiptInput, UpsertGoogleIntegrationInput,
    UpsertResendIntegrationInput, UpsertUserSparkWalletInput,
};
use crate::observability::{Observability, RecordingAuditSink};
use crate::{
    CACHE_IMMUTABLE_ONE_YEAR, CACHE_MANIFEST, CACHE_SHORT_LIVED, MAINTENANCE_CACHE_CONTROL,
};

fn test_config(static_dir: PathBuf) -> Config {
    Config::for_tests(static_dir)
}

fn workos_required_config(static_dir: PathBuf) -> Config {
    let mut config = test_config(static_dir);
    config.auth_provider_mode = "workos".to_string();
    config.workos_client_id = None;
    config.workos_api_key = None;
    config
}

fn compat_enforced_config(static_dir: PathBuf) -> Config {
    let mut config = test_config(static_dir);
    config.compat_control_enforced = true;
    config.compat_control_protocol_version = "openagents.control.v1".to_string();
    config.compat_control_min_client_build_id = "20260221T120000Z".to_string();
    config.compat_control_max_client_build_id = Some("20260221T180000Z".to_string());
    config.compat_control_min_schema_version = 1;
    config.compat_control_max_schema_version = 1;
    config
}

fn maintenance_enabled_config(static_dir: PathBuf) -> Config {
    let mut config = test_config(static_dir);
    config.maintenance_mode_enabled = true;
    config.maintenance_bypass_token = Some("maintenance-token".to_string());
    config.maintenance_bypass_cookie_name = "oa_maintenance_bypass".to_string();
    config.maintenance_bypass_cookie_ttl_seconds = 300;
    config.maintenance_allowed_paths = vec!["/healthz".to_string(), "/readyz".to_string()];
    config
}

fn test_app_state(config: Config) -> super::AppState {
    let auth = super::AuthService::from_config(&config);
    let route_split = super::RouteSplitService::from_config(&config);
    let runtime_routing = super::RuntimeRoutingService::from_config(&config);
    let khala_token_issuer = super::KhalaTokenIssuer::from_config(&config);
    let sync_token_issuer = super::SyncTokenIssuer::from_config(&config);
    let codex_thread_store = super::CodexThreadStore::from_config(&config);
    let domain_store = super::DomainStore::from_config(&config);
    let runtime_revocation_client = super::RuntimeRevocationClient::from_config(&config);
    super::AppState {
        config: Arc::new(config),
        auth,
        observability: Observability::default(),
        route_split,
        runtime_routing,
        khala_token_issuer,
        sync_token_issuer,
        codex_thread_store,
        _domain_store: domain_store,
        runtime_revocation_client,
        throttle_state: super::ThrottleState::default(),
        codex_control_receipts: super::CodexControlReceiptState::default(),
        runtime_tool_receipts: super::RuntimeToolReceiptState::default(),
        runtime_skill_registry: super::RuntimeSkillRegistryState::default(),
        runtime_workers: super::RuntimeWorkerState::default(),
        lightning_ops_control_plane: super::LightningOpsControlPlaneState::default(),
        runtime_internal_nonces: super::RuntimeInternalNonceState::default(),
        google_oauth_states: super::GoogleOauthStateStore::default(),
        started_at: std::time::SystemTime::now(),
    }
}

fn runtime_internal_signed_headers(
    body: &str,
    secret: &str,
    key_id: &str,
    nonce: &str,
    timestamp: i64,
) -> Result<HeaderMap> {
    let body_hash = super::sha256_hex(body.as_bytes());
    let signing_payload = format!("{timestamp}\n{nonce}\n{body_hash}");
    let mut mac = super::HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(signing_payload.as_bytes());
    let signature = super::sha256_bytes_hex(&mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        super::RUNTIME_INTERNAL_KEY_ID_HEADER,
        HeaderValue::from_str(key_id)?,
    );
    headers.insert(
        super::RUNTIME_INTERNAL_TIMESTAMP_HEADER,
        HeaderValue::from_str(&timestamp.to_string())?,
    );
    headers.insert(
        super::RUNTIME_INTERNAL_NONCE_HEADER,
        HeaderValue::from_str(nonce)?,
    );
    headers.insert(
        super::RUNTIME_INTERNAL_BODY_HASH_HEADER,
        HeaderValue::from_str(&body_hash)?,
    );
    headers.insert(
        super::RUNTIME_INTERNAL_SIGNATURE_HEADER,
        HeaderValue::from_str(&signature)?,
    );
    Ok(headers)
}

fn runtime_internal_signed_request(
    path: &str,
    body: &str,
    headers: &HeaderMap,
) -> Result<Request<Body>> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    Ok(builder.body(Body::from(body.to_string()))?)
}

fn sample_autopilot_aggregate() -> AutopilotAggregate {
    let now = Utc::now();
    AutopilotAggregate {
        autopilot: AutopilotRecord {
            id: "ap_test_1".to_string(),
            owner_user_id: "usr_test_1".to_string(),
            handle: "test-pilot".to_string(),
            display_name: "Test Pilot".to_string(),
            avatar: None,
            status: "active".to_string(),
            visibility: "private".to_string(),
            tagline: Some("Pragmatic and concise".to_string()),
            config_version: 3,
            deleted_at: None,
            created_at: now,
            updated_at: now,
        },
        profile: AutopilotProfileRecord {
            autopilot_id: "ap_test_1".to_string(),
            owner_display_name: "Chris".to_string(),
            persona_summary: Some("Pragmatic and concise".to_string()),
            autopilot_voice: Some("calm and direct".to_string()),
            principles: Some(json!(["be concise", "be direct"])),
            preferences: Some(json!({
                "user": {
                    "addressAs": "Chris",
                    "timeZone": "UTC"
                },
                "character": {
                    "boundaries": ["No fluff", "No hype"]
                }
            })),
            onboarding_answers: json!({
                "bootstrapState": {
                    "status": "ready",
                    "stage": "profile"
                }
            }),
            schema_version: 1,
            created_at: now,
            updated_at: now,
        },
        policy: AutopilotPolicyRecord {
            autopilot_id: "ap_test_1".to_string(),
            model_provider: None,
            model: None,
            tool_allowlist: vec![
                "openagents_api".to_string(),
                "lightning_l402_fetch".to_string(),
            ],
            tool_denylist: vec!["lightning_l402_fetch".to_string()],
            l402_require_approval: true,
            l402_max_spend_msats_per_call: Some(100_000),
            l402_max_spend_msats_per_day: Some(500_000),
            l402_allowed_hosts: vec!["sats4ai.com".to_string()],
            data_policy: Some(json!(["redact_secrets"])),
            created_at: now,
            updated_at: now,
        },
        runtime_bindings: vec![
            AutopilotRuntimeBindingRecord {
                id: "arb_primary".to_string(),
                autopilot_id: "ap_test_1".to_string(),
                runtime_type: "runtime".to_string(),
                runtime_ref: Some("desktopw:autopilot".to_string()),
                is_primary: true,
                last_seen_at: Some(now),
                meta: Some(json!({"region":"us-central1"})),
                created_at: now,
                updated_at: now,
            },
            AutopilotRuntimeBindingRecord {
                id: "arb_secondary".to_string(),
                autopilot_id: "ap_test_1".to_string(),
                runtime_type: "legacy".to_string(),
                runtime_ref: Some("legacy:autopilot".to_string()),
                is_primary: false,
                last_seen_at: None,
                meta: None,
                created_at: now,
                updated_at: now,
            },
        ],
    }
}

async fn read_json(response: axum::response::Response) -> Result<Value> {
    let bytes = response.into_body().collect().await?.to_bytes();
    let value = serde_json::from_slice::<Value>(&bytes)?;
    Ok(value)
}

async fn read_text(response: axum::response::Response) -> Result<String> {
    let bytes = response.into_body().collect().await?.to_bytes();
    Ok(String::from_utf8(bytes.to_vec())?)
}

fn sse_event_types(wire: &str) -> Vec<String> {
    let mut events = Vec::new();
    for line in wire.lines() {
        let Some(payload) = line.strip_prefix("data:") else {
            continue;
        };
        let payload = payload.trim();
        if payload == "[DONE]" {
            continue;
        }
        let Ok(parsed) = serde_json::from_str::<Value>(payload) else {
            continue;
        };
        if let Some(kind) = parsed.get("type").and_then(Value::as_str) {
            events.push(kind.to_string());
        }
    }
    events
}

fn sse_done_count(wire: &str) -> usize {
    wire.lines()
        .filter(|line| line.trim() == "data: [DONE]")
        .count()
}

fn legacy_stream_request(
    path: &str,
    access_token: Option<&str>,
    body: &str,
) -> Result<Request<Body>> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("x-oa-client-build-id", "20260221T130000Z")
        .header("x-oa-protocol-version", "openagents.control.v1")
        .header("x-oa-schema-version", "1");
    if let Some(token) = access_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    Ok(builder.body(Body::from(body.to_string()))?)
}

fn cookie_value(response: &axum::response::Response) -> Option<String> {
    let header = response.headers().get(SET_COOKIE)?;
    let raw = header.to_str().ok()?;
    raw.split(';').next().map(|value| value.to_string())
}

fn all_set_cookie_values(response: &axum::response::Response) -> Vec<String> {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(ToString::to_string)
        .collect()
}

fn cookie_from_set_cookie_header(set_cookie: &str) -> Option<String> {
    set_cookie.split(';').next().map(|value| value.to_string())
}

fn cookie_value_for_name(response: &axum::response::Response, name: &str) -> Option<String> {
    all_set_cookie_values(response)
        .into_iter()
        .filter_map(|set_cookie| cookie_from_set_cookie_header(&set_cookie))
        .find_map(|cookie| {
            let mut parts = cookie.splitn(2, '=');
            let key = parts.next().unwrap_or_default();
            let value = parts.next().unwrap_or_default();
            if key == name && !value.is_empty() {
                Some(value.to_string())
            } else {
                None
            }
        })
}

fn signed_test_login_url(
    signing_key: &str,
    email: &str,
    expires: i64,
    name: Option<&str>,
) -> String {
    let mut unsigned = format!("/internal/test-login?email={email}&expires={expires}");
    if let Some(value) = name {
        unsigned.push_str("&name=");
        unsigned.push_str(value);
    }

    let mut mac =
        super::HmacSha256::new_from_slice(signing_key.as_bytes()).expect("valid signing key");
    mac.update(unsigned.as_bytes());
    let signature = super::sha256_bytes_hex(&mac.finalize().into_bytes());
    format!("{unsigned}&signature={signature}")
}

async fn start_runtime_revocation_stub(
    captured: Arc<Mutex<Vec<Value>>>,
) -> Result<(SocketAddr, JoinHandle<()>)> {
    let app = Router::new()
        .route(
            "/internal/v1/sync/sessions/revoke",
            post(
                |State(captured): State<Arc<Mutex<Vec<Value>>>>,
                 headers: HeaderMap,
                 Json(payload): Json<Value>| async move {
                    let signature = headers
                        .get("x-oa-runtime-signature")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string();

                    captured.lock().await.push(json!({
                        "signature": signature,
                        "payload": payload,
                    }));

                    (StatusCode::OK, Json(json!({"data": {"ok": true}})))
                },
            ),
        )
        .with_state(captured);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .expect("runtime revocation stub server failed");
    });

    Ok((addr, handle))
}

async fn start_runtime_comms_delivery_stub(
    statuses: Arc<Mutex<Vec<u16>>>,
    captured: Arc<Mutex<Vec<Value>>>,
) -> Result<(SocketAddr, JoinHandle<()>)> {
    #[derive(Clone)]
    struct StubState {
        statuses: Arc<Mutex<Vec<u16>>>,
        captured: Arc<Mutex<Vec<Value>>>,
    }

    let stub_state = StubState { statuses, captured };
    let app = Router::new()
        .route(
            "/internal/v1/comms/delivery-events",
            post(
                |State(stub_state): State<StubState>, Json(payload): Json<Value>| async move {
                    stub_state.captured.lock().await.push(payload);
                    let status_code = {
                        let mut statuses = stub_state.statuses.lock().await;
                        if statuses.is_empty() {
                            202
                        } else {
                            statuses.remove(0)
                        }
                    };
                    if status_code < 300 {
                        (
                            StatusCode::from_u16(status_code).unwrap_or(StatusCode::ACCEPTED),
                            Json(json!({
                                "eventId": "evt_runtime_projection",
                                "status": "accepted",
                                "idempotentReplay": false
                            })),
                        )
                    } else {
                        (
                            StatusCode::from_u16(status_code)
                                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                            Json(json!({
                                "error": "temporary"
                            })),
                        )
                    }
                },
            ),
        )
        .with_state(stub_state);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .expect("runtime comms delivery stub server failed");
    });

    Ok((addr, handle))
}

fn signed_resend_webhook_headers(
    payload: &str,
    webhook_secret: &str,
    svix_id: &str,
    svix_timestamp: i64,
) -> HeaderMap {
    let secret_bytes = super::resolve_resend_webhook_secret_bytes(webhook_secret);
    let signed_content = format!("{svix_id}.{svix_timestamp}.{payload}");
    let mut mac =
        super::HmacSha256::new_from_slice(secret_bytes.as_slice()).expect("valid hmac key");
    mac.update(signed_content.as_bytes());
    let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        "svix-id",
        HeaderValue::from_str(svix_id).expect("svix id header"),
    );
    headers.insert(
        "svix-timestamp",
        HeaderValue::from_str(&svix_timestamp.to_string()).expect("svix timestamp header"),
    );
    headers.insert(
        "svix-signature",
        HeaderValue::from_str(&format!("v1,{signature}")).expect("svix signature header"),
    );
    headers
}

async fn wait_for_webhook_status(
    state: &super::AppState,
    idempotency_key: &str,
    expected_status: &str,
) -> Result<()> {
    for _ in 0..100 {
        let current = state
            ._domain_store
            .webhook_event_by_idempotency_key(idempotency_key)
            .await?;
        if current
            .as_ref()
            .map(|event| event.status.as_str() == expected_status)
            .unwrap_or(false)
        {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    anyhow::bail!("timed out waiting for webhook status {expected_status}");
}

async fn start_google_oauth_token_stub(
    captured_bodies: Arc<Mutex<Vec<String>>>,
) -> Result<(SocketAddr, JoinHandle<()>)> {
    let app = Router::new()
        .route(
            "/oauth2/token",
            post(
                |State(captured_bodies): State<Arc<Mutex<Vec<String>>>>, body: String| async move {
                    captured_bodies.lock().await.push(body);
                    (
                        StatusCode::OK,
                        Json(json!({
                            "refresh_token": "refresh_token_1234567890",
                            "access_token": "access_token_abcdef",
                            "scope": "https://www.googleapis.com/auth/gmail.readonly",
                            "token_type": "Bearer",
                            "expires_in": 3600,
                        })),
                    )
                },
            ),
        )
        .with_state(captured_bodies);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .expect("google oauth token stub server failed");
    });

    Ok((addr, handle))
}

async fn start_gmail_inbox_stub(
    token_calls: Arc<Mutex<Vec<String>>>,
    send_calls: Arc<Mutex<Vec<Value>>>,
) -> Result<(SocketAddr, JoinHandle<()>)> {
    #[derive(Clone)]
    struct GmailStubState {
        token_calls: Arc<Mutex<Vec<String>>>,
        send_calls: Arc<Mutex<Vec<Value>>>,
    }

    let state = GmailStubState {
        token_calls,
        send_calls,
    };
    let app = Router::new()
            .route(
                "/oauth2/token",
                post(
                    |State(state): State<GmailStubState>, body: String| async move {
                        state.token_calls.lock().await.push(body);
                        (
                            StatusCode::OK,
                            Json(json!({
                                "access_token": "fresh_access_token",
                                "token_type": "Bearer",
                                "scope": "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.send",
                                "expires_in": 3600
                            })),
                        )
                    },
                ),
            )
            .route(
                "/gmail/v1/users/me/threads",
                get(|headers: HeaderMap| async move {
                    let auth = headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    if auth.contains("stale_access_token") {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(json!({"error": {"message": "expired"}})),
                        );
                    }
                    (
                        StatusCode::OK,
                        Json(json!({
                            "threads": [
                                { "id": "thread_1" }
                            ]
                        })),
                    )
                }),
            )
            .route(
                "/gmail/v1/users/me/threads/:thread_id",
                get(
                    |axum::extract::Path(thread_id): axum::extract::Path<String>,
                     headers: HeaderMap| async move {
                        let auth = headers
                            .get("authorization")
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or_default()
                            .to_string();
                        if auth.contains("stale_access_token") {
                            return (
                                StatusCode::UNAUTHORIZED,
                                Json(json!({"error": {"message": "expired"}})),
                            );
                        }

                        let body = base64::engine::general_purpose::URL_SAFE_NO_PAD
                            .encode("Can we move tomorrow's walkthrough to next week?");
                        (
                            StatusCode::OK,
                            Json(json!({
                                "id": thread_id,
                                "snippet": "Can we move tomorrow's call to next week?",
                                "messages": [
                                    {
                                        "id": "gmail_msg_1",
                                        "snippet": "Can we move tomorrow's call to next week?",
                                        "internalDate": "1765584000000",
                                        "payload": {
                                            "headers": [
                                                {"name": "Subject", "value": "Can we reschedule the walkthrough?"},
                                                {"name": "From", "value": "alex@acme.com"},
                                                {"name": "To", "value": "you@openagents.com"}
                                            ],
                                            "body": {"data": body}
                                        }
                                    }
                                ]
                            })),
                        )
                    },
                ),
            )
            .route(
                "/gmail/v1/users/me/messages/send",
                post(
                    |State(state): State<GmailStubState>, Json(payload): Json<Value>| async move {
                        state.send_calls.lock().await.push(payload);
                        (
                            StatusCode::OK,
                            Json(json!({
                                "id": "gmail_msg_sent_1",
                                "threadId": "thread_1"
                            })),
                        )
                    },
                ),
            )
            .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, app.into_make_service())
            .await
            .expect("gmail inbox stub server failed");
    });

    Ok((addr, handle))
}

async fn authenticate_token(app: Router, email: &str) -> Result<String> {
    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "email": email }).to_string()))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;

    Ok(verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

async fn authenticated_user_id(app: Router, token: &str) -> Result<String> {
    let request = Request::builder()
        .method("GET")
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    let body = read_json(response).await?;
    Ok(body["data"]["user"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

struct L402Fixture {
    token: String,
    autopilot_id: String,
    autopilot_handle: String,
    paid_receipt_event_id: u64,
}

async fn seed_local_test_token(config: &Config, email: &str) -> Result<String> {
    let auth = super::AuthService::from_config(config);
    let verify = auth
        .local_test_sign_in(email.to_string(), None, Some("autopilot-ios"), None)
        .await?;
    Ok(verify.access_token)
}

async fn seed_l402_fixture(config: &Config, email: &str) -> Result<L402Fixture> {
    let auth = super::AuthService::from_config(config);
    let verify = auth
        .local_test_sign_in(email.to_string(), None, Some("autopilot-ios"), None)
        .await?;
    let user_id = verify.user.id.clone();
    let token = verify.access_token.clone();

    let store = super::DomainStore::from_config(config);
    let autopilot = store
        .create_autopilot(CreateAutopilotInput {
            owner_user_id: user_id.clone(),
            owner_display_name: "Owner".to_string(),
            display_name: "Payments Bot".to_string(),
            handle_seed: None,
            avatar: None,
            status: None,
            visibility: None,
            tagline: None,
        })
        .await
        .expect("create autopilot");

    store
        .upsert_user_spark_wallet(UpsertUserSparkWalletInput {
            user_id: user_id.clone(),
            wallet_id: "wallet_123".to_string(),
            mnemonic: "mnemonic words".to_string(),
            spark_address: Some("spark:abc".to_string()),
            lightning_address: Some("ln@openagents.com".to_string()),
            identity_pubkey: Some("pubkey_1".to_string()),
            last_balance_sats: Some(4200),
            status: Some("active".to_string()),
            provider: Some("spark_executor".to_string()),
            last_error: None,
            meta: None,
            last_synced_at: Some(Utc::now()),
        })
        .await
        .expect("seed wallet");

    let paid_receipt = store
        .record_l402_receipt(RecordL402ReceiptInput {
            user_id: user_id.clone(),
            thread_id: "thread_1".to_string(),
            run_id: "run_1".to_string(),
            autopilot_id: Some(autopilot.autopilot.id.clone()),
            thread_title: Some("Conversation 1".to_string()),
            run_status: Some("completed".to_string()),
            run_started_at: Some(Utc::now()),
            run_completed_at: Some(Utc::now()),
            payload: json!({
                "status": "paid",
                "host": "sats4ai.com",
                "scope": "fetch",
                "paid": true,
                "amountMsats": 2100,
                "cacheHit": false,
                "approvalRequired": false,
            }),
            created_at: Some(Utc::now()),
        })
        .await
        .expect("seed paid receipt");

    store
        .record_l402_receipt(RecordL402ReceiptInput {
            user_id: user_id.clone(),
            thread_id: "thread_2".to_string(),
            run_id: "run_2".to_string(),
            autopilot_id: Some(autopilot.autopilot.id.clone()),
            thread_title: Some("Conversation 2".to_string()),
            run_status: Some("completed".to_string()),
            run_started_at: Some(Utc::now()),
            run_completed_at: Some(Utc::now()),
            payload: json!({
                "status": "cached",
                "host": "sats4ai.com",
                "scope": "fetch",
                "paid": false,
                "cacheStatus": "hit",
                "cacheHit": true,
            }),
            created_at: Some(Utc::now()),
        })
        .await
        .expect("seed cached receipt");

    store
        .record_l402_receipt(RecordL402ReceiptInput {
            user_id: user_id.clone(),
            thread_id: "thread_3".to_string(),
            run_id: "run_3".to_string(),
            autopilot_id: None,
            thread_title: Some("Conversation 3".to_string()),
            run_status: Some("failed".to_string()),
            run_started_at: Some(Utc::now()),
            run_completed_at: Some(Utc::now()),
            payload: json!({
                "status": "blocked",
                "paid": false,
                "denyCode": "policy_denied",
            }),
            created_at: Some(Utc::now()),
        })
        .await
        .expect("seed blocked receipt");

    store
        .record_l402_gateway_event(RecordL402GatewayEventInput {
            user_id: user_id.clone(),
            autopilot_id: Some(autopilot.autopilot.id.clone()),
            event_type: "l402_gateway_event".to_string(),
            payload: json!({"status":"ok"}),
            created_at: Some(Utc::now()),
        })
        .await
        .expect("seed gateway event");

    store
        .record_l402_gateway_event(RecordL402GatewayEventInput {
            user_id: user_id.clone(),
            autopilot_id: None,
            event_type: "unrelated_event".to_string(),
            payload: json!({"ignored":true}),
            created_at: Some(Utc::now()),
        })
        .await
        .expect("seed non-l402 event");

    Ok(L402Fixture {
        token,
        autopilot_id: autopilot.autopilot.id,
        autopilot_handle: autopilot.autopilot.handle,
        paid_receipt_event_id: paid_receipt.id,
    })
}

#[tokio::test]
async fn healthz_route_returns_ok() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let request = Request::builder().uri("/healthz").body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await?;
    assert_eq!(body["status"], "ok");
    assert_eq!(body["service"], "openagents-control-service");
    assert_eq!(body["auth_provider"], "mock");
    Ok(())
}

#[tokio::test]
async fn root_route_serves_landing_page_with_desktop_download_link() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let request = Request::builder().uri("/").body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/html; charset=utf-8")
    );
    let body = read_text(response).await?;
    assert!(body.contains("Download desktop app"));
    assert!(body.contains("href=\"/download-desktop\""));
    Ok(())
}

#[tokio::test]
async fn download_desktop_route_redirects_to_configured_release_url() -> Result<()> {
    let config = test_config(std::env::temp_dir());
    let expected = config.desktop_download_url.clone();
    let app = build_router(config);
    let request = Request::builder()
        .uri("/download-desktop")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some(expected.as_str())
    );
    Ok(())
}

#[tokio::test]
async fn interactive_web_routes_are_not_mounted() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let cases = [
        ("GET", "/compute"),
        ("GET", "/stats"),
        ("GET", "/feed"),
        ("POST", "/chat/new"),
        ("POST", "/admin/route-split/evaluate"),
    ];

    for (method, uri) in cases {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "expected {uri} to be removed"
        );
    }

    Ok(())
}

#[tokio::test]
async fn api_preflight_options_returns_cors_headers_without_auth() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let request = Request::builder()
        .method("OPTIONS")
        .uri("/api/settings/profile")
        .header("origin", "https://console.openagents.com")
        .header("access-control-request-method", "PATCH")
        .header(
            "access-control-request-headers",
            "authorization,content-type",
        )
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://console.openagents.com")
    );
    let allow_methods = response
        .headers()
        .get("access-control-allow-methods")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(allow_methods.contains("OPTIONS"));
    assert!(allow_methods.contains("PATCH"));
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(super::CACHE_API_NO_STORE)
    );

    Ok(())
}

#[tokio::test]
async fn auth_email_route_enforces_throttle_limit() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    for _ in 0..super::THROTTLE_AUTH_EMAIL_LIMIT {
        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.10")
            .body(Body::from(r#"{"email":"throttle@openagents.com"}"#))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let exceeded = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::from(r#"{"email":"throttle@openagents.com"}"#))?;
    let response = app.oneshot(exceeded).await?;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "rate_limited");

    Ok(())
}

#[tokio::test]
async fn auth_email_throttle_is_scoped_per_client_key() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    for _ in 0..super::THROTTLE_AUTH_EMAIL_LIMIT {
        let request = Request::builder()
            .method("POST")
            .uri("/api/auth/email")
            .header("content-type", "application/json")
            .header("x-forwarded-for", "203.0.113.10")
            .body(Body::from(r#"{"email":"throttle-a@openagents.com"}"#))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let second_key_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "198.51.100.22")
        .body(Body::from(r#"{"email":"throttle-b@openagents.com"}"#))?;
    let second_key_response = app.clone().oneshot(second_key_request).await?;
    assert_eq!(second_key_response.status(), StatusCode::OK);

    let exceeded_primary = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .header("x-forwarded-for", "203.0.113.10")
        .body(Body::from(r#"{"email":"throttle-a@openagents.com"}"#))?;
    let exceeded_response = app.oneshot(exceeded_primary).await?;
    assert_eq!(exceeded_response.status(), StatusCode::TOO_MANY_REQUESTS);

    Ok(())
}

#[tokio::test]
async fn thread_message_route_enforces_throttle_limit() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "thread-throttle@openagents.com").await?;

    for index in 0..super::THROTTLE_THREAD_MESSAGE_LIMIT {
        let request = Request::builder()
            .method("POST")
            .uri("/api/runtime/threads/thread-1/messages")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .header("x-forwarded-for", "198.51.100.22")
            .body(Body::from(format!(r#"{{"text":"message-{index}"}}"#)))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let exceeded = Request::builder()
        .method("POST")
        .uri("/api/runtime/threads/thread-1/messages")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .header("x-forwarded-for", "198.51.100.22")
        .body(Body::from(r#"{"text":"over-limit"}"#))?;
    let response = app.oneshot(exceeded).await?;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "rate_limited");

    Ok(())
}

#[tokio::test]
async fn route_split_override_requires_admin_email() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;

    let mut config = test_config(static_dir.path().to_path_buf());
    config.admin_emails = vec!["admin@openagents.com".to_string()];
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "not-admin@openagents.com").await?;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"target":"legacy"}"#))?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "forbidden");

    Ok(())
}

#[tokio::test]
async fn route_split_htmx_override_requires_domain() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));
    let token = authenticate_token(app.clone(), "routes@openagents.com").await?;

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"target":"htmx_full_page"}"#))?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], json!("invalid_request"));
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("Domain is required")
    );

    Ok(())
}

#[tokio::test]
async fn runtime_internal_signature_validation_rejects_nonce_replay() -> Result<()> {
    let mut config = test_config(std::env::temp_dir());
    config.runtime_internal_shared_secret = Some("runtime-internal-secret".to_string());
    config.runtime_internal_key_id = "runtime-internal-v1".to_string();
    config.runtime_internal_signature_ttl_seconds = 60;
    let state = test_app_state(config);

    let body = br#"{"provider":"resend","integration_id":"int_runtime"}"#;
    let timestamp = chrono::Utc::now().timestamp().to_string();
    let nonce = "nonce-runtime-internal-1";
    let body_hash = super::sha256_hex(body);
    let signing_payload = format!("{timestamp}\n{nonce}\n{body_hash}");

    let mut mac = super::HmacSha256::new_from_slice(b"runtime-internal-secret").expect("hmac key");
    mac.update(signing_payload.as_bytes());
    let signature = super::sha256_bytes_hex(&mac.finalize().into_bytes());

    let mut headers = HeaderMap::new();
    headers.insert(
        super::RUNTIME_INTERNAL_KEY_ID_HEADER,
        HeaderValue::from_static("runtime-internal-v1"),
    );
    headers.insert(
        super::RUNTIME_INTERNAL_TIMESTAMP_HEADER,
        HeaderValue::from_str(&timestamp)?,
    );
    headers.insert(
        super::RUNTIME_INTERNAL_NONCE_HEADER,
        HeaderValue::from_static(nonce),
    );
    headers.insert(
        super::RUNTIME_INTERNAL_BODY_HASH_HEADER,
        HeaderValue::from_str(&body_hash)?,
    );
    headers.insert(
        super::RUNTIME_INTERNAL_SIGNATURE_HEADER,
        HeaderValue::from_str(&signature)?,
    );

    let first = super::verify_runtime_internal_headers(&state, &headers, body).await;
    assert!(first.is_ok());

    let replay = super::verify_runtime_internal_headers(&state, &headers, body)
        .await
        .expect_err("expected nonce replay rejection");
    assert_eq!(replay.0, StatusCode::UNAUTHORIZED);
    assert_eq!(replay.1.0.error.code, "nonce_replay");
    assert_eq!(replay.1.0.error.message, "nonce replay detected");

    Ok(())
}

#[tokio::test]
async fn runtime_internal_secret_fetch_returns_scoped_secret_for_signed_request() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.runtime_internal_shared_secret = Some("runtime-internal-secret".to_string());
    config.runtime_internal_key_id = "runtime-internal-v1".to_string();
    config.runtime_internal_signature_ttl_seconds = 60;
    config.runtime_internal_secret_cache_ttl_ms = 45_000;
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let store = DomainStore::from_config(&config);
    store
        .upsert_resend_integration(UpsertResendIntegrationInput {
            user_id: "usr_runtime_secret".to_string(),
            api_key: "re_live_1234567890".to_string(),
            sender_email: None,
            sender_name: None,
        })
        .await?;

    let app = build_router(config.clone());
    let body = r#"{"user_id":"usr_runtime_secret","provider":"resend","integration_id":"resend.primary","run_id":"run_123","tool_call_id":"tool_123","org_id":"org_abc"}"#;
    let headers = runtime_internal_signed_headers(
        body,
        "runtime-internal-secret",
        "runtime-internal-v1",
        "nonce-runtime-secret-1",
        Utc::now().timestamp(),
    )?;
    let request = runtime_internal_signed_request(
        &config.runtime_internal_secret_fetch_path,
        body,
        &headers,
    )?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let payload = read_json(response).await?;
    assert_eq!(payload["data"]["provider"], "resend");
    assert_eq!(payload["data"]["secret"], "re_live_1234567890");
    assert_eq!(payload["data"]["cache_ttl_ms"], 45_000);
    assert_eq!(payload["data"]["scope"]["user_id"], "usr_runtime_secret");
    assert_eq!(payload["data"]["scope"]["provider"], "resend");
    assert_eq!(payload["data"]["scope"]["integration_id"], "resend.primary");
    assert_eq!(payload["data"]["scope"]["run_id"], "run_123");
    assert_eq!(payload["data"]["scope"]["tool_call_id"], "tool_123");
    assert_eq!(payload["data"]["scope"]["org_id"], "org_abc");
    assert!(payload["data"]["fetched_at"].as_str().is_some());

    Ok(())
}

#[tokio::test]
async fn runtime_internal_secret_fetch_rejects_invalid_signature_code() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.runtime_internal_shared_secret = Some("runtime-internal-secret".to_string());
    config.runtime_internal_key_id = "runtime-internal-v1".to_string();
    config.runtime_internal_signature_ttl_seconds = 60;

    let app = build_router(config.clone());
    let body = r#"{"user_id":"usr_runtime_secret","provider":"resend","integration_id":"resend.primary","run_id":"run_123","tool_call_id":"tool_123"}"#;
    let mut headers = runtime_internal_signed_headers(
        body,
        "runtime-internal-secret",
        "runtime-internal-v1",
        "nonce-runtime-secret-invalid",
        Utc::now().timestamp(),
    )?;
    headers.insert(
        super::RUNTIME_INTERNAL_SIGNATURE_HEADER,
        HeaderValue::from_static("invalid-signature"),
    );
    let request = runtime_internal_signed_request(
        &config.runtime_internal_secret_fetch_path,
        body,
        &headers,
    )?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let payload = read_json(response).await?;
    assert_eq!(payload["error"]["code"], "invalid_signature");

    Ok(())
}

#[tokio::test]
async fn runtime_internal_secret_fetch_rejects_nonce_replay() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.runtime_internal_shared_secret = Some("runtime-internal-secret".to_string());
    config.runtime_internal_key_id = "runtime-internal-v1".to_string();
    config.runtime_internal_signature_ttl_seconds = 60;
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let store = DomainStore::from_config(&config);
    store
        .upsert_resend_integration(UpsertResendIntegrationInput {
            user_id: "usr_runtime_secret".to_string(),
            api_key: "re_live_1234567890".to_string(),
            sender_email: None,
            sender_name: None,
        })
        .await?;

    let app = build_router(config.clone());
    let body = r#"{"user_id":"usr_runtime_secret","provider":"resend","integration_id":"resend.primary","run_id":"run_123","tool_call_id":"tool_123"}"#;
    let headers = runtime_internal_signed_headers(
        body,
        "runtime-internal-secret",
        "runtime-internal-v1",
        "nonce-runtime-secret-replay",
        Utc::now().timestamp(),
    )?;

    let first_request = runtime_internal_signed_request(
        &config.runtime_internal_secret_fetch_path,
        body,
        &headers,
    )?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::OK);

    let second_request = runtime_internal_signed_request(
        &config.runtime_internal_secret_fetch_path,
        body,
        &headers,
    )?;
    let second_response = app.oneshot(second_request).await?;
    assert_eq!(second_response.status(), StatusCode::UNAUTHORIZED);
    let second_body = read_json(second_response).await?;
    assert_eq!(second_body["error"]["code"], "nonce_replay");

    Ok(())
}

#[tokio::test]
async fn runtime_internal_secret_fetch_returns_not_found_after_revoke() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.runtime_internal_shared_secret = Some("runtime-internal-secret".to_string());
    config.runtime_internal_key_id = "runtime-internal-v1".to_string();
    config.runtime_internal_signature_ttl_seconds = 60;
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let store = DomainStore::from_config(&config);
    store
        .upsert_resend_integration(UpsertResendIntegrationInput {
            user_id: "usr_runtime_secret".to_string(),
            api_key: "re_live_1234567890".to_string(),
            sender_email: None,
            sender_name: None,
        })
        .await?;

    let body = r#"{"user_id":"usr_runtime_secret","provider":"resend","integration_id":"resend.primary","run_id":"run_123","tool_call_id":"tool_123"}"#;
    let before_revoke_headers = runtime_internal_signed_headers(
        body,
        "runtime-internal-secret",
        "runtime-internal-v1",
        "nonce-runtime-secret-before-revoke",
        Utc::now().timestamp(),
    )?;
    let app_before_revoke = build_router(config.clone());
    let first_request = runtime_internal_signed_request(
        &config.runtime_internal_secret_fetch_path,
        body,
        &before_revoke_headers,
    )?;
    let first_response = app_before_revoke.oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::OK);

    let revoked = store
        .revoke_integration("usr_runtime_secret", "resend")
        .await?;
    assert!(revoked.is_some());

    let app_after_revoke = build_router(config.clone());
    let after_revoke_headers = runtime_internal_signed_headers(
        body,
        "runtime-internal-secret",
        "runtime-internal-v1",
        "nonce-runtime-secret-after-revoke",
        Utc::now().timestamp(),
    )?;
    let second_request = runtime_internal_signed_request(
        &config.runtime_internal_secret_fetch_path,
        body,
        &after_revoke_headers,
    )?;
    let second_response = app_after_revoke.oneshot(second_request).await?;
    assert_eq!(second_response.status(), StatusCode::NOT_FOUND);
    let second_body = read_json(second_response).await?;
    assert_eq!(second_body["error"]["code"], "secret_not_found");

    Ok(())
}

#[tokio::test]
async fn readiness_route_is_not_ready_when_static_dir_missing() -> Result<()> {
    let base = tempdir()?;
    let missing_dir = base.path().join("missing-assets");
    let app = build_router(test_config(missing_dir));

    let request = Request::builder().uri("/readyz").body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = read_json(response).await?;
    assert_eq!(body["status"], "not_ready");
    Ok(())
}

#[tokio::test]
async fn readiness_route_is_ready_when_static_dir_exists() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder().uri("/readyz").body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await?;
    assert_eq!(body["status"], "ready");
    Ok(())
}

#[tokio::test]
async fn maintenance_mode_blocks_non_allowed_routes_with_503() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

    let request = Request::builder().uri("/").body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(MAINTENANCE_CACHE_CONTROL)
    );

    let body = response.into_body().collect().await?.to_bytes();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("Maintenance in progress"));
    Ok(())
}

#[tokio::test]
async fn maintenance_mode_allows_health_and_readiness_routes() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

    let health = Request::builder().uri("/healthz").body(Body::empty())?;
    let health_response = app.clone().oneshot(health).await?;
    assert_eq!(health_response.status(), StatusCode::OK);

    let ready = Request::builder().uri("/readyz").body(Body::empty())?;
    let ready_response = app.oneshot(ready).await?;
    assert_eq!(ready_response.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn maintenance_mode_valid_bypass_sets_cookie_and_redirects() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/workspace?maintenance_bypass=maintenance-token")
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/workspace")
    );

    let set_cookie = response
        .headers()
        .get(SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(set_cookie.contains("oa_maintenance_bypass="));
    assert!(set_cookie.contains("Secure"));
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("Max-Age=300"));

    let cookie = set_cookie.split(';').next().unwrap_or_default().to_string();
    assert!(!cookie.contains("maintenance-token"));

    let follow_request = Request::builder()
        .uri("/workspace")
        .header("cookie", cookie)
        .body(Body::empty())?;
    let follow_response = app.oneshot(follow_request).await?;
    assert_eq!(follow_response.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn maintenance_mode_invalid_bypass_token_does_not_grant_access() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(maintenance_enabled_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/?maintenance_bypass=bad-token")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(response.headers().get(SET_COOKIE).is_none());
    Ok(())
}

#[tokio::test]
async fn maintenance_cookie_validation_enforces_signature_and_ttl() -> Result<()> {
    let token = "maintenance-token";
    let now = chrono::Utc::now().timestamp().max(0) as u64;

    let valid = super::maintenance_bypass_cookie_payload(token, now + 300).unwrap_or_default();
    assert!(super::maintenance_cookie_is_valid(&valid, token));

    let expired =
        super::maintenance_bypass_cookie_payload(token, now.saturating_sub(1)).unwrap_or_default();
    assert!(!super::maintenance_cookie_is_valid(&expired, token));
    assert!(!super::maintenance_cookie_is_valid("invalid", token));
    Ok(())
}

#[tokio::test]
async fn maintenance_allowed_paths_can_include_control_endpoints() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = maintenance_enabled_config(static_dir.path().to_path_buf());
    config
        .maintenance_allowed_paths
        .push("/api/v1/control/status".to_string());
    let app = build_router(config);

    let request = Request::builder()
        .uri("/api/v1/control/status")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn static_hashed_asset_uses_immutable_cache_header() -> Result<()> {
    let static_dir = tempdir()?;
    let assets_dir = static_dir.path().join("assets");
    std::fs::create_dir_all(&assets_dir)?;
    std::fs::write(
        assets_dir.join("app-0a1b2c3d4e5f.js"),
        "console.log('openagents');",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/assets/app-0a1b2c3d4e5f.js")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_IMMUTABLE_ONE_YEAR)
    );
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("text/javascript"));
    assert_eq!(
        response
            .headers()
            .get(super::HEADER_X_CONTENT_TYPE_OPTIONS)
            .and_then(|value| value.to_str().ok()),
        Some(super::X_CONTENT_TYPE_OPTIONS_NOSNIFF)
    );

    Ok(())
}

#[tokio::test]
async fn static_asset_prefers_brotli_then_gzip_when_variants_exist() -> Result<()> {
    let static_dir = tempdir()?;
    let assets_dir = static_dir.path().join("assets");
    std::fs::create_dir_all(&assets_dir)?;
    let asset_path = assets_dir.join("app-0a1b2c3d4e5f.js");
    std::fs::write(&asset_path, "console.log('openagents');")?;

    let mut br_path = asset_path.as_os_str().to_os_string();
    br_path.push(".br");
    std::fs::write(std::path::PathBuf::from(br_path), "brotli-bytes")?;

    let mut gz_path = asset_path.as_os_str().to_os_string();
    gz_path.push(".gz");
    std::fs::write(std::path::PathBuf::from(gz_path), "gzip-bytes")?;

    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let br_request = Request::builder()
        .uri("/assets/app-0a1b2c3d4e5f.js")
        .header(ACCEPT_ENCODING, "br, gzip")
        .body(Body::empty())?;
    let br_response = app.clone().oneshot(br_request).await?;
    assert_eq!(br_response.status(), StatusCode::OK);
    assert_eq!(
        br_response
            .headers()
            .get(CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok()),
        Some("br")
    );
    let br_body = br_response.into_body().collect().await?.to_bytes();
    assert_eq!(br_body.as_ref(), b"brotli-bytes");

    let gz_request = Request::builder()
        .uri("/assets/app-0a1b2c3d4e5f.js")
        .header(ACCEPT_ENCODING, "gzip")
        .body(Body::empty())?;
    let gz_response = app.oneshot(gz_request).await?;
    assert_eq!(gz_response.status(), StatusCode::OK);
    assert_eq!(
        gz_response
            .headers()
            .get(CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok()),
        Some("gzip")
    );
    let gz_body = gz_response.into_body().collect().await?.to_bytes();
    assert_eq!(gz_body.as_ref(), b"gzip-bytes");

    Ok(())
}

#[tokio::test]
async fn file_like_root_wasm_path_resolves_to_assets_fallback() -> Result<()> {
    let static_dir = tempdir()?;
    let assets_dir = static_dir.path().join("assets");
    std::fs::create_dir_all(&assets_dir)?;
    let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d];
    std::fs::write(
        assets_dir.join("openagents_web_shell_bg.wasm"),
        wasm_bytes.clone(),
    )?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/openagents_web_shell_bg.wasm")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_SHORT_LIVED)
    );
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("application/wasm"));

    let body = response.into_body().collect().await?.to_bytes();
    assert_eq!(body.as_ref(), wasm_bytes.as_slice());

    Ok(())
}

#[tokio::test]
async fn file_like_missing_path_returns_not_found_instead_of_html_shell() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/missing-module.wasm")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "not_found");

    Ok(())
}

#[tokio::test]
async fn static_asset_supports_etag_conditional_get() -> Result<()> {
    let static_dir = tempdir()?;
    let assets_dir = static_dir.path().join("assets");
    std::fs::create_dir_all(&assets_dir)?;
    std::fs::write(
        assets_dir.join("app-0a1b2c3d4e5f.js"),
        "console.log('openagents');",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let first_request = Request::builder()
        .uri("/assets/app-0a1b2c3d4e5f.js")
        .body(Body::empty())?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::OK);
    let etag = first_response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(!etag.is_empty());

    let conditional_request = Request::builder()
        .uri("/assets/app-0a1b2c3d4e5f.js")
        .header(IF_NONE_MATCH, etag.clone())
        .body(Body::empty())?;
    let conditional_response = app.oneshot(conditional_request).await?;
    assert_eq!(conditional_response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(
        conditional_response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok()),
        Some(etag.as_str())
    );
    assert_eq!(
        conditional_response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_IMMUTABLE_ONE_YEAR)
    );
    let body = conditional_response.into_body().collect().await?.to_bytes();
    assert_eq!(body.len(), 0);

    Ok(())
}

#[tokio::test]
async fn pinned_htmx_asset_is_served_with_cache_compression_and_etag() -> Result<()> {
    let static_dir = tempdir()?;
    let assets_dir = static_dir.path().join("assets");
    std::fs::create_dir_all(&assets_dir)?;

    let asset_name = "htmx-2_0_8-22283ef6.js";
    let asset_path = assets_dir.join(asset_name);
    std::fs::write(&asset_path, "window.htmxVersion='2.0.8';")?;

    let mut br_path = asset_path.as_os_str().to_os_string();
    br_path.push(".br");
    std::fs::write(std::path::PathBuf::from(br_path), "htmx-br-bytes")?;

    let mut gz_path = asset_path.as_os_str().to_os_string();
    gz_path.push(".gz");
    std::fs::write(std::path::PathBuf::from(gz_path), "htmx-gzip-bytes")?;

    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let first_request = Request::builder()
        .uri(format!("/assets/{asset_name}"))
        .header(ACCEPT_ENCODING, "br, gzip")
        .body(Body::empty())?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::OK);
    assert_eq!(
        first_response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_IMMUTABLE_ONE_YEAR)
    );
    assert_eq!(
        first_response
            .headers()
            .get(CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok()),
        Some("br")
    );
    let content_type = first_response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("text/javascript"));
    let etag = first_response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(!etag.is_empty());
    let first_body = first_response.into_body().collect().await?.to_bytes();
    assert_eq!(first_body.as_ref(), b"htmx-br-bytes");

    let conditional_request = Request::builder()
        .uri(format!("/assets/{asset_name}"))
        .header(ACCEPT_ENCODING, "br, gzip")
        .header(IF_NONE_MATCH, etag.clone())
        .body(Body::empty())?;
    let conditional_response = app.oneshot(conditional_request).await?;
    assert_eq!(conditional_response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(
        conditional_response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok()),
        Some(etag.as_str())
    );
    assert_eq!(
        conditional_response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_IMMUTABLE_ONE_YEAR)
    );
    let body = conditional_response.into_body().collect().await?.to_bytes();
    assert_eq!(body.len(), 0);

    Ok(())
}

#[tokio::test]
async fn manifest_uses_no_store_cache_header() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("manifest.json"),
        r#"{"app":"assets/app-0a1b2c3d4e5f.js"}"#,
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/manifest.json")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_MANIFEST)
    );

    Ok(())
}

#[tokio::test]
async fn service_worker_script_uses_no_store_cache_header() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("sw.js"),
        "self.addEventListener('install', () => {});",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder().uri("/sw.js").body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_MANIFEST)
    );

    Ok(())
}

#[tokio::test]
async fn openapi_route_serves_generated_minified_json() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let request = Request::builder()
        .uri(super::ROUTE_OPENAPI_JSON)
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_MANIFEST)
    );
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("application/json"));

    let body = response.into_body().collect().await?.to_bytes();
    let body_text = String::from_utf8(body.to_vec())?;
    assert!(body_text.starts_with("{\"openapi\":\"3.0.2\""));
    assert!(!body_text.contains('\n'));

    let parsed = serde_json::from_str::<Value>(&body_text)?;
    assert_eq!(parsed["openapi"], "3.0.2");
    assert!(parsed["paths"]["/api/auth/email"].is_object());
    assert!(parsed["components"]["securitySchemes"]["bearerAuth"].is_object());

    Ok(())
}

#[tokio::test]
async fn openapi_route_supports_etag_conditional_get() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let first_request = Request::builder()
        .uri(super::ROUTE_OPENAPI_JSON)
        .body(Body::empty())?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::OK);
    let etag = first_response
        .headers()
        .get(ETAG)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(!etag.is_empty());

    let conditional_request = Request::builder()
        .uri(super::ROUTE_OPENAPI_JSON)
        .header(IF_NONE_MATCH, etag.clone())
        .body(Body::empty())?;
    let conditional_response = app.oneshot(conditional_request).await?;
    assert_eq!(conditional_response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(
        conditional_response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok()),
        Some(etag.as_str())
    );
    assert_eq!(
        conditional_response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(CACHE_MANIFEST)
    );

    Ok(())
}

#[tokio::test]
async fn api_list_routes_default_to_no_store_cache_header() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let request = Request::builder()
        .method("GET")
        .uri("/api/shouts")
        .header("origin", "https://console.openagents.com")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some(super::CACHE_API_NO_STORE)
    );
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://console.openagents.com")
    );

    Ok(())
}

#[tokio::test]
async fn smoke_stream_requires_secret_header() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let missing_request = Request::builder()
        .uri(super::ROUTE_SMOKE_STREAM)
        .body(Body::empty())?;
    let missing_response = app.clone().oneshot(missing_request).await?;
    assert_eq!(missing_response.status(), StatusCode::UNAUTHORIZED);

    let wrong_request = Request::builder()
        .uri(super::ROUTE_SMOKE_STREAM)
        .header(super::HEADER_OA_SMOKE_SECRET, "wrong")
        .body(Body::empty())?;
    let wrong_response = app.oneshot(wrong_request).await?;
    assert_eq!(wrong_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn smoke_stream_returns_khala_ws_contract_metadata() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let request = Request::builder()
        .uri(super::ROUTE_SMOKE_STREAM)
        .header(super::HEADER_OA_SMOKE_SECRET, "secret")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(super::HEADER_OA_SMOKE)
            .and_then(|value| value.to_str().ok()),
        Some("1")
    );
    assert_eq!(
        response
            .headers()
            .get(CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );

    let body = read_json(response).await?;
    assert_eq!(body["data"]["status"], "ok");
    assert_eq!(body["data"]["stream_protocol"], "khala_ws");
    assert_eq!(body["data"]["delivery"]["transport"], "khala_ws");
    assert_eq!(body["data"]["delivery"]["sseEnabled"], false);
    assert_eq!(
        body["data"]["delivery"]["syncTokenRoute"],
        super::ROUTE_SYNC_TOKEN
    );

    Ok(())
}

#[tokio::test]
async fn static_asset_rejects_path_traversal_segments() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/assets/../manifest.json")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "not_found");
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_rejects_missing_client_version_headers() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/api/v1/control/status")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    assert_eq!(
        response
            .headers()
            .get(super::HEADER_OA_COMPAT_CODE)
            .and_then(|value| value.to_str().ok()),
        Some("invalid_client_build")
    );
    assert_eq!(
        response
            .headers()
            .get(super::HEADER_OA_COMPAT_UPGRADE_REQUIRED)
            .and_then(|value| value.to_str().ok()),
        Some("true")
    );
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "invalid_client_build");
    assert_eq!(body["compatibility"]["upgrade_required"], true);
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_rejects_client_below_minimum_build() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/api/v1/control/status")
        .header("x-oa-client-build-id", "20260221T110000Z")
        .header("x-oa-protocol-version", "openagents.control.v1")
        .header("x-oa-schema-version", "1")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "upgrade_required");
    assert_eq!(
        body["compatibility"]["min_client_build_id"],
        "20260221T120000Z"
    );
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_allows_supported_client_version() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .uri("/api/v1/control/status")
        .header("x-oa-client-build-id", "20260221T130000Z")
        .header("x-oa-protocol-version", "openagents.control.v1")
        .header("x-oa-schema-version", "1")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_rejects_legacy_chat_stream_missing_headers() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/chat/stream")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"messages":[{"role":"user","content":"compat please"}]}"#,
        ))?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    assert_eq!(
        response
            .headers()
            .get(super::HEADER_OA_COMPAT_CODE)
            .and_then(|value| value.to_str().ok()),
        Some("invalid_client_build")
    );
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "invalid_client_build");
    assert_eq!(body["compatibility"]["upgrade_required"], true);
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_rejects_legacy_chat_stream_protocol_mismatch() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/chats/thread-compat/stream")
        .header("content-type", "application/json")
        .header("x-oa-client-build-id", "20260221T130000Z")
        .header("x-oa-protocol-version", "openagents.control.v0")
        .header("x-oa-schema-version", "1")
        .body(Body::from(
            r#"{"messages":[{"role":"user","content":"compat protocol"}]}"#,
        ))?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "unsupported_protocol_version");
    assert_eq!(
        body["compatibility"]["protocol_version"],
        "openagents.control.v1"
    );
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_rejects_legacy_chat_stream_schema_mismatch() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/chat/stream")
        .header("content-type", "application/json")
        .header("x-oa-client-build-id", "20260221T130000Z")
        .header("x-oa-protocol-version", "openagents.control.v1")
        .header("x-oa-schema-version", "99")
        .body(Body::from(
            r#"{"messages":[{"role":"user","content":"compat schema"}]}"#,
        ))?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "unsupported_schema_version");
    assert_eq!(body["compatibility"]["min_schema_version"], 1);
    assert_eq!(body["compatibility"]["max_schema_version"], 1);
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_allows_legacy_chat_stream_supported_client() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/chat/stream")
        .header("content-type", "application/json")
        .header("x-oa-client-build-id", "20260221T130000Z")
        .header("x-oa-protocol-version", "openagents.control.v1")
        .header("x-oa-schema-version", "1")
        .body(Body::from(
            r#"{"messages":[{"role":"user","content":"compat accepted"}]}"#,
        ))?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "unauthorized");
    Ok(())
}

#[tokio::test]
async fn compatibility_gate_skips_auth_bootstrap_routes() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"compat-skip@openagents.com"}"#))?;
    let response = app.oneshot(request).await?;

    assert_eq!(response.status(), StatusCode::OK);
    Ok(())
}

#[tokio::test]
async fn compatibility_rejections_emit_audit_with_surface_and_build() -> Result<()> {
    let static_dir = tempdir()?;
    let sink = RecordingAuditSink::default();
    let app = build_router_with_observability(
        compat_enforced_config(static_dir.path().to_path_buf()),
        Observability::new(Arc::new(sink.clone())),
    );

    let request = Request::builder()
        .uri("/api/v1/control/status")
        .header("x-client", "autopilot-ios")
        .header("x-oa-client-build-id", "20260221T110000Z")
        .header("x-oa-protocol-version", "openagents.control.v1")
        .header("x-oa-schema-version", "1")
        .body(Body::empty())?;

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UPGRADE_REQUIRED);

    let events = sink.events();
    let compat_event = events
        .iter()
        .find(|event| event.event_name == "compatibility.rejected")
        .expect("missing compatibility rejection audit event");

    assert_eq!(
        compat_event.attributes.get("surface").map(String::as_str),
        Some("control_api")
    );
    assert_eq!(
        compat_event.attributes.get("client").map(String::as_str),
        Some("autopilot-ios")
    );
    assert_eq!(
        compat_event
            .attributes
            .get("client_build_id")
            .map(String::as_str),
        Some("20260221T110000Z")
    );

    Ok(())
}

#[tokio::test]
async fn auth_email_and_verify_flow_returns_session_tokens() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"test@example.com"}"#))?;

    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::OK);
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;

    let verify_response = app.clone().oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_body = read_json(verify_response).await?;

    assert_eq!(verify_body["status"], "authenticated");
    assert_eq!(verify_body["tokenType"], "Bearer");
    assert_eq!(verify_body["tokenName"], "mobile:autopilot-ios");

    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!token.is_empty());

    let session_request = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;

    let session_response = app.oneshot(session_request).await?;
    assert_eq!(session_response.status(), StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn auth_verify_requires_pending_challenge_cookie() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"code":"123456"}"#))?;

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "invalid_request");
    Ok(())
}

#[tokio::test]
async fn auth_email_rejects_when_workos_is_not_configured() -> Result<()> {
    let app = build_router(workos_required_config(std::env::temp_dir()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"workos-required@example.com"}"#))?;

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "service_unavailable");

    let message = body["error"]["message"].as_str().unwrap_or_default();
    assert!(message.contains("WorkOS identity provider is required"));

    Ok(())
}

#[tokio::test]
async fn auth_register_is_not_found_when_disabled() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"staging-user-1@staging.openagents.com"}"#,
        ))?;

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn auth_register_creates_user_and_returns_pat_when_enabled() -> Result<()> {
    let mut config = test_config(std::env::temp_dir());
    config.auth_api_signup_enabled = true;
    let app = build_router(config);

    let create_request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"staging-user-1@staging.openagents.com","name":"Staging User 1","tokenName":"staging-e2e"}"#,
            ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    assert_eq!(create_body["data"]["created"], true);
    assert_eq!(
        create_body["data"]["user"]["email"],
        "staging-user-1@staging.openagents.com"
    );
    assert_eq!(create_body["data"]["tokenName"], "staging-e2e");

    let token = create_body["data"]["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!token.is_empty());

    let second_request = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"staging-user-1@staging.openagents.com","name":"Updated Name"}"#,
        ))?;
    let second_response = app.oneshot(second_request).await?;
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = read_json(second_response).await?;
    assert_eq!(second_body["data"]["created"], false);
    assert_eq!(second_body["data"]["user"]["name"], "Updated Name");

    Ok(())
}

#[tokio::test]
async fn auth_register_enforces_allowed_domains_and_can_create_autopilot() -> Result<()> {
    let mut config = test_config(std::env::temp_dir());
    config.auth_api_signup_enabled = true;
    config.auth_api_signup_allowed_domains = vec!["staging.openagents.com".to_string()];
    let app = build_router(config);

    let blocked_request = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"blocked@example.com"}"#))?;
    let blocked_response = app.clone().oneshot(blocked_request).await?;
    assert_eq!(blocked_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let blocked_body = read_json(blocked_response).await?;
    assert_eq!(blocked_body["error"]["code"], "invalid_request");

    let allowed_request = Request::builder()
            .method("POST")
            .uri("/api/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"email":"creator@staging.openagents.com","createAutopilot":true,"autopilotDisplayName":"Creator Agent"}"#,
            ))?;
    let allowed_response = app.oneshot(allowed_request).await?;
    assert_eq!(allowed_response.status(), StatusCode::CREATED);
    let allowed_body = read_json(allowed_response).await?;
    assert_eq!(
        allowed_body["data"]["autopilot"]["displayName"],
        "Creator Agent"
    );
    assert!(allowed_body["data"]["autopilot"]["id"].as_str().is_some());

    Ok(())
}

#[tokio::test]
async fn login_page_redirects_home_when_already_authenticated() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"already-authed@example.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let challenge_cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("cookie", challenge_cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let access_token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let login_request = Request::builder()
        .uri("/login")
        .header("authorization", format!("Bearer {access_token}"))
        .body(Body::empty())?;
    let login_response = app.oneshot(login_request).await?;
    assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        login_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    Ok(())
}

#[tokio::test]
async fn web_login_email_and_verify_routes_set_auth_cookies() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/login/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("email=web-login%40openagents.com"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        send_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/login?status=code-sent")
    );
    let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
        .expect("missing challenge cookie");

    let verify_request = Request::builder()
        .method("POST")
        .uri("/login/verify")
        .header("content-type", "application/x-www-form-urlencoded")
        .header(
            "cookie",
            format!("{}={challenge_cookie}", super::CHALLENGE_COOKIE_NAME),
        )
        .body(Body::from("code=123456"))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        verify_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    let set_cookies = all_set_cookie_values(&verify_response);
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=", super::AUTH_ACCESS_COOKIE_NAME)))
    );
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=", super::AUTH_REFRESH_COOKIE_NAME)))
    );
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=;", super::CHALLENGE_COOKIE_NAME)))
    );

    let access_cookie = cookie_value_for_name(&verify_response, super::AUTH_ACCESS_COOKIE_NAME)
        .expect("missing access cookie");
    let login_request = Request::builder()
        .uri("/login")
        .header(
            "cookie",
            format!("{}={access_cookie}", super::AUTH_ACCESS_COOKIE_NAME),
        )
        .body(Body::empty())?;
    let login_response = app.oneshot(login_request).await?;
    assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        login_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    Ok(())
}

#[tokio::test]
async fn web_login_email_hx_request_returns_notice_fragment() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/login/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .body(Body::from("email=web-login-hx%40openagents.com"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::OK);
    assert_eq!(
        send_response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/html; charset=utf-8")
    );
    let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
        .expect("missing challenge cookie");
    assert!(!challenge_cookie.is_empty());
    let body = read_text(send_response).await?;
    assert!(body.contains("id=\"login-status\""));
    assert!(body.contains("A verification code was sent."));

    Ok(())
}

#[tokio::test]
async fn web_login_verify_hx_request_returns_hx_redirect_and_auth_cookies() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/login/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("email=web-login-hx-verify%40openagents.com"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
        .expect("missing challenge cookie");

    let verify_request = Request::builder()
        .method("POST")
        .uri("/login/verify")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header(
            "cookie",
            format!("{}={challenge_cookie}", super::CHALLENGE_COOKIE_NAME),
        )
        .body(Body::from("code=123456"))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::OK);
    assert_eq!(
        verify_response
            .headers()
            .get("HX-Redirect")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    let set_cookies = all_set_cookie_values(&verify_response);
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=", super::AUTH_ACCESS_COOKIE_NAME)))
    );
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=", super::AUTH_REFRESH_COOKIE_NAME)))
    );
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=;", super::CHALLENGE_COOKIE_NAME)))
    );

    Ok(())
}

#[tokio::test]
async fn web_login_verify_hx_request_invalid_code_returns_error_fragment() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/login/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("email=web-login-hx-invalid%40openagents.com"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
        .expect("missing challenge cookie");

    let verify_request = Request::builder()
        .method("POST")
        .uri("/login/verify")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header(
            "cookie",
            format!("{}={challenge_cookie}", super::CHALLENGE_COOKIE_NAME),
        )
        .body(Body::from("code=000000"))?;
    let verify_response = app.oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = read_text(verify_response).await?;
    assert!(body.contains("id=\"login-status\""));
    assert!(body.contains("Invalid sign-in code. Try again."));

    Ok(())
}

#[tokio::test]
async fn web_auth_cookies_include_secure_same_site_and_host_scope() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/login/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("email=cookie-scope%40openagents.com"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::TEMPORARY_REDIRECT);

    let challenge_cookie_header = all_set_cookie_values(&send_response)
        .into_iter()
        .find(|value| value.starts_with(&format!("{}=", super::CHALLENGE_COOKIE_NAME)))
        .expect("missing challenge set-cookie header");
    for required in ["HttpOnly", "Secure", "SameSite=Lax"] {
        assert!(
            challenge_cookie_header.contains(required),
            "challenge cookie missing attribute: {required}"
        );
    }
    assert!(
        !challenge_cookie_header.contains("Domain="),
        "challenge cookie should remain host-scoped"
    );

    let challenge_cookie_value =
        cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
            .expect("missing challenge cookie value");
    let verify_request = Request::builder()
        .method("POST")
        .uri("/login/verify")
        .header("content-type", "application/x-www-form-urlencoded")
        .header(
            "cookie",
            format!("{}={challenge_cookie_value}", super::CHALLENGE_COOKIE_NAME),
        )
        .body(Body::from("code=123456"))?;
    let verify_response = app.oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::TEMPORARY_REDIRECT);

    let verify_cookies = all_set_cookie_values(&verify_response);
    for name in [
        super::AUTH_ACCESS_COOKIE_NAME,
        super::AUTH_REFRESH_COOKIE_NAME,
    ] {
        let cookie = verify_cookies
            .iter()
            .find(|value| value.starts_with(&format!("{name}=")))
            .expect("missing auth set-cookie header");
        for required in ["HttpOnly", "Secure", "SameSite=Lax"] {
            assert!(
                cookie.contains(required),
                "cookie {name} missing attribute: {required}"
            );
        }
        assert!(
            !cookie.contains("Domain="),
            "cookie {name} should remain host-scoped"
        );
    }

    Ok(())
}

#[tokio::test]
async fn web_logout_clears_auth_cookies_and_redirects() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/login/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("email=logout-user%40openagents.com"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
        .expect("missing challenge cookie");

    let verify_request = Request::builder()
        .method("POST")
        .uri("/login/verify")
        .header("content-type", "application/x-www-form-urlencoded")
        .header(
            "cookie",
            format!("{}={challenge_cookie}", super::CHALLENGE_COOKIE_NAME),
        )
        .body(Body::from("code=123456"))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let access_cookie = cookie_value_for_name(&verify_response, super::AUTH_ACCESS_COOKIE_NAME)
        .expect("missing access cookie");
    let refresh_cookie = cookie_value_for_name(&verify_response, super::AUTH_REFRESH_COOKIE_NAME)
        .expect("missing refresh cookie");

    let logout_request = Request::builder()
        .method("POST")
        .uri("/logout")
        .header(
            "cookie",
            format!(
                "{}={access_cookie}; {}={refresh_cookie}",
                super::AUTH_ACCESS_COOKIE_NAME,
                super::AUTH_REFRESH_COOKIE_NAME
            ),
        )
        .body(Body::empty())?;
    let logout_response = app.oneshot(logout_request).await?;
    assert_eq!(logout_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        logout_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    let set_cookies = all_set_cookie_values(&logout_response);
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=;", super::AUTH_ACCESS_COOKIE_NAME)))
    );
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=;", super::AUTH_REFRESH_COOKIE_NAME)))
    );

    Ok(())
}

#[tokio::test]
async fn web_logout_hx_request_returns_hx_redirect_and_clears_cookies() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/login/email")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from("email=logout-hx-user%40openagents.com"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let challenge_cookie = cookie_value_for_name(&send_response, super::CHALLENGE_COOKIE_NAME)
        .expect("missing challenge cookie");

    let verify_request = Request::builder()
        .method("POST")
        .uri("/login/verify")
        .header("content-type", "application/x-www-form-urlencoded")
        .header(
            "cookie",
            format!("{}={challenge_cookie}", super::CHALLENGE_COOKIE_NAME),
        )
        .body(Body::from("code=123456"))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let access_cookie = cookie_value_for_name(&verify_response, super::AUTH_ACCESS_COOKIE_NAME)
        .expect("missing access cookie");
    let refresh_cookie = cookie_value_for_name(&verify_response, super::AUTH_REFRESH_COOKIE_NAME)
        .expect("missing refresh cookie");

    let logout_request = Request::builder()
        .method("POST")
        .uri("/logout")
        .header("hx-request", "true")
        .header(
            "cookie",
            format!(
                "{}={access_cookie}; {}={refresh_cookie}",
                super::AUTH_ACCESS_COOKIE_NAME,
                super::AUTH_REFRESH_COOKIE_NAME
            ),
        )
        .body(Body::empty())?;
    let logout_response = app.oneshot(logout_request).await?;
    assert_eq!(logout_response.status(), StatusCode::OK);
    assert_eq!(
        logout_response
            .headers()
            .get("HX-Redirect")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );

    let set_cookies = all_set_cookie_values(&logout_response);
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=;", super::AUTH_ACCESS_COOKIE_NAME)))
    );
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=;", super::AUTH_REFRESH_COOKIE_NAME)))
    );

    Ok(())
}

#[tokio::test]
async fn web_settings_profile_requires_auth_and_renders_settings_panel() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let unauth_request = Request::builder()
        .method("GET")
        .uri("/settings/profile")
        .body(Body::empty())?;
    let unauth_response = app.clone().oneshot(unauth_request).await?;
    assert_eq!(unauth_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        unauth_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/login")
    );

    let unauth_hx_request = Request::builder()
        .method("GET")
        .uri("/settings/profile")
        .header("hx-request", "true")
        .header("hx-target", "oa-main-shell")
        .body(Body::empty())?;
    let unauth_hx_response = app.clone().oneshot(unauth_hx_request).await?;
    assert_eq!(unauth_hx_response.status(), StatusCode::OK);
    assert_eq!(
        unauth_hx_response
            .headers()
            .get("HX-Redirect")
            .and_then(|value| value.to_str().ok()),
        Some("/login")
    );

    let token = authenticate_token(app.clone(), "settings-web@openagents.com").await?;
    let auth_request = Request::builder()
        .method("GET")
        .uri("/settings/profile")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let auth_response = app.oneshot(auth_request).await?;
    assert_eq!(auth_response.status(), StatusCode::OK);
    let body = read_text(auth_response).await?;
    assert!(body.contains("id=\"settings-main-panel\""));
    assert!(body.contains("Save profile"));
    assert!(body.contains("Connect or rotate Resend"));

    Ok(())
}

#[tokio::test]
async fn web_settings_profile_update_supports_hx_and_redirect_modes() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "settings-profile-form@openagents.com").await?;

    let hx_update_request = Request::builder()
        .method("POST")
        .uri("/settings/profile/update")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("name=Updated+Name"))?;
    let hx_update_response = app.clone().oneshot(hx_update_request).await?;
    assert_eq!(hx_update_response.status(), StatusCode::OK);
    let hx_body = read_text(hx_update_response).await?;
    assert!(hx_body.contains("id=\"settings-status\""));
    assert!(hx_body.contains("Profile updated."));

    let show_request = Request::builder()
        .method("GET")
        .uri("/settings/profile")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let show_response = app.clone().oneshot(show_request).await?;
    assert_eq!(show_response.status(), StatusCode::OK);
    let show_body = read_text(show_response).await?;
    assert!(show_body.contains("value=\"Updated Name\""));

    let redirect_update_request = Request::builder()
        .method("POST")
        .uri("/settings/profile/update")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("name=Updated+Again"))?;
    let redirect_update_response = app.oneshot(redirect_update_request).await?;
    assert_eq!(
        redirect_update_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(
        redirect_update_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/settings/profile?status=profile-updated")
    );

    Ok(())
}

#[tokio::test]
async fn web_settings_resend_forms_support_hx_lifecycle() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "settings-resend-form@openagents.com").await?;

    let connect_request = Request::builder()
            .method("POST")
            .uri("/settings/integrations/resend/upsert")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("hx-request", "true")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                "resend_api_key=re_live_1234567890&sender_email=bot%40openagents.com&sender_name=OpenAgents",
            ))?;
    let connect_response = app.clone().oneshot(connect_request).await?;
    assert_eq!(connect_response.status(), StatusCode::OK);
    let connect_body = read_text(connect_response).await?;
    assert!(connect_body.contains("Resend connected."));

    let test_request = Request::builder()
        .method("POST")
        .uri("/settings/integrations/resend/test-request")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let test_response = app.clone().oneshot(test_request).await?;
    assert_eq!(test_response.status(), StatusCode::OK);
    let test_body = read_text(test_response).await?;
    assert!(test_body.contains("Resend test event queued."));

    let disconnect_request = Request::builder()
        .method("POST")
        .uri("/settings/integrations/resend/disconnect")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let disconnect_response = app.oneshot(disconnect_request).await?;
    assert_eq!(disconnect_response.status(), StatusCode::OK);
    let disconnect_body = read_text(disconnect_response).await?;
    assert!(disconnect_body.contains("Resend disconnected."));

    Ok(())
}

#[tokio::test]
async fn web_settings_google_connect_unconfigured_returns_web_friendly_failure() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "settings-google-form@openagents.com").await?;

    let non_hx_request = Request::builder()
        .method("GET")
        .uri("/settings/integrations/google/connect")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let non_hx_response = app.clone().oneshot(non_hx_request).await?;
    assert_eq!(non_hx_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/settings/profile?status=settings-action-failed")
    );

    let hx_request = Request::builder()
        .method("GET")
        .uri("/settings/integrations/google/connect")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let hx_response = app.oneshot(hx_request).await?;
    assert_eq!(hx_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let hx_body = read_text(hx_response).await?;
    assert!(hx_body.contains("Settings action failed."));

    Ok(())
}

#[tokio::test]
async fn web_l402_and_billing_routes_render_l402_surface_for_authenticated_user() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let fixture = seed_l402_fixture(&config, "web-l402-reader@openagents.com").await?;
    let app = build_router(config);

    let l402_request = Request::builder()
        .method("GET")
        .uri("/l402")
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let l402_response = app.clone().oneshot(l402_request).await?;
    assert_eq!(l402_response.status(), StatusCode::OK);
    let l402_body = read_text(l402_response).await?;
    assert!(l402_body.contains("id=\"l402-main-panel\""));
    assert!(l402_body.contains("Billing + L402"));
    assert!(l402_body.contains("sats4ai.com"));
    assert!(l402_body.contains("Recent transactions"));

    let billing_request = Request::builder()
        .method("GET")
        .uri("/billing")
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let billing_response = app.clone().oneshot(billing_request).await?;
    assert_eq!(billing_response.status(), StatusCode::OK);
    let billing_body = read_text(billing_response).await?;
    assert!(billing_body.contains("id=\"l402-main-panel\""));
    assert!(billing_body.contains("Paywalls"));

    let unauth_request = Request::builder()
        .method("GET")
        .uri("/l402")
        .body(Body::empty())?;
    let unauth_response = app.oneshot(unauth_request).await?;
    assert_eq!(unauth_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        unauth_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/login")
    );

    Ok(())
}

#[tokio::test]
async fn web_l402_paywall_mutations_require_admin_and_support_htmx_flows() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let config_for_lookup = config.clone();

    let admin_token = seed_local_test_token(&config, "routes@openagents.com").await?;
    let member_token = seed_local_test_token(&config, "member-web-l402@openagents.com").await?;
    let app = build_router(config);

    let forbidden_create_request = Request::builder()
            .method("POST")
            .uri("/l402/paywalls/web/create")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("hx-request", "true")
            .header("authorization", format!("Bearer {member_token}"))
            .body(Body::from(
                "name=Default&host_regexp=sats4ai%5C.com&path_regexp=%5E%2Fapi%2F.*&price_msats=1000&upstream=https%3A%2F%2Fupstream.openagents.com&enabled=on",
            ))?;
    let forbidden_create_response = app.clone().oneshot(forbidden_create_request).await?;
    assert_eq!(forbidden_create_response.status(), StatusCode::FORBIDDEN);
    let forbidden_create_body = read_text(forbidden_create_response).await?;
    assert!(forbidden_create_body.contains("Admin role required for this action."));

    let create_request = Request::builder()
            .method("POST")
            .uri("/l402/paywalls/web/create")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("hx-request", "true")
            .header("authorization", format!("Bearer {admin_token}"))
            .body(Body::from(
                "name=Default&host_regexp=sats4ai%5C.com&path_regexp=%5E%2Fapi%2F.*&price_msats=1000&upstream=https%3A%2F%2Fupstream.openagents.com&enabled=on",
            ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::OK);
    let create_body = read_text(create_response).await?;
    assert!(create_body.contains("L402 paywall created."));

    let auth = super::AuthService::from_config(&config_for_lookup);
    let admin_bundle = auth
        .session_from_access_token(&admin_token)
        .await
        .expect("admin session");
    let store_after_create = super::DomainStore::from_config(&config_for_lookup);
    let created_paywalls = store_after_create
        .list_l402_paywalls_for_owner(&admin_bundle.user.id, false)
        .await
        .expect("created paywalls");
    assert_eq!(created_paywalls.len(), 1);
    let paywall_id = created_paywalls[0].id.clone();
    assert!(created_paywalls[0].enabled);

    let toggle_request = Request::builder()
        .method("POST")
        .uri(format!("/l402/paywalls/web/{paywall_id}/toggle"))
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::empty())?;
    let toggle_response = app.clone().oneshot(toggle_request).await?;
    assert_eq!(toggle_response.status(), StatusCode::OK);
    let toggle_body = read_text(toggle_response).await?;
    assert!(toggle_body.contains("L402 paywall updated."));

    let store_after_toggle = super::DomainStore::from_config(&config_for_lookup);
    let toggled_paywalls = store_after_toggle
        .list_l402_paywalls_for_owner(&admin_bundle.user.id, false)
        .await
        .expect("toggled paywalls");
    assert_eq!(toggled_paywalls.len(), 1);
    assert!(!toggled_paywalls[0].enabled);

    let delete_request = Request::builder()
        .method("POST")
        .uri(format!("/l402/paywalls/web/{paywall_id}/delete"))
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::empty())?;
    let delete_response = app.clone().oneshot(delete_request).await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_body = read_text(delete_response).await?;
    assert!(delete_body.contains("L402 paywall deleted."));

    let store_after_delete = super::DomainStore::from_config(&config_for_lookup);
    let deleted_paywalls = store_after_delete
        .list_l402_paywalls_for_owner(&admin_bundle.user.id, true)
        .await
        .expect("deleted paywalls");
    let deleted = deleted_paywalls
        .iter()
        .find(|row| row.id == paywall_id)
        .expect("deleted paywall present");
    assert!(deleted.deleted_at.is_some());

    Ok(())
}

#[tokio::test]
async fn web_admin_page_shows_forbidden_state_for_members_and_controls_for_admins() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let admin_token = seed_local_test_token(&config, "routes@openagents.com").await?;
    let member_token = seed_local_test_token(&config, "member-web-admin@openagents.com").await?;
    let app = build_router(config);

    let member_request = Request::builder()
        .method("GET")
        .uri("/admin")
        .header("authorization", format!("Bearer {member_token}"))
        .body(Body::empty())?;
    let member_response = app.clone().oneshot(member_request).await?;
    assert_eq!(member_response.status(), StatusCode::OK);
    let member_body = read_text(member_response).await?;
    assert!(member_body.contains("Admin role required for control-plane actions."));
    assert!(member_body.contains("Control actions are blocked for non-admin accounts."));

    let admin_request = Request::builder()
        .method("GET")
        .uri("/admin")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::empty())?;
    let admin_response = app.oneshot(admin_request).await?;
    assert_eq!(admin_response.status(), StatusCode::OK);
    let admin_body = read_text(admin_response).await?;
    assert!(admin_body.contains("Route Split Status"));
    assert!(admin_body.contains("Runtime Routing Status"));
    assert!(admin_body.contains("/admin/route-split/evaluate"));
    assert!(admin_body.contains("/admin/lightning-ops/query"));

    Ok(())
}

#[tokio::test]
async fn web_admin_route_split_and_runtime_override_render_htmx_result_fragments() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let admin_token = seed_local_test_token(&config, "routes@openagents.com").await?;
    let member_token =
        seed_local_test_token(&config, "member-web-admin-gate@openagents.com").await?;
    let app = build_router(config);

    let forbidden_request = Request::builder()
        .method("POST")
        .uri("/admin/route-split/evaluate")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {member_token}"))
        .body(Body::from("path=%2Fchat"))?;
    let forbidden_response = app.clone().oneshot(forbidden_request).await?;
    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);
    let forbidden_body = read_text(forbidden_response).await?;
    assert!(forbidden_body.contains("Admin role required."));

    let evaluate_request = Request::builder()
        .method("POST")
        .uri("/admin/route-split/evaluate")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("path=%2Fchat%2Fthread_123&cohort_key=user%3A1"))?;
    let evaluate_response = app.clone().oneshot(evaluate_request).await?;
    assert_eq!(evaluate_response.status(), StatusCode::OK);
    let evaluate_body = read_text(evaluate_response).await?;
    assert!(evaluate_body.contains("id=\"admin-result\""));
    assert!(evaluate_body.contains("&quot;action&quot;: &quot;route_split.evaluate&quot;"));

    let invalid_override_request = Request::builder()
        .method("POST")
        .uri("/admin/runtime-routing/override")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("scope_type=user&scope_id=usr_1&driver=invalid"))?;
    let invalid_override_response = app.clone().oneshot(invalid_override_request).await?;
    assert_eq!(
        invalid_override_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let invalid_override_body = read_text(invalid_override_response).await?;
    assert!(invalid_override_body.contains("id=\"admin-result\""));
    assert!(invalid_override_body.contains("invalid_driver"));

    let override_request = Request::builder()
        .method("POST")
        .uri("/admin/runtime-routing/override")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from(
            "scope_type=user&scope_id=usr_1&driver=legacy&is_active=on&reason=manual",
        ))?;
    let override_response = app.oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);
    let override_body = read_text(override_response).await?;
    assert!(override_body.contains("id=\"admin-result\""));
    assert!(override_body.contains("&quot;action&quot;: &quot;runtime_routing.override&quot;"));

    Ok(())
}

#[tokio::test]
async fn web_admin_lightning_ops_forms_render_query_and_validation_results() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let admin_token = seed_local_test_token(&config, "routes@openagents.com").await?;
    let app = build_router(config);

    let invalid_args_request = Request::builder()
        .method("POST")
        .uri("/admin/lightning-ops/query")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from(
            "function_name=lightning%2Fops%3AlistPaywallControlPlaneState&args_json=not-json",
        ))?;
    let invalid_args_response = app.clone().oneshot(invalid_args_request).await?;
    assert_eq!(
        invalid_args_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let invalid_args_body = read_text(invalid_args_response).await?;
    assert!(invalid_args_body.contains("id=\"admin-result\""));
    assert!(invalid_args_body.contains("invalid_args_json"));

    let query_request = Request::builder()
            .method("POST")
            .uri("/admin/lightning-ops/query")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("hx-request", "true")
            .header("authorization", format!("Bearer {admin_token}"))
            .body(Body::from(
                "function_name=lightning%2Fops%3AlistPaywallControlPlaneState&args_json=%7B%22secret%22%3A%22ops-secret-test%22%7D",
            ))?;
    let query_response = app.oneshot(query_request).await?;
    assert_eq!(query_response.status(), StatusCode::OK);
    let query_body = read_text(query_response).await?;
    assert!(query_body.contains("id=\"admin-result\""));
    assert!(query_body.contains("&quot;ok&quot;: true"));

    Ok(())
}

#[tokio::test]
async fn htmx_dual_mode_contract_matrix_for_web_mutation_endpoints() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));

    let admin_token = seed_local_test_token(&config, "routes@openagents.com").await?;
    let app = build_router(config);

    let chat_new_hx_request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::empty())?;
    let chat_new_hx_response = app.clone().oneshot(chat_new_hx_request).await?;
    assert_eq!(chat_new_hx_response.status(), StatusCode::OK);
    assert_eq!(
        chat_new_hx_response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/html; charset=utf-8")
    );
    let chat_new_hx_body = read_text(chat_new_hx_response).await?;
    assert!(chat_new_hx_body.contains("id=\"chat-thread-content-panel\""));
    assert!(!chat_new_hx_body.contains("<html"));

    let chat_new_non_hx_request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::empty())?;
    let chat_new_non_hx_response = app.clone().oneshot(chat_new_non_hx_request).await?;
    assert_eq!(
        chat_new_non_hx_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    let chat_location = chat_new_non_hx_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(chat_location.starts_with("/chat/thread_"));
    let thread_id = chat_location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(!thread_id.is_empty());

    let chat_send_hx_request = Request::builder()
        .method("POST")
        .uri(format!("/chat/{thread_id}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("text=contract+matrix"))?;
    let chat_send_hx_response = app.clone().oneshot(chat_send_hx_request).await?;
    assert_eq!(chat_send_hx_response.status(), StatusCode::OK);
    assert_eq!(
        chat_send_hx_response
            .headers()
            .get("HX-Trigger")
            .and_then(|value| value.to_str().ok()),
        Some("chat-message-sent")
    );
    let chat_send_hx_body = read_text(chat_send_hx_response).await?;
    assert!(chat_send_hx_body.contains("id=\"chat-status\""));
    assert!(!chat_send_hx_body.contains("<html"));

    let chat_send_non_hx_request = Request::builder()
        .method("POST")
        .uri(format!("/chat/{thread_id}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("text=contract+matrix+nonhx"))?;
    let chat_send_non_hx_response = app.clone().oneshot(chat_send_non_hx_request).await?;
    assert_eq!(
        chat_send_non_hx_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    let expected_chat_location = format!("/chat/{thread_id}?status=message-sent");
    assert_eq!(
        chat_send_non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some(expected_chat_location.as_str())
    );

    let feed_hx_request = Request::builder()
        .method("POST")
        .uri("/feed/shout")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("zone=dev&body=dual+mode"))?;
    let feed_hx_response = app.clone().oneshot(feed_hx_request).await?;
    assert_eq!(feed_hx_response.status(), StatusCode::OK);
    assert_eq!(
        feed_hx_response
            .headers()
            .get("HX-Trigger")
            .and_then(|value| value.to_str().ok()),
        Some("feed-shout-posted")
    );
    let feed_hx_body = read_text(feed_hx_response).await?;
    assert!(feed_hx_body.contains("id=\"feed-status\""));
    assert!(!feed_hx_body.contains("<html"));

    let feed_non_hx_request = Request::builder()
        .method("POST")
        .uri("/feed/shout")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("zone=dev&body=dual+mode+nonhx"))?;
    let feed_non_hx_response = app.clone().oneshot(feed_non_hx_request).await?;
    assert_eq!(
        feed_non_hx_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(
        feed_non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/feed?zone=dev&status=shout-posted")
    );

    let settings_hx_request = Request::builder()
        .method("POST")
        .uri("/settings/profile/update")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("name=Dual+Mode+Tester"))?;
    let settings_hx_response = app.clone().oneshot(settings_hx_request).await?;
    assert_eq!(settings_hx_response.status(), StatusCode::OK);
    let settings_hx_body = read_text(settings_hx_response).await?;
    assert!(settings_hx_body.contains("id=\"settings-status\""));
    assert!(!settings_hx_body.contains("<html"));

    let settings_non_hx_request = Request::builder()
        .method("POST")
        .uri("/settings/profile/update")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("name=Dual+Mode+Tester+NonHX"))?;
    let settings_non_hx_response = app.clone().oneshot(settings_non_hx_request).await?;
    assert_eq!(
        settings_non_hx_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(
        settings_non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/settings/profile?status=profile-updated")
    );

    let l402_hx_request = Request::builder()
            .method("POST")
            .uri("/l402/paywalls/web/create")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("hx-request", "true")
            .header("authorization", format!("Bearer {admin_token}"))
            .body(Body::from(
                "name=Dual%20Mode&host_regexp=sats4ai%5C.com&path_regexp=%5E%2Fapi%2F.*&price_msats=1000&upstream=https%3A%2F%2Fupstream.openagents.com&enabled=on",
            ))?;
    let l402_hx_response = app.clone().oneshot(l402_hx_request).await?;
    assert_eq!(l402_hx_response.status(), StatusCode::OK);
    let l402_hx_body = read_text(l402_hx_response).await?;
    assert!(l402_hx_body.contains("id=\"billing-status\""));
    assert!(!l402_hx_body.contains("<html"));

    let l402_non_hx_request = Request::builder()
            .method("POST")
            .uri("/l402/paywalls/web/create")
            .header("content-type", "application/x-www-form-urlencoded")
            .header("authorization", format!("Bearer {admin_token}"))
            .body(Body::from(
                "name=Dual%20Mode%20NonHX&host_regexp=sats4ai%5C.com&path_regexp=%5E%2Fapi%2F.*&price_msats=1000&upstream=https%3A%2F%2Fupstream.openagents.com&enabled=on",
            ))?;
    let l402_non_hx_response = app.clone().oneshot(l402_non_hx_request).await?;
    assert_eq!(
        l402_non_hx_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(
        l402_non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/l402?status=l402-paywall-created")
    );

    let admin_hx_request = Request::builder()
        .method("POST")
        .uri("/admin/route-split/evaluate")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("path=%2Fchat%2Fthread_1"))?;
    let admin_hx_response = app.clone().oneshot(admin_hx_request).await?;
    assert_eq!(admin_hx_response.status(), StatusCode::OK);
    let admin_hx_body = read_text(admin_hx_response).await?;
    assert!(admin_hx_body.contains("id=\"admin-result\""));
    assert!(!admin_hx_body.contains("<html"));

    let admin_non_hx_request = Request::builder()
        .method("POST")
        .uri("/admin/route-split/evaluate")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("path=%2Fchat%2Fthread_1"))?;
    let admin_non_hx_response = app.oneshot(admin_non_hx_request).await?;
    assert_eq!(
        admin_non_hx_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    assert_eq!(
        admin_non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/admin?status=admin-action-completed")
    );

    Ok(())
}

#[tokio::test]
async fn web_chat_new_thread_hx_returns_partial_fragment_and_push_url() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));
    let token = seed_local_test_token(&config, "chat-hx-new@openagents.com").await?;
    let app = build_router(config);

    let request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let push_url = response
        .headers()
        .get("HX-Push-Url")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(push_url.starts_with("/chat/thread_"));

    let body = read_text(response).await?;
    assert!(body.contains("id=\"chat-thread-content-panel\""));
    assert!(body.contains("id=\"chat-thread-list-panel\""));
    assert!(body.contains("hx-swap-oob=\"outerHTML\""));
    assert!(body.contains("Thread created."));
    assert!(!body.contains("<html"));
    assert!(!body.contains("<body"));

    Ok(())
}

#[tokio::test]
async fn web_chat_thread_select_hx_updates_partial_and_non_hx_redirects() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));
    let token = seed_local_test_token(&config, "chat-hx-select@openagents.com").await?;
    let app = build_router(config);

    let first_create = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let first_create_response = app.clone().oneshot(first_create).await?;
    assert_eq!(
        first_create_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    let first_location = first_create_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let first_thread_id = first_location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(first_thread_id.starts_with("thread_"));

    let second_create = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let second_create_response = app.clone().oneshot(second_create).await?;
    assert_eq!(
        second_create_response.status(),
        StatusCode::TEMPORARY_REDIRECT
    );
    let second_location = second_create_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let second_thread_id = second_location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(second_thread_id.starts_with("thread_"));
    assert_ne!(second_thread_id, first_thread_id);

    let select_request = Request::builder()
        .method("GET")
        .uri(format!("/chat/fragments/thread/{second_thread_id}"))
        .header("hx-request", "true")
        .header("hx-target", "chat-thread-content-panel")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let select_response = app.clone().oneshot(select_request).await?;
    assert_eq!(select_response.status(), StatusCode::OK);
    let expected_push_url = format!("/chat/{second_thread_id}");
    assert_eq!(
        select_response
            .headers()
            .get("HX-Push-Url")
            .and_then(|value| value.to_str().ok()),
        Some(expected_push_url.as_str())
    );
    let select_body = read_text(select_response).await?;
    assert!(select_body.contains("id=\"chat-thread-content-panel\""));
    assert!(select_body.contains("hx-swap-oob=\"outerHTML\""));
    assert!(select_body.contains(&format!("Thread: <code>{second_thread_id}</code>")));
    assert!(select_body.contains("oa-thread-link active"));
    assert!(!select_body.contains("<html"));

    let non_hx_request = Request::builder()
        .method("GET")
        .uri(format!("/chat/fragments/thread/{second_thread_id}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let non_hx_response = app.oneshot(non_hx_request).await?;
    assert_eq!(non_hx_response.status(), StatusCode::TEMPORARY_REDIRECT);
    let expected_location = format!("/chat/{second_thread_id}");
    assert_eq!(
        non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some(expected_location.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn web_chat_send_message_hx_success_triggers_incremental_refresh() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));
    let token = seed_local_test_token(&config, "chat-send-success@openagents.com").await?;
    let app = build_router(config);

    let create_request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let create_response = app.clone().oneshot(create_request).await?;
    let location = create_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let thread_id = location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(!thread_id.is_empty());

    let send_request = Request::builder()
        .method("POST")
        .uri(format!("/chat/{thread_id}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("text=hello"))?;
    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::OK);
    assert_eq!(
        send_response
            .headers()
            .get("HX-Trigger")
            .and_then(|value| value.to_str().ok()),
        Some("chat-message-sent")
    );
    let send_body = read_text(send_response).await?;
    assert!(send_body.contains("id=\"chat-status\""));
    assert!(send_body.contains("Message queued in thread."));

    let refresh_request = Request::builder()
        .method("GET")
        .uri(format!("/chat/fragments/thread/{thread_id}"))
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let refresh_response = app.oneshot(refresh_request).await?;
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_body = read_text(refresh_response).await?;
    assert!(refresh_body.contains("hello"));

    Ok(())
}

#[tokio::test]
async fn web_chat_send_message_hx_empty_body_returns_inline_error() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));
    let token = seed_local_test_token(&config, "chat-send-empty@openagents.com").await?;
    let app = build_router(config);

    let create_request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let create_response = app.clone().oneshot(create_request).await?;
    let location = create_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let thread_id = location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(!thread_id.is_empty());

    let send_request = Request::builder()
        .method("POST")
        .uri(format!("/chat/{thread_id}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("text=%20%20"))?;
    let send_response = app.oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        send_response.headers().get("HX-Trigger").is_none(),
        "validation error should not emit success trigger"
    );
    let send_body = read_text(send_response).await?;
    assert!(send_body.contains("id=\"chat-status\""));
    assert!(send_body.contains("Message body cannot be empty."));

    Ok(())
}

#[tokio::test]
async fn web_chat_send_message_hx_oversized_body_returns_inline_error() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));
    let token = seed_local_test_token(&config, "chat-send-oversize@openagents.com").await?;
    let app = build_router(config);

    let create_request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let create_response = app.clone().oneshot(create_request).await?;
    let location = create_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let thread_id = location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(!thread_id.is_empty());

    let oversized = "a".repeat(20_001);
    let send_request = Request::builder()
        .method("POST")
        .uri(format!("/chat/{thread_id}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!("text={oversized}")))?;
    let send_response = app.oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(send_response.headers().get("HX-Trigger").is_none());
    let send_body = read_text(send_response).await?;
    assert!(send_body.contains("Could not send message."));

    Ok(())
}

#[tokio::test]
async fn web_chat_send_message_hx_store_failure_returns_inline_error() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));
    let owner_token = seed_local_test_token(&config, "chat-send-owner@openagents.com").await?;
    let other_token = seed_local_test_token(&config, "chat-send-other@openagents.com").await?;
    let app = build_router(config);

    let create_request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {owner_token}"))
        .body(Body::empty())?;
    let create_response = app.clone().oneshot(create_request).await?;
    let location = create_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let thread_id = location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(!thread_id.is_empty());

    let send_request = Request::builder()
        .method("POST")
        .uri(format!("/chat/{thread_id}/send"))
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::from("text=hello"))?;
    let send_response = app.oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(send_response.headers().get("HX-Trigger").is_none());
    let send_body = read_text(send_response).await?;
    assert!(send_body.contains("Could not send message."));

    Ok(())
}

#[tokio::test]
async fn web_chat_fragment_renders_turn_finish_error_tool_from_worker_event_store() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.codex_thread_store_path = Some(static_dir.path().join("thread-store.json"));
    let token = seed_local_test_token(&config, "chat-worker-bridge@openagents.com").await?;
    let app = build_router(config);

    let create_request = Request::builder()
        .method("POST")
        .uri("/chat/new")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let create_response = app.clone().oneshot(create_request).await?;
    let location = create_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let thread_id = location
        .trim_start_matches("/chat/")
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(!thread_id.is_empty());

    let events = [
        serde_json::json!({
            "event": {
                "event_type": "worker.event",
                "payload": {
                    "method": "turn/start",
                    "thread_id": thread_id.clone(),
                    "turn": { "id": "turn_1" },
                    "occurred_at": "2026-02-22T00:00:00Z"
                }
            }
        }),
        serde_json::json!({
            "event": {
                "event_type": "worker.event",
                "payload": {
                    "method": "turn/tool",
                    "thread_id": thread_id.clone(),
                    "tool": { "name": "search", "status": "running" },
                    "occurred_at": "2026-02-22T00:00:01Z"
                }
            }
        }),
        serde_json::json!({
            "event": {
                "event_type": "worker.event",
                "payload": {
                    "method": "turn/error",
                    "thread_id": thread_id.clone(),
                    "error": { "message": "network" },
                    "occurred_at": "2026-02-22T00:00:02Z"
                }
            }
        }),
        serde_json::json!({
            "event": {
                "event_type": "worker.event",
                "payload": {
                    "method": "turn/finish",
                    "thread_id": thread_id.clone(),
                    "output_text": "final answer",
                    "occurred_at": "2026-02-22T00:00:03Z"
                }
            }
        }),
    ];

    for event in events {
        let request = Request::builder()
            .method("POST")
            .uri(format!("/api/runtime/codex/workers/{thread_id}/events"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(event.to_string()))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
    }

    let fragment_request = Request::builder()
        .method("GET")
        .uri(format!("/chat/fragments/thread/{thread_id}"))
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let fragment_response = app.oneshot(fragment_request).await?;
    assert_eq!(fragment_response.status(), StatusCode::OK);
    let fragment = read_text(fragment_response).await?;

    let start_index = fragment
        .find("Turn started: turn_1")
        .expect("missing turn.start render");
    let tool_index = fragment
        .find("Tool search: running")
        .expect("missing turn.tool render");
    let error_index = fragment
        .find("Turn error: network")
        .expect("missing turn.error render");
    let finish_index = fragment
        .find("Turn finished: final answer")
        .expect("missing turn.finish render");
    assert!(start_index < tool_index);
    assert!(tool_index < error_index);
    assert!(error_index < finish_index);

    Ok(())
}

#[tokio::test]
async fn local_test_login_route_enforces_gates_and_accepts_valid_signature() -> Result<()> {
    let mut config = test_config(std::env::temp_dir());
    config.auth_local_test_login_enabled = true;
    config.auth_local_test_login_allowed_emails = vec!["tester@openagents.com".to_string()];
    config.auth_local_test_login_signing_key = Some("local-test-signing-key".to_string());
    let app = build_router(config);

    let unsigned_request = Request::builder()
        .uri("/internal/test-login?email=tester@openagents.com&expires=4102444800")
        .body(Body::empty())?;
    let unsigned_response = app.clone().oneshot(unsigned_request).await?;
    assert_eq!(unsigned_response.status(), StatusCode::FORBIDDEN);

    let blocked_url = signed_test_login_url(
        "local-test-signing-key",
        "blocked@example.com",
        4_102_444_800,
        Some("MaintenanceTester"),
    );
    let blocked_request = Request::builder().uri(blocked_url).body(Body::empty())?;
    let blocked_response = app.clone().oneshot(blocked_request).await?;
    assert_eq!(blocked_response.status(), StatusCode::FORBIDDEN);

    let allowed_url = signed_test_login_url(
        "local-test-signing-key",
        "tester@openagents.com",
        4_102_444_800,
        Some("MaintenanceTester"),
    );
    let allowed_request = Request::builder().uri(allowed_url).body(Body::empty())?;
    let allowed_response = app.oneshot(allowed_request).await?;
    assert_eq!(allowed_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        allowed_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/")
    );
    let set_cookies = all_set_cookie_values(&allowed_response);
    assert!(
        set_cookies
            .iter()
            .any(|value| value.starts_with(&format!("{}=", super::AUTH_ACCESS_COOKIE_NAME)))
    );
    assert!(
        set_cookies.iter().any(|value| {
            value.starts_with(&format!("{}=1", super::LOCAL_TEST_AUTH_COOKIE_NAME))
        })
    );

    Ok(())
}

#[tokio::test]
async fn local_test_login_route_returns_not_found_when_disabled() -> Result<()> {
    let mut config = test_config(std::env::temp_dir());
    config.auth_local_test_login_enabled = false;
    config.auth_local_test_login_allowed_emails = vec!["tester@openagents.com".to_string()];
    config.auth_local_test_login_signing_key = Some("local-test-signing-key".to_string());
    let app = build_router(config);

    let request = Request::builder()
        .uri(signed_test_login_url(
            "local-test-signing-key",
            "tester@openagents.com",
            4_102_444_800,
            Some("MaintenanceTester"),
        ))
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn refresh_rotates_refresh_token_and_logout_revokes_session() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"rotation@example.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-desktop")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;

    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let refresh_token = verify_body["refreshToken"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let refresh_request = Request::builder()
        .method("POST")
        .uri("/api/auth/refresh")
        .header("content-type", "application/json")
        .body(Body::from(format!(
            r#"{{"refresh_token":"{refresh_token}","rotate_refresh_token":true}}"#
        )))?;
    let refresh_response = app.clone().oneshot(refresh_request).await?;
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_body = read_json(refresh_response).await?;
    let new_token = refresh_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let logout_request = Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header("authorization", format!("Bearer {new_token}"))
        .body(Body::empty())?;
    let logout_response = app.clone().oneshot(logout_request).await?;
    assert_eq!(logout_response.status(), StatusCode::OK);

    let old_session_request = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let old_session_response = app.oneshot(old_session_request).await?;
    assert_eq!(old_session_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn refresh_token_is_single_use_and_replay_revokes_session() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"refresh-replay@example.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-replay-device")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_body = read_json(verify_response).await?;
    let refresh_token = verify_body["refreshToken"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let rotate_request = Request::builder()
            .method("POST")
            .uri("/api/auth/refresh")
            .header("content-type", "application/json")
            .header("x-device-id", "ios-replay-device")
            .body(Body::from(format!(
                r#"{{"refresh_token":"{refresh_token}","rotate_refresh_token":true,"device_id":"ios-replay-device"}}"#
            )))?;
    let rotate_response = app.clone().oneshot(rotate_request).await?;
    assert_eq!(rotate_response.status(), StatusCode::OK);
    let rotate_body = read_json(rotate_response).await?;
    let rotated_access_token = rotate_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let replay_request = Request::builder()
            .method("POST")
            .uri("/api/auth/refresh")
            .header("content-type", "application/json")
            .header("x-device-id", "ios-replay-device")
            .body(Body::from(format!(
                r#"{{"refresh_token":"{refresh_token}","rotate_refresh_token":true,"device_id":"ios-replay-device"}}"#
            )))?;
    let replay_response = app.clone().oneshot(replay_request).await?;
    assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);

    let session_after_replay = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {rotated_access_token}"))
        .body(Body::empty())?;
    let session_after_replay_response = app.oneshot(session_after_replay).await?;
    assert_eq!(
        session_after_replay_response.status(),
        StatusCode::UNAUTHORIZED
    );

    Ok(())
}

#[tokio::test]
async fn refresh_parallel_rotation_race_revokes_rotated_session() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"refresh-race@example.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-race-device")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::OK);
    let verify_body = read_json(verify_response).await?;
    let refresh_token = verify_body["refreshToken"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let refresh_payload = format!(
        r#"{{"refresh_token":"{refresh_token}","rotate_refresh_token":true,"device_id":"ios-race-device"}}"#
    );
    let refresh_request_a = Request::builder()
        .method("POST")
        .uri("/api/auth/refresh")
        .header("content-type", "application/json")
        .header("x-device-id", "ios-race-device")
        .body(Body::from(refresh_payload.clone()))?;
    let refresh_request_b = Request::builder()
        .method("POST")
        .uri("/api/auth/refresh")
        .header("content-type", "application/json")
        .header("x-device-id", "ios-race-device")
        .body(Body::from(refresh_payload))?;

    let (refresh_a_result, refresh_b_result) = tokio::join!(
        app.clone().oneshot(refresh_request_a),
        app.clone().oneshot(refresh_request_b)
    );
    let refresh_a = refresh_a_result?;
    let refresh_b = refresh_b_result?;

    let mut status_codes = vec![refresh_a.status().as_u16(), refresh_b.status().as_u16()];
    status_codes.sort();
    assert_eq!(status_codes, vec![200, 401]);

    let mut rotated_access_token = String::new();
    for response in [refresh_a, refresh_b] {
        let status = response.status();
        let body = read_json(response).await?;
        if status == StatusCode::OK {
            rotated_access_token = body["token"].as_str().unwrap_or_default().to_string();
        } else {
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(body["error"]["code"], json!("unauthorized"));
        }
    }

    assert!(!rotated_access_token.is_empty());

    let session_after_race = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {rotated_access_token}"))
        .body(Body::empty())?;
    let session_after_race_response = app.oneshot(session_after_race).await?;
    assert_eq!(
        session_after_race_response.status(),
        StatusCode::UNAUTHORIZED
    );

    Ok(())
}

#[tokio::test]
async fn session_listing_and_device_revocation_are_supported() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_a = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"device-revoke@example.com"}"#))?;
    let send_a_response = app.clone().oneshot(send_a).await?;
    let cookie_a = cookie_value(&send_a_response).unwrap_or_default();

    let verify_a = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-device-a")
        .header("cookie", cookie_a)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_a_response = app.clone().oneshot(verify_a).await?;
    let verify_a_body = read_json(verify_a_response).await?;
    let token_a = verify_a_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let send_b = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"device-revoke@example.com"}"#))?;
    let send_b_response = app.clone().oneshot(send_b).await?;
    let cookie_b = cookie_value(&send_b_response).unwrap_or_default();

    let verify_b = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-device-b")
        .header("cookie", cookie_b)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_b_response = app.clone().oneshot(verify_b).await?;
    let verify_b_body = read_json(verify_b_response).await?;
    let token_b = verify_b_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let list_request = Request::builder()
        .uri("/api/auth/sessions")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::empty())?;
    let list_response = app.clone().oneshot(list_request).await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await?;
    let sessions = list_body["data"]["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(sessions.len(), 2);

    let revoke_request = Request::builder()
        .method("POST")
        .uri("/api/auth/sessions/revoke")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::from(
            r#"{"device_id":"ios-device-b","reason":"user_requested","include_current":false}"#,
        ))?;
    let revoke_response = app.clone().oneshot(revoke_request).await?;
    assert_eq!(revoke_response.status(), StatusCode::OK);
    let revoke_body = read_json(revoke_response).await?;
    let revoked_sessions = revoke_body["revokedSessionIds"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let revoked_devices = revoke_body["revokedDeviceIds"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(revoked_sessions.len(), 1);
    assert_eq!(revoked_devices, vec![json!("ios-device-b")]);

    let current_a = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::empty())?;
    let current_a_response = app.clone().oneshot(current_a).await?;
    assert_eq!(current_a_response.status(), StatusCode::OK);

    let current_b = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token_b}"))
        .body(Body::empty())?;
    let current_b_response = app.oneshot(current_b).await?;
    assert_eq!(current_b_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn global_revocation_supports_include_current_toggle() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_a = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"global-revoke@example.com"}"#))?;
    let send_a_response = app.clone().oneshot(send_a).await?;
    let cookie_a = cookie_value(&send_a_response).unwrap_or_default();

    let verify_a = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-global-a")
        .header("cookie", cookie_a)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_a_response = app.clone().oneshot(verify_a).await?;
    let verify_a_body = read_json(verify_a_response).await?;
    let token_a = verify_a_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let send_b = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"global-revoke@example.com"}"#))?;
    let send_b_response = app.clone().oneshot(send_b).await?;
    let cookie_b = cookie_value(&send_b_response).unwrap_or_default();

    let verify_b = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-global-b")
        .header("cookie", cookie_b)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_b_response = app.clone().oneshot(verify_b).await?;
    let verify_b_body = read_json(verify_b_response).await?;
    let token_b = verify_b_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let revoke_others_request = Request::builder()
        .method("POST")
        .uri("/api/auth/sessions/revoke")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::from(
            r#"{"revoke_all_sessions":true,"include_current":false,"reason":"user_requested"}"#,
        ))?;
    let revoke_others_response = app.clone().oneshot(revoke_others_request).await?;
    assert_eq!(revoke_others_response.status(), StatusCode::OK);

    let current_a = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::empty())?;
    let current_a_response = app.clone().oneshot(current_a).await?;
    assert_eq!(current_a_response.status(), StatusCode::OK);

    let current_b = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token_b}"))
        .body(Body::empty())?;
    let current_b_response = app.clone().oneshot(current_b).await?;
    assert_eq!(current_b_response.status(), StatusCode::UNAUTHORIZED);

    let revoke_all_request = Request::builder()
        .method("POST")
        .uri("/api/auth/sessions/revoke")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::from(
            r#"{"revoke_all_sessions":true,"include_current":true,"reason":"user_requested"}"#,
        ))?;
    let revoke_all_response = app.clone().oneshot(revoke_all_request).await?;
    assert_eq!(revoke_all_response.status(), StatusCode::OK);

    let current_a_after = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::empty())?;
    let current_a_after_response = app.oneshot(current_a_after).await?;
    assert_eq!(current_a_after_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn logout_propagates_runtime_revocation_when_configured() -> Result<()> {
    let captured = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (runtime_addr, runtime_handle) = start_runtime_revocation_stub(captured.clone()).await?;

    let mut config = test_config(std::env::temp_dir());
    config.runtime_sync_revoke_base_url = Some(format!("http://{runtime_addr}"));
    config.runtime_sync_revoke_path = "/internal/v1/sync/sessions/revoke".to_string();
    config.runtime_signature_secret = Some("runtime-signature-secret".to_string());
    config.runtime_signature_ttl_seconds = 60;

    let app = build_router(config);

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"runtime-revoke@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-runtime-revoke")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let logout_request = Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let logout_response = app.clone().oneshot(logout_request).await?;
    assert_eq!(logout_response.status(), StatusCode::OK);
    let logout_body = read_json(logout_response).await?;
    let revoked_session_id = logout_body["sessionId"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!revoked_session_id.is_empty());

    let records = captured.lock().await.clone();
    assert_eq!(records.len(), 1);
    assert!(
        records[0]["signature"]
            .as_str()
            .unwrap_or_default()
            .starts_with("v1.")
    );
    assert_eq!(records[0]["payload"]["reason"], "user_requested");
    assert_eq!(
        records[0]["payload"]["session_ids"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        vec![Value::String(revoked_session_id)]
    );
    assert_eq!(
        records[0]["payload"]["device_ids"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        vec![json!("ios-runtime-revoke")]
    );

    runtime_handle.abort();

    Ok(())
}

#[tokio::test]
async fn revoke_other_device_propagates_runtime_revocation_device_ids() -> Result<()> {
    let captured = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (runtime_addr, runtime_handle) = start_runtime_revocation_stub(captured.clone()).await?;

    let mut config = test_config(std::env::temp_dir());
    config.runtime_sync_revoke_base_url = Some(format!("http://{runtime_addr}"));
    config.runtime_sync_revoke_path = "/internal/v1/sync/sessions/revoke".to_string();
    config.runtime_signature_secret = Some("runtime-signature-secret".to_string());
    config.runtime_signature_ttl_seconds = 60;
    let app = build_router(config);

    let send_a = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"runtime-device-revoke@openagents.com"}"#,
        ))?;
    let send_a_response = app.clone().oneshot(send_a).await?;
    let cookie_a = cookie_value(&send_a_response).unwrap_or_default();
    let verify_a = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-runtime-a")
        .header("cookie", cookie_a)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_a_response = app.clone().oneshot(verify_a).await?;
    let verify_a_body = read_json(verify_a_response).await?;
    let token_a = verify_a_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let send_b = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"email":"runtime-device-revoke@openagents.com"}"#,
        ))?;
    let send_b_response = app.clone().oneshot(send_b).await?;
    let cookie_b = cookie_value(&send_b_response).unwrap_or_default();
    let verify_b = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("x-device-id", "ios-runtime-b")
        .header("cookie", cookie_b)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_b_response = app.clone().oneshot(verify_b).await?;
    let verify_b_body = read_json(verify_b_response).await?;
    let token_b = verify_b_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let revoke_request = Request::builder()
        .method("POST")
        .uri("/api/auth/sessions/revoke")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::from(
            r#"{"device_id":"ios-runtime-b","reason":"user_requested","include_current":false}"#,
        ))?;
    let revoke_response = app.clone().oneshot(revoke_request).await?;
    assert_eq!(revoke_response.status(), StatusCode::OK);
    let revoke_body = read_json(revoke_response).await?;
    let revoked_session_ids = revoke_body["revokedSessionIds"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(revoked_session_ids.len(), 1);
    assert_eq!(
        revoke_body["revokedDeviceIds"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        vec![json!("ios-runtime-b")]
    );

    let records = captured.lock().await.clone();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["payload"]["reason"], "user_requested");
    assert_eq!(
        records[0]["payload"]["device_ids"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        vec![json!("ios-runtime-b")]
    );
    assert_eq!(
        records[0]["payload"]["session_ids"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        revoked_session_ids
    );

    let current_a = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token_a}"))
        .body(Body::empty())?;
    let current_a_response = app.clone().oneshot(current_a).await?;
    assert_eq!(current_a_response.status(), StatusCode::OK);

    let current_b = Request::builder()
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token_b}"))
        .body(Body::empty())?;
    let current_b_response = app.oneshot(current_b).await?;
    assert_eq!(current_b_response.status(), StatusCode::UNAUTHORIZED);

    runtime_handle.abort();

    Ok(())
}

#[tokio::test]
async fn org_membership_and_policy_matrix_enforces_boundaries() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"policy@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let memberships_request = Request::builder()
        .uri("/api/orgs/memberships")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let memberships_response = app.clone().oneshot(memberships_request).await?;
    assert_eq!(memberships_response.status(), StatusCode::OK);
    let memberships_body = read_json(memberships_response).await?;
    let memberships = memberships_body["data"]["memberships"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let has_openagents_org = memberships.iter().any(|membership| {
        membership["org_id"]
            .as_str()
            .map(|org_id| org_id == "org:openagents")
            .unwrap_or(false)
    });
    assert!(has_openagents_org);

    let set_org_request = Request::builder()
        .method("POST")
        .uri("/api/orgs/active")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"org_id":"org:openagents"}"#))?;
    let set_org_response = app.clone().oneshot(set_org_request).await?;
    assert_eq!(set_org_response.status(), StatusCode::OK);

    let deny_scope_request = Request::builder()
            .method("POST")
            .uri("/api/policy/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"org_id":"org:openagents","required_scopes":["runtime.write"],"requested_topics":["org:openagents:workers"]}"#,
            ))?;
    let deny_scope_response = app.clone().oneshot(deny_scope_request).await?;
    assert_eq!(deny_scope_response.status(), StatusCode::OK);
    let deny_scope_body = read_json(deny_scope_response).await?;
    assert_eq!(deny_scope_body["data"]["allowed"], false);

    let allow_request = Request::builder()
            .method("POST")
            .uri("/api/policy/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"org_id":"org:openagents","required_scopes":["runtime.read"],"requested_topics":["org:openagents:workers"]}"#,
            ))?;
    let allow_response = app.clone().oneshot(allow_request).await?;
    assert_eq!(allow_response.status(), StatusCode::OK);
    let allow_body = read_json(allow_response).await?;
    assert_eq!(allow_body["data"]["allowed"], true);

    let deny_topic_request = Request::builder()
            .method("POST")
            .uri("/api/policy/authorize")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"org_id":"org:openagents","required_scopes":["runtime.read"],"requested_topics":["org:other:workers"]}"#,
            ))?;
    let deny_topic_response = app.oneshot(deny_topic_request).await?;
    assert_eq!(deny_topic_response.status(), StatusCode::OK);
    let deny_topic_body = read_json(deny_topic_response).await?;
    assert_eq!(deny_topic_body["data"]["allowed"], false);

    Ok(())
}

#[tokio::test]
async fn me_route_returns_user_profile_and_thread_summaries() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "me-route@openagents.com").await?;

    for thread_id in ["thread-a", "thread-b"] {
        let request = Request::builder()
            .method("POST")
            .uri(format!("/api/runtime/threads/{thread_id}/messages"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"text":"hello"}"#))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let me_request = Request::builder()
        .uri("/api/me?chat_limit=1")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let me_response = app.oneshot(me_request).await?;
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_body = read_json(me_response).await?;
    assert_eq!(me_body["data"]["user"]["email"], "me-route@openagents.com");
    assert_eq!(
        me_body["data"]["chatThreads"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(me_body["data"]["chatThreads"][0]["id"], "thread-b");

    Ok(())
}

#[test]
fn autopilot_tool_resolution_audit_applies_allowlist_and_denylist() {
    let aggregate = sample_autopilot_aggregate();
    let audit = super::autopilot_tool_resolution_audit(&aggregate, true);

    assert_eq!(audit["policyApplied"], json!(true));
    assert_eq!(audit["authRestricted"], json!(false));
    assert_eq!(audit["sessionAuthenticated"], json!(true));
    assert_eq!(audit["autopilotId"], json!("ap_test_1"));
    assert_eq!(audit["exposedTools"], json!(vec!["openagents_api"]));
    assert_eq!(
        audit["removedByDenylist"],
        json!(vec!["lightning_l402_fetch"])
    );

    let removed_by_allowlist = audit["removedByAllowlist"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        removed_by_allowlist
            .iter()
            .any(|tool| tool == &json!("lightning_l402_approve"))
    );
}

#[test]
fn autopilot_prompt_context_includes_profile_policy_and_limits() {
    let aggregate = sample_autopilot_aggregate();
    let context = super::autopilot_prompt_context(&aggregate).expect("context should exist");

    assert!(context.contains("autopilot_id=ap_test_1"));
    assert!(context.contains("config_version=3"));
    assert!(context.contains("owner_display_name=Chris"));
    assert!(context.contains("persona_summary=Pragmatic and concise"));
    assert!(context.contains("autopilot_voice=calm and direct"));
    assert!(context.contains("tool_allowlist=openagents_api,lightning_l402_fetch"));
    assert!(context.contains("tool_denylist=lightning_l402_fetch"));
    assert!(context.contains("l402_require_approval=true"));
    assert!(context.chars().count() <= 3200);
}

#[test]
fn autopilot_runtime_binding_payload_prefers_primary_binding() {
    let aggregate = sample_autopilot_aggregate();
    let binding = super::autopilot_runtime_binding_payload(&aggregate);

    assert_eq!(binding["id"], json!("arb_primary"));
    assert_eq!(binding["runtimeType"], json!("runtime"));
    assert_eq!(binding["runtimeRef"], json!("desktopw:autopilot"));
    assert_eq!(binding["driverHint"], json!("elixir"));
    assert_eq!(
        super::autopilot_runtime_binding_worker_ref(&aggregate),
        Some("desktopw:autopilot".to_string())
    );
}

#[tokio::test]
async fn autopilot_crud_routes_support_create_list_show_and_update() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "autopilot-owner@openagents.com").await?;

    let create_request = Request::builder()
            .method("POST")
            .uri("/api/autopilots")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"handle":"ep212-bot","displayName":"EP212 Bot","status":"active","visibility":"private"}"#,
            ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    let autopilot_id = create_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(autopilot_id.starts_with("ap_"));
    assert_eq!(create_body["data"]["handle"], json!("ep212-bot"));
    assert_eq!(create_body["data"]["displayName"], json!("EP212 Bot"));
    assert_eq!(create_body["data"]["configVersion"], json!(1));

    let list_request = Request::builder()
        .method("GET")
        .uri("/api/autopilots?limit=200")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_response = app.clone().oneshot(list_request).await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await?;
    let listed = list_body["data"].as_array().cloned().unwrap_or_default();
    assert!(!listed.is_empty());
    assert!(
        listed
            .iter()
            .any(|row| row["id"] == json!(autopilot_id.clone()))
    );

    let show_by_id_request = Request::builder()
        .method("GET")
        .uri(format!("/api/autopilots/{autopilot_id}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let show_by_id_response = app.clone().oneshot(show_by_id_request).await?;
    assert_eq!(show_by_id_response.status(), StatusCode::OK);
    let show_by_id_body = read_json(show_by_id_response).await?;
    assert_eq!(show_by_id_body["data"]["handle"], json!("ep212-bot"));

    let show_by_handle_request = Request::builder()
        .method("GET")
        .uri("/api/autopilots/ep212-bot")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let show_by_handle_response = app.clone().oneshot(show_by_handle_request).await?;
    assert_eq!(show_by_handle_response.status(), StatusCode::OK);
    let show_by_handle_body = read_json(show_by_handle_response).await?;
    assert_eq!(
        show_by_handle_body["data"]["id"],
        json!(autopilot_id.clone())
    );

    let update_request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"displayName":"EP212 Bot Updated","profile":{"ownerDisplayName":"Chris","personaSummary":"Pragmatic and concise","autopilotVoice":"calm and direct"},"policy":{"toolAllowlist":["openagents_api"],"toolDenylist":["lightning_l402_fetch"],"l402RequireApproval":true,"l402MaxSpendMsatsPerCall":100000,"l402AllowedHosts":["sats4ai.com"]}}"#,
            ))?;
    let update_response = app.clone().oneshot(update_request).await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await?;
    assert_eq!(
        update_body["data"]["displayName"],
        json!("EP212 Bot Updated")
    );
    assert_eq!(update_body["data"]["configVersion"], json!(2));
    assert_eq!(
        update_body["data"]["profile"]["ownerDisplayName"],
        json!("Chris")
    );
    assert_eq!(
        update_body["data"]["policy"]["toolAllowlist"][0],
        json!("openagents_api")
    );
    assert_eq!(
        update_body["data"]["policy"]["l402MaxSpendMsatsPerCall"],
        json!(100000)
    );

    Ok(())
}

#[tokio::test]
async fn autopilot_thread_routes_support_create_and_list() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "autopilot-threads@openagents.com").await?;

    let create_autopilot_request = Request::builder()
        .method("POST")
        .uri("/api/autopilots")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"handle":"thread-bot","displayName":"Thread Bot"}"#,
        ))?;
    let create_autopilot_response = app.clone().oneshot(create_autopilot_request).await?;
    assert_eq!(create_autopilot_response.status(), StatusCode::CREATED);
    let create_autopilot_body = read_json(create_autopilot_response).await?;
    let autopilot_id = create_autopilot_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let update_autopilot_request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"profile":{"ownerDisplayName":"Chris","personaSummary":"Pragmatic and concise","autopilotVoice":"calm and direct"},"policy":{"toolAllowlist":["openagents_api","lightning_l402_fetch"],"toolDenylist":["lightning_l402_fetch"],"l402RequireApproval":true,"l402AllowedHosts":["sats4ai.com"]}}"#,
            ))?;
    let update_autopilot_response = app.clone().oneshot(update_autopilot_request).await?;
    assert_eq!(update_autopilot_response.status(), StatusCode::OK);
    assert!(autopilot_id.starts_with("ap_"));

    let create_thread_request = Request::builder()
        .method("POST")
        .uri(format!("/api/autopilots/{autopilot_id}/threads"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"title":"Autopilot test thread"}"#))?;
    let create_thread_response = app.clone().oneshot(create_thread_request).await?;
    assert_eq!(create_thread_response.status(), StatusCode::CREATED);
    let create_thread_body = read_json(create_thread_response).await?;
    let thread_id = create_thread_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(thread_id.starts_with("thread_"));
    assert_eq!(
        create_thread_body["data"]["autopilotId"],
        json!(autopilot_id.clone())
    );
    assert_eq!(
        create_thread_body["data"]["title"],
        json!("Autopilot test thread")
    );

    let create_default_title_request = Request::builder()
        .method("POST")
        .uri(format!("/api/autopilots/{autopilot_id}/threads"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{}"#))?;
    let create_default_title_response = app.clone().oneshot(create_default_title_request).await?;
    assert_eq!(create_default_title_response.status(), StatusCode::CREATED);
    let create_default_title_body = read_json(create_default_title_response).await?;
    assert_eq!(
        create_default_title_body["data"]["title"],
        json!("New conversation")
    );

    let list_threads_request = Request::builder()
        .method("GET")
        .uri(format!("/api/autopilots/{autopilot_id}/threads?limit=200"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_threads_response = app.clone().oneshot(list_threads_request).await?;
    assert_eq!(list_threads_response.status(), StatusCode::OK);
    let list_threads_body = read_json(list_threads_response).await?;
    let listed_threads = list_threads_body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(listed_threads.len() >= 2);
    assert!(
        listed_threads.iter().all(|row| {
            row["autopilotId"] == json!(autopilot_id.clone()) && row["id"].is_string()
        })
    );
    assert!(
        listed_threads
            .iter()
            .any(|row| row["id"] == json!(thread_id.clone()))
    );

    Ok(())
}

#[tokio::test]
async fn autopilot_stream_route_bootstraps_codex_and_returns_ws_delivery() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "autopilot-stream@openagents.com").await?;

    let create_autopilot_request = Request::builder()
        .method("POST")
        .uri("/api/autopilots")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"handle":"stream-bot","displayName":"Stream Bot"}"#,
        ))?;
    let create_autopilot_response = app.clone().oneshot(create_autopilot_request).await?;
    assert_eq!(create_autopilot_response.status(), StatusCode::CREATED);
    let create_autopilot_body = read_json(create_autopilot_response).await?;
    let autopilot_id = create_autopilot_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let stream_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"messages":[{"id":"m1","role":"user","content":"hello from autopilot stream alias"}]}"#,
            ))?;
    let stream_response = app.clone().oneshot(stream_request).await?;
    assert_eq!(stream_response.status(), StatusCode::OK);
    let stream_body = read_json(stream_response).await?;
    let thread_id = stream_body["data"]["threadId"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(thread_id.starts_with("thread_"));
    assert_eq!(stream_body["data"]["accepted"], json!(true));
    assert_eq!(
        stream_body["data"]["autopilotId"],
        json!(autopilot_id.clone())
    );
    assert_eq!(
        stream_body["data"]["delivery"]["transport"],
        json!("khala_ws")
    );
    let delivery_topic = stream_body["data"]["delivery"]["topic"]
        .as_str()
        .unwrap_or_default();
    assert!(delivery_topic.ends_with(":worker_events"));
    assert_eq!(
        stream_body["data"]["control"]["method"],
        json!("turn/start")
    );
    assert_eq!(
        stream_body["data"]["response"]["thread_id"],
        json!(thread_id.clone())
    );
    let prompt_context = stream_body["data"]["promptContext"]
        .as_str()
        .unwrap_or_default();
    assert!(!prompt_context.is_empty());
    assert!(prompt_context.contains("autopilot_id="));
    assert!(prompt_context.contains("l402_require_approval=true"));
    assert_eq!(
        stream_body["data"]["toolPolicy"]["policyApplied"],
        json!(true)
    );
    let exposed_tools = stream_body["data"]["toolPolicy"]["exposedTools"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!exposed_tools.is_empty());
    let removed_by_denylist = stream_body["data"]["toolPolicy"]["removedByDenylist"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(removed_by_denylist.len() <= exposed_tools.len());
    assert!(
        stream_body["data"]["runtimeBinding"].is_null()
            || stream_body["data"]["runtimeBinding"].is_object()
    );

    let list_request = Request::builder()
        .method("GET")
        .uri(format!("/api/autopilots/{autopilot_id}/threads"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_response = app.clone().oneshot(list_request).await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await?;
    let listed = list_body["data"].as_array().cloned().unwrap_or_default();
    assert!(
        listed
            .iter()
            .any(|row| row["id"] == json!(thread_id.clone()))
    );

    let resume_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(
                r#"{{"conversationId":"{thread_id}","messages":[{{"id":"m2","role":"user","content":"continue this thread"}}]}}"#
            )))?;
    let resume_response = app.oneshot(resume_request).await?;
    assert_eq!(resume_response.status(), StatusCode::OK);
    let resume_body = read_json(resume_response).await?;
    assert_eq!(resume_body["data"]["threadId"], json!(thread_id));

    Ok(())
}

#[tokio::test]
async fn autopilot_routes_enforce_owner_boundary() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let owner_token = authenticate_token(app.clone(), "autopilot-owner-a@openagents.com").await?;
    let other_token = authenticate_token(app.clone(), "autopilot-owner-b@openagents.com").await?;

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/autopilots")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {owner_token}"))
        .body(Body::from(
            r#"{"handle":"owner-bot","displayName":"Owner Bot"}"#,
        ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    let autopilot_id = create_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let other_show_request = Request::builder()
        .method("GET")
        .uri(format!("/api/autopilots/{autopilot_id}"))
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::empty())?;
    let other_show_response = app.clone().oneshot(other_show_request).await?;
    assert_eq!(other_show_response.status(), StatusCode::NOT_FOUND);

    let other_update_request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/autopilots/{autopilot_id}"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::from(r#"{"displayName":"Hacked"}"#))?;
    let other_update_response = app.clone().oneshot(other_update_request).await?;
    assert_eq!(other_update_response.status(), StatusCode::NOT_FOUND);

    let other_show_handle_request = Request::builder()
        .method("GET")
        .uri("/api/autopilots/owner-bot")
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::empty())?;
    let other_show_handle_response = app.clone().oneshot(other_show_handle_request).await?;
    assert_eq!(other_show_handle_response.status(), StatusCode::NOT_FOUND);

    let other_threads_request = Request::builder()
        .method("GET")
        .uri(format!("/api/autopilots/{autopilot_id}/threads"))
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::empty())?;
    let other_threads_response = app.clone().oneshot(other_threads_request).await?;
    assert_eq!(other_threads_response.status(), StatusCode::NOT_FOUND);

    let other_create_thread_request = Request::builder()
        .method("POST")
        .uri(format!("/api/autopilots/{autopilot_id}/threads"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::from(r#"{"title":"intruder thread"}"#))?;
    let other_create_thread_response = app.clone().oneshot(other_create_thread_request).await?;
    assert_eq!(other_create_thread_response.status(), StatusCode::NOT_FOUND);

    let other_stream_request = Request::builder()
        .method("POST")
        .uri(format!("/api/autopilots/{autopilot_id}/stream"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::from(
            r#"{"messages":[{"id":"m1","role":"user","content":"intruder stream"}]}"#,
        ))?;
    let other_stream_response = app.oneshot(other_stream_request).await?;
    assert_eq!(other_stream_response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn autopilot_routes_enforce_validation_semantics() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "autopilot-validation@openagents.com").await?;

    let bad_create_request = Request::builder()
        .method("POST")
        .uri("/api/autopilots")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"handle":"invalid handle!"}"#))?;
    let bad_create_response = app.clone().oneshot(bad_create_request).await?;
    assert_eq!(
        bad_create_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let bad_create_body = read_json(bad_create_response).await?;
    assert_eq!(bad_create_body["error"]["code"], json!("invalid_request"));

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/autopilots")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"handle":"valid-bot"}"#))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    let autopilot_id = create_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let bad_update_request = Request::builder()
            .method("PATCH")
            .uri(format!("/api/autopilots/{autopilot_id}"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"status":"ACTIVE","profile":{"schemaVersion":0},"policy":{"l402MaxSpendMsatsPerCall":0}}"#,
            ))?;
    let bad_update_response = app.clone().oneshot(bad_update_request).await?;
    assert_eq!(
        bad_update_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let bad_update_body = read_json(bad_update_response).await?;
    assert_eq!(bad_update_body["error"]["code"], json!("invalid_request"));

    let oversized_title = "x".repeat(201);
    let bad_thread_request = Request::builder()
        .method("POST")
        .uri(format!("/api/autopilots/{autopilot_id}/threads"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(r#"{{"title":"{oversized_title}"}}"#)))?;
    let bad_thread_response = app.clone().oneshot(bad_thread_request).await?;
    assert_eq!(
        bad_thread_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let bad_thread_body = read_json(bad_thread_response).await?;
    assert_eq!(bad_thread_body["error"]["code"], json!("invalid_request"));

    let bad_stream_request = Request::builder()
        .method("POST")
        .uri(format!("/api/autopilots/{autopilot_id}/stream"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"messages":[{"id":"m1","role":"assistant","content":"missing user prompt"}]}"#,
        ))?;
    let bad_stream_response = app.clone().oneshot(bad_stream_request).await?;
    assert_eq!(
        bad_stream_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let bad_stream_body = read_json(bad_stream_response).await?;
    assert_eq!(bad_stream_body["error"]["code"], json!("invalid_request"));

    let missing_thread_request = Request::builder()
            .method("POST")
            .uri(format!("/api/autopilots/{autopilot_id}/stream"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"conversationId":"thread_missing","messages":[{"id":"m2","role":"user","content":"hello"}]}"#,
            ))?;
    let missing_thread_response = app.oneshot(missing_thread_request).await?;
    assert_eq!(missing_thread_response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn settings_profile_routes_support_read_update_delete() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "profile-user@openagents.com").await?;

    let show_request = Request::builder()
        .uri("/api/settings/profile")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let show_response = app.clone().oneshot(show_request).await?;
    assert_eq!(show_response.status(), StatusCode::OK);
    let show_body = read_json(show_response).await?;
    assert_eq!(show_body["data"]["email"], "profile-user@openagents.com");

    let update_request = Request::builder()
        .method("PATCH")
        .uri("/api/settings/profile")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"name":"Updated Name"}"#))?;
    let update_response = app.clone().oneshot(update_request).await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await?;
    assert_eq!(update_body["data"]["name"], "Updated Name");

    let wrong_delete_request = Request::builder()
        .method("DELETE")
        .uri("/api/settings/profile")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"email":"wrong@openagents.com"}"#))?;
    let wrong_delete_response = app.clone().oneshot(wrong_delete_request).await?;
    assert_eq!(
        wrong_delete_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let wrong_delete_body = read_json(wrong_delete_response).await?;
    assert_eq!(
        wrong_delete_body["message"],
        "Email confirmation does not match the authenticated user."
    );

    let delete_request = Request::builder()
        .method("DELETE")
        .uri("/api/settings/profile")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"email":"profile-user@openagents.com"}"#))?;
    let delete_response = app.clone().oneshot(delete_request).await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_body = read_json(delete_response).await?;
    assert_eq!(delete_body["data"]["deleted"], true);

    let show_after_delete_request = Request::builder()
        .uri("/api/settings/profile")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let show_after_delete_response = app.oneshot(show_after_delete_request).await?;
    assert_eq!(
        show_after_delete_response.status(),
        StatusCode::UNAUTHORIZED
    );

    Ok(())
}

#[tokio::test]
async fn settings_autopilot_route_supports_create_update_and_validation() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "autopilot-settings@openagents.com").await?;

    let update_request = Request::builder()
            .method("PATCH")
            .uri("/settings/autopilot")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"displayName":"Chris Autopilot","tagline":"Persistent and practical","ownerDisplayName":"Chris","personaSummary":"Keep it concise and engineering-minded.","autopilotVoice":"calm and direct","principlesText":"Prefer verification over guessing\nAsk before irreversible actions"}"#,
            ))?;
    let update_response = app.clone().oneshot(update_request).await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await?;
    assert_eq!(update_body["data"]["status"], "autopilot-updated");
    assert_eq!(
        update_body["data"]["autopilot"]["displayName"],
        "Chris Autopilot"
    );
    assert_eq!(
        update_body["data"]["autopilot"]["tagline"],
        "Persistent and practical"
    );
    assert_eq!(
        update_body["data"]["autopilot"]["profile"]["ownerDisplayName"],
        "Chris"
    );
    assert_eq!(
        update_body["data"]["autopilot"]["profile"]["personaSummary"],
        "Keep it concise and engineering-minded."
    );
    assert_eq!(
        update_body["data"]["autopilot"]["profile"]["autopilotVoice"],
        "calm and direct"
    );
    let principles = update_body["data"]["autopilot"]["profile"]["principles"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        principles
            .iter()
            .any(|value| value == "Prefer verification over guessing")
    );
    assert!(
        principles
            .iter()
            .any(|value| value == "Ask before irreversible actions")
    );
    assert!(
        update_body["data"]["autopilot"]["configVersion"]
            .as_u64()
            .unwrap_or_default()
            > 1
    );

    let too_long_display_name = "x".repeat(121);
    let invalid_request = Request::builder()
        .method("PATCH")
        .uri("/settings/autopilot")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{"displayName":"{too_long_display_name}"}}"#
        )))?;
    let invalid_response = app.oneshot(invalid_request).await?;
    assert_eq!(invalid_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let invalid_body = read_json(invalid_response).await?;
    assert_eq!(invalid_body["error"]["code"], json!("invalid_request"));
    assert_eq!(
        invalid_body["errors"]["displayName"],
        json!(["Value may not be greater than 120 characters."])
    );

    Ok(())
}

#[tokio::test]
async fn settings_integrations_resend_routes_support_lifecycle_and_validation() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "integrations-resend@openagents.com").await?;

    let upsert_request = Request::builder()
            .method("POST")
            .uri("/settings/integrations/resend")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"resend_api_key":"re_live_1234567890","sender_email":"bot@openagents.com","sender_name":"OpenAgents Bot"}"#,
            ))?;
    let upsert_response = app.clone().oneshot(upsert_request).await?;
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upsert_body = read_json(upsert_response).await?;
    assert_eq!(upsert_body["data"]["status"], "resend-connected");
    assert_eq!(upsert_body["data"]["action"], "secret_created");
    assert_eq!(upsert_body["data"]["integration"]["provider"], "resend");
    assert_eq!(upsert_body["data"]["integration"]["connected"], true);
    assert_eq!(upsert_body["data"]["integration"]["secretLast4"], "7890");

    let test_request = Request::builder()
        .method("POST")
        .uri("/settings/integrations/resend/test")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let test_response = app.clone().oneshot(test_request).await?;
    assert_eq!(test_response.status(), StatusCode::OK);
    let test_body = read_json(test_response).await?;
    assert_eq!(test_body["data"]["status"], "resend-test-queued");
    assert_eq!(test_body["data"]["action"], "test_requested");

    let disconnect_request = Request::builder()
        .method("DELETE")
        .uri("/settings/integrations/resend")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let disconnect_response = app.clone().oneshot(disconnect_request).await?;
    assert_eq!(disconnect_response.status(), StatusCode::OK);
    let disconnect_body = read_json(disconnect_response).await?;
    assert_eq!(disconnect_body["data"]["status"], "resend-disconnected");
    assert_eq!(disconnect_body["data"]["action"], "secret_revoked");
    assert_eq!(disconnect_body["data"]["integration"]["connected"], false);

    let test_after_disconnect_request = Request::builder()
        .method("POST")
        .uri("/settings/integrations/resend/test")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let test_after_disconnect_response = app.oneshot(test_after_disconnect_request).await?;
    assert_eq!(
        test_after_disconnect_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let test_after_disconnect_body = read_json(test_after_disconnect_response).await?;
    assert_eq!(
        test_after_disconnect_body["error"]["code"],
        "invalid_request"
    );
    assert_eq!(
        test_after_disconnect_body["message"],
        "Connect an active Resend key before running a test."
    );

    Ok(())
}

#[tokio::test]
async fn settings_integrations_google_routes_support_redirect_callback_and_disconnect() -> Result<()>
{
    let captured_bodies = Arc::new(Mutex::new(Vec::<String>::new()));
    let (token_stub_addr, token_stub_handle) =
        start_google_oauth_token_stub(captured_bodies.clone()).await?;

    let mut config = test_config(std::env::temp_dir());
    config.google_oauth_client_id = Some("google-client-id".to_string());
    config.google_oauth_client_secret = Some("google-client-secret".to_string());
    config.google_oauth_redirect_uri = Some("https://openagents.test/google/callback".to_string());
    config.google_oauth_token_url = format!("http://{token_stub_addr}/oauth2/token");
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "integrations-google@openagents.com").await?;

    let redirect_request = Request::builder()
        .method("GET")
        .uri("/settings/integrations/google/redirect")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let redirect_response = app.clone().oneshot(redirect_request).await?;
    assert_eq!(redirect_response.status(), StatusCode::SEE_OTHER);

    let location = redirect_response
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    assert!(location.starts_with("https://accounts.google.com/o/oauth2/v2/auth"));

    let redirect_url = reqwest::Url::parse(&location)?;
    let state = redirect_url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.to_string())
        .unwrap_or_default();
    assert!(!state.is_empty());

    let callback_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/settings/integrations/google/callback?state={state}&code=google-auth-code-123"
        ))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let callback_response = app.clone().oneshot(callback_request).await?;
    assert_eq!(callback_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        callback_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/settings/integrations?status=google-connected")
    );

    let captured = captured_bodies.lock().await.clone();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].contains("grant_type=authorization_code"));
    assert!(captured[0].contains("code=google-auth-code-123"));

    let disconnect_request = Request::builder()
        .method("DELETE")
        .uri("/settings/integrations/google")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let disconnect_response = app.clone().oneshot(disconnect_request).await?;
    assert_eq!(disconnect_response.status(), StatusCode::OK);
    let disconnect_body = read_json(disconnect_response).await?;
    assert_eq!(disconnect_body["data"]["status"], "google-disconnected");
    assert_eq!(disconnect_body["data"]["action"], "secret_revoked");
    assert_eq!(disconnect_body["data"]["integration"]["connected"], false);

    let bad_state_request = Request::builder()
        .method("GET")
        .uri("/settings/integrations/google/callback?state=bad-state&code=another")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let bad_state_response = app.oneshot(bad_state_request).await?;
    assert_eq!(
        bad_state_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let bad_state_body = read_json(bad_state_response).await?;
    assert_eq!(bad_state_body["error"]["code"], "invalid_request");
    assert_eq!(
        bad_state_body["message"],
        "OAuth state mismatch. Please retry connecting Google."
    );

    token_stub_handle.abort();
    Ok(())
}

#[tokio::test]
async fn inbox_routes_fetch_gmail_threads_and_support_actions() -> Result<()> {
    let token_calls = Arc::new(Mutex::new(Vec::<String>::new()));
    let send_calls = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (gmail_stub_addr, gmail_stub_handle) =
        start_gmail_inbox_stub(token_calls.clone(), send_calls.clone()).await?;

    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.google_oauth_client_id = Some("google-client-id".to_string());
    config.google_oauth_client_secret = Some("google-client-secret".to_string());
    config.google_oauth_token_url = format!("http://{gmail_stub_addr}/oauth2/token");
    config.google_gmail_api_base_url = format!("http://{gmail_stub_addr}");
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let auth = super::AuthService::from_config(&config);
    let verify = auth
        .local_test_sign_in(
            "inbox-google@openagents.com".to_string(),
            None,
            Some("autopilot-ios"),
            None,
        )
        .await?;
    let token = verify.access_token.clone();
    let user_id = verify.user.id.clone();

    let store = DomainStore::from_config(&config);
    store
            .upsert_google_integration(UpsertGoogleIntegrationInput {
                user_id: user_id.clone(),
                refresh_token: Some("refresh_token_1234567890".to_string()),
                access_token: Some("stale_access_token".to_string()),
                scope: Some(
                    "https://www.googleapis.com/auth/gmail.readonly https://www.googleapis.com/auth/gmail.send"
                        .to_string(),
                ),
                token_type: Some("Bearer".to_string()),
                expires_at: Some(Utc::now() - Duration::minutes(5)),
            })
            .await?;
    let app = build_router(config.clone());

    let list_request = Request::builder()
        .method("GET")
        .uri("/api/inbox/threads")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_response = app.clone().oneshot(list_request).await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await?;
    assert_eq!(
        list_body["data"]["snapshot"]["threads"][0]["id"],
        "thread_1"
    );

    let detail_request = Request::builder()
        .method("GET")
        .uri("/api/inbox/threads/thread_1")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let detail_response = app.clone().oneshot(detail_request).await?;
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = read_json(detail_response).await?;
    assert_eq!(detail_body["data"]["thread"]["id"], "thread_1");

    let approve_request = Request::builder()
        .method("POST")
        .uri("/api/inbox/threads/thread_1/draft/approve")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"detail":"approved in test"}"#))?;
    let approve_response = app.clone().oneshot(approve_request).await?;
    assert_eq!(approve_response.status(), StatusCode::OK);
    let approve_body = read_json(approve_response).await?;
    assert_eq!(
        approve_body["data"]["snapshot"]["threads"][0]["pending_approval"],
        false
    );

    let reject_request = Request::builder()
        .method("POST")
        .uri("/api/inbox/threads/thread_1/draft/reject")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"detail":"reject in test"}"#))?;
    let reject_response = app.clone().oneshot(reject_request).await?;
    assert_eq!(reject_response.status(), StatusCode::OK);
    let reject_body = read_json(reject_response).await?;
    assert_eq!(
        reject_body["data"]["snapshot"]["threads"][0]["pending_approval"],
        true
    );

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/inbox/threads/thread_1/reply/send")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"body":"Thanks for the update. Tuesday works for us."}"#,
        ))?;
    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::OK);
    let send_body = read_json(send_response).await?;
    assert_eq!(send_body["data"]["status"], "sent");
    assert_eq!(send_body["data"]["message_id"], "gmail_msg_sent_1");

    let refresh_calls = token_calls.lock().await.clone();
    assert!(!refresh_calls.is_empty());
    assert!(refresh_calls[0].contains("grant_type=refresh_token"));

    let sends = send_calls.lock().await.clone();
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0]["threadId"], "thread_1");

    gmail_stub_handle.abort();
    Ok(())
}

#[tokio::test]
async fn inbox_threads_fail_when_refresh_token_is_missing() -> Result<()> {
    let static_dir = tempdir()?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.google_oauth_client_id = Some("google-client-id".to_string());
    config.google_oauth_client_secret = Some("google-client-secret".to_string());
    config.google_oauth_token_url = "http://127.0.0.1:9/oauth2/token".to_string();
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let auth = super::AuthService::from_config(&config);
    let verify = auth
        .local_test_sign_in(
            "inbox-missing-refresh@openagents.com".to_string(),
            None,
            Some("autopilot-ios"),
            None,
        )
        .await?;
    let token = verify.access_token.clone();
    let user_id = verify.user.id.clone();

    let store = DomainStore::from_config(&config);
    store
        .upsert_google_integration(UpsertGoogleIntegrationInput {
            user_id,
            refresh_token: Some("   ".to_string()),
            access_token: Some("stale_access_token".to_string()),
            scope: Some("https://www.googleapis.com/auth/gmail.readonly".to_string()),
            token_type: Some("Bearer".to_string()),
            expires_at: Some(Utc::now() - Duration::minutes(5)),
        })
        .await?;
    let app = build_router(config.clone());

    let list_request = Request::builder()
        .method("GET")
        .uri("/api/inbox/threads")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_response = app.oneshot(list_request).await?;
    assert_eq!(list_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = read_json(list_response).await?;
    assert_eq!(body["error"]["code"], "invalid_request");
    assert_eq!(
        body["message"],
        "Google refresh token is missing. Reconnect Google integration."
    );

    Ok(())
}

#[tokio::test]
async fn personal_access_token_routes_support_current_and_bulk_revocation() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let session_token = authenticate_token(app.clone(), "token-lifecycle@openagents.com").await?;

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/tokens")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {session_token}"))
        .body(Body::from(
            r#"{"name":"api-cli","abilities":["chat:read","chat:write"]}"#,
        ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    let pat_token = create_body["data"]["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(pat_token.starts_with("oa_pat_"));

    let list_by_pat_request = Request::builder()
        .uri("/api/tokens")
        .header("authorization", format!("Bearer {pat_token}"))
        .body(Body::empty())?;
    let list_by_pat_response = app.clone().oneshot(list_by_pat_request).await?;
    assert_eq!(list_by_pat_response.status(), StatusCode::OK);
    let list_by_pat_body = read_json(list_by_pat_response).await?;
    let tokens = list_by_pat_body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(tokens.iter().any(|token| token["isCurrent"] == true));

    let delete_current_request = Request::builder()
        .method("DELETE")
        .uri("/api/tokens/current")
        .header("authorization", format!("Bearer {pat_token}"))
        .body(Body::empty())?;
    let delete_current_response = app.clone().oneshot(delete_current_request).await?;
    assert_eq!(delete_current_response.status(), StatusCode::OK);
    let delete_current_body = read_json(delete_current_response).await?;
    assert_eq!(delete_current_body["data"]["deleted"], true);

    for name in ["bulk-a", "bulk-b"] {
        let request = Request::builder()
            .method("POST")
            .uri("/api/tokens")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {session_token}"))
            .body(Body::from(format!(r#"{{"name":"{name}"}}"#)))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let delete_all_request = Request::builder()
        .method("DELETE")
        .uri("/api/tokens")
        .header("authorization", format!("Bearer {session_token}"))
        .body(Body::empty())?;
    let delete_all_response = app.clone().oneshot(delete_all_request).await?;
    assert_eq!(delete_all_response.status(), StatusCode::OK);
    let delete_all_body = read_json(delete_all_response).await?;
    assert_eq!(delete_all_body["data"]["deletedCount"], 2);

    let final_list_request = Request::builder()
        .uri("/api/tokens")
        .header("authorization", format!("Bearer {session_token}"))
        .body(Body::empty())?;
    let final_list_response = app.oneshot(final_list_request).await?;
    assert_eq!(final_list_response.status(), StatusCode::OK);
    let final_list_body = read_json(final_list_response).await?;
    assert_eq!(final_list_body["data"].as_array().map(Vec::len), Some(0));

    Ok(())
}

#[tokio::test]
async fn khala_token_route_mints_and_surfaces_configuration_errors() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "khala-route@openagents.com").await?;

    let mint_request = Request::builder()
            .method("POST")
            .uri("/api/khala/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"scope":["codex:read","codex:write"],"workspace_id":"workspace_42","role":"admin"}"#,
            ))?;
    let mint_response = app.clone().oneshot(mint_request).await?;
    assert_eq!(mint_response.status(), StatusCode::OK);
    let mint_body = read_json(mint_response).await?;
    assert_eq!(mint_body["data"]["token_type"], "Bearer");
    assert_eq!(mint_body["data"]["issuer"], "https://openagents.test");
    assert_eq!(mint_body["data"]["audience"], "openagents-khala-test");
    assert_eq!(mint_body["data"]["claims_version"], "oa_khala_claims_v1");

    let mut config = test_config(std::env::temp_dir());
    config.khala_token_signing_key = None;
    let misconfigured_app = build_router(config);
    let misconfigured_token = authenticate_token(
        misconfigured_app.clone(),
        "khala-misconfigured@openagents.com",
    )
    .await?;
    let unavailable_request = Request::builder()
        .method("POST")
        .uri("/api/khala/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {misconfigured_token}"))
        .body(Body::from("{}"))?;
    let unavailable_response = misconfigured_app.oneshot(unavailable_request).await?;
    assert_eq!(
        unavailable_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let unavailable_body = read_json(unavailable_response).await?;
    assert_eq!(unavailable_body["error"]["code"], "khala_token_unavailable");

    Ok(())
}

#[tokio::test]
async fn sync_token_route_accepts_personal_access_token_auth() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let session_token = authenticate_token(app.clone(), "sync-pat@openagents.com").await?;

    let create_pat_request = Request::builder()
        .method("POST")
        .uri("/api/tokens")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {session_token}"))
        .body(Body::from(r#"{"name":"sync-pat"}"#))?;
    let create_pat_response = app.clone().oneshot(create_pat_request).await?;
    assert_eq!(create_pat_response.status(), StatusCode::CREATED);
    let create_pat_body = read_json(create_pat_response).await?;
    let pat_token = create_pat_body["data"]["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let sync_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {pat_token}"))
        .body(Body::from(
            r#"{"scopes":["runtime.codex_worker_events"],"device_id":"mobile:custom"}"#,
        ))?;
    let sync_response = app.oneshot(sync_request).await?;
    assert_eq!(sync_response.status(), StatusCode::OK);
    let sync_body = read_json(sync_response).await?;
    assert_eq!(sync_body["data"]["token_type"], "Bearer");
    assert!(
        sync_body["data"]["session_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("pat:")
    );

    Ok(())
}

#[tokio::test]
async fn sync_token_v1_alias_matches_primary_contract_shape() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "sync-v1-alias@openagents.com").await?;
    let payload = r#"{"scopes":["runtime.codex_worker_events"]}"#;

    let primary_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(payload))?;
    let primary_response = app.clone().oneshot(primary_request).await?;
    assert_eq!(primary_response.status(), StatusCode::OK);
    let primary_body = read_json(primary_response).await?;

    let alias_request = Request::builder()
        .method("POST")
        .uri("/api/v1/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(payload))?;
    let alias_response = app.oneshot(alias_request).await?;
    assert_eq!(alias_response.status(), StatusCode::OK);
    let alias_body = read_json(alias_response).await?;

    for key in [
        "token_type",
        "issuer",
        "audience",
        "claims_version",
        "session_id",
        "device_id",
        "org_id",
    ] {
        assert_eq!(alias_body["data"][key], primary_body["data"][key]);
    }
    assert_eq!(alias_body["data"]["scopes"], primary_body["data"]["scopes"]);
    assert_eq!(
        alias_body["data"]["granted_topics"],
        primary_body["data"]["granted_topics"]
    );

    Ok(())
}

#[tokio::test]
async fn sync_token_v1_alias_uses_compatibility_handshake_controls() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let missing_handshake_request = Request::builder()
        .method("POST")
        .uri("/api/v1/sync/token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"scopes":["runtime.codex_worker_events"]}"#))?;
    let missing_handshake_response = app.clone().oneshot(missing_handshake_request).await?;
    assert_eq!(
        missing_handshake_response.status(),
        StatusCode::UPGRADE_REQUIRED
    );
    let missing_body = read_json(missing_handshake_response).await?;
    assert_eq!(missing_body["error"]["code"], json!("invalid_client_build"));

    let handshake_without_auth = Request::builder()
        .method("POST")
        .uri("/api/v1/sync/token")
        .header("content-type", "application/json")
        .header("x-oa-client-build-id", "20260221T130000Z")
        .header("x-oa-protocol-version", "openagents.control.v1")
        .header("x-oa-schema-version", "1")
        .body(Body::from(r#"{"scopes":["runtime.codex_worker_events"]}"#))?;
    let handshake_without_auth_response = app.clone().oneshot(handshake_without_auth).await?;
    assert_eq!(
        handshake_without_auth_response.status(),
        StatusCode::UNAUTHORIZED
    );

    let primary_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"scopes":["runtime.codex_worker_events"]}"#))?;
    let primary_response = app.oneshot(primary_request).await?;
    assert_eq!(primary_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn sync_token_mint_enforces_scope_and_org_policy() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"sync@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let memberships_request = Request::builder()
        .uri("/api/orgs/memberships")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let memberships_response = app.clone().oneshot(memberships_request).await?;
    let memberships_body = read_json(memberships_response).await?;
    let memberships = memberships_body["data"]["memberships"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let personal_org_id = memberships
        .iter()
        .find(|membership| membership["default_org"].as_bool().unwrap_or(false))
        .and_then(|membership| membership["org_id"].as_str())
        .unwrap_or_default()
        .to_string();

    let set_openagents_org = Request::builder()
        .method("POST")
        .uri("/api/orgs/active")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"org_id":"org:openagents"}"#))?;
    let set_openagents_response = app.clone().oneshot(set_openagents_org).await?;
    assert_eq!(set_openagents_response.status(), StatusCode::OK);

    let denied_by_policy_request = Request::builder()
            .method("POST")
            .uri("/api/sync/token")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"scopes":["runtime.codex_worker_events"],"topics":["org:openagents:worker_events"]}"#,
            ))?;
    let denied_by_policy_response = app.clone().oneshot(denied_by_policy_request).await?;
    assert_eq!(denied_by_policy_response.status(), StatusCode::FORBIDDEN);

    let set_personal_org = Request::builder()
        .method("POST")
        .uri("/api/orgs/active")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(r#"{{"org_id":"{personal_org_id}"}}"#)))?;
    let set_personal_response = app.clone().oneshot(set_personal_org).await?;
    assert_eq!(set_personal_response.status(), StatusCode::OK);

    let invalid_scope_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"scopes":["runtime.unknown_scope"]}"#))?;
    let invalid_scope_response = app.clone().oneshot(invalid_scope_request).await?;
    assert_eq!(
        invalid_scope_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let invalid_scope_body = read_json(invalid_scope_response).await?;
    assert_eq!(invalid_scope_body["error"]["code"], "invalid_scope");

    let mismatched_device_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"scopes":["runtime.codex_worker_events"],"device_id":"mobile:other-device"}"#,
        ))?;
    let mismatched_device_response = app.clone().oneshot(mismatched_device_request).await?;
    assert_eq!(mismatched_device_response.status(), StatusCode::FORBIDDEN);

    let unsupported_topic_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"scopes":["runtime.codex_worker_events"],"topics":["org:openagents:unknown"]}"#,
        ))?;
    let unsupported_topic_response = app.clone().oneshot(unsupported_topic_request).await?;
    assert_eq!(
        unsupported_topic_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let unsupported_topic_body = read_json(unsupported_topic_response).await?;
    assert_eq!(unsupported_topic_body["error"]["code"], "invalid_request");

    let success_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"scopes":["runtime.codex_worker_events","runtime.run_summaries"]}"#,
        ))?;
    let success_response = app.oneshot(success_request).await?;
    assert_eq!(success_response.status(), StatusCode::OK);
    let success_body = read_json(success_response).await?;
    assert_eq!(success_body["data"]["token_type"], "Bearer");
    assert_eq!(success_body["data"]["issuer"], "https://openagents.test");
    assert_eq!(success_body["data"]["claims_version"], "oa_sync_claims_v1");

    Ok(())
}

#[tokio::test]
async fn sync_token_requires_active_non_revoked_session() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"sync-revoke@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let logout_request = Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let logout_response = app.clone().oneshot(logout_request).await?;
    assert_eq!(logout_response.status(), StatusCode::OK);

    let sync_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"scopes":["runtime.codex_worker_events"]}"#))?;
    let sync_response = app.oneshot(sync_request).await?;
    assert_eq!(sync_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn thread_message_command_requires_authentication() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let request = Request::builder()
        .method("POST")
        .uri("/api/runtime/threads/thread-1/messages")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"text":"hello"}"#))?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn thread_message_command_accepts_authenticated_message() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"thread-command@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let command_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/threads/thread-42/messages")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("{\"text\":\"Who are you?\"}"))?;
    let command_response = app.oneshot(command_request).await?;
    assert_eq!(command_response.status(), StatusCode::OK);
    let command_body = read_json(command_response).await?;
    assert_eq!(command_body["data"]["accepted"], true);
    assert_eq!(command_body["data"]["message"]["thread_id"], "thread-42");
    assert_eq!(command_body["data"]["message"]["text"], "Who are you?");
    assert!(
        command_body["data"]["message"]["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("msg_")
    );

    Ok(())
}

#[tokio::test]
async fn runtime_thread_read_paths_return_projected_threads_and_messages() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "thread-read@openagents.com").await?;

    for thread_id in ["thread-42", "thread-42", "thread-99"] {
        let request = Request::builder()
            .method("POST")
            .uri(format!("/api/runtime/threads/{thread_id}/messages"))
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(r#"{"text":"hello"}"#))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK);
    }

    let list_threads_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/threads")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_threads_response = app.clone().oneshot(list_threads_request).await?;
    assert_eq!(list_threads_response.status(), StatusCode::OK);
    let list_threads_body = read_json(list_threads_response).await?;
    assert_eq!(
        list_threads_body["data"]["threads"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    let list_messages_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/threads/thread-42/messages")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_messages_response = app.oneshot(list_messages_request).await?;
    assert_eq!(list_messages_response.status(), StatusCode::OK);
    let list_messages_body = read_json(list_messages_response).await?;
    assert_eq!(
        list_messages_body["data"]["messages"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );

    Ok(())
}

#[tokio::test]
async fn runtime_thread_message_read_path_enforces_owner_boundary() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let owner_token = authenticate_token(app.clone(), "thread-owner@openagents.com").await?;
    let other_token = authenticate_token(app.clone(), "thread-other@openagents.com").await?;

    let append_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/threads/thread-private/messages")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {owner_token}"))
        .body(Body::from(r#"{"text":"private"}"#))?;
    let append_response = app.clone().oneshot(append_request).await?;
    assert_eq!(append_response.status(), StatusCode::OK);

    let read_other_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/threads/thread-private/messages")
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::empty())?;
    let read_other_response = app.oneshot(read_other_request).await?;
    assert_eq!(read_other_response.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn legacy_guest_session_route_requires_authenticated_codex_account() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));

    let request = Request::builder()
        .method("GET")
        .uri("/api/chat/guest-session?conversationId=g-1234567890abcdef1234567890abcdef")
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], json!("unauthorized"));
    let message = body["message"].as_str().unwrap_or_default();
    assert!(
        matches!(
            message,
            "Codex chat requires an authenticated ChatGPT account." | "Unauthenticated."
        ),
        "unexpected unauthorized message: {message}"
    );

    Ok(())
}

#[tokio::test]
async fn legacy_guest_session_route_returns_retirement_contract_for_authenticated_user()
-> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "legacy-guest-session@openagents.com").await?;

    let request = Request::builder()
        .method("GET")
        .uri("/api/chat/guest-session?conversationId=g-1234567890abcdef1234567890abcdef")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let response = app.clone().oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::GONE);
    assert!(response.headers().get("x-oa-legacy-chat-retired").is_none());
    assert!(
        response
            .headers()
            .get("x-oa-legacy-chat-canonical")
            .is_none()
    );
    assert_eq!(
        response
            .headers()
            .get("x-oa-chat-auth-policy")
            .and_then(|value| value.to_str().ok()),
        Some("codex-auth-required")
    );
    let body = read_json(response).await?;
    assert_eq!(body["data"]["retired"], json!(true));
    assert_eq!(body["data"]["status"], json!("codex_auth_required"));
    assert_eq!(body["data"]["canonical"], json!("/api/runtime/threads"));

    Ok(())
}

#[tokio::test]
async fn legacy_chats_aliases_map_to_codex_threads() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "legacy-chats@openagents.com").await?;

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/chats")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"title":"Migration Chat"}"#))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    assert!(
        create_response
            .headers()
            .get("x-oa-legacy-chat-retired")
            .is_none()
    );
    assert!(
        create_response
            .headers()
            .get("x-oa-legacy-chat-canonical")
            .is_none()
    );
    let create_body = read_json(create_response).await?;
    let conversation_id = create_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(conversation_id.starts_with("thread_"));

    let runtime_threads_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/threads")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let runtime_threads_response = app.clone().oneshot(runtime_threads_request).await?;
    assert_eq!(runtime_threads_response.status(), StatusCode::OK);
    let runtime_threads_body = read_json(runtime_threads_response).await?;
    let contains_thread = runtime_threads_body["data"]["threads"]
        .as_array()
        .map(|threads| {
            threads
                .iter()
                .any(|thread| thread["thread_id"] == json!(conversation_id.clone()))
        })
        .unwrap_or(false);
    assert!(contains_thread);

    let send_message_request = Request::builder()
        .method("POST")
        .uri(format!("/api/runtime/threads/{conversation_id}/messages"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"text":"legacy bridge message"}"#))?;
    let send_message_response = app.clone().oneshot(send_message_request).await?;
    assert_eq!(send_message_response.status(), StatusCode::OK);

    let show_request = Request::builder()
        .method("GET")
        .uri(format!("/api/chats/{conversation_id}"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let show_response = app.clone().oneshot(show_request).await?;
    assert_eq!(show_response.status(), StatusCode::OK);
    let show_body = read_json(show_response).await?;
    assert_eq!(
        show_body["data"]["conversation"]["id"],
        json!(conversation_id.clone())
    );
    assert_eq!(
        show_body["data"]["messages"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default(),
        1
    );
    assert_eq!(
        show_body["data"]["messages"][0]["content"],
        json!("legacy bridge message")
    );
    assert_eq!(
        show_body["data"]["runs"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default(),
        0
    );

    let runs_request = Request::builder()
        .method("GET")
        .uri(format!("/api/chats/{conversation_id}/runs"))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let runs_response = app.clone().oneshot(runs_request).await?;
    assert_eq!(runs_response.status(), StatusCode::OK);
    let runs_body = read_json(runs_response).await?;
    assert_eq!(
        runs_body["data"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default(),
        0
    );

    let events_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/chats/{conversation_id}/runs/run_legacy/events"
        ))
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let events_response = app.oneshot(events_request).await?;
    assert_eq!(events_response.status(), StatusCode::OK);
    let events_body = read_json(events_response).await?;
    assert_eq!(events_body["data"]["run"]["status"], json!("retired"));
    assert_eq!(
        events_body["data"]["events"]
            .as_array()
            .map(Vec::len)
            .unwrap_or_default(),
        0
    );

    Ok(())
}

#[tokio::test]
async fn legacy_chat_show_rejects_cross_user_access() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let owner_token = authenticate_token(app.clone(), "legacy-owner@openagents.com").await?;
    let other_token = authenticate_token(app.clone(), "legacy-other@openagents.com").await?;

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/chats")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {owner_token}"))
        .body(Body::from(r#"{"title":"Owner Chat"}"#))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    let conversation_id = create_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!conversation_id.is_empty());

    let other_show_request = Request::builder()
        .method("GET")
        .uri(format!("/api/chats/{conversation_id}"))
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::empty())?;
    let other_show_response = app.oneshot(other_show_request).await?;
    assert_eq!(other_show_response.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn legacy_chat_stream_alias_bridges_to_codex_control_request() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "legacy-stream@openagents.com").await?;

    let stream_request = Request::builder()
        .method("POST")
        .uri("/api/chat/stream")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"id":"thread-stream-alias","messages":[{"role":"user","content":"bridge hello"}]}"#,
        ))?;
    let stream_response = app.clone().oneshot(stream_request).await?;
    assert_eq!(stream_response.status(), StatusCode::OK);
    assert_eq!(
        stream_response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream; charset=utf-8")
    );
    assert_eq!(
        stream_response
            .headers()
            .get("x-vercel-ai-ui-message-stream")
            .and_then(|value| value.to_str().ok()),
        Some("v1")
    );
    assert!(
        stream_response
            .headers()
            .get("x-oa-legacy-chat-retired")
            .is_none()
    );
    assert!(
        stream_response
            .headers()
            .get("x-oa-legacy-chat-canonical")
            .is_none()
    );
    assert!(
        stream_response
            .headers()
            .get("x-oa-legacy-chat-stream-protocol")
            .is_none()
    );
    let stream_body = read_text(stream_response).await?;
    assert!(stream_body.contains("\"type\":\"start\""));
    assert!(stream_body.contains("\"type\":\"start-step\""));
    assert!(stream_body.contains("\"threadId\":\"thread-stream-alias\""));
    assert!(stream_body.contains("\"type\":\"finish-step\""));
    assert!(stream_body.contains("\"type\":\"finish\""));
    assert!(stream_body.ends_with("data: [DONE]\n\n"));

    let messages_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/threads/thread-stream-alias/messages")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let messages_response = app.oneshot(messages_request).await?;
    assert_eq!(messages_response.status(), StatusCode::OK);
    let messages_body = read_json(messages_response).await?;
    assert_eq!(
        messages_body["data"]["messages"][0]["text"],
        json!("bridge hello")
    );

    Ok(())
}

#[tokio::test]
async fn legacy_chats_stream_alias_uses_path_thread_id_and_accepts_structured_content() -> Result<()>
{
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "legacy-stream-path@openagents.com").await?;

    let stream_request = Request::builder()
        .method("POST")
        .uri("/api/chats/thread-stream-path/stream")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"messages":[{"role":"user","content":[{"type":"text","text":"path bridge"}]}]}"#,
        ))?;
    let stream_response = app.clone().oneshot(stream_request).await?;
    assert_eq!(stream_response.status(), StatusCode::OK);
    assert_eq!(
        stream_response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream; charset=utf-8")
    );
    let stream_body = read_text(stream_response).await?;
    assert!(stream_body.contains("\"threadId\":\"thread-stream-path\""));
    assert!(stream_body.ends_with("data: [DONE]\n\n"));

    let messages_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/threads/thread-stream-path/messages")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let messages_response = app.oneshot(messages_request).await?;
    assert_eq!(messages_response.status(), StatusCode::OK);
    let messages_body = read_json(messages_response).await?;
    assert_eq!(
        messages_body["data"]["messages"][0]["text"],
        json!("path bridge")
    );

    Ok(())
}

#[tokio::test]
async fn legacy_chat_stream_alias_rejects_payload_without_user_text() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "legacy-stream-bad@openagents.com").await?;

    let stream_request = Request::builder()
        .method("POST")
        .uri("/api/chat/stream")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"messages":[{"role":"assistant","content":"no user message"}]}"#,
        ))?;
    let stream_response = app.oneshot(stream_request).await?;
    assert_eq!(stream_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let stream_body = read_json(stream_response).await?;
    assert_eq!(stream_body["error"]["code"], json!("invalid_request"));

    Ok(())
}

#[tokio::test]
async fn legacy_chat_stream_edge_malformed_payloads_reject_deterministically() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));
    let token =
        authenticate_token(app.clone(), "legacy-stream-edge-malformed@openagents.com").await?;

    let malformed_json_request = legacy_stream_request(
        "/api/chat/stream",
        Some(&token),
        r#"{"messages":[{"role":"user","content":"oops"}"#,
    )?;
    let malformed_json_response = app.clone().oneshot(malformed_json_request).await?;
    assert_eq!(malformed_json_response.status(), StatusCode::BAD_REQUEST);

    let missing_text_request = legacy_stream_request(
        "/api/chat/stream",
        Some(&token),
        r#"{"messages":[{"role":"assistant","content":"no user payload"}]}"#,
    )?;
    let missing_text_response = app.oneshot(missing_text_request).await?;
    assert_eq!(
        missing_text_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let missing_text_body = read_json(missing_text_response).await?;
    assert_eq!(missing_text_body["error"]["code"], json!("invalid_request"));

    Ok(())
}

#[tokio::test]
async fn legacy_chat_stream_edge_guest_auth_gating_requires_bearer_token() -> Result<()> {
    let static_dir = tempdir()?;
    let app = build_router(compat_enforced_config(static_dir.path().to_path_buf()));

    let unauthenticated_request = legacy_stream_request(
        "/api/chat/stream",
        None,
        r#"{"messages":[{"role":"user","content":"missing auth"}]}"#,
    )?;
    let unauthenticated_response = app.clone().oneshot(unauthenticated_request).await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);
    let unauthenticated_body = read_json(unauthenticated_response).await?;
    assert_eq!(unauthenticated_body["error"]["code"], json!("unauthorized"));

    let token = authenticate_token(app.clone(), "legacy-stream-edge-auth@openagents.com").await?;
    let authenticated_request = legacy_stream_request(
        "/api/chat/stream",
        Some(&token),
        r#"{"messages":[{"role":"user","content":"authorized"}]}"#,
    )?;
    let authenticated_response = app.oneshot(authenticated_request).await?;
    assert_eq!(authenticated_response.status(), StatusCode::OK);
    let wire = read_text(authenticated_response).await?;
    assert_eq!(
        sse_event_types(&wire),
        vec!["start", "start-step", "finish-step", "finish"]
    );
    assert_eq!(sse_done_count(&wire), 1);

    Ok(())
}

#[tokio::test]
async fn legacy_chat_stream_edge_tool_event_ordering_matches_legacy_contract() -> Result<()> {
    let stream = super::vercel_sse_adapter::translate_codex_events(&[
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "thread/started".to_string(),
            params: json!({"thread_id":"thread_edge_tools"}),
        },
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "turn/started".to_string(),
            params: json!({
                "thread_id":"thread_edge_tools",
                "turn_id":"turn_edge_tools",
                "model":"gpt-5.2-codex"
            }),
        },
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "item/started".to_string(),
            params: json!({
                "item_id":"tool_edge_1",
                "item_kind":"mcp_tool_call",
                "item":{"tool_name":"openagents_api","arguments":{"q":"status"}}
            }),
        },
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "item/toolOutput/delta".to_string(),
            params: json!({"item_id":"tool_edge_1","delta":"{\"ok\":true}"}),
        },
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "item/completed".to_string(),
            params: json!({
                "item_id":"tool_edge_1",
                "item_kind":"mcp_tool_call",
                "item_status":"completed"
            }),
        },
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "turn/completed".to_string(),
            params: json!({"turn_id":"turn_edge_tools","status":"completed"}),
        },
    ])?;

    assert_eq!(
        sse_event_types(&stream.wire),
        vec![
            "start",
            "start-step",
            "tool-input",
            "tool-output",
            "tool-output",
            "finish-step",
            "finish"
        ]
    );
    assert_eq!(sse_done_count(&stream.wire), 1);

    Ok(())
}

#[tokio::test]
async fn legacy_chat_stream_edge_terminal_error_sequence_is_stable() -> Result<()> {
    let stream = super::vercel_sse_adapter::translate_codex_events(&[
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "thread/started".to_string(),
            params: json!({"thread_id":"thread_edge_error"}),
        },
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "turn/started".to_string(),
            params: json!({
                "thread_id":"thread_edge_error",
                "turn_id":"turn_edge_error",
                "model":"gpt-5.2-codex"
            }),
        },
        super::vercel_sse_adapter::CodexCompatibilityEvent {
            method: "turn/failed".to_string(),
            params: json!({"message":"forced failure","will_retry":false}),
        },
    ])?;

    let event_types = sse_event_types(&stream.wire);
    assert_eq!(event_types, vec!["start", "start-step", "error", "finish"]);
    assert_eq!(
        stream.events[2].get("code").and_then(Value::as_str),
        Some("turn_failed")
    );
    assert_eq!(
        stream.events[3].get("status").and_then(Value::as_str),
        Some("error")
    );
    assert_eq!(sse_done_count(&stream.wire), 1);

    Ok(())
}

#[tokio::test]
async fn runtime_codex_control_request_accepts_turn_start_and_persists_message() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "codex-control@openagents.com").await?;

    let control_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"request":{"request_id":"req_turn_1","method":"turn/start","params":{"thread_id":"thread-control-1","text":"continue"}}}"#,
            ))?;
    let control_response = app.clone().oneshot(control_request).await?;
    assert_eq!(control_response.status(), StatusCode::OK);
    let control_body = read_json(control_response).await?;
    assert_eq!(control_body["data"]["method"], "turn/start");
    assert_eq!(control_body["data"]["request_id"], "req_turn_1");
    assert_eq!(control_body["data"]["idempotent_replay"], false);
    assert_eq!(
        control_body["data"]["response"]["thread_id"],
        "thread-control-1"
    );
    assert!(
        control_body["data"]["response"]["turn"]["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("turn_")
    );

    let list_messages_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/threads/thread-control-1/messages")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_messages_response = app.oneshot(list_messages_request).await?;
    assert_eq!(list_messages_response.status(), StatusCode::OK);
    let list_messages_body = read_json(list_messages_response).await?;
    assert_eq!(
        list_messages_body["data"]["messages"][0]["text"],
        serde_json::json!("continue")
    );

    Ok(())
}

#[tokio::test]
async fn runtime_codex_control_request_replays_duplicate_request_ids() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "codex-replay@openagents.com").await?;
    let payload = r#"{"request":{"request_id":"req_replay_1","method":"thread/start","params":{"thread_id":"thread-replay-1"}}}"#;

    let first_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(payload))?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_body = read_json(first_response).await?;
    assert_eq!(first_body["data"]["idempotent_replay"], false);

    let second_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(payload))?;
    let second_response = app.oneshot(second_request).await?;
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = read_json(second_response).await?;
    assert_eq!(second_body["data"]["idempotent_replay"], true);
    assert_eq!(
        second_body["data"]["response"]["thread_id"],
        serde_json::json!("thread-replay-1")
    );

    Ok(())
}

#[tokio::test]
async fn runtime_codex_control_request_rejects_non_allowlisted_methods() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "codex-invalid@openagents.com").await?;

    let request = Request::builder()
        .method("POST")
        .uri("/api/runtime/codex/workers/desktopw%3Ashared/requests")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"request":{"request_id":"req_bad_1","method":"shell/exec","params":{}}}"#,
        ))?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "invalid_request");

    Ok(())
}

#[tokio::test]
async fn runtime_codex_worker_lifecycle_routes_support_create_events_stream_and_stop() -> Result<()>
{
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "codex-worker-lifecycle@openagents.com").await?;

    let create_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"worker_id":"desktopw:lifecycle","workspace_ref":"ws-local","adapter":"codex_cli"}"#,
            ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::ACCEPTED);
    let create_body = read_json(create_response).await?;
    assert_eq!(create_body["data"]["workerId"], "desktopw:lifecycle");
    assert_eq!(create_body["data"]["status"], "running");
    assert_eq!(create_body["data"]["latestSeq"], 0);
    assert_eq!(create_body["data"]["idempotentReplay"], false);

    let list_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/codex/workers?status=running&workspace_ref=ws-local")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_response = app.clone().oneshot(list_request).await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await?;
    let workers = list_body["data"].as_array().cloned().unwrap_or_default();
    assert!(
        workers
            .iter()
            .any(|worker| worker["worker_id"] == json!("desktopw:lifecycle"))
    );

    let show_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/codex/workers/desktopw%3Alifecycle")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let show_response = app.clone().oneshot(show_request).await?;
    assert_eq!(show_response.status(), StatusCode::OK);
    let show_body = read_json(show_response).await?;
    assert_eq!(show_body["data"]["worker_id"], "desktopw:lifecycle");
    assert_eq!(show_body["data"]["status"], "running");
    assert_eq!(show_body["data"]["latest_seq"], 0);

    let event_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Alifecycle/events")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"event":{"event_type":"worker.event","payload":{"source":"autopilot-ios","method":"ios/handshake","handshake_id":"hs_1","occurred_at":"2026-02-22T00:00:00Z","device_id":"ios:device"}}}"#,
            ))?;
    let event_response = app.clone().oneshot(event_request).await?;
    assert_eq!(event_response.status(), StatusCode::ACCEPTED);
    let event_body = read_json(event_response).await?;
    assert_eq!(event_body["data"]["worker_id"], "desktopw:lifecycle");
    assert_eq!(event_body["data"]["event_type"], "worker.event");
    assert_eq!(event_body["data"]["seq"], 1);

    let stream_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/codex/workers/desktopw%3Alifecycle/stream?cursor=0&tail_ms=5000")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let stream_response = app.clone().oneshot(stream_request).await?;
    assert_eq!(stream_response.status(), StatusCode::OK);
    let stream_body = read_json(stream_response).await?;
    assert_eq!(stream_body["data"]["stream_protocol"], "khala_ws");
    assert_eq!(stream_body["data"]["delivery"]["transport"], "khala_ws");
    let events = stream_body["data"]["events"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["event_type"], "worker.event");
    assert_eq!(events[0]["seq"], 1);

    let stop_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/codex/workers/desktopw%3Alifecycle/stop")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"reason":"test_complete"}"#))?;
    let stop_response = app.clone().oneshot(stop_request).await?;
    assert_eq!(stop_response.status(), StatusCode::ACCEPTED);
    let stop_body = read_json(stop_response).await?;
    assert_eq!(stop_body["data"]["status"], "stopped");
    assert_eq!(stop_body["data"]["seq"], 2);
    assert_eq!(stop_body["data"]["idempotent_replay"], false);
    assert_eq!(stop_body["data"]["idempotentReplay"], false);

    let stop_replay_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/codex/workers/desktopw%3Alifecycle/stop")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("{}"))?;
    let stop_replay_response = app.clone().oneshot(stop_replay_request).await?;
    assert_eq!(stop_replay_response.status(), StatusCode::ACCEPTED);
    let stop_replay_body = read_json(stop_replay_response).await?;
    assert_eq!(stop_replay_body["data"]["seq"], 2);
    assert_eq!(stop_replay_body["data"]["idempotent_replay"], true);
    assert_eq!(stop_replay_body["data"]["idempotentReplay"], true);

    let stream_after_stop_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/codex/workers/desktopw%3Alifecycle/stream?cursor=1")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let stream_after_stop_response = app.oneshot(stream_after_stop_request).await?;
    assert_eq!(stream_after_stop_response.status(), StatusCode::OK);
    let stream_after_stop_body = read_json(stream_after_stop_response).await?;
    let events_after_stop = stream_after_stop_body["data"]["events"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(events_after_stop.len(), 1);
    assert_eq!(events_after_stop[0]["event_type"], "worker.stopped");
    assert_eq!(events_after_stop[0]["seq"], 2);

    Ok(())
}

#[tokio::test]
async fn runtime_codex_worker_events_validate_handshake_requirements() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "codex-worker-validation@openagents.com").await?;

    let missing_device_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Avalidation/events")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"event":{"event_type":"worker.event","payload":{"source":"autopilot-ios","method":"ios/handshake","handshake_id":"hs_2","occurred_at":"2026-02-22T00:00:00Z"}}}"#,
            ))?;
    let missing_device_response = app.oneshot(missing_device_request).await?;
    assert_eq!(
        missing_device_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let missing_device_body = read_json(missing_device_response).await?;
    assert_eq!(missing_device_body["error"]["code"], "invalid_request");

    Ok(())
}

#[tokio::test]
async fn runtime_codex_workers_index_rejects_invalid_status_filter() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "codex-worker-list-invalid@openagents.com").await?;

    let request = Request::builder()
        .method("GET")
        .uri("/api/runtime/codex/workers?status=unsupported")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], "invalid_request");

    Ok(())
}

#[tokio::test]
async fn runtime_codex_control_request_conflicts_for_stopped_worker() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "codex-worker-stopped@openagents.com").await?;

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/codex/workers")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"worker_id":"desktopw:stopped"}"#))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::ACCEPTED);

    let stop_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/codex/workers/desktopw%3Astopped/stop")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("{}"))?;
    let stop_response = app.clone().oneshot(stop_request).await?;
    assert_eq!(stop_response.status(), StatusCode::ACCEPTED);

    let control_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/codex/workers/desktopw%3Astopped/requests")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"request":{"request_id":"req_stopped_1","method":"turn/start","params":{"thread_id":"thread-stopped-1","text":"continue"}}}"#,
            ))?;
    let control_response = app.oneshot(control_request).await?;
    assert_eq!(control_response.status(), StatusCode::CONFLICT);
    let control_body = read_json(control_response).await?;
    assert_eq!(control_body["error"]["code"], "conflict");

    Ok(())
}

#[tokio::test]
async fn runtime_tools_execute_returns_deterministic_receipts_and_replays() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "runtime-tools@openagents.com").await?;
    let payload = r#"{
            "tool_pack":"coding.v1",
            "mode":"replay",
            "run_id":"run_tools_1",
            "thread_id":"thread_tools_1",
            "manifest_ref":{"integration_id":"github.primary"},
            "request":{
                "integration_id":"github.primary",
                "operation":"get_issue",
                "repository":"OpenAgentsInc/openagents",
                "issue_number":1747,
                "tool_call_id":"tool_call_001"
            },
            "policy":{
                "authorization_id":"auth_123",
                "authorization_mode":"delegated_budget",
                "budget":{"max_per_call_sats":100}
            }
        }"#;

    let first_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/tools/execute")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(payload))?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::OK);
    let first_body = read_json(first_response).await?;
    assert_eq!(first_body["data"]["state"], "succeeded");
    assert_eq!(first_body["data"]["decision"], "allowed");
    assert_eq!(first_body["data"]["reason_code"], "policy_allowed.default");
    assert_eq!(first_body["data"]["idempotentReplay"], false);
    assert!(
        first_body["data"]["receipt"]["receipt_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("coding_")
    );

    let second_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/tools/execute")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(payload))?;
    let second_response = app.oneshot(second_request).await?;
    assert_eq!(second_response.status(), StatusCode::OK);
    let second_body = read_json(second_response).await?;
    assert_eq!(second_body["data"]["idempotentReplay"], true);
    assert_eq!(
        second_body["data"]["receipt"]["replay_hash"],
        first_body["data"]["receipt"]["replay_hash"]
    );

    Ok(())
}

#[tokio::test]
async fn runtime_tools_execute_rejects_user_mismatch_and_missing_manifest() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "runtime-mismatch@openagents.com").await?;

    let session_request = Request::builder()
        .method("GET")
        .uri("/api/auth/session")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let session_response = app.clone().oneshot(session_request).await?;
    assert_eq!(session_response.status(), StatusCode::OK);
    let session_body = read_json(session_response).await?;
    let user_id = session_body["data"]["user"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let authenticated_user_id = crate::runtime_tools_principal_user_id(&user_id);

    let mismatch_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/tools/execute")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(
                r#"{{
                    "tool_pack":"coding.v1",
                    "manifest_ref":{{"integration_id":"github.primary"}},
                    "request":{{"integration_id":"github.primary","operation":"get_issue","repository":"OpenAgentsInc/openagents","issue_number":1747}},
                    "user_id":{}
                }}"#,
                authenticated_user_id.saturating_add(1)
            )))?;
    let mismatch_response = app.clone().oneshot(mismatch_request).await?;
    assert_eq!(mismatch_response.status(), StatusCode::FORBIDDEN);
    let mismatch_body = read_json(mismatch_response).await?;
    assert_eq!(mismatch_body["error"]["code"], "forbidden");

    let missing_manifest_request = Request::builder()
            .method("POST")
            .uri("/api/runtime/tools/execute")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(
                r#"{"tool_pack":"coding.v1","request":{"integration_id":"github.primary","operation":"get_issue","repository":"OpenAgentsInc/openagents","issue_number":1747}}"#,
            ))?;
    let missing_manifest_response = app.oneshot(missing_manifest_request).await?;
    assert_eq!(
        missing_manifest_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let missing_manifest_body = read_json(missing_manifest_response).await?;
    assert_eq!(missing_manifest_body["error"]["code"], "invalid_request");
    assert_eq!(
        missing_manifest_body["message"],
        "manifest or manifest_ref is required"
    );

    Ok(())
}

#[tokio::test]
async fn runtime_tools_execute_enforces_write_approval_policy() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "runtime-write@openagents.com").await?;

    let denied_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/tools/execute")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{
                    "tool_pack":"coding.v1",
                    "manifest":{
                        "capabilities":["add_issue_comment"],
                        "policy":{"write_operations_mode":"enforce"}
                    },
                    "request":{
                        "integration_id":"github.primary",
                        "operation":"add_issue_comment",
                        "repository":"OpenAgentsInc/openagents",
                        "issue_number":1747,
                        "body":"Ship it."
                    },
                    "policy":{"write_approved":false}
                }"#,
        ))?;
    let denied_response = app.clone().oneshot(denied_request).await?;
    assert_eq!(denied_response.status(), StatusCode::OK);
    let denied_body = read_json(denied_response).await?;
    assert_eq!(denied_body["data"]["decision"], "denied");
    assert_eq!(denied_body["data"]["state"], "blocked");
    assert_eq!(
        denied_body["data"]["reason_code"],
        "policy_denied.write_approval_required"
    );

    let allowed_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/tools/execute")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{
                    "tool_pack":"coding.v1",
                    "manifest":{
                        "capabilities":["add_issue_comment"],
                        "policy":{"write_operations_mode":"enforce"}
                    },
                    "request":{
                        "integration_id":"github.primary",
                        "operation":"add_issue_comment",
                        "repository":"OpenAgentsInc/openagents",
                        "issue_number":1747,
                        "body":"Ship it."
                    },
                    "policy":{"write_approved":true}
                }"#,
        ))?;
    let allowed_response = app.oneshot(allowed_request).await?;
    assert_eq!(allowed_response.status(), StatusCode::OK);
    let allowed_body = read_json(allowed_response).await?;
    assert_eq!(allowed_body["data"]["decision"], "allowed");
    assert_eq!(allowed_body["data"]["state"], "succeeded");
    assert_eq!(
        allowed_body["data"]["result"]["comment"]["body"],
        "Ship it."
    );

    Ok(())
}

#[tokio::test]
async fn runtime_skill_registry_routes_support_list_upsert_publish_and_release() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "runtime-skills@openagents.com").await?;

    let list_tools_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/skills/tool-specs")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let list_tools_response = app.clone().oneshot(list_tools_request).await?;
    assert_eq!(list_tools_response.status(), StatusCode::OK);
    let list_tools_body = read_json(list_tools_response).await?;
    let tool_specs = list_tools_body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        tool_specs
            .iter()
            .any(|tool| tool["tool_id"] == serde_json::json!("github.primary"))
    );

    let store_tool_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/skills/tool-specs")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{
                    "state":"validated",
                    "tool_spec":{
                        "tool_id":"github.custom",
                        "version":1,
                        "tool_pack":"coding.v1",
                        "name":"GitHub Custom",
                        "execution_kind":"http",
                        "integration_manifest":{
                            "manifest_version":"coding.integration.v1",
                            "integration_id":"github.custom",
                            "provider":"github",
                            "status":"active",
                            "tool_pack":"coding.v1",
                            "capabilities":["get_issue","get_pull_request"]
                        }
                    }
                }"#,
        ))?;
    let store_tool_response = app.clone().oneshot(store_tool_request).await?;
    assert_eq!(store_tool_response.status(), StatusCode::CREATED);
    let store_tool_body = read_json(store_tool_response).await?;
    assert_eq!(store_tool_body["data"]["tool_id"], "github.custom");
    assert_eq!(store_tool_body["data"]["state"], "validated");

    let store_skill_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/skills/skill-specs")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{
                    "state":"validated",
                    "skill_spec":{
                        "skill_id":"github-coding-custom",
                        "version":1,
                        "name":"GitHub Coding Custom",
                        "allowed_tools":[{"tool_id":"github.custom","version":1}],
                        "compatibility":{"runtime":"runtime"}
                    }
                }"#,
        ))?;
    let store_skill_response = app.clone().oneshot(store_skill_request).await?;
    assert_eq!(store_skill_response.status(), StatusCode::CREATED);
    let store_skill_body = read_json(store_skill_response).await?;
    assert_eq!(store_skill_body["data"]["skill_id"], "github-coding-custom");
    assert_eq!(store_skill_body["data"]["state"], "validated");

    let publish_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/skills/skill-specs/github-coding-custom/1/publish")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let publish_response = app.clone().oneshot(publish_request).await?;
    assert_eq!(publish_response.status(), StatusCode::CREATED);
    let publish_body = read_json(publish_response).await?;
    assert_eq!(publish_body["data"]["skill_id"], "github-coding-custom");
    assert!(
        publish_body["data"]["bundle_hash"]
            .as_str()
            .unwrap_or_default()
            .len()
            == 64
    );

    let release_request = Request::builder()
        .method("GET")
        .uri("/api/runtime/skills/releases/github-coding-custom/1")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let release_response = app.oneshot(release_request).await?;
    assert_eq!(release_response.status(), StatusCode::OK);
    let release_body = read_json(release_response).await?;
    assert_eq!(
        release_body["data"]["bundle"]["bundle_format"],
        "agent_skills.v1"
    );

    Ok(())
}

#[tokio::test]
async fn runtime_skill_registry_routes_validate_schema_and_state() -> Result<()> {
    let app = build_router(test_config(std::env::temp_dir()));
    let token = authenticate_token(app.clone(), "runtime-skills-invalid@openagents.com").await?;

    let invalid_state_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/skills/tool-specs")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{
                    "state":"unsupported",
                    "tool_spec":{
                        "tool_id":"github.custom",
                        "version":1,
                        "tool_pack":"coding.v1"
                    }
                }"#,
        ))?;
    let invalid_state_response = app.clone().oneshot(invalid_state_request).await?;
    assert_eq!(
        invalid_state_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let invalid_state_body = read_json(invalid_state_response).await?;
    assert_eq!(invalid_state_body["error"]["code"], "invalid_request");

    let missing_version_request = Request::builder()
        .method("POST")
        .uri("/api/runtime/skills/tool-specs")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{
                    "tool_spec":{
                        "tool_id":"github.custom",
                        "tool_pack":"coding.v1"
                    }
                }"#,
        ))?;
    let missing_version_response = app.oneshot(missing_version_request).await?;
    assert_eq!(
        missing_version_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let missing_version_body = read_json(missing_version_response).await?;
    assert_eq!(missing_version_body["error"]["code"], "invalid_request");
    assert_eq!(
        missing_version_body["errors"]["tool_spec.version"],
        serde_json::json!(["The tool_spec.version field must be an integer."])
    );

    Ok(())
}

#[tokio::test]
async fn auth_failure_paths_emit_failure_audit_events() -> Result<()> {
    let static_dir = tempdir()?;
    let sink = Arc::new(RecordingAuditSink::default());
    let app = build_router_with_observability(
        test_config(static_dir.path().to_path_buf()),
        Observability::new(sink.clone()),
    );

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("x-request-id", "req-auth-send")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"failure-audit@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::OK);
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("x-request-id", "req-auth-verify-failed")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"000000"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    assert_eq!(verify_response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let refresh_request = Request::builder()
        .method("POST")
        .uri("/api/auth/refresh")
        .header("x-request-id", "req-auth-refresh-failed")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"refresh_token":"oa_rt_invalid","rotate_refresh_token":true}"#,
        ))?;
    let refresh_response = app.clone().oneshot(refresh_request).await?;
    assert_eq!(refresh_response.status(), StatusCode::UNAUTHORIZED);

    let valid_session_token =
        authenticate_token(app.clone(), "logout-failure@openagents.com").await?;
    let create_pat_request = Request::builder()
        .method("POST")
        .uri("/api/tokens")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {valid_session_token}"))
        .body(Body::from(r#"{"name":"logout-failure-pat"}"#))?;
    let create_pat_response = app.clone().oneshot(create_pat_request).await?;
    assert_eq!(create_pat_response.status(), StatusCode::CREATED);
    let create_pat_body = read_json(create_pat_response).await?;
    let pat_token = create_pat_body["data"]["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let logout_request = Request::builder()
        .method("POST")
        .uri("/api/auth/logout")
        .header("x-request-id", "req-auth-logout-failed")
        .header("authorization", format!("Bearer {pat_token}"))
        .body(Body::empty())?;
    let logout_response = app.oneshot(logout_request).await?;
    assert_eq!(logout_response.status(), StatusCode::UNAUTHORIZED);

    let events = sink.events();
    let verify_failed = events
        .iter()
        .find(|event| event.event_name == "auth.verify.failed")
        .expect("missing auth.verify.failed audit event");
    assert_eq!(verify_failed.request_id, "req-auth-verify-failed");
    assert_eq!(verify_failed.outcome, "failure");
    assert_eq!(
        verify_failed.attributes.get("reason").map(String::as_str),
        Some("invalid_request")
    );

    let refresh_failed = events
        .iter()
        .find(|event| event.event_name == "auth.refresh.failed")
        .expect("missing auth.refresh.failed audit event");
    assert_eq!(refresh_failed.request_id, "req-auth-refresh-failed");
    assert_eq!(refresh_failed.outcome, "failure");
    assert_eq!(
        refresh_failed.attributes.get("reason").map(String::as_str),
        Some("unauthorized")
    );

    let logout_failed = events
        .iter()
        .find(|event| event.event_name == "auth.logout.failed")
        .expect("missing auth.logout.failed audit event");
    assert_eq!(logout_failed.request_id, "req-auth-logout-failed");
    assert_eq!(logout_failed.outcome, "failure");
    assert_eq!(
        logout_failed.attributes.get("reason").map(String::as_str),
        Some("unauthorized")
    );

    Ok(())
}

#[tokio::test]
async fn audit_events_include_request_correlation_and_identity_fields() -> Result<()> {
    let static_dir = tempdir()?;
    let sink = Arc::new(RecordingAuditSink::default());
    let app = build_router_with_observability(
        test_config(static_dir.path().to_path_buf()),
        Observability::new(sink.clone()),
    );

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("x-request-id", "req-auth-email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"audit@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("x-request-id", "req-auth-verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let sync_request = Request::builder()
        .method("POST")
        .uri("/api/sync/token")
        .header("x-request-id", "req-sync-token")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"scopes":["runtime.codex_worker_events"]}"#))?;
    let sync_response = app.oneshot(sync_request).await?;
    assert_eq!(sync_response.status(), StatusCode::OK);

    let events = sink.events();
    let verify_event = events
        .iter()
        .find(|event| event.event_name == "auth.verify.completed")
        .expect("missing auth.verify.completed event");
    assert_eq!(verify_event.request_id, "req-auth-verify");
    assert_eq!(verify_event.outcome, "success");
    assert!(verify_event.user_id.is_some());
    assert!(verify_event.session_id.is_some());
    assert!(verify_event.org_id.is_some());
    assert!(verify_event.device_id.is_some());

    let sync_event = events
        .iter()
        .find(|event| event.event_name == "sync.token.issued")
        .expect("missing sync.token.issued event");
    assert_eq!(sync_event.request_id, "req-sync-token");
    assert_eq!(sync_event.outcome, "success");
    assert!(sync_event.attributes.contains_key("scope_count"));
    assert!(sync_event.attributes.contains_key("topic_count"));
    assert!(sync_event.attributes.contains_key("expires_in"));

    Ok(())
}

#[tokio::test]
async fn route_split_serves_rust_shell_and_audits_decision() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;

    let sink = Arc::new(RecordingAuditSink::default());
    let app = build_router_with_observability(
        test_config(static_dir.path().to_path_buf()),
        Observability::new(sink.clone()),
    );

    let request = Request::builder()
        .uri("/chat/thread-1")
        .header("x-request-id", "req-route-split")
        .header("x-oa-route-key", "user:route")
        .header("user-agent", "autopilot-ios")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await?.to_bytes();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("rust shell"));

    let events = sink.events();
    let decision_event = events
        .iter()
        .find(|event| event.event_name == "route.split.decision")
        .expect("missing route.split.decision audit event");
    assert_eq!(decision_event.request_id, "req-route-split");
    assert_eq!(
        decision_event
            .attributes
            .get("target")
            .map(String::as_str)
            .unwrap_or_default(),
        "rust_shell"
    );

    Ok(())
}

#[tokio::test]
async fn route_split_serves_management_route_prefixes_in_rust_cohort() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    for path in [
        "/account/session",
        "/settings/profile",
        "/l402/paywalls",
        "/billing/deployments",
        "/admin",
    ] {
        let request = Request::builder()
            .uri(path)
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let response = app.clone().oneshot(request).await?;
        if [
            "/settings/profile",
            "/l402/paywalls",
            "/billing/deployments",
            "/admin",
        ]
        .contains(&path)
        {
            assert_eq!(
                response.status(),
                StatusCode::TEMPORARY_REDIRECT,
                "unexpected status for {path}"
            );
            assert_eq!(
                response
                    .headers()
                    .get("location")
                    .and_then(|value| value.to_str().ok()),
                Some("/login"),
                "management route should remain rust-owned auth redirect: {path}"
            );
        } else {
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "unexpected status for {path}"
            );
            let body = response.into_body().collect().await?.to_bytes();
            let html = String::from_utf8_lossy(&body);
            assert!(
                html.contains("rust shell"),
                "management route was not served by rust shell: {path}"
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn route_split_serves_auth_entry_routes_in_rust_cohort() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    for path in [
        "/login",
        "/register",
        "/authenticate",
        "/onboarding/checklist",
    ] {
        let request = Request::builder()
            .uri(path)
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "unexpected status for {path}"
        );
        let body = response.into_body().collect().await?.to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(
            html.contains("rust shell"),
            "auth route was not served by rust shell: {path}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn route_split_rust_mode_with_root_prefix_serves_unlisted_paths() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;

    let mut config = test_config(static_dir.path().to_path_buf());
    config.route_split_mode = "rust".to_string();
    config.route_split_rust_routes = vec!["/".to_string()];
    let app = build_router(config);

    for path in ["/feed", "/new-surface/path", "/login"] {
        let request = Request::builder()
            .uri(path)
            .header("x-oa-route-key", "user:route")
            .body(Body::empty())?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "unexpected status for {path}"
        );
        let body = response.into_body().collect().await?.to_bytes();
        let html = String::from_utf8_lossy(&body);
        assert!(
            html.contains("rust shell"),
            "path was not served by rust shell: {path}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn route_split_override_keeps_chat_pilot_on_rust_shell() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/auth/email")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"email":"routes@openagents.com"}"#))?;
    let send_response = app.clone().oneshot(send_request).await?;
    let cookie = cookie_value(&send_response).unwrap_or_default();

    let verify_request = Request::builder()
        .method("POST")
        .uri("/api/auth/verify")
        .header("content-type", "application/json")
        .header("x-client", "autopilot-ios")
        .header("cookie", cookie)
        .body(Body::from(r#"{"code":"123456"}"#))?;
    let verify_response = app.clone().oneshot(verify_request).await?;
    let verify_body = read_json(verify_response).await?;
    let token = verify_body["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"target":"legacy"}"#))?;
    let override_response = app.clone().oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);

    let route_request = Request::builder()
        .uri("/chat/thread-1")
        .header("x-oa-route-key", "user:route")
        .body(Body::empty())?;
    let route_response = app.clone().oneshot(route_request).await?;
    assert_eq!(route_response.status(), StatusCode::OK);
    let route_body = route_response.into_body().collect().await?.to_bytes();
    let route_html = String::from_utf8_lossy(&route_body);
    assert!(route_html.contains("rust shell"));

    let root_request = Request::builder()
        .uri("/")
        .header("x-oa-route-key", "user:route")
        .body(Body::empty())?;
    let root_response = app.clone().oneshot(root_request).await?;
    assert_eq!(root_response.status(), StatusCode::OK);
    let root_body = root_response.into_body().collect().await?.to_bytes();
    let root_html = String::from_utf8_lossy(&root_body);
    assert!(root_html.contains("rust shell"));

    let workspace_request = Request::builder()
        .uri("/workspace/session-1")
        .header("x-oa-route-key", "user:route")
        .body(Body::empty())?;
    let workspace_response = app.oneshot(workspace_request).await?;
    assert_eq!(workspace_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        workspace_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("https://legacy.openagents.test/workspace/session-1")
    );

    Ok(())
}

#[tokio::test]
async fn retired_aui_route_returns_not_found() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder().uri("/aui").body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn route_split_domain_override_only_affects_selected_route_group() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.route_split_mode = "rust".to_string();
    config.route_split_rust_routes = vec!["/".to_string()];
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "routes@openagents.com").await?;

    let override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"target":"legacy","domain":"billing_l402"}"#))?;
    let override_response = app.clone().oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);

    let billing_request = Request::builder()
        .uri("/l402/paywalls")
        .header("x-oa-route-key", "user:route")
        .body(Body::empty())?;
    let billing_response = app.clone().oneshot(billing_request).await?;
    assert_eq!(billing_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        billing_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("https://legacy.openagents.test/l402/paywalls")
    );

    let settings_request = Request::builder()
        .uri("/settings/profile")
        .header("x-oa-route-key", "user:route")
        .body(Body::empty())?;
    let settings_response = app.oneshot(settings_request).await?;
    assert_eq!(settings_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        settings_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/login")
    );

    Ok(())
}

#[tokio::test]
async fn route_split_status_exposes_rollback_matrix_and_domain_overrides() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.route_split_mode = "rust".to_string();
    config.route_split_rust_routes = vec!["/".to_string()];
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "routes@openagents.com").await?;

    let override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"target":"rollback","domain":"billing_l402"}"#,
        ))?;
    let override_response = app.clone().oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);

    let htmx_override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"target":"htmx_full_page","domain":"billing_l402"}"#,
        ))?;
    let htmx_override_response = app.clone().oneshot(htmx_override_request).await?;
    assert_eq!(htmx_override_response.status(), StatusCode::OK);

    let status_request = Request::builder()
        .method("GET")
        .uri("/api/v1/control/route-split/status")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let status_response = app.oneshot(status_request).await?;
    assert_eq!(status_response.status(), StatusCode::OK);

    let body = read_json(status_response).await?;
    assert_eq!(
        body["data"]["rollback_matrix"]["billing_l402"],
        json!("legacy")
    );
    assert_eq!(
        body["data"]["rollback_matrix"]["chat_pilot"],
        json!("rust_shell")
    );
    assert_eq!(
        body["data"]["domain_overrides"]["billing_l402"],
        json!("legacy")
    );
    assert_eq!(
        body["data"]["htmx_rollback_matrix"]["billing_l402"],
        json!("full_page")
    );
    assert_eq!(
        body["data"]["htmx_domain_overrides"]["billing_l402"],
        json!("full_page")
    );

    Ok(())
}

#[tokio::test]
async fn route_split_evaluate_pins_api_paths_to_rust_authority() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.route_split_mode = "legacy".to_string();
    config.route_split_force_legacy = true;
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "routes@openagents.com").await?;

    let override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"target":"legacy"}"#))?;
    let override_response = app.clone().oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);

    let evaluate_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/evaluate")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"path":"/api/auth/email","cohort_key":"user:route"}"#,
        ))?;
    let evaluate_response = app.oneshot(evaluate_request).await?;
    assert_eq!(evaluate_response.status(), StatusCode::OK);

    let body = read_json(evaluate_response).await?;
    assert_eq!(body["data"]["target"], json!("rust_shell"));
    assert_eq!(body["data"]["reason"], json!("api_rust_authority"));
    assert_eq!(body["data"]["route_domain"], json!("api_rust_authority"));
    assert_eq!(body["data"]["rollback_target"], json!("rust_shell"));

    Ok(())
}

#[tokio::test]
async fn l402_read_routes_match_wallet_transactions_and_deployments_shape() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let fixture = seed_l402_fixture(&config, "l402-reader@openagents.com").await?;
    let app = build_router(config);

    let wallet_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/l402/wallet?autopilot={}",
            fixture.autopilot_handle
        ))
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let wallet_response = app.clone().oneshot(wallet_request).await?;
    assert_eq!(wallet_response.status(), StatusCode::OK);
    let wallet_body = read_json(wallet_response).await?;
    assert_eq!(wallet_body["data"]["summary"]["totalAttempts"], json!(2));
    assert_eq!(wallet_body["data"]["summary"]["paidCount"], json!(1));
    assert_eq!(wallet_body["data"]["summary"]["cachedCount"], json!(1));
    assert_eq!(wallet_body["data"]["summary"]["blockedCount"], json!(0));
    assert_eq!(
        wallet_body["data"]["sparkWallet"]["walletId"],
        json!("wallet_123")
    );
    assert_eq!(
        wallet_body["data"]["settings"]["invoicePayer"],
        json!("fake")
    );
    assert_eq!(
        wallet_body["data"]["settings"]["allowlistHosts"],
        json!(vec!["sats4ai.com", "l402.openagents.com"])
    );
    assert_eq!(
        wallet_body["data"]["settings"]["credentialTtlSeconds"],
        json!(600)
    );
    assert_eq!(
        wallet_body["data"]["settings"]["paymentTimeoutMs"],
        json!(12_000)
    );
    assert_eq!(
        wallet_body["data"]["filter"]["autopilot"]["id"],
        json!(fixture.autopilot_id.clone())
    );

    let transactions_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/l402/transactions?autopilot={}&per_page=1&page=2",
            fixture.autopilot_id
        ))
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let transactions_response = app.clone().oneshot(transactions_request).await?;
    assert_eq!(transactions_response.status(), StatusCode::OK);
    let transactions_body = read_json(transactions_response).await?;
    assert_eq!(
        transactions_body["data"]["transactions"]
            .as_array()
            .map(|rows| rows.len()),
        Some(1)
    );
    assert_eq!(
        transactions_body["data"]["pagination"]["currentPage"],
        json!(2)
    );
    assert_eq!(
        transactions_body["data"]["pagination"]["lastPage"],
        json!(2)
    );
    assert_eq!(
        transactions_body["data"]["pagination"]["hasMorePages"],
        json!(false)
    );

    let transaction_show_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/l402/transactions/{}",
            fixture.paid_receipt_event_id
        ))
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let transaction_show_response = app.clone().oneshot(transaction_show_request).await?;
    assert_eq!(transaction_show_response.status(), StatusCode::OK);
    let transaction_show_body = read_json(transaction_show_response).await?;
    assert_eq!(
        transaction_show_body["data"]["transaction"]["eventId"],
        json!(fixture.paid_receipt_event_id)
    );
    assert_eq!(
        transaction_show_body["data"]["transaction"]["status"],
        json!("paid")
    );

    let paywalls_request = Request::builder()
        .method("GET")
        .uri("/api/l402/paywalls")
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let paywalls_response = app.clone().oneshot(paywalls_request).await?;
    assert_eq!(paywalls_response.status(), StatusCode::OK);
    let paywalls_body = read_json(paywalls_response).await?;
    assert_eq!(paywalls_body["data"]["summary"]["uniqueTargets"], json!(2));
    assert_eq!(paywalls_body["data"]["summary"]["totalAttempts"], json!(3));
    assert_eq!(paywalls_body["data"]["summary"]["totalPaidCount"], json!(1));

    let settlements_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/l402/settlements?autopilot={}",
            fixture.autopilot_id
        ))
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let settlements_response = app.clone().oneshot(settlements_request).await?;
    assert_eq!(settlements_response.status(), StatusCode::OK);
    let settlements_body = read_json(settlements_response).await?;
    assert_eq!(
        settlements_body["data"]["summary"]["settledCount"],
        json!(1)
    );
    assert_eq!(
        settlements_body["data"]["summary"]["totalMsats"],
        json!(2100)
    );

    let deployments_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/l402/deployments?autopilot={}",
            fixture.autopilot_id
        ))
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let deployments_response = app.oneshot(deployments_request).await?;
    assert_eq!(deployments_response.status(), StatusCode::OK);
    let deployments_body = read_json(deployments_response).await?;
    assert_eq!(
        deployments_body["data"]["deployments"]
            .as_array()
            .map(|rows| rows.len()),
        Some(1)
    );
    assert_eq!(
        deployments_body["data"]["deployments"][0]["type"],
        json!("l402_gateway_event")
    );
    assert_eq!(
        deployments_body["data"]["configSnapshot"]["invoicePayer"],
        json!("fake")
    );
    assert_eq!(
        deployments_body["data"]["configSnapshot"]["allowlistHosts"],
        json!(vec!["sats4ai.com", "l402.openagents.com"])
    );
    assert_eq!(
        deployments_body["data"]["configSnapshot"]["demoPresets"],
        json!(vec![
            "sats4ai",
            "ep212_openagents_premium",
            "ep212_openagents_expensive",
            "fake"
        ])
    );

    Ok(())
}

#[tokio::test]
async fn l402_transaction_show_returns_not_found_for_missing_event() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let fixture = seed_l402_fixture(&config, "l402-detail-missing@openagents.com").await?;
    let app = build_router(config);

    let request = Request::builder()
        .method("GET")
        .uri("/api/l402/transactions/999999")
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = read_json(response).await?;
    assert_eq!(body["error"]["code"], json!("not_found"));

    Ok(())
}

#[tokio::test]
async fn l402_autopilot_filter_returns_not_found_and_forbidden() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let fixture = seed_l402_fixture(&config, "l402-access@openagents.com").await?;

    let foreign_auth = super::AuthService::from_config(&config);
    let foreign_verify = foreign_auth
        .local_test_sign_in(
            "l402-foreign@openagents.com".to_string(),
            None,
            Some("autopilot-ios"),
            None,
        )
        .await?;
    let store = super::DomainStore::from_config(&config);
    let foreign_autopilot = store
        .create_autopilot(CreateAutopilotInput {
            owner_user_id: foreign_verify.user.id,
            owner_display_name: "Other".to_string(),
            display_name: "Foreign Pilot".to_string(),
            handle_seed: None,
            avatar: None,
            status: None,
            visibility: None,
            tagline: None,
        })
        .await
        .expect("seed foreign autopilot");

    let app = build_router(config);

    let missing_request = Request::builder()
        .method("GET")
        .uri("/api/l402/wallet?autopilot=missing-pilot")
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let missing_response = app.clone().oneshot(missing_request).await?;
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
    let missing_body = read_json(missing_response).await?;
    assert_eq!(missing_body["message"], json!("autopilot_not_found"));

    let forbidden_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/l402/wallet?autopilot={}",
            foreign_autopilot.autopilot.id
        ))
        .header("authorization", format!("Bearer {}", fixture.token))
        .body(Body::empty())?;
    let forbidden_response = app.oneshot(forbidden_request).await?;
    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);
    let forbidden_body = read_json(forbidden_response).await?;
    assert_eq!(forbidden_body["message"], json!("autopilot_forbidden"));

    Ok(())
}

#[tokio::test]
async fn l402_paywall_lifecycle_requires_admin_and_records_mutation_events() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let admin_token = seed_local_test_token(&config, "routes@openagents.com").await?;
    let member_token = seed_local_test_token(&config, "member@openagents.com").await?;
    let app = build_router(config);

    let forbidden_request = Request::builder()
        .method("POST")
        .uri("/api/l402/paywalls")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {member_token}"))
        .body(Body::from(
            r#"{
                    "name":"Default",
                    "hostRegexp":"sats4ai\\.com",
                    "pathRegexp":"^/api/.*",
                    "priceMsats":1000,
                    "upstream":"https://upstream.openagents.com",
                    "enabled":true
                }"#,
        ))?;
    let forbidden_response = app.clone().oneshot(forbidden_request).await?;
    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);

    let invalid_request = Request::builder()
        .method("POST")
        .uri("/api/l402/paywalls")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from(
            r#"{
                    "name":"Invalid",
                    "hostRegexp":"sats4ai\\.com",
                    "pathRegexp":"/api/.*",
                    "priceMsats":1000,
                    "upstream":"https://upstream.openagents.com"
                }"#,
        ))?;
    let invalid_response = app.clone().oneshot(invalid_request).await?;
    assert_eq!(invalid_response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/l402/paywalls")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from(
            r#"{
                    "name":"Default",
                    "hostRegexp":"sats4ai\\.com",
                    "pathRegexp":"^/api/.*",
                    "priceMsats":1000,
                    "upstream":"https://upstream.openagents.com",
                    "enabled":true,
                    "metadata":{"tier":"default"}
                }"#,
        ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    let paywall_id = create_body["data"]["paywall"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!paywall_id.is_empty());
    assert_eq!(create_body["data"]["paywall"]["name"], json!("Default"));
    assert_eq!(
        create_body["data"]["deployment"]["status"],
        json!("applied")
    );
    assert!(create_body["data"]["mutationEventId"].as_u64().unwrap_or(0) > 0);

    let empty_update_request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/l402/paywalls/{paywall_id}"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from("{}"))?;
    let empty_update_response = app.clone().oneshot(empty_update_request).await?;
    assert_eq!(
        empty_update_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let empty_update_body = read_json(empty_update_response).await?;
    assert_eq!(
        empty_update_body["message"],
        json!("At least one mutable paywall field must be provided.")
    );

    let update_request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/l402/paywalls/{paywall_id}"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::from(
            r#"{
                    "priceMsats":2500,
                    "enabled":false,
                    "metadata":{"tier":"burst"}
                }"#,
        ))?;
    let update_response = app.clone().oneshot(update_request).await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await?;
    assert_eq!(update_body["data"]["paywall"]["priceMsats"], json!(2500));
    assert_eq!(update_body["data"]["paywall"]["enabled"], json!(false));
    assert_eq!(
        update_body["data"]["deployment"]["eventType"],
        json!("l402_paywall_updated")
    );

    let delete_request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/l402/paywalls/{paywall_id}"))
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::empty())?;
    let delete_response = app.clone().oneshot(delete_request).await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    let delete_body = read_json(delete_response).await?;
    assert_eq!(delete_body["data"]["deleted"], json!(true));
    assert_eq!(
        delete_body["data"]["deployment"]["eventType"],
        json!("l402_paywall_deleted")
    );

    let deployments_request = Request::builder()
        .method("GET")
        .uri("/api/l402/deployments")
        .header("authorization", format!("Bearer {admin_token}"))
        .body(Body::empty())?;
    let deployments_response = app.oneshot(deployments_request).await?;
    assert_eq!(deployments_response.status(), StatusCode::OK);
    let deployments_body = read_json(deployments_response).await?;
    let deployment_types = deployments_body["data"]["deployments"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|event| event["type"].as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    assert!(deployment_types.contains(&"l402_paywall_created".to_string()));
    assert!(deployment_types.contains(&"l402_paywall_updated".to_string()));
    assert!(deployment_types.contains(&"l402_paywall_deleted".to_string()));

    Ok(())
}

#[tokio::test]
async fn lightning_ops_control_plane_query_and_mutation_contracts() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;

    let mut config = test_config(static_dir.path().to_path_buf());
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let store = DomainStore::from_config(&config);
    let owner_user_id = "usr_ops".to_string();

    let active_paywall = store
        .create_l402_paywall(CreateL402PaywallInput {
            owner_user_id: owner_user_id.clone(),
            name: "Ops Active".to_string(),
            host_regexp: "sats4ai\\.com".to_string(),
            path_regexp: "^/v1/.*".to_string(),
            price_msats: 1200,
            upstream: "https://upstream.openagents.com".to_string(),
            enabled: Some(true),
            meta: Some(json!({
                "timeoutMs": 3000,
                "priority": 5,
                "allowedHosts": ["sats4ai.com"],
            })),
        })
        .await
        .expect("create active paywall");

    let _paused_paywall = store
        .create_l402_paywall(CreateL402PaywallInput {
            owner_user_id: owner_user_id.clone(),
            name: "Ops Paused".to_string(),
            host_regexp: "api\\.openagents\\.com".to_string(),
            path_regexp: "^/ops/.*".to_string(),
            price_msats: 3000,
            upstream: "http://localhost:8080".to_string(),
            enabled: Some(false),
            meta: None,
        })
        .await
        .expect("create paused paywall");

    let archived_paywall = store
        .create_l402_paywall(CreateL402PaywallInput {
            owner_user_id: owner_user_id.clone(),
            name: "Ops Archived".to_string(),
            host_regexp: "archive\\.example\\.com".to_string(),
            path_regexp: "^/archive/.*".to_string(),
            price_msats: 5000,
            upstream: "https://archive.openagents.com".to_string(),
            enabled: Some(true),
            meta: None,
        })
        .await
        .expect("create archived paywall");
    let _ = store
        .soft_delete_owned_l402_paywall(&owner_user_id, &archived_paywall.id)
        .await
        .expect("archive paywall");

    let app = build_router(config);

    let bad_secret_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/query")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/ops:listPaywallControlPlaneState",
                "args": {
                    "secret": "nope"
                }
            })
            .to_string(),
        ))?;
    let bad_secret_response = app.clone().oneshot(bad_secret_request).await?;
    assert_eq!(bad_secret_response.status(), StatusCode::UNAUTHORIZED);
    let bad_secret_body = read_json(bad_secret_response).await?;
    assert_eq!(
        bad_secret_body["error"]["code"],
        json!("invalid_ops_secret")
    );

    let query_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/query")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/ops:listPaywallControlPlaneState",
                "args": {
                    "secret": "ops-secret-test"
                }
            })
            .to_string(),
        ))?;
    let query_response = app.clone().oneshot(query_request).await?;
    assert_eq!(query_response.status(), StatusCode::OK);
    let query_body = read_json(query_response).await?;
    assert_eq!(query_body["ok"], json!(true));
    let paywalls = query_body["paywalls"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(paywalls.len(), 2);
    let statuses = paywalls
        .iter()
        .filter_map(|row| row["status"].as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    assert!(statuses.contains(&"active".to_string()));
    assert!(statuses.contains(&"paused".to_string()));
    assert_eq!(paywalls[0]["routes"][0]["timeoutMs"], json!(3000));
    assert_eq!(paywalls[0]["routes"][0]["priority"], json!(5));

    let security_before_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/query")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/security:getControlPlaneSecurityState",
                "args": {
                    "secret": "ops-secret-test"
                }
            })
            .to_string(),
        ))?;
    let security_before_response = app.clone().oneshot(security_before_request).await?;
    assert_eq!(security_before_response.status(), StatusCode::OK);
    let security_before_body = read_json(security_before_response).await?;
    assert_eq!(security_before_body["global"]["globalPause"], json!(false));
    assert_eq!(security_before_body["ownerControls"], json!([]));
    assert_eq!(security_before_body["credentialRoles"], json!([]));

    let global_pause_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/security:setGlobalPause",
                "args": {
                    "secret": "ops-secret-test",
                    "active": true,
                    "reason": "Emergency pause",
                    "updatedBy": "ops@openagents.com"
                }
            })
            .to_string(),
        ))?;
    let global_pause_response = app.clone().oneshot(global_pause_request).await?;
    assert_eq!(global_pause_response.status(), StatusCode::OK);
    let global_pause_body = read_json(global_pause_response).await?;
    assert_eq!(global_pause_body["global"]["globalPause"], json!(true));
    assert_eq!(
        global_pause_body["global"]["denyReasonCode"],
        json!("global_pause_active")
    );

    let owner_kill_switch_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/security:setOwnerKillSwitch",
                "args": {
                    "secret": "ops-secret-test",
                    "ownerId": "owner_usr_ops",
                    "active": true,
                    "reason": "Owner pause",
                    "updatedBy": "ops@openagents.com"
                }
            })
            .to_string(),
        ))?;
    let owner_kill_switch_response = app.clone().oneshot(owner_kill_switch_request).await?;
    assert_eq!(owner_kill_switch_response.status(), StatusCode::OK);
    let owner_kill_switch_body = read_json(owner_kill_switch_response).await?;
    assert_eq!(
        owner_kill_switch_body["ownerControl"]["ownerId"],
        json!("owner_usr_ops")
    );
    assert_eq!(
        owner_kill_switch_body["ownerControl"]["killSwitch"],
        json!(true)
    );

    let role_rotate_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/security:rotateCredentialRole",
                "args": {
                    "secret": "ops-secret-test",
                    "role": "gateway_signer",
                    "fingerprint": "fingerprint_v1",
                    "note": "rotation start"
                }
            })
            .to_string(),
        ))?;
    let role_rotate_response = app.clone().oneshot(role_rotate_request).await?;
    assert_eq!(role_rotate_response.status(), StatusCode::OK);
    let role_rotate_body = read_json(role_rotate_response).await?;
    assert_eq!(role_rotate_body["role"]["status"], json!("rotating"));
    assert_eq!(role_rotate_body["role"]["version"], json!(1));

    let role_activate_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/security:activateCredentialRole",
                "args": {
                    "secret": "ops-secret-test",
                    "role": "gateway_signer",
                    "fingerprint": "fingerprint_v1"
                }
            })
            .to_string(),
        ))?;
    let role_activate_response = app.clone().oneshot(role_activate_request).await?;
    assert_eq!(role_activate_response.status(), StatusCode::OK);
    let role_activate_body = read_json(role_activate_response).await?;
    assert_eq!(role_activate_body["role"]["status"], json!("active"));
    assert_eq!(role_activate_body["role"]["version"], json!(1));

    let role_revoke_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/security:revokeCredentialRole",
                "args": {
                    "secret": "ops-secret-test",
                    "role": "gateway_signer",
                    "note": "revoke key"
                }
            })
            .to_string(),
        ))?;
    let role_revoke_response = app.clone().oneshot(role_revoke_request).await?;
    assert_eq!(role_revoke_response.status(), StatusCode::OK);
    let role_revoke_body = read_json(role_revoke_response).await?;
    assert_eq!(role_revoke_body["role"]["status"], json!("revoked"));
    assert_eq!(role_revoke_body["role"]["version"], json!(1));

    let security_after_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/query")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/security:getControlPlaneSecurityState",
                "args": {
                    "secret": "ops-secret-test"
                }
            })
            .to_string(),
        ))?;
    let security_after_response = app.clone().oneshot(security_after_request).await?;
    assert_eq!(security_after_response.status(), StatusCode::OK);
    let security_after_body = read_json(security_after_response).await?;
    assert_eq!(security_after_body["global"]["globalPause"], json!(true));
    assert_eq!(
        security_after_body["ownerControls"]
            .as_array()
            .map(|rows| rows.len()),
        Some(1)
    );
    assert_eq!(
        security_after_body["credentialRoles"]
            .as_array()
            .map(|rows| rows.len()),
        Some(1)
    );

    let compile_intent_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/ops:recordGatewayCompileIntent",
                "args": {
                    "secret": "ops-secret-test",
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "configHash": "cfg_hash_1",
                    "imageDigest": "sha256:abc",
                    "status": "queued",
                    "diagnostics": {"queue":"ready"},
                    "metadata": {"region":"us-central1"},
                    "requestId": "req_1",
                    "appliedAtMs": 12345
                }
            })
            .to_string(),
        ))?;
    let compile_intent_response = app.clone().oneshot(compile_intent_request).await?;
    assert_eq!(compile_intent_response.status(), StatusCode::OK);
    let compile_intent_body = read_json(compile_intent_response).await?;
    assert_eq!(compile_intent_body["deployment"]["status"], json!("queued"));
    let deployment_id = compile_intent_body["deployment"]["deploymentId"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!deployment_id.is_empty());

    let compile_intent_update_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/ops:recordGatewayCompileIntent",
                "args": {
                    "secret": "ops-secret-test",
                    "deploymentId": deployment_id,
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "configHash": "cfg_hash_2",
                    "status": "applied"
                }
            })
            .to_string(),
        ))?;
    let compile_intent_update_response = app.clone().oneshot(compile_intent_update_request).await?;
    assert_eq!(compile_intent_update_response.status(), StatusCode::OK);
    let compile_intent_update_body = read_json(compile_intent_update_response).await?;
    assert_eq!(
        compile_intent_update_body["deployment"]["status"],
        json!("applied")
    );

    let deployment_event_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/ops:recordGatewayDeploymentEvent",
                "args": {
                    "secret": "ops-secret-test",
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "eventType": "deployment_applied",
                    "level": "info",
                    "requestId": "req_2",
                    "metadata": {"target":"gateway"}
                }
            })
            .to_string(),
        ))?;
    let deployment_event_response = app.clone().oneshot(deployment_event_request).await?;
    assert_eq!(deployment_event_response.status(), StatusCode::OK);
    let deployment_event_body = read_json(deployment_event_response).await?;
    assert!(
        deployment_event_body["event"]["eventId"]
            .as_str()
            .unwrap_or_default()
            .starts_with("evt_")
    );

    let invoice_lifecycle_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/settlements:ingestInvoiceLifecycle",
                "args": {
                    "secret": "ops-secret-test",
                    "invoiceId": "inv_1",
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "amountMsats": 2100,
                    "status": "open",
                    "requestId": "inv_req_1"
                }
            })
            .to_string(),
        ))?;
    let invoice_lifecycle_response = app.clone().oneshot(invoice_lifecycle_request).await?;
    assert_eq!(invoice_lifecycle_response.status(), StatusCode::OK);
    let invoice_lifecycle_body = read_json(invoice_lifecycle_response).await?;
    assert_eq!(invoice_lifecycle_body["changed"], json!(true));
    assert_eq!(invoice_lifecycle_body["invoice"]["status"], json!("open"));

    let invoice_settled_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/settlements:ingestInvoiceLifecycle",
                "args": {
                    "secret": "ops-secret-test",
                    "invoiceId": "inv_1",
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "amountMsats": 2100,
                    "status": "settled",
                    "paymentHash": "hash_123",
                    "settledAtMs": 33333
                }
            })
            .to_string(),
        ))?;
    let invoice_settled_response = app.clone().oneshot(invoice_settled_request).await?;
    assert_eq!(invoice_settled_response.status(), StatusCode::OK);
    let invoice_settled_body = read_json(invoice_settled_response).await?;
    assert_eq!(invoice_settled_body["invoice"]["status"], json!("settled"));
    assert_eq!(invoice_settled_body["invoice"]["settledAtMs"], json!(33333));

    let settlement_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/settlements:ingestSettlement",
                "args": {
                    "secret": "ops-secret-test",
                    "settlementId": "settle_1",
                    "invoiceId": "inv_1",
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "amountMsats": 2100,
                    "paymentProofType": "lightning_preimage",
                    "paymentProofValue": "AABBCC11223344556677889900aabbcc",
                    "metadata": {"source":"test"}
                }
            })
            .to_string(),
        ))?;
    let settlement_response = app.clone().oneshot(settlement_request).await?;
    assert_eq!(settlement_response.status(), StatusCode::OK);
    let settlement_body = read_json(settlement_response).await?;
    assert_eq!(settlement_body["existed"], json!(false));
    assert!(
        settlement_body["settlement"]["paymentProofRef"]
            .as_str()
            .unwrap_or_default()
            .starts_with("lightning_preimage:")
    );

    let settlement_replay_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/settlements:ingestSettlement",
                "args": {
                    "secret": "ops-secret-test",
                    "settlementId": "settle_1",
                    "invoiceId": "inv_1",
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "amountMsats": 2100,
                    "paymentProofType": "lightning_preimage",
                    "paymentProofValue": "aabbcc11223344556677889900aabbcc"
                }
            })
            .to_string(),
        ))?;
    let settlement_replay_response = app.clone().oneshot(settlement_replay_request).await?;
    assert_eq!(settlement_replay_response.status(), StatusCode::OK);
    let settlement_replay_body = read_json(settlement_replay_response).await?;
    assert_eq!(settlement_replay_body["existed"], json!(true));

    let invalid_invoice_status_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/mutation")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/settlements:ingestInvoiceLifecycle",
                "args": {
                    "secret": "ops-secret-test",
                    "invoiceId": "inv_invalid",
                    "paywallId": active_paywall.id,
                    "ownerId": "owner_usr_ops",
                    "amountMsats": 1,
                    "status": "bad_status"
                }
            })
            .to_string(),
        ))?;
    let invalid_invoice_status_response =
        app.clone().oneshot(invalid_invoice_status_request).await?;
    assert_eq!(
        invalid_invoice_status_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let invalid_invoice_status_body = read_json(invalid_invoice_status_response).await?;
    assert_eq!(
        invalid_invoice_status_body["error"]["code"],
        json!("invalid_arguments")
    );
    assert_eq!(
        invalid_invoice_status_body["error"]["message"],
        json!("invalid_invoice_status")
    );

    let unsupported_function_request = Request::builder()
        .method("POST")
        .uri("/api/internal/lightning-ops/control-plane/query")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "functionName": "lightning/ops:missing",
                "args": {
                    "secret": "ops-secret-test"
                }
            })
            .to_string(),
        ))?;
    let unsupported_function_response = app.oneshot(unsupported_function_request).await?;
    assert_eq!(
        unsupported_function_response.status(),
        StatusCode::NOT_FOUND
    );
    let unsupported_function_body = read_json(unsupported_function_response).await?;
    assert_eq!(
        unsupported_function_body["error"]["code"],
        json!("unsupported_function")
    );

    Ok(())
}

#[tokio::test]
async fn agent_payments_wallet_balance_and_alias_routes_match() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let token = seed_local_test_token(&config, "agent-payments-user@openagents.com").await?;
    let app = build_router(config);

    let missing_wallet_request = Request::builder()
        .method("GET")
        .uri("/api/agent-payments/wallet")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let missing_wallet_response = app.clone().oneshot(missing_wallet_request).await?;
    assert_eq!(missing_wallet_response.status(), StatusCode::NOT_FOUND);
    let missing_wallet_body = read_json(missing_wallet_response).await?;
    assert_eq!(missing_wallet_body["message"], json!("wallet_not_found"));

    let upsert_request = Request::builder()
        .method("POST")
        .uri("/api/agent-payments/wallet")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("{}"))?;
    let upsert_response = app.clone().oneshot(upsert_request).await?;
    assert_eq!(upsert_response.status(), StatusCode::OK);
    let upsert_body = read_json(upsert_response).await?;
    let wallet_id = upsert_body["data"]["wallet"]["walletId"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!wallet_id.is_empty());
    assert_eq!(upsert_body["data"]["action"], json!("ensured"));

    let wallet_request = Request::builder()
        .method("GET")
        .uri("/api/agent-payments/wallet")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let wallet_response = app.clone().oneshot(wallet_request).await?;
    assert_eq!(wallet_response.status(), StatusCode::OK);
    let wallet_body = read_json(wallet_response).await?;
    assert_eq!(
        wallet_body["data"]["wallet"]["walletId"],
        json!(wallet_id.clone())
    );

    let wallet_alias_request = Request::builder()
        .method("GET")
        .uri("/api/agents/me/wallet")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let wallet_alias_response = app.clone().oneshot(wallet_alias_request).await?;
    assert_eq!(wallet_alias_response.status(), StatusCode::OK);
    let wallet_alias_body = read_json(wallet_alias_response).await?;
    assert_eq!(
        wallet_alias_body["data"]["wallet"]["walletId"],
        json!(wallet_id.clone())
    );

    let balance_request = Request::builder()
        .method("GET")
        .uri("/api/agent-payments/balance")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let balance_response = app.clone().oneshot(balance_request).await?;
    assert_eq!(balance_response.status(), StatusCode::OK);
    let balance_body = read_json(balance_response).await?;
    assert_eq!(balance_body["data"]["walletId"], json!(wallet_id.clone()));

    let balance_alias_request = Request::builder()
        .method("GET")
        .uri("/api/agents/me/balance")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let balance_alias_response = app.oneshot(balance_alias_request).await?;
    assert_eq!(balance_alias_response.status(), StatusCode::OK);
    let balance_alias_body = read_json(balance_alias_response).await?;
    assert_eq!(balance_alias_body["data"]["walletId"], json!(wallet_id));

    Ok(())
}

#[tokio::test]
async fn agent_payments_invoice_pay_send_and_alias_routes_match() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let token = seed_local_test_token(&config, "agent-payments-ops@openagents.com").await?;
    let app = build_router(config);

    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/agent-payments/wallet")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from("{}"))?,
        )
        .await?;

    let create_invoice_request = Request::builder()
        .method("POST")
        .uri("/api/agent-payments/invoice")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"amountSats":42,"description":"OpenAgents test invoice"}"#,
        ))?;
    let create_invoice_response = app.clone().oneshot(create_invoice_request).await?;
    assert_eq!(create_invoice_response.status(), StatusCode::OK);
    let create_invoice_body = read_json(create_invoice_response).await?;
    let payment_request = create_invoice_body["data"]["invoice"]["paymentRequest"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!payment_request.is_empty());

    let create_invoice_alias_request = Request::builder()
        .method("POST")
        .uri("/api/payments/invoice")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"amountSats":21}"#))?;
    let create_invoice_alias_response = app.clone().oneshot(create_invoice_alias_request).await?;
    assert_eq!(create_invoice_alias_response.status(), StatusCode::OK);
    let create_invoice_alias_body = read_json(create_invoice_alias_response).await?;
    assert!(
        create_invoice_alias_body["data"]["invoice"]["paymentRequest"]
            .as_str()
            .unwrap_or_default()
            .starts_with("lnbc")
    );

    let rejected_pay_request = Request::builder()
        .method("POST")
        .uri("/api/agent-payments/pay")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"invoice":"lnbc1popenagentsinvoice0000000000000"}"#,
        ))?;
    let rejected_pay_response = app.clone().oneshot(rejected_pay_request).await?;
    assert_eq!(
        rejected_pay_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let rejected_pay_body = read_json(rejected_pay_response).await?;
    assert_eq!(
        rejected_pay_body["error"]["code"],
        json!("max_amount_missing")
    );

    let pay_request = Request::builder()
        .method("POST")
        .uri("/api/agent-payments/pay")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{"invoice":"{payment_request}","maxAmountSats":42}}"#
        )))?;
    let pay_response = app.clone().oneshot(pay_request).await?;
    assert_eq!(pay_response.status(), StatusCode::OK);
    let pay_body = read_json(pay_response).await?;
    assert_eq!(pay_body["data"]["payment"]["status"], json!("completed"));
    let pay_payment_id = pay_body["data"]["payment"]["paymentId"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let pay_preimage = pay_body["data"]["payment"]["preimage"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(pay_payment_id.starts_with("fake:"));
    assert_eq!(pay_preimage.len(), 64);

    let pay_alias_request = Request::builder()
        .method("POST")
        .uri("/api/payments/pay")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{"invoice":"{payment_request}","maxAmountSats":42}}"#
        )))?;
    let pay_alias_response = app.clone().oneshot(pay_alias_request).await?;
    assert_eq!(pay_alias_response.status(), StatusCode::OK);
    let pay_alias_body = read_json(pay_alias_response).await?;
    assert_eq!(
        pay_alias_body["data"]["payment"]["status"],
        json!("completed")
    );
    assert_eq!(
        pay_alias_body["data"]["payment"]["paymentId"],
        json!(pay_payment_id)
    );
    assert_eq!(
        pay_alias_body["data"]["payment"]["preimage"],
        json!(pay_preimage)
    );

    let send_request = Request::builder()
        .method("POST")
        .uri("/api/agent-payments/send-spark")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"sparkAddress":"spark:recipient","amountSats":21}"#,
        ))?;
    let send_response = app.clone().oneshot(send_request).await?;
    assert_eq!(send_response.status(), StatusCode::OK);
    let send_body = read_json(send_response).await?;
    assert_eq!(send_body["data"]["transfer"]["status"], json!("completed"));

    let send_alias_request = Request::builder()
        .method("POST")
        .uri("/api/payments/send-spark")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"sparkAddress":"spark:recipient","amountSats":21}"#,
        ))?;
    let send_alias_response = app.oneshot(send_alias_request).await?;
    assert_eq!(send_alias_response.status(), StatusCode::OK);
    let send_alias_body = read_json(send_alias_response).await?;
    assert_eq!(
        send_alias_body["data"]["transfer"]["status"],
        json!("completed")
    );

    Ok(())
}

#[tokio::test]
async fn agent_payments_fake_payer_is_deterministic_and_cap_guard_rejects_overage() {
    let invoice = "lnbc42n1invoicefixture";
    let payment = super::agent_payments_pay_invoice_fake(
        invoice,
        42_000,
        12_000,
        Some("Sats4Ai.Com".to_string()),
    )
    .await
    .expect("fake payer should succeed");

    let expected_payment_hash = super::sha256_hex(format!("payment:{invoice}").as_bytes());
    let expected_payment_id = format!("fake:{}", &expected_payment_hash[..16]);
    let expected_preimage = super::sha256_hex(format!("preimage:{invoice}").as_bytes());
    assert_eq!(payment.payment_id, Some(expected_payment_id));
    assert_eq!(payment.preimage, expected_preimage);
    assert_eq!(payment.raw["host"], json!("sats4ai.com"));

    let cap_error = super::agent_payments_invoice_cap_guard("lnbc2000n1toobig", 10_000)
        .expect_err("expected over-cap guard failure");
    assert_eq!(cap_error.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(cap_error.code, "quoted_amount_exceeds_cap");
}

#[test]
fn normalize_preimage_hex_handles_hex_and_base64_forms() {
    let hex_input = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let normalized_hex = super::normalize_preimage_hex(hex_input).expect("hex normalization");
    assert_eq!(
        normalized_hex,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );

    let bytes = vec![0x11u8; 32];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let normalized_b64 =
        super::normalize_preimage_hex(&encoded).expect("base64 normalization to hex");
    assert_eq!(normalized_b64, super::bytes_to_hex(&bytes));
}

#[tokio::test]
async fn resend_webhook_rejects_invalid_signature_and_reuses_audit_event_id() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let secret = format!(
        "whsec_{}",
        base64::engine::general_purpose::STANDARD.encode("resend-webhook-test-secret")
    );
    let mut config = test_config(static_dir.path().to_path_buf());
    config.resend_webhook_secret = Some(secret.clone());
    let app = build_router(config);

    let payload = serde_json::json!({
        "type": "email.delivered",
        "created_at": "2026-02-22T00:00:00Z",
        "data": {
            "email_id": "email_invalid_signature",
            "to": ["user@example.com"],
            "tags": [{"name":"integration_id","value":"resend.primary"}]
        }
    })
    .to_string();

    let mut headers = signed_resend_webhook_headers(
        &payload,
        &secret,
        "evt_invalid_signature",
        Utc::now().timestamp(),
    );
    headers.insert("svix-signature", HeaderValue::from_static("v1,invalid"));

    let mut request_builder = Request::builder()
        .method("POST")
        .uri("/api/webhooks/resend")
        .header("content-type", "application/json");
    for (name, value) in &headers {
        request_builder = request_builder.header(name, value);
    }
    let first_request = request_builder.body(Body::from(payload.clone()))?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::UNAUTHORIZED);
    let first_body = read_json(first_response).await?;
    assert_eq!(first_body["error"]["code"], json!("invalid_signature"));
    let first_event_id = first_body["audit"]["event_id"].as_u64().expect("event id");

    let mut replay_builder = Request::builder()
        .method("POST")
        .uri("/api/webhooks/resend")
        .header("content-type", "application/json");
    for (name, value) in &headers {
        replay_builder = replay_builder.header(name, value);
    }
    let replay_request = replay_builder.body(Body::from(payload))?;
    let replay_response = app.oneshot(replay_request).await?;
    assert_eq!(replay_response.status(), StatusCode::UNAUTHORIZED);
    let replay_body = read_json(replay_response).await?;
    assert_eq!(replay_body["error"]["code"], json!("invalid_signature"));
    assert_eq!(replay_body["audit"]["event_id"], json!(first_event_id));

    Ok(())
}

#[tokio::test]
async fn resend_webhook_rejects_stale_timestamp_signature() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let secret = format!(
        "whsec_{}",
        base64::engine::general_purpose::STANDARD.encode("resend-webhook-test-secret")
    );
    let mut config = test_config(static_dir.path().to_path_buf());
    config.resend_webhook_secret = Some(secret.clone());
    config.resend_webhook_tolerance_seconds = 60;
    let app = build_router(config);

    let payload = serde_json::json!({
        "type": "email.delivered",
        "created_at": "2026-02-22T00:00:00Z",
        "data": {
            "email_id": "email_stale_timestamp",
            "to": ["user@example.com"],
            "tags": [{"name":"integration_id","value":"resend.primary"}]
        }
    })
    .to_string();
    let headers = signed_resend_webhook_headers(
        &payload,
        &secret,
        "evt_stale_timestamp",
        Utc::now().timestamp() - 600,
    );

    let mut request_builder = Request::builder()
        .method("POST")
        .uri("/api/webhooks/resend")
        .header("content-type", "application/json");
    for (name, value) in &headers {
        request_builder = request_builder.header(name, value);
    }
    let request = request_builder.body(Body::from(payload))?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let response_body = read_json(response).await?;
    assert_eq!(response_body["error"]["code"], json!("invalid_signature"));

    Ok(())
}

#[tokio::test]
async fn resend_webhook_deduplicates_replays_and_detects_conflicts() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let secret = format!(
        "whsec_{}",
        base64::engine::general_purpose::STANDARD.encode("resend-webhook-test-secret")
    );
    let mut config = test_config(static_dir.path().to_path_buf());
    config.resend_webhook_secret = Some(secret.clone());
    config.runtime_comms_delivery_max_retries = 0;
    let app = build_router(config);

    let delivered_payload = serde_json::json!({
        "type": "email.delivered",
        "created_at": "2026-02-22T00:00:00Z",
        "data": {
            "email_id": "email_conflict_1",
            "to": ["user@example.com"],
            "tags": [{"name":"integration_id","value":"resend.primary"}]
        }
    })
    .to_string();
    let bounced_payload = serde_json::json!({
        "type": "email.bounced",
        "created_at": "2026-02-22T00:00:00Z",
        "data": {
            "email_id": "email_conflict_2",
            "to": ["user@example.com"],
            "reason": "mailbox_not_found",
            "tags": [{"name":"integration_id","value":"resend.primary"}]
        }
    })
    .to_string();
    let headers = signed_resend_webhook_headers(
        &delivered_payload,
        &secret,
        "evt_conflict",
        Utc::now().timestamp(),
    );

    let mut first_builder = Request::builder()
        .method("POST")
        .uri("/api/webhooks/resend")
        .header("content-type", "application/json");
    for (name, value) in &headers {
        first_builder = first_builder.header(name, value);
    }
    let first_request = first_builder.body(Body::from(delivered_payload.clone()))?;
    let first_response = app.clone().oneshot(first_request).await?;
    assert_eq!(first_response.status(), StatusCode::ACCEPTED);
    let first_body = read_json(first_response).await?;
    assert_eq!(first_body["data"]["status"], json!("received"));
    assert_eq!(first_body["data"]["idempotent_replay"], json!(false));
    let event_id = first_body["data"]["event_id"].as_u64().expect("event id");

    let mut replay_builder = Request::builder()
        .method("POST")
        .uri("/api/webhooks/resend")
        .header("content-type", "application/json");
    for (name, value) in &headers {
        replay_builder = replay_builder.header(name, value);
    }
    let replay_request = replay_builder.body(Body::from(delivered_payload))?;
    let replay_response = app.clone().oneshot(replay_request).await?;
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_body = read_json(replay_response).await?;
    assert_eq!(replay_body["data"]["idempotent_replay"], json!(true));
    assert_eq!(replay_body["data"]["event_id"], json!(event_id));

    let conflict_headers = signed_resend_webhook_headers(
        &bounced_payload,
        &secret,
        "evt_conflict",
        Utc::now().timestamp(),
    );
    let mut conflict_builder = Request::builder()
        .method("POST")
        .uri("/api/webhooks/resend")
        .header("content-type", "application/json");
    for (name, value) in &conflict_headers {
        conflict_builder = conflict_builder.header(name, value);
    }
    let conflict_request = conflict_builder.body(Body::from(bounced_payload))?;
    let conflict_response = app.oneshot(conflict_request).await?;
    assert_eq!(conflict_response.status(), StatusCode::CONFLICT);
    let conflict_body = read_json(conflict_response).await?;
    assert_eq!(
        conflict_body["error"]["code"],
        json!("idempotency_conflict")
    );

    Ok(())
}

#[tokio::test]
async fn resend_webhook_forwarding_retries_and_projects_delivery() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let secret = format!(
        "whsec_{}",
        base64::engine::general_purpose::STANDARD.encode("resend-webhook-test-secret")
    );
    let response_statuses = Arc::new(Mutex::new(vec![500u16, 202u16]));
    let captured_payloads = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (runtime_addr, runtime_handle) =
        start_runtime_comms_delivery_stub(response_statuses.clone(), captured_payloads.clone())
            .await?;

    let mut config = test_config(static_dir.path().to_path_buf());
    config.resend_webhook_secret = Some(secret.clone());
    config.runtime_elixir_base_url = Some(format!("http://{runtime_addr}"));
    config.runtime_signing_key = Some("runtime-signing-key".to_string());
    config.runtime_signing_key_id = "runtime-v1".to_string();
    config.runtime_comms_delivery_ingest_path = "/internal/v1/comms/delivery-events".to_string();
    config.runtime_comms_delivery_timeout_ms = 1000;
    config.runtime_comms_delivery_max_retries = 1;
    config.runtime_comms_delivery_retry_backoff_ms = 1;
    let state = test_app_state(config);

    state
        ._domain_store
        .upsert_resend_integration(UpsertResendIntegrationInput {
            user_id: "123".to_string(),
            api_key: "re_runtime_projection_1234567890".to_string(),
            sender_email: Some("noreply@example.com".to_string()),
            sender_name: Some("OpenAgents".to_string()),
        })
        .await
        .expect("seed resend integration");

    let payload = serde_json::json!({
        "type": "email.delivered",
        "created_at": "2026-02-22T00:00:00Z",
        "data": {
            "email_id": "email_projection",
            "to": ["user@example.com"],
            "tags": [
                {"name":"integration_id","value":"resend.primary"},
                {"name":"user_id","value":"123"}
            ]
        }
    })
    .to_string();
    let headers =
        signed_resend_webhook_headers(&payload, &secret, "evt_projection", Utc::now().timestamp());

    let response =
        super::webhooks_resend_store(State(state.clone()), headers, Bytes::from(payload)).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let response_body = read_json(response).await?;
    assert_eq!(response_body["data"]["status"], json!("received"));

    wait_for_webhook_status(&state, "resend:evt_projection", "forwarded").await?;

    let event = state
        ._domain_store
        .webhook_event_by_idempotency_key("resend:evt_projection")
        .await?
        .expect("webhook event");
    assert_eq!(event.status, "forwarded");
    assert_eq!(event.runtime_attempts, 2);
    assert_eq!(event.runtime_status_code, Some(202));
    assert!(event.forwarded_at.is_some());

    let projection = state
        ._domain_store
        .delivery_projection("123", "resend", Some("resend.primary"))
        .await?
        .expect("delivery projection");
    assert_eq!(projection.last_state.as_deref(), Some("delivered"));
    assert_eq!(
        projection.last_message_id.as_deref(),
        Some("email_projection")
    );
    assert_eq!(projection.source, "runtime_forwarder");
    assert_eq!(projection.last_webhook_event_id, Some(event.id));

    let audits = state
        ._domain_store
        .list_integration_audits_for_user("123")
        .await?;
    assert!(
        audits
            .iter()
            .any(|audit| audit.action == "delivery_projection_updated")
    );

    let captured = captured_payloads.lock().await.clone();
    assert_eq!(captured.len(), 2);

    runtime_handle.abort();
    Ok(())
}

#[tokio::test]
async fn resend_webhook_records_forward_retrying_state_before_success() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let secret = format!(
        "whsec_{}",
        base64::engine::general_purpose::STANDARD.encode("resend-webhook-test-secret")
    );
    let response_statuses = Arc::new(Mutex::new(vec![500u16, 202u16]));
    let captured_payloads = Arc::new(Mutex::new(Vec::<Value>::new()));
    let (runtime_addr, runtime_handle) =
        start_runtime_comms_delivery_stub(response_statuses.clone(), captured_payloads.clone())
            .await?;

    let mut config = test_config(static_dir.path().to_path_buf());
    config.resend_webhook_secret = Some(secret.clone());
    config.runtime_elixir_base_url = Some(format!("http://{runtime_addr}"));
    config.runtime_signing_key = Some("runtime-signing-key".to_string());
    config.runtime_signing_key_id = "runtime-v1".to_string();
    config.runtime_comms_delivery_ingest_path = "/internal/v1/comms/delivery-events".to_string();
    config.runtime_comms_delivery_timeout_ms = 1000;
    config.runtime_comms_delivery_max_retries = 1;
    config.runtime_comms_delivery_retry_backoff_ms = 250;
    let state = test_app_state(config);

    state
        ._domain_store
        .upsert_resend_integration(UpsertResendIntegrationInput {
            user_id: "456".to_string(),
            api_key: "re_runtime_projection_9876543210".to_string(),
            sender_email: Some("noreply@example.com".to_string()),
            sender_name: Some("OpenAgents".to_string()),
        })
        .await
        .expect("seed resend integration");

    let payload = serde_json::json!({
        "type": "email.delivered",
        "created_at": "2026-02-22T00:00:00Z",
        "data": {
            "email_id": "email_transition",
            "to": ["user@example.com"],
            "tags": [
                {"name":"integration_id","value":"resend.primary"},
                {"name":"user_id","value":"456"}
            ]
        }
    })
    .to_string();
    let headers = signed_resend_webhook_headers(
        &payload,
        &secret,
        "evt_retry_transition",
        Utc::now().timestamp(),
    );

    let response =
        super::webhooks_resend_store(State(state.clone()), headers, Bytes::from(payload)).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let mut saw_retrying = false;
    for _ in 0..80 {
        if let Some(event) = state
            ._domain_store
            .webhook_event_by_idempotency_key("resend:evt_retry_transition")
            .await?
        {
            if event.status == "forward_retrying" {
                saw_retrying = true;
                break;
            }
            if event.status == "forwarded" {
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(saw_retrying);

    wait_for_webhook_status(&state, "resend:evt_retry_transition", "forwarded").await?;
    let event = state
        ._domain_store
        .webhook_event_by_idempotency_key("resend:evt_retry_transition")
        .await?
        .expect("webhook event");
    assert_eq!(event.runtime_attempts, 2);
    assert_eq!(event.status, "forwarded");

    runtime_handle.abort();
    Ok(())
}

#[tokio::test]
async fn shouts_create_list_and_zones_match_contract() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let author_token = seed_local_test_token(&config, "shout-author@openagents.com").await?;
    let other_token = seed_local_test_token(&config, "shout-other@openagents.com").await?;
    let app = build_router(config.clone());

    let unauthorized_request = Request::builder()
        .method("POST")
        .uri("/api/shouts")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"body":"unauthorized"}"#))?;
    let unauthorized_response = app.clone().oneshot(unauthorized_request).await?;
    assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);

    let create_l402_request = Request::builder()
        .method("POST")
        .uri("/api/shouts")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {author_token}"))
        .body(Body::from(
            r#"{"body":"L402 payment shipped","zone":"L402"}"#,
        ))?;
    let create_l402_response = app.clone().oneshot(create_l402_request).await?;
    assert_eq!(create_l402_response.status(), StatusCode::CREATED);
    let create_l402_body = read_json(create_l402_response).await?;
    assert_eq!(create_l402_body["data"]["zone"], json!("l402"));
    assert_eq!(
        create_l402_body["data"]["body"],
        json!("L402 payment shipped")
    );
    assert_eq!(
        create_l402_body["data"]["author"]["handle"],
        json!("shout-author")
    );

    let create_text_alias_request = Request::builder()
        .method("POST")
        .uri("/api/shouts")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {author_token}"))
        .body(Body::from(
            r#"{"text":"compat text alias shout","zone":"Global"}"#,
        ))?;
    let create_text_alias_response = app.clone().oneshot(create_text_alias_request).await?;
    assert_eq!(create_text_alias_response.status(), StatusCode::CREATED);
    let create_text_alias_body = read_json(create_text_alias_response).await?;
    assert_eq!(create_text_alias_body["data"]["zone"], json!("global"));
    assert_eq!(
        create_text_alias_body["data"]["body"],
        json!("compat text alias shout")
    );

    let create_global_request = Request::builder()
        .method("POST")
        .uri("/api/shouts")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {other_token}"))
        .body(Body::from(
            r#"{"body":"hello from global","zone":"global"}"#,
        ))?;
    let create_global_response = app.clone().oneshot(create_global_request).await?;
    assert_eq!(create_global_response.status(), StatusCode::CREATED);

    let list_l402_request = Request::builder()
        .method("GET")
        .uri("/api/shouts?zone=l402")
        .body(Body::empty())?;
    let list_l402_response = app.clone().oneshot(list_l402_request).await?;
    assert_eq!(list_l402_response.status(), StatusCode::OK);
    let list_l402_body = read_json(list_l402_response).await?;
    assert_eq!(
        list_l402_body["data"].as_array().map(|rows| rows.len()),
        Some(1)
    );
    assert_eq!(list_l402_body["data"][0]["zone"], json!("l402"));

    let zones_request = Request::builder()
        .method("GET")
        .uri("/api/shouts/zones")
        .body(Body::empty())?;
    let zones_response = app.oneshot(zones_request).await?;
    assert_eq!(zones_response.status(), StatusCode::OK);
    let zones_body = read_json(zones_response).await?;
    assert_eq!(zones_body["data"][0]["zone"], json!("global"));
    assert_eq!(zones_body["data"][0]["count24h"], json!(2));

    Ok(())
}

#[tokio::test]
async fn shouts_feed_caps_limit_and_supports_before_id_pagination() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let token = seed_local_test_token(&config, "shout-pagination@openagents.com").await?;
    let app = build_router(config.clone());
    for idx in 0..205 {
        let create_request = Request::builder()
            .method("POST")
            .uri("/api/shouts")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(
                r#"{{"body":"feed-{idx}","zone":"global"}}"#
            )))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
    }

    let page_one_request = Request::builder()
        .method("GET")
        .uri("/api/shouts?limit=999")
        .body(Body::empty())?;
    let page_one_response = app.clone().oneshot(page_one_request).await?;
    assert_eq!(page_one_response.status(), StatusCode::OK);
    let page_one_body = read_json(page_one_response).await?;
    assert_eq!(
        page_one_body["data"].as_array().map(|rows| rows.len()),
        Some(200)
    );
    let last_id = page_one_body["data"][199]["id"]
        .as_u64()
        .expect("expected cursor id");
    assert_eq!(
        page_one_body["meta"]["nextCursor"],
        json!(last_id.to_string())
    );

    let page_two_request = Request::builder()
        .method("GET")
        .uri(format!("/api/shouts?limit=200&before_id={last_id}"))
        .body(Body::empty())?;
    let page_two_response = app.oneshot(page_two_request).await?;
    assert_eq!(page_two_response.status(), StatusCode::OK);
    let page_two_body = read_json(page_two_response).await?;
    assert_eq!(
        page_two_body["data"].as_array().map(|rows| rows.len()),
        Some(5)
    );

    Ok(())
}

#[tokio::test]
async fn feed_page_renders_html_and_respects_zone_filters() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let author_token = seed_local_test_token(&config, "feed-author@openagents.com").await?;
    let app = build_router(config.clone());

    for payload in [
        r#"{"body":"L402-only shout body","zone":"l402"}"#,
        r#"{"body":"Dev-only shout body","zone":"dev"}"#,
        r#"{"body":"Global shout body"}"#,
    ] {
        let create_request = Request::builder()
            .method("POST")
            .uri("/api/shouts")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {author_token}"))
            .body(Body::from(payload.to_string()))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
    }

    let zone_request = Request::builder()
        .method("GET")
        .uri("/feed?zone=l402")
        .body(Body::empty())?;
    let zone_response = app.clone().oneshot(zone_request).await?;
    assert_eq!(zone_response.status(), StatusCode::OK);
    let zone_body = zone_response.into_body().collect().await?.to_bytes();
    let zone_html = String::from_utf8_lossy(&zone_body);
    assert!(zone_html.contains("rust shell"));
    assert!(zone_html.contains("L402-only shout body"));
    assert!(!zone_html.contains("Dev-only shout body"));

    let all_request = Request::builder()
        .method("GET")
        .uri("/feed?zone=all&limit=999")
        .body(Body::empty())?;
    let all_response = app.clone().oneshot(all_request).await?;
    assert_eq!(all_response.status(), StatusCode::OK);
    let all_body = all_response.into_body().collect().await?.to_bytes();
    let all_html = String::from_utf8_lossy(&all_body);
    assert!(all_html.contains("rust shell"));
    assert!(all_html.contains("L402-only shout body"));
    assert!(all_html.contains("Dev-only shout body"));
    assert!(all_html.contains("Global shout body"));

    let invalid_since_request = Request::builder()
        .method("GET")
        .uri("/feed?since=not-a-date")
        .body(Body::empty())?;
    let invalid_since_response = app.oneshot(invalid_since_request).await?;
    assert_eq!(
        invalid_since_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let invalid_since_body = read_json(invalid_since_response).await?;
    assert_eq!(
        invalid_since_body["error"]["code"],
        json!("invalid_request")
    );
    assert_eq!(
        invalid_since_body["errors"]["since"],
        json!(["The since field must be a valid date."])
    );

    Ok(())
}

#[tokio::test]
async fn feed_page_without_inertia_header_serves_rust_shell() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.route_split_mode = "rust".to_string();
    config.route_split_rust_routes = vec!["/".to_string()];
    let app = build_router(config);

    let request = Request::builder().uri("/feed").body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await?.to_bytes();
    let html = String::from_utf8_lossy(&body);
    assert!(html.contains("rust shell"));

    Ok(())
}

#[tokio::test]
async fn feed_main_fragment_zone_transitions_support_named_and_all() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let token = seed_local_test_token(&config, "feed-fragment-zone@openagents.com").await?;
    let app = build_router(config);

    for payload in [
        r#"{"body":"Zone-L402-only","zone":"l402"}"#,
        r#"{"body":"Zone-Dev-only","zone":"dev"}"#,
        r#"{"body":"Zone-Global-only"}"#,
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/api/shouts")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(payload.to_string()))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let named_zone_request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/main?zone=l402")
        .header("hx-request", "true")
        .body(Body::empty())?;
    let named_zone_response = app.clone().oneshot(named_zone_request).await?;
    assert_eq!(named_zone_response.status(), StatusCode::OK);
    let named_zone_body = read_text(named_zone_response).await?;
    assert!(named_zone_body.contains("id=\"feed-main-panel\""));
    assert!(named_zone_body.contains("id=\"feed-zone-panel\""));
    assert!(named_zone_body.contains("hx-swap-oob=\"outerHTML\""));
    assert!(named_zone_body.contains("Zone-L402-only"));
    assert!(!named_zone_body.contains("Zone-Dev-only"));
    assert!(!named_zone_body.contains("Zone-Global-only"));

    let all_zone_request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/main?zone=all")
        .header("hx-request", "true")
        .body(Body::empty())?;
    let all_zone_response = app.clone().oneshot(all_zone_request).await?;
    assert_eq!(all_zone_response.status(), StatusCode::OK);
    let all_zone_body = read_text(all_zone_response).await?;
    assert!(all_zone_body.contains("Zone-L402-only"));
    assert!(all_zone_body.contains("Zone-Dev-only"));
    assert!(all_zone_body.contains("Zone-Global-only"));

    let non_hx_request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/main?zone=l402")
        .body(Body::empty())?;
    let non_hx_response = app.clone().oneshot(non_hx_request).await?;
    assert_eq!(non_hx_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        non_hx_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/feed?zone=l402")
    );

    let direct_request = Request::builder()
        .method("GET")
        .uri("/feed?zone=l402")
        .body(Body::empty())?;
    let direct_response = app.oneshot(direct_request).await?;
    assert_eq!(direct_response.status(), StatusCode::OK);
    let direct_body = read_text(direct_response).await?;
    assert!(direct_body.contains("Zone-L402-only"));
    assert!(!direct_body.contains("Zone-Dev-only"));
    assert!(!direct_body.contains("Zone-Global-only"));

    Ok(())
}

#[tokio::test]
async fn feed_items_fragment_supports_multi_page_loading_without_duplicates() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let token = seed_local_test_token(&config, "feed-pagination-multi@openagents.com").await?;
    let app = build_router(config);

    for idx in 0..120 {
        let request = Request::builder()
            .method("POST")
            .uri("/api/shouts")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(
                r#"{{"body":"inc-{idx}","zone":"global"}}"#
            )))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let main_request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/main?zone=all&limit=50")
        .header("hx-request", "true")
        .body(Body::empty())?;
    let main_response = app.clone().oneshot(main_request).await?;
    assert_eq!(main_response.status(), StatusCode::OK);
    let main_body = read_text(main_response).await?;
    assert!(main_body.contains("inc-119"));
    assert!(main_body.contains("inc-70"));
    assert!(!main_body.contains("inc-69"));
    let cursor_one = main_body
        .split("before_id=")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .unwrap_or_default()
        .to_string();
    assert!(!cursor_one.is_empty());

    let page_two_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/feed/fragments/items?zone=all&limit=50&before_id={cursor_one}"
        ))
        .header("hx-request", "true")
        .body(Body::empty())?;
    let page_two_response = app.clone().oneshot(page_two_request).await?;
    assert_eq!(page_two_response.status(), StatusCode::OK);
    let page_two_body = read_text(page_two_response).await?;
    assert!(!page_two_body.contains("inc-119"));
    assert!(page_two_body.contains("inc-69"));
    assert!(page_two_body.contains("inc-20"));
    assert!(!page_two_body.contains("inc-19"));
    let cursor_two = page_two_body
        .split("before_id=")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .unwrap_or_default()
        .to_string();
    assert!(!cursor_two.is_empty());

    let page_three_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/feed/fragments/items?zone=all&limit=50&before_id={cursor_two}"
        ))
        .header("hx-request", "true")
        .body(Body::empty())?;
    let page_three_response = app.oneshot(page_three_request).await?;
    assert_eq!(page_three_response.status(), StatusCode::OK);
    let page_three_body = read_text(page_three_response).await?;
    assert!(page_three_body.contains("inc-19"));
    assert!(page_three_body.contains("inc-0"));
    assert!(!page_three_body.contains("inc-20"));
    assert!(page_three_body.contains("No more items."));
    assert!(!page_three_body.contains("before_id="));

    Ok(())
}

#[tokio::test]
async fn feed_items_fragment_partial_page_returns_no_more_marker() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let token = seed_local_test_token(&config, "feed-pagination-partial@openagents.com").await?;
    let app = build_router(config);

    for idx in 0..60 {
        let request = Request::builder()
            .method("POST")
            .uri("/api/shouts")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .body(Body::from(format!(
                r#"{{"body":"partial-{idx}","zone":"global"}}"#
            )))?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    let main_request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/main?zone=all&limit=50")
        .header("hx-request", "true")
        .body(Body::empty())?;
    let main_response = app.clone().oneshot(main_request).await?;
    let main_body = read_text(main_response).await?;
    let cursor = main_body
        .split("before_id=")
        .nth(1)
        .and_then(|tail| tail.split('"').next())
        .unwrap_or_default()
        .to_string();
    assert!(!cursor.is_empty());

    let partial_request = Request::builder()
        .method("GET")
        .uri(format!(
            "/feed/fragments/items?zone=all&limit=50&before_id={cursor}"
        ))
        .header("hx-request", "true")
        .body(Body::empty())?;
    let partial_response = app.oneshot(partial_request).await?;
    assert_eq!(partial_response.status(), StatusCode::OK);
    let partial_body = read_text(partial_response).await?;
    assert!(partial_body.contains("partial-9"));
    assert!(partial_body.contains("partial-0"));
    assert!(partial_body.contains("No more items."));
    assert!(!partial_body.contains("before_id="));

    Ok(())
}

#[tokio::test]
async fn feed_items_fragment_empty_result_is_explicit() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/items?zone=all&limit=50&before_id=999999")
        .header("hx-request", "true")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_text(response).await?;
    assert!(body.contains("No more items."));
    assert!(!body.contains("oa-feed-item"));
    assert!(!body.contains("before_id="));

    Ok(())
}

#[tokio::test]
async fn web_feed_shout_hx_success_emits_trigger_and_refreshes_list() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let token = seed_local_test_token(&config, "feed-shout-hx@openagents.com").await?;
    let app = build_router(config);

    let post_request = Request::builder()
        .method("POST")
        .uri("/feed/shout")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("body=HTMX+zone+post&zone=l402"))?;
    let post_response = app.clone().oneshot(post_request).await?;
    assert_eq!(post_response.status(), StatusCode::OK);
    assert_eq!(
        post_response
            .headers()
            .get("HX-Trigger")
            .and_then(|value| value.to_str().ok()),
        Some("feed-shout-posted")
    );
    let post_body = read_text(post_response).await?;
    assert!(post_body.contains("id=\"feed-status\""));
    assert!(post_body.contains("Shout posted."));

    let fragment_request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/main?zone=l402")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())?;
    let fragment_response = app.oneshot(fragment_request).await?;
    assert_eq!(fragment_response.status(), StatusCode::OK);
    let fragment_body = read_text(fragment_response).await?;
    assert!(fragment_body.contains("HTMX zone post"));
    assert!(fragment_body.contains("id=\"feed-main-panel\""));
    assert!(!fragment_body.contains("<html"));

    Ok(())
}

#[tokio::test]
async fn web_feed_shout_hx_validation_errors_render_inline_without_trigger() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let token = seed_local_test_token(&config, "feed-shout-hx-errors@openagents.com").await?;
    let app = build_router(config);

    let empty_request = Request::builder()
        .method("POST")
        .uri("/feed/shout")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("body=%20%20"))?;
    let empty_response = app.clone().oneshot(empty_request).await?;
    assert_eq!(empty_response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(empty_response.headers().get("HX-Trigger").is_none());
    let empty_body = read_text(empty_response).await?;
    assert!(empty_body.contains("Message body cannot be empty."));

    let invalid_zone_request = Request::builder()
        .method("POST")
        .uri("/feed/shout")
        .header("content-type", "application/x-www-form-urlencoded")
        .header("hx-request", "true")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from("body=hello&zone=bad%20zone"))?;
    let invalid_zone_response = app.oneshot(invalid_zone_request).await?;
    assert_eq!(
        invalid_zone_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert!(invalid_zone_response.headers().get("HX-Trigger").is_none());
    let invalid_zone_body = read_text(invalid_zone_response).await?;
    assert!(invalid_zone_body.contains("Zone format is invalid."));

    Ok(())
}

#[tokio::test]
async fn feed_page_hx_boost_request_returns_main_fragment() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let app = build_router(test_config(static_dir.path().to_path_buf()));

    let request = Request::builder()
        .method("GET")
        .uri("/feed")
        .header("hx-request", "true")
        .header("hx-boosted", "true")
        .body(Body::empty())?;
    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(content_type.starts_with("text/html"));

    let body = response.into_body().collect().await?.to_bytes();
    let html = String::from_utf8_lossy(&body);
    assert!(html.starts_with("<main id=\"oa-main-shell\""));
    assert!(html.contains("oa-feed"));
    assert!(!html.contains("<html"));
    assert!(!html.contains("<head"));
    assert!(!html.contains("<body"));

    Ok(())
}

#[tokio::test]
async fn htmx_get_routes_return_fragment_while_direct_get_returns_shell() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let app = build_router(config);

    for path in ["/", "/login", "/feed"] {
        let hx_request = Request::builder()
            .method("GET")
            .uri(path)
            .header("hx-request", "true")
            .header("hx-boosted", "true")
            .header("hx-target", "oa-main-shell")
            .body(Body::empty())?;
        let hx_response = app.clone().oneshot(hx_request).await?;
        assert_eq!(hx_response.status(), StatusCode::OK);
        let hx_body = hx_response.into_body().collect().await?.to_bytes();
        let hx_html = String::from_utf8_lossy(&hx_body);
        assert!(hx_html.starts_with("<main id=\"oa-main-shell\""));
        assert!(!hx_html.contains("<html"));

        let direct_request = Request::builder()
            .method("GET")
            .uri(path)
            .body(Body::empty())?;
        let direct_response = app.clone().oneshot(direct_request).await?;
        assert_eq!(direct_response.status(), StatusCode::OK);
        let direct_body = direct_response.into_body().collect().await?.to_bytes();
        let direct_html = String::from_utf8_lossy(&direct_body);
        assert!(direct_html.contains("<html"));
        assert!(direct_html.contains("id=\"oa-shell\""));
        assert!(direct_html.contains("id=\"oa-main-shell\""));
    }

    Ok(())
}

#[tokio::test]
async fn htmx_route_group_override_can_force_full_page_mode_per_domain() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.route_split_mode = "rust".to_string();
    config.route_split_rust_routes = vec!["/".to_string()];
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "routes@openagents.com").await?;

    let override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/route-split/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{"target":"htmx_full_page","domain":"chat_pilot"}"#,
        ))?;
    let override_response = app.clone().oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);

    let hx_feed_request = Request::builder()
        .method("GET")
        .uri("/feed")
        .header("hx-request", "true")
        .header("hx-boosted", "true")
        .header("hx-target", "oa-main-shell")
        .body(Body::empty())?;
    let hx_feed_response = app.clone().oneshot(hx_feed_request).await?;
    assert_eq!(hx_feed_response.status(), StatusCode::OK);
    assert_eq!(
        hx_feed_response
            .headers()
            .get("HX-Redirect")
            .and_then(|value| value.to_str().ok()),
        Some("/feed")
    );

    let full_feed_request = Request::builder()
        .method("GET")
        .uri("/feed")
        .body(Body::empty())?;
    let full_feed_response = app.clone().oneshot(full_feed_request).await?;
    assert_eq!(full_feed_response.status(), StatusCode::OK);
    let full_feed_body = full_feed_response.into_body().collect().await?.to_bytes();
    let full_feed_html = String::from_utf8_lossy(&full_feed_body);
    assert!(full_feed_html.contains("<html"));
    assert!(full_feed_html.contains("id=\"oa-shell\" hx-disable=\"true\""));
    assert!(full_feed_html.contains("name=\"openagents-htmx-mode\" content=\"full_page\""));

    let fragment_request = Request::builder()
        .method("GET")
        .uri("/feed/fragments/main?zone=all")
        .header("hx-request", "true")
        .body(Body::empty())?;
    let fragment_response = app.clone().oneshot(fragment_request).await?;
    assert_eq!(fragment_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        fragment_response
            .headers()
            .get("location")
            .and_then(|value| value.to_str().ok()),
        Some("/feed?zone=all")
    );

    Ok(())
}

#[tokio::test]
async fn htmx_history_restore_preserves_feed_zone_and_status_queries() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let author_token = seed_local_test_token(&config, "history-author@openagents.com").await?;
    let app = build_router(config);

    for payload in [
        r#"{"body":"History-L402 shout","zone":"l402"}"#,
        r#"{"body":"History-Dev shout","zone":"dev"}"#,
    ] {
        let create_request = Request::builder()
            .method("POST")
            .uri("/api/shouts")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {author_token}"))
            .body(Body::from(payload.to_string()))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
    }

    let l402_request = Request::builder()
        .method("GET")
        .uri("/feed?zone=l402&status=shout-posted")
        .header("hx-request", "true")
        .header("hx-boosted", "true")
        .header("hx-target", "oa-main-shell")
        .body(Body::empty())?;
    let l402_response = app.clone().oneshot(l402_request).await?;
    assert_eq!(l402_response.status(), StatusCode::OK);
    let l402_body = l402_response.into_body().collect().await?.to_bytes();
    let l402_html = String::from_utf8_lossy(&l402_body);
    assert!(l402_html.contains("History-L402 shout"));
    assert!(!l402_html.contains("History-Dev shout"));
    assert!(l402_html.contains("Shout posted."));

    let dev_request = Request::builder()
        .method("GET")
        .uri("/feed?zone=dev")
        .header("hx-request", "true")
        .header("hx-boosted", "true")
        .header("hx-target", "oa-main-shell")
        .body(Body::empty())?;
    let dev_response = app.clone().oneshot(dev_request).await?;
    assert_eq!(dev_response.status(), StatusCode::OK);
    let dev_body = dev_response.into_body().collect().await?.to_bytes();
    let dev_html = String::from_utf8_lossy(&dev_body);
    assert!(!dev_html.contains("History-L402 shout"));
    assert!(dev_html.contains("History-Dev shout"));

    let history_restore_request = Request::builder()
        .method("GET")
        .uri("/feed?zone=l402&status=shout-posted")
        .header("hx-request", "true")
        .header("hx-history-restore-request", "true")
        .header("hx-target", "oa-main-shell")
        .body(Body::empty())?;
    let history_restore_response = app.clone().oneshot(history_restore_request).await?;
    assert_eq!(history_restore_response.status(), StatusCode::OK);
    let history_restore_body = history_restore_response
        .into_body()
        .collect()
        .await?
        .to_bytes();
    let history_restore_html = String::from_utf8_lossy(&history_restore_body);
    assert!(history_restore_html.starts_with("<main id=\"oa-main-shell\""));
    assert!(history_restore_html.contains("History-L402 shout"));
    assert!(!history_restore_html.contains("History-Dev shout"));
    assert!(history_restore_html.contains("Shout posted."));

    let refresh_request = Request::builder()
        .method("GET")
        .uri("/feed?zone=l402&status=shout-posted")
        .body(Body::empty())?;
    let refresh_response = app.oneshot(refresh_request).await?;
    assert_eq!(refresh_response.status(), StatusCode::OK);
    let refresh_body = refresh_response.into_body().collect().await?.to_bytes();
    let refresh_html = String::from_utf8_lossy(&refresh_body);
    assert!(refresh_html.contains("<html"));
    assert!(refresh_html.contains("History-L402 shout"));
    assert!(refresh_html.contains("Shout posted."));

    Ok(())
}

#[tokio::test]
async fn html_routes_include_csp_and_security_headers() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));
    let app = build_router(config);

    for path in ["/", "/login", "/feed"] {
        let request = Request::builder().uri(path).body(Body::empty())?;
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), StatusCode::OK);

        assert_eq!(
            response
                .headers()
                .get(super::HEADER_CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some(super::HTML_CONTENT_SECURITY_POLICY)
        );
        assert_eq!(
            response
                .headers()
                .get(super::HEADER_X_CONTENT_TYPE_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some(super::X_CONTENT_TYPE_OPTIONS_NOSNIFF)
        );
        assert_eq!(
            response
                .headers()
                .get(super::HEADER_REFERRER_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some(super::HTML_REFERRER_POLICY)
        );
        assert_eq!(
            response
                .headers()
                .get(super::HEADER_X_FRAME_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some(super::HTML_X_FRAME_OPTIONS)
        );
        assert_eq!(
            response
                .headers()
                .get(super::HEADER_PERMISSIONS_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some(super::HTML_PERMISSIONS_POLICY)
        );
    }

    Ok(())
}

#[tokio::test]
async fn whispers_create_list_and_read_match_contract() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let sender_token = seed_local_test_token(&config, "whisper-sender@openagents.com").await?;
    let recipient_token =
        seed_local_test_token(&config, "whisper-recipient@openagents.com").await?;
    let third_token = seed_local_test_token(&config, "whisper-third@openagents.com").await?;
    let app = build_router(config.clone());

    let unauthorized_request = Request::builder()
        .method("POST")
        .uri("/api/whispers")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"recipientHandle":"whisper-recipient","body":"unauthorized"}"#,
        ))?;
    let unauthorized_response = app.clone().oneshot(unauthorized_request).await?;
    assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);

    let create_request = Request::builder()
        .method("POST")
        .uri("/api/whispers")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {sender_token}"))
        .body(Body::from(
            r#"{"recipientHandle":"whisper-recipient","body":"hey"}"#,
        ))?;
    let create_response = app.clone().oneshot(create_request).await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await?;
    assert_eq!(create_body["data"]["body"], json!("hey"));
    assert_eq!(
        create_body["data"]["sender"]["handle"],
        json!("whisper-sender")
    );
    assert_eq!(
        create_body["data"]["recipient"]["handle"],
        json!("whisper-recipient")
    );
    assert_eq!(create_body["data"]["readAt"], json!(null));
    let whisper_id = create_body["data"]["id"]
        .as_u64()
        .expect("expected whisper id");

    let sender_list_request = Request::builder()
        .method("GET")
        .uri("/api/whispers?with=whisper-recipient")
        .header("authorization", format!("Bearer {sender_token}"))
        .body(Body::empty())?;
    let sender_list_response = app.clone().oneshot(sender_list_request).await?;
    assert_eq!(sender_list_response.status(), StatusCode::OK);
    let sender_list_body = read_json(sender_list_response).await?;
    assert_eq!(
        sender_list_body["data"].as_array().map(|rows| rows.len()),
        Some(1)
    );
    assert_eq!(sender_list_body["meta"]["with"], json!("whisper-recipient"));

    let recipient_list_request = Request::builder()
        .method("GET")
        .uri("/api/whispers?with=whisper-sender")
        .header("authorization", format!("Bearer {recipient_token}"))
        .body(Body::empty())?;
    let recipient_list_response = app.clone().oneshot(recipient_list_request).await?;
    assert_eq!(recipient_list_response.status(), StatusCode::OK);
    let recipient_list_body = read_json(recipient_list_response).await?;
    assert_eq!(
        recipient_list_body["data"]
            .as_array()
            .map(|rows| rows.len()),
        Some(1)
    );

    let mark_read_request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/whispers/{whisper_id}/read"))
        .header("authorization", format!("Bearer {recipient_token}"))
        .body(Body::empty())?;
    let mark_read_response = app.clone().oneshot(mark_read_request).await?;
    assert_eq!(mark_read_response.status(), StatusCode::OK);
    let mark_read_body = read_json(mark_read_response).await?;
    assert!(mark_read_body["data"]["readAt"].as_str().is_some());

    let sender_mark_read_request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/whispers/{whisper_id}/read"))
        .header("authorization", format!("Bearer {sender_token}"))
        .body(Body::empty())?;
    let sender_mark_read_response = app.clone().oneshot(sender_mark_read_request).await?;
    assert_eq!(sender_mark_read_response.status(), StatusCode::FORBIDDEN);

    let third_mark_read_request = Request::builder()
        .method("PATCH")
        .uri(format!("/api/whispers/{whisper_id}/read"))
        .header("authorization", format!("Bearer {third_token}"))
        .body(Body::empty())?;
    let third_mark_read_response = app.oneshot(third_mark_read_request).await?;
    assert_eq!(third_mark_read_response.status(), StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn whispers_pagination_and_validation_edges_match_contract() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.auth_store_path = Some(static_dir.path().join("auth-store.json"));
    config.domain_store_path = Some(static_dir.path().join("domain-store.json"));

    let sender_token = seed_local_test_token(&config, "whisper-page-sender@openagents.com").await?;
    let recipient_token =
        seed_local_test_token(&config, "whisper-page-recipient@openagents.com").await?;
    let app = build_router(config.clone());

    let recipient_id = authenticated_user_id(app.clone(), &recipient_token).await?;
    for idx in 0..205 {
        let create_request = Request::builder()
            .method("POST")
            .uri("/api/whispers")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {sender_token}"))
            .body(Body::from(format!(
                r#"{{"recipientId":"{recipient_id}","body":"feed-{idx}"}}"#
            )))?;
        let create_response = app.clone().oneshot(create_request).await?;
        assert_eq!(create_response.status(), StatusCode::CREATED);
    }

    let page_one_request = Request::builder()
        .method("GET")
        .uri("/api/whispers?limit=999")
        .header("authorization", format!("Bearer {recipient_token}"))
        .body(Body::empty())?;
    let page_one_response = app.clone().oneshot(page_one_request).await?;
    assert_eq!(page_one_response.status(), StatusCode::OK);
    let page_one_body = read_json(page_one_response).await?;
    assert_eq!(
        page_one_body["data"].as_array().map(|rows| rows.len()),
        Some(200)
    );
    let last_id = page_one_body["data"][199]["id"]
        .as_u64()
        .expect("expected cursor id");
    assert_eq!(
        page_one_body["meta"]["nextCursor"],
        json!(last_id.to_string())
    );

    let page_two_request = Request::builder()
        .method("GET")
        .uri(format!("/api/whispers?limit=200&before_id={last_id}"))
        .header("authorization", format!("Bearer {recipient_token}"))
        .body(Body::empty())?;
    let page_two_response = app.clone().oneshot(page_two_request).await?;
    assert_eq!(page_two_response.status(), StatusCode::OK);
    let page_two_body = read_json(page_two_response).await?;
    assert_eq!(
        page_two_body["data"].as_array().map(|rows| rows.len()),
        Some(5)
    );

    let invalid_with_request = Request::builder()
        .method("GET")
        .uri("/api/whispers?with=bad*handle")
        .header("authorization", format!("Bearer {recipient_token}"))
        .body(Body::empty())?;
    let invalid_with_response = app.clone().oneshot(invalid_with_request).await?;
    assert_eq!(
        invalid_with_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let missing_recipient_request = Request::builder()
        .method("POST")
        .uri("/api/whispers")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {sender_token}"))
        .body(Body::from(r#"{"body":"no recipient"}"#))?;
    let missing_recipient_response = app.clone().oneshot(missing_recipient_request).await?;
    assert_eq!(
        missing_recipient_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let both_recipient_fields_request = Request::builder()
            .method("POST")
            .uri("/api/whispers")
            .header("content-type", "application/json")
            .header("authorization", format!("Bearer {sender_token}"))
            .body(Body::from(format!(
                r#"{{"recipientId":"{recipient_id}","recipientHandle":"whisper-page-recipient","body":"mutually exclusive"}}"#
            )))?;
    let both_recipient_fields_response = app.clone().oneshot(both_recipient_fields_request).await?;
    assert_eq!(
        both_recipient_fields_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let sender_id = authenticated_user_id(app.clone(), &sender_token).await?;
    let self_whisper_request = Request::builder()
        .method("POST")
        .uri("/api/whispers")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {sender_token}"))
        .body(Body::from(format!(
            r#"{{"recipientId":"{sender_id}","body":"self whisper"}}"#
        )))?;
    let self_whisper_response = app.clone().oneshot(self_whisper_request).await?;
    assert_eq!(
        self_whisper_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let invalid_id_read_request = Request::builder()
        .method("PATCH")
        .uri("/api/whispers/not-a-number/read")
        .header("authorization", format!("Bearer {recipient_token}"))
        .body(Body::empty())?;
    let invalid_id_read_response = app.clone().oneshot(invalid_id_read_request).await?;
    assert_eq!(invalid_id_read_response.status(), StatusCode::NOT_FOUND);

    let missing_read_request = Request::builder()
        .method("PATCH")
        .uri("/api/whispers/999999/read")
        .header("authorization", format!("Bearer {recipient_token}"))
        .body(Body::empty())?;
    let missing_read_response = app.oneshot(missing_read_request).await?;
    assert_eq!(missing_read_response.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn runtime_routing_force_legacy_wins_over_user_override() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.admin_emails = vec!["runtime-router@openagents.com".to_string()];
    config.runtime_driver = "elixir".to_string();
    config.runtime_force_legacy = true;
    config.runtime_overrides_enabled = true;
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "runtime-router@openagents.com").await?;
    let user_id = authenticated_user_id(app.clone(), &token).await?;

    let override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/runtime-routing/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{
                    "scope_type":"user",
                    "scope_id":"{user_id}",
                    "driver":"elixir",
                    "is_active":true,
                    "reason":"canary"
                }}"#
        )))?;
    let override_response = app.clone().oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);

    let evaluate_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/runtime-routing/evaluate")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"thread_id":"thread-runtime-1"}"#))?;
    let evaluate_response = app.oneshot(evaluate_request).await?;
    assert_eq!(evaluate_response.status(), StatusCode::OK);
    let body = read_json(evaluate_response).await?;
    assert_eq!(body["data"]["driver"], json!("legacy"));
    assert_eq!(body["data"]["reason"], json!("force_legacy"));

    Ok(())
}

#[tokio::test]
async fn runtime_routing_applies_autopilot_override_from_thread_binding() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut config = test_config(static_dir.path().to_path_buf());
    config.admin_emails = vec!["runtime-router@openagents.com".to_string()];
    config.runtime_driver = "legacy".to_string();
    config.runtime_force_legacy = false;
    config.runtime_overrides_enabled = true;
    let app = build_router(config);
    let token = authenticate_token(app.clone(), "runtime-router@openagents.com").await?;

    let create_autopilot_request = Request::builder()
        .method("POST")
        .uri("/api/autopilots")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(
            r#"{
                    "displayName":"Runtime Override Pilot",
                    "handle":"runtime-override-pilot",
                    "status":"active",
                    "visibility":"private"
                }"#,
        ))?;
    let create_autopilot_response = app.clone().oneshot(create_autopilot_request).await?;
    assert_eq!(create_autopilot_response.status(), StatusCode::CREATED);
    let autopilot_body = read_json(create_autopilot_response).await?;
    let autopilot_id = autopilot_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let create_thread_request = Request::builder()
        .method("POST")
        .uri(format!("/api/autopilots/{autopilot_id}/threads"))
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(r#"{"title":"Runtime override thread"}"#))?;
    let create_thread_response = app.clone().oneshot(create_thread_request).await?;
    assert_eq!(create_thread_response.status(), StatusCode::CREATED);
    let thread_body = read_json(create_thread_response).await?;
    let thread_id = thread_body["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let override_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/runtime-routing/override")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(
            r#"{{
                    "scope_type":"autopilot",
                    "scope_id":"{autopilot_id}",
                    "driver":"elixir",
                    "is_active":true
                }}"#
        )))?;
    let override_response = app.clone().oneshot(override_request).await?;
    assert_eq!(override_response.status(), StatusCode::OK);

    let evaluate_request = Request::builder()
        .method("POST")
        .uri("/api/v1/control/runtime-routing/evaluate")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(format!(r#"{{"thread_id":"{thread_id}"}}"#)))?;
    let evaluate_response = app.oneshot(evaluate_request).await?;
    assert_eq!(evaluate_response.status(), StatusCode::OK);
    let body = read_json(evaluate_response).await?;
    assert_eq!(body["data"]["driver"], json!("elixir"));
    assert_eq!(body["data"]["reason"], json!("autopilot_override"));
    assert_eq!(body["data"]["autopilot_id"], json!(autopilot_id));

    Ok(())
}

#[tokio::test]
async fn runtime_routing_canary_and_shadow_semantics_match_config() -> Result<()> {
    let static_dir = tempdir()?;
    std::fs::write(
        static_dir.path().join("index.html"),
        "<!doctype html><html><body>rust shell</body></html>",
    )?;
    let mut canary_config = test_config(static_dir.path().to_path_buf());
    canary_config.runtime_driver = "legacy".to_string();
    canary_config.runtime_overrides_enabled = false;
    canary_config.runtime_canary_seed = "test-seed".to_string();
    canary_config.runtime_canary_user_percent = 100;
    canary_config.runtime_shadow_enabled = true;
    canary_config.runtime_shadow_sample_rate = 1.0;
    let canary_app = build_router(canary_config);
    let canary_token =
        authenticate_token(canary_app.clone(), "runtime-canary@openagents.com").await?;

    let canary_eval = Request::builder()
        .method("POST")
        .uri("/api/v1/control/runtime-routing/evaluate")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {canary_token}"))
        .body(Body::from(r#"{"thread_id":"thread-canary-1"}"#))?;
    let canary_response = canary_app.oneshot(canary_eval).await?;
    assert_eq!(canary_response.status(), StatusCode::OK);
    let canary_body = read_json(canary_response).await?;
    assert_eq!(canary_body["data"]["driver"], json!("elixir"));
    assert_eq!(canary_body["data"]["reason"], json!("user_canary"));
    assert_eq!(canary_body["data"]["shadow"]["mirrored"], json!(false));

    let mut default_config = test_config(static_dir.path().to_path_buf());
    default_config.runtime_driver = "legacy".to_string();
    default_config.runtime_overrides_enabled = false;
    default_config.runtime_canary_user_percent = 0;
    default_config.runtime_shadow_enabled = true;
    default_config.runtime_shadow_sample_rate = 1.0;
    let default_app = build_router(default_config);
    let default_token =
        authenticate_token(default_app.clone(), "runtime-default@openagents.com").await?;

    let default_eval = Request::builder()
        .method("POST")
        .uri("/api/v1/control/runtime-routing/evaluate")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {default_token}"))
        .body(Body::from(r#"{"thread_id":"thread-default-1"}"#))?;
    let default_response = default_app.oneshot(default_eval).await?;
    assert_eq!(default_response.status(), StatusCode::OK);
    let default_body = read_json(default_response).await?;
    assert_eq!(default_body["data"]["driver"], json!("legacy"));
    assert_eq!(default_body["data"]["reason"], json!("default_driver"));
    assert_eq!(default_body["data"]["shadow"]["mirrored"], json!(true));
    assert_eq!(
        default_body["data"]["shadow"]["shadow_driver"],
        json!("elixir")
    );

    Ok(())
}

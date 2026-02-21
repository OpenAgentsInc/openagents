use std::sync::Arc;

use lightning_wallet_executor::compat::WalletCompatService;
use lightning_wallet_executor::config::WalletExecutorConfig;
use lightning_wallet_executor::gateway::{MockGatewayConfig, MockPaymentGateway};
use lightning_wallet_executor::http::{WalletExecutorHttpServer, make_wallet_executor_http_server};
use lightning_wallet_executor::service::WalletExecutorService;
use serde_json::Value;

fn test_config() -> WalletExecutorConfig {
    let mut config = WalletExecutorConfig::default_mock();
    config.host = "127.0.0.1".to_string();
    config.port = 0;
    config.wallet_id = "test-wallet".to_string();
    config
}

async fn spawn_server(
    config: WalletExecutorConfig,
    mock: Option<MockGatewayConfig>,
) -> WalletExecutorHttpServer {
    let service = Arc::new(WalletExecutorService::new(
        config.clone(),
        Arc::new(MockPaymentGateway::new(mock)),
    ));
    let compat = Arc::new(WalletCompatService::new(config));
    make_wallet_executor_http_server(service, compat)
        .await
        .expect("test server should start")
}

#[tokio::test]
async fn serves_status_and_pay_endpoints() {
    let server = spawn_server(test_config(), None).await;
    let client = reqwest::Client::new();

    let status = client
        .get(format!("{}/status", server.address))
        .send()
        .await
        .expect("status call should succeed");
    assert_eq!(status.status(), reqwest::StatusCode::OK);

    let status_json: Value = status.json().await.expect("status body should parse");
    assert_eq!(status_json["ok"], Value::Bool(true));
    assert_eq!(
        status_json["status"]["walletId"],
        Value::String("test-wallet".to_string())
    );
    assert_eq!(
        status_json["status"]["authMode"],
        Value::String("disabled".to_string())
    );

    let pay = client
        .post(format!("{}/pay-bolt11", server.address))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "requestId": "http-integration-pay-1",
                "payment": {
                    "invoice": "lnbc1httpsuccess",
                    "maxAmountMsats": 100_000,
                    "host": "sats4ai.com"
                }
            })
            .to_string(),
        )
        .send()
        .await
        .expect("pay call should succeed");

    assert_eq!(pay.status(), reqwest::StatusCode::OK);
    let pay_json: Value = pay.json().await.expect("pay body should parse");
    assert_eq!(pay_json["ok"], Value::Bool(true));

    let payment_id = pay_json["result"]["payment"]["paymentId"]
        .as_str()
        .unwrap_or_default();
    assert!(payment_id.starts_with("mock-pay-"));

    let preimage_hex = pay_json["result"]["payment"]["preimageHex"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(preimage_hex.len(), 64);

    let receipt_version = pay_json["result"]["receipt"]["receiptVersion"]
        .as_str()
        .unwrap_or_default();
    assert_eq!(receipt_version, "openagents.lightning.wallet_receipt.v1");

    let _ = server.close().await;
}

#[tokio::test]
async fn returns_typed_deny_reason_for_disallowed_host() {
    let server = spawn_server(test_config(), None).await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/pay-bolt11", server.address))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "requestId": "http-deny-1",
                "payment": {
                    "invoice": "lnbc1denied",
                    "maxAmountMsats": 100_000,
                    "host": "example.com"
                }
            })
            .to_string(),
        )
        .send()
        .await
        .expect("pay call should complete");

    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    let body: Value = response.json().await.expect("json body should parse");
    assert_eq!(body["ok"], Value::Bool(false));
    assert_eq!(
        body["error"]["code"],
        Value::String("host_not_allowed".to_string())
    );

    let _ = server.close().await;
}

#[tokio::test]
async fn returns_400_for_malformed_request_body() {
    let server = spawn_server(test_config(), None).await;
    let client = reqwest::Client::new();

    let response = client
        .post(format!("{}/pay-bolt11", server.address))
        .header("content-type", "application/json")
        .body("{\"requestId\":\"bad\",\"payment\":{\"invoice\":\"\"}}")
        .send()
        .await
        .expect("pay call should complete");

    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: Value = response.json().await.expect("json body should parse");
    assert_eq!(body["ok"], Value::Bool(false));
    assert_eq!(
        body["error"]["code"],
        Value::String("invalid_request".to_string())
    );

    let _ = server.close().await;
}

#[tokio::test]
async fn enforces_bearer_auth_when_configured() {
    let mut config = test_config();
    config.auth_token = Some("test-token".to_string());

    let server = spawn_server(config, None).await;
    let client = reqwest::Client::new();

    let unauthorized = client
        .get(format!("{}/status", server.address))
        .send()
        .await
        .expect("status call should complete");
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    let authorized = client
        .get(format!("{}/status", server.address))
        .header("authorization", "Bearer test-token")
        .send()
        .await
        .expect("status call should complete");

    assert_eq!(authorized.status(), reqwest::StatusCode::OK);
    let body: Value = authorized.json().await.expect("json body should parse");
    assert_eq!(
        body["status"]["authMode"],
        Value::String("bearer_static".to_string())
    );
    assert_eq!(body["status"]["authEnforced"], Value::Bool(true));

    let _ = server.close().await;
}

#[tokio::test]
async fn serves_compat_wallet_routes() {
    let server = spawn_server(test_config(), None).await;
    let client = reqwest::Client::new();

    let create_payer = client
        .post(format!("{}/wallets/create", server.address))
        .header("content-type", "application/json")
        .body(serde_json::json!({ "walletId": "mock-payer" }).to_string())
        .send()
        .await
        .expect("create payer should succeed");
    assert_eq!(create_payer.status(), reqwest::StatusCode::OK);
    let payer_body: Value = create_payer.json().await.expect("json body should parse");
    let payer_mnemonic = payer_body["result"]["mnemonic"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(!payer_mnemonic.is_empty());

    let create_receiver = client
        .post(format!("{}/wallets/create", server.address))
        .header("content-type", "application/json")
        .body(serde_json::json!({ "walletId": "mock-receiver" }).to_string())
        .send()
        .await
        .expect("create receiver should succeed");
    assert_eq!(create_receiver.status(), reqwest::StatusCode::OK);
    let receiver_body: Value = create_receiver
        .json()
        .await
        .expect("json body should parse");
    let receiver_mnemonic = receiver_body["result"]["mnemonic"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let receiver_spark_address = receiver_body["result"]["sparkAddress"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let create_invoice = client
        .post(format!("{}/wallets/create-invoice", server.address))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "walletId": "mock-receiver",
                "mnemonic": receiver_mnemonic,
                "amountSats": 5,
                "description": "integration test"
            })
            .to_string(),
        )
        .send()
        .await
        .expect("create invoice should succeed");

    assert_eq!(create_invoice.status(), reqwest::StatusCode::OK);
    let invoice_body: Value = create_invoice.json().await.expect("json body should parse");
    let invoice = invoice_body["result"]["paymentRequest"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(invoice.starts_with("lnmock"));

    let pay_invoice = client
        .post(format!("{}/wallets/pay-bolt11", server.address))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "walletId": "mock-payer",
                "mnemonic": payer_mnemonic,
                "invoice": invoice,
                "maxAmountMsats": 20_000,
                "timeoutMs": 12_000,
                "host": "sats4ai.com"
            })
            .to_string(),
        )
        .send()
        .await
        .expect("pay invoice should succeed");
    assert_eq!(pay_invoice.status(), reqwest::StatusCode::OK);

    let send_spark = client
        .post(format!("{}/wallets/send-spark", server.address))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "walletId": "mock-payer",
                "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
                "sparkAddress": receiver_spark_address,
                "amountSats": 1,
                "timeoutMs": 12_000
            })
            .to_string(),
        )
        .send()
        .await
        .expect("send spark should succeed");
    assert_eq!(send_spark.status(), reqwest::StatusCode::OK);

    let receiver_status = client
        .post(format!("{}/wallets/status", server.address))
        .header("content-type", "application/json")
        .body(
            serde_json::json!({
                "walletId": "mock-receiver",
                "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            })
            .to_string(),
        )
        .send()
        .await
        .expect("status should succeed");
    assert_eq!(receiver_status.status(), reqwest::StatusCode::OK);
    let receiver_status_body: Value = receiver_status
        .json()
        .await
        .expect("json body should parse");
    assert!(
        receiver_status_body["result"]["balanceSats"]
            .as_u64()
            .unwrap_or_default()
            >= 1001
    );

    let _ = server.close().await;
}

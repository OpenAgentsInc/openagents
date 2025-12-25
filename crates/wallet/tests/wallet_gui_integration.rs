//! Integration tests for Wallet GUI
//!
//! Tests verify:
//! - Server startup and shutdown
//! - Dashboard rendering with/without identity
//! - Send/receive/history/settings page rendering
//! - Form submissions
//! - Error handling when no identity present

use bip39::Mnemonic;
use std::sync::Arc;
use wallet::core::identity::UnifiedIdentity;
use wallet::gui::server::start_server;

/// Create a test identity from a known mnemonic
fn create_test_identity() -> Arc<UnifiedIdentity> {
    let mnemonic = Mnemonic::parse(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    ).unwrap();

    let identity = UnifiedIdentity::from_mnemonic(mnemonic).unwrap();
    Arc::new(identity)
}

/// Test server startup and shutdown
#[tokio::test]
async fn test_server_startup_and_shutdown() {
    let identity = create_test_identity();

    // Start server
    let port = start_server(Some(identity)).await.expect("Server should start");

    // Verify port is valid
    assert!(port > 0);

    // Server runs in background, test succeeds if startup works
}

/// Test dashboard rendering with identity
#[tokio::test]
async fn test_dashboard_with_identity() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    // Make request to dashboard
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/", port);

    let response = client.get(&url).send().await.unwrap();

    // Verify response
    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();

    // Verify key content is present
    assert!(body.contains("<!DOCTYPE html>"), "Should contain HTML doctype");
    assert!(body.contains("Wallet"), "Should contain wallet title");
    assert!(body.contains("npub"), "Should display npub");
    assert!(body.contains("Balance"), "Should show balance section");
}

/// Test dashboard without identity
#[tokio::test]
async fn test_dashboard_without_identity() {
    let port = start_server(None).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/", port);

    let response = client.get(&url).send().await.unwrap();

    // Should still return 200 but with "no wallet" message
    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();
    assert!(body.contains("No Wallet Found"));
    assert!(body.contains("cargo wallet init"));
}

/// Test send page with identity
#[tokio::test]
async fn test_send_page_with_identity() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/send", port);

    let response = client.get(&url).send().await.unwrap();

    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();
    assert!(body.contains("Send"));
    assert!(body.contains("form")); // Should have a form
}

/// Test send page without identity
#[tokio::test]
async fn test_send_page_without_identity() {
    let port = start_server(None).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/send", port);

    let response = client.get(&url).send().await.unwrap();

    // Should return 401 Unauthorized
    assert_eq!(response.status(), 401);

    let body = response.text().await.unwrap();
    assert!(body.contains("No wallet identity"));
}

/// Test receive page with identity
#[tokio::test]
async fn test_receive_page_with_identity() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/receive", port);

    let response = client.get(&url).send().await.unwrap();

    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();
    assert!(body.contains("Receive"));
    assert!(body.contains("spark")); // Should display Spark address
}

/// Test receive page without identity
#[tokio::test]
async fn test_receive_page_without_identity() {
    let port = start_server(None).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/receive", port);

    let response = client.get(&url).send().await.unwrap();

    assert_eq!(response.status(), 401);
}

/// Test history page with identity
#[tokio::test]
async fn test_history_page_with_identity() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/history", port);

    let response = client.get(&url).send().await.unwrap();

    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();
    assert!(body.contains("History") || body.contains("Transaction"));
}

/// Test history page without identity
#[tokio::test]
async fn test_history_page_without_identity() {
    let port = start_server(None).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/history", port);

    let response = client.get(&url).send().await.unwrap();

    assert_eq!(response.status(), 401);
}

/// Test settings page with identity
#[tokio::test]
async fn test_settings_page_with_identity() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/settings", port);

    let response = client.get(&url).send().await.unwrap();

    assert_eq!(response.status(), 200);

    let body = response.text().await.unwrap();
    assert!(body.contains("Settings") || body.contains("Relays"));
}

/// Test settings page without identity
#[tokio::test]
async fn test_settings_page_without_identity() {
    let port = start_server(None).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/settings", port);

    let response = client.get(&url).send().await.unwrap();

    assert_eq!(response.status(), 401);
}

/// Test send payment form submission
#[tokio::test]
async fn test_send_payment_submission() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/send", port);

    // Submit form
    let params = [("address", "spark1test"), ("amount", "1000")];
    let response = client.post(&url)
        .form(&params)
        .send()
        .await
        .unwrap();

    // Should redirect to home
    assert!(response.status().is_redirection() || response.status().is_success());
}

/// Test relay settings update
#[tokio::test]
async fn test_relay_update_submission() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/settings/relays", port);

    // Submit relay update
    let params = [("relay_url", "wss://relay.example.com")];
    let response = client.post(&url)
        .form(&params)
        .send()
        .await
        .unwrap();

    // Should redirect to settings
    assert!(response.status().is_redirection() || response.status().is_success());
}

/// Test all routes return valid status codes
#[tokio::test]
async fn test_all_routes_valid_with_identity() {
    let identity = create_test_identity();
    let port = start_server(Some(identity)).await.unwrap();

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{}", port);

    // Test all GET routes
    let routes = vec!["/", "/send", "/receive", "/history", "/settings"];

    for route in routes {
        let url = format!("{}{}", base_url, route);
        let response = client.get(&url).send().await.unwrap();

        assert_eq!(
            response.status(),
            200,
            "Route {} should return 200 OK",
            route
        );
    }
}

/// Test all routes return 401 without identity (except dashboard)
#[tokio::test]
async fn test_all_routes_unauthorized_without_identity() {
    let port = start_server(None).await.unwrap();

    let client = reqwest::Client::new();
    let base_url = format!("http://127.0.0.1:{}", port);

    // These routes should return 401
    let protected_routes = vec!["/send", "/receive", "/history", "/settings"];

    for route in protected_routes {
        let url = format!("{}{}", base_url, route);
        let response = client.get(&url).send().await.unwrap();

        assert_eq!(
            response.status(),
            401,
            "Route {} should return 401 without identity",
            route
        );
    }

    // Dashboard should return 200 even without identity
    let dashboard_url = format!("{}/", base_url);
    let dashboard_response = client.get(&dashboard_url).send().await.unwrap();
    assert_eq!(dashboard_response.status(), 200);
}
#![cfg(feature = "legacy-web")]

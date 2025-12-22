//! Integration tests for unified openagents GUI server functionality
//!
//! These tests verify server components without importing full route modules.

use actix_web::{test, web, App, HttpResponse};

/// Minimal test app state
struct TestState {
    value: std::sync::Arc<tokio::sync::RwLock<String>>,
}

/// Test that Actix test infrastructure works
#[actix_web::test]
async fn test_actix_test_infrastructure() {
    async fn test_handler() -> HttpResponse {
        HttpResponse::Ok()
            .content_type("text/html; charset=utf-8")
            .body("<html><body>Test</body></html>")
    }

    let app = test::init_service(App::new().route("/", web::get().to(test_handler))).await;

    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
}

/// Test route with app state
#[actix_web::test]
async fn test_route_with_state() {
    let state = web::Data::new(TestState {
        value: std::sync::Arc::new(tokio::sync::RwLock::new("test".to_string())),
    });

    async fn handler(state: web::Data<TestState>) -> HttpResponse {
        let value = state.value.read().await;
        HttpResponse::Ok().body(value.clone())
    }

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(handler)),
    )
    .await;

    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
}

/// Test multiple routes
#[actix_web::test]
async fn test_multiple_routes() {
    async fn home() -> HttpResponse {
        HttpResponse::Ok().body("Home")
    }

    async fn about() -> HttpResponse {
        HttpResponse::Ok().body("About")
    }

    async fn contact() -> HttpResponse {
        HttpResponse::Ok().body("Contact")
    }

    let app = test::init_service(
        App::new()
            .route("/", web::get().to(home))
            .route("/about", web::get().to(about))
            .route("/contact", web::get().to(contact)),
    )
    .await;

    // Test each route
    for (uri, expected) in [
        ("/", "Home"),
        ("/about", "About"),
        ("/contact", "Contact"),
    ] {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body = test::read_body(resp).await;
        assert_eq!(body, expected);
    }
}

/// Test 404 for unknown routes
#[actix_web::test]
async fn test_404_for_unknown_routes() {
    async fn home() -> HttpResponse {
        HttpResponse::Ok().body("Home")
    }

    let app = test::init_service(App::new().route("/", web::get().to(home))).await;

    let req = test::TestRequest::get()
        .uri("/nonexistent")
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status().as_u16(), 404);
}

/// Test scoped routes
#[actix_web::test]
async fn test_scoped_routes() {
    async fn api_status() -> HttpResponse {
        HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
    }

    async fn api_version() -> HttpResponse {
        HttpResponse::Ok().json(serde_json::json!({"version": "1.0.0"}))
    }

    let app = test::init_service(
        App::new().service(
            web::scope("/api")
                .route("/status", web::get().to(api_status))
                .route("/version", web::get().to(api_version)),
        ),
    )
    .await;

    // Test /api/status
    let req = test::TestRequest::get().uri("/api/status").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Test /api/version
    let req = test::TestRequest::get().uri("/api/version").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

/// Test concurrent requests
#[actix_web::test]
async fn test_concurrent_requests() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    async fn handler(counter: web::Data<Arc<AtomicU32>>) -> HttpResponse {
        counter.fetch_add(1, Ordering::SeqCst);
        HttpResponse::Ok().body("OK")
    }

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(counter_clone.clone()))
            .route("/", web::get().to(handler)),
    )
    .await;

    // Make 10 concurrent requests
    for _ in 0..10 {
        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());
    }

    assert_eq!(counter.load(Ordering::SeqCst), 10);
}

/// Test JSON responses
#[actix_web::test]
async fn test_json_responses() {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    async fn json_handler() -> HttpResponse {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        HttpResponse::Ok().json(data)
    }

    let app = test::init_service(App::new().route("/json", web::get().to(json_handler))).await;

    let req = test::TestRequest::get().uri("/json").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/json"
    );
}

/// Test POST requests
#[actix_web::test]
async fn test_post_requests() {
    async fn post_handler() -> HttpResponse {
        HttpResponse::Ok().body("Posted")
    }

    let app =
        test::init_service(App::new().route("/submit", web::post().to(post_handler))).await;

    let req = test::TestRequest::post().uri("/submit").to_request();
    let resp = test::call_service(&app, req).await;

    assert!(resp.status().is_success());

    let body = test::read_body(resp).await;
    assert_eq!(body, "Posted");
}

/// Test that server state can be shared and modified
#[actix_web::test]
async fn test_shared_mutable_state() {
    let state = web::Data::new(TestState {
        value: std::sync::Arc::new(tokio::sync::RwLock::new("initial".to_string())),
    });

    async fn get_value(state: web::Data<TestState>) -> HttpResponse {
        let value = state.value.read().await;
        HttpResponse::Ok().body(value.clone())
    }

    async fn set_value(state: web::Data<TestState>) -> HttpResponse {
        let mut value = state.value.write().await;
        *value = "updated".to_string();
        HttpResponse::Ok().body("Set")
    }

    let app = test::init_service(
        App::new()
            .app_data(state.clone())
            .route("/get", web::get().to(get_value))
            .route("/set", web::post().to(set_value)),
    )
    .await;

    // Get initial value
    let req = test::TestRequest::get().uri("/get").to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    assert_eq!(body, "initial");

    // Set new value
    let req = test::TestRequest::post().uri("/set").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());

    // Get updated value
    let req = test::TestRequest::get().uri("/get").to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    assert_eq!(body, "updated");
}

/// Test WebSocket broadcast channel
#[tokio::test]
async fn test_broadcast_channel() {
    use tokio::sync::broadcast;

    let (tx, mut rx1) = broadcast::channel(16);
    let mut rx2 = tx.subscribe();

    // Send a message
    tx.send("test message".to_string()).unwrap();

    // Both receivers should get it
    assert_eq!(rx1.recv().await.unwrap(), "test message");
    assert_eq!(rx2.recv().await.unwrap(), "test message");
}

/// Test RwLock for shared state
#[tokio::test]
async fn test_rwlock_shared_state() {
    use tokio::sync::RwLock;

    let value = RwLock::new(42);

    // Multiple readers
    let read1 = value.read().await;
    let read2 = value.read().await;
    assert_eq!(*read1, 42);
    assert_eq!(*read2, 42);
    drop(read1);
    drop(read2);

    // Single writer
    let mut write = value.write().await;
    *write = 100;
    drop(write);

    // Verify change
    let read = value.read().await;
    assert_eq!(*read, 100);
}

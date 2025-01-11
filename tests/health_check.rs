use actix_web::{test, App, web};

#[tokio::test]
async fn health_check_works() {
    // Arrange
    let app = test::init_service(
        App::new()
            .route("/health", web::get().to(|| async { web::Json(serde_json::json!({"status": "healthy"})) }))
    ).await;

    // Act
    let req = test::TestRequest::get().uri("/health").to_request();
    let resp = test::call_service(&app, req).await;

    // Assert
    assert!(resp.status().is_success());
    
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "healthy");
}

use openagents::server::admin::middleware::AdminAuth;
use actix_web::{test, web, App, HttpResponse};

async fn test_endpoint() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

#[actix_web::test]
async fn test_admin_auth_valid_token() {
    let app = test::init_service(
        App::new().service(
            web::scope("/admin")
                .wrap(AdminAuth::new())
                .route("/test", web::get().to(test_endpoint)),
        ),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/admin/test")
        .insert_header(("Authorization", "Bearer admin-token"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_admin_auth_invalid_token() {
    std::env::set_var("APP_ENVIRONMENT", "production");

    let app = test::init_service(
        App::new().service(
            web::scope("/admin")
                .wrap(AdminAuth::new())
                .route("/test", web::get().to(test_endpoint)),
        ),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/admin/test")
        .insert_header(("Authorization", "Bearer wrong-token"))
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    std::env::remove_var("APP_ENVIRONMENT");
}

#[actix_web::test]
async fn test_admin_auth_missing_token() {
    let original_env = std::env::var("APP_ENVIRONMENT").ok();
    std::env::set_var("APP_ENVIRONMENT", "production");

    let app = test::init_service(
        App::new().service(
            web::scope("/admin")
                .wrap(AdminAuth::new())
                .route("/test", web::get().to(test_endpoint)),
        ),
    )
    .await;

    let req = test::TestRequest::get().uri("/admin/test").to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), 401);

    // Restore original environment
    if let Some(env) = original_env {
        std::env::set_var("APP_ENVIRONMENT", env);
    } else {
        std::env::remove_var("APP_ENVIRONMENT");
    }
}

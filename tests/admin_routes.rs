use actix_web::{test, web, App};
use lazy_static::lazy_static;
use openagents::nostr::event::Event;
use openagents::server::admin::routes::{admin_stats, create_demo_event};
use tokio::sync::Mutex;

lazy_static! {
    static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
}

async fn setup_test_db() -> sqlx::PgPool {
    // Use a dedicated test database
    let pool = sqlx::PgPool::connect("postgres://postgres:password@localhost:5432/openagents_test")
        .await
        .unwrap();

    // Drop and recreate test table
    sqlx::query("DROP TABLE IF EXISTS events")
        .execute(&pool)
        .await
        .unwrap();

    // Create table
    sqlx::query(
        "CREATE TABLE events (
            id TEXT PRIMARY KEY,
            pubkey TEXT NOT NULL,
            created_at BIGINT NOT NULL,
            kind INTEGER NOT NULL,
            tags JSONB NOT NULL,
            content TEXT NOT NULL,
            sig TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

#[actix_web::test]
async fn test_create_demo_event() {
    // Ensure tests run sequentially
    let _lock = TEST_MUTEX.lock().await;

    let pool = setup_test_db().await;

    // Set test database URL for the application code
    std::env::set_var(
        "DATABASE_URL",
        "postgres://postgres:password@localhost:5432/openagents_test",
    );

    // Initialize app with test database
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .service(create_demo_event),
    )
    .await;

    let req = test::TestRequest::post().uri("/demo-event").to_request();

    let resp: serde_json::Value = test::call_and_read_body_json(&app, req).await;

    assert_eq!(resp["status"], "success");

    let event = &resp["event"];
    assert!(!event["id"].as_str().unwrap().is_empty());
    assert!(!event["pubkey"].as_str().unwrap().is_empty());
    assert!(!event["sig"].as_str().unwrap().is_empty());
    assert_eq!(event["kind"], 1);
    assert_eq!(event["content"], "This is a demo event");
    assert_eq!(event["tags"][0][0], "t");
    assert_eq!(event["tags"][0][1], "demo");

    // Verify event was saved to database
    let final_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(final_count, 1);

    // Verify event details in database
    let saved_event = sqlx::query_as::<_, Event>("SELECT * FROM events WHERE id = $1")
        .bind(event["id"].as_str().unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(saved_event.id, event["id"].as_str().unwrap());
    assert_eq!(saved_event.pubkey, event["pubkey"].as_str().unwrap());
    assert_eq!(saved_event.kind, 1);
    assert_eq!(saved_event.content, "This is a demo event");
}

#[actix_web::test]
async fn test_admin_stats() {
    // Ensure tests run sequentially
    let _lock = TEST_MUTEX.lock().await;

    let pool = setup_test_db().await;

    // Set test database URL for the application code
    std::env::set_var(
        "DATABASE_URL",
        "postgres://postgres:password@localhost:5432/openagents_test",
    );

    // Initialize app with test database
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .service(admin_stats),
    )
    .await;

    let req = test::TestRequest::get().uri("/stats").to_request();

    let resp: serde_json::Value = test::call_and_read_body_json(&app, req).await;

    assert!(resp.get("total_events").is_some());
    assert!(resp.get("status").is_some());
    assert!(resp.get("events_by_kind").is_some());
    assert!(resp.get("storage_usage").is_some());
}

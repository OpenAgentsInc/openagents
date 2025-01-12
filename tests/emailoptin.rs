use actix_web::{test, web, App};
use openagents::configuration::get_configuration;
use openagents::emailoptin::subscribe;
use sqlx::PgPool;

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    // Arrange
    let configuration = get_configuration().expect("Failed to read configuration");
    let connection_pool = PgPool::connect_with(configuration.database.connect_options())
        .await
        .expect("Failed to connect to Postgres");

    // Create subscriptions table if it doesn't exist
    sqlx::query!(
        r#"
        CREATE TABLE IF NOT EXISTS subscriptions (
            id uuid PRIMARY KEY,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            subscribed_at timestamptz NOT NULL
        )
        "#,
    )
    .execute(&connection_pool)
    .await
    .expect("Failed to create subscriptions table");
    
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(connection_pool.clone()))
            .route("/subscriptions", web::post().to(subscribe))
    ).await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    // Act
    let request = test::TestRequest::post()
        .uri("/subscriptions")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload(body)
        .to_request();

    let response = test::call_service(&app, request).await;

    // Print debug information
    println!("Response status: {}", response.status());
    let body = test::read_body(response).await;
    println!("Response body: {:?}", body);

    // Create new request since the previous one was consumed
    let request = test::TestRequest::post()
        .uri("/subscriptions")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload(body)
        .to_request();
    
    let response = test::call_service(&app, request).await;
    
    // Assert
    assert!(response.status().is_success());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&connection_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

#[tokio::test]
async fn subscribe_returns_a_400_when_data_is_missing() {
    // Arrange
    let configuration = get_configuration().expect("Failed to read configuration");
    let connection_pool = PgPool::connect_with(configuration.database.connect_options())
        .await
        .expect("Failed to connect to Postgres");
    
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(connection_pool.clone()))
            .route("/subscriptions", web::post().to(subscribe))
    ).await;

    let test_cases = vec![
        ("name=le%20guin", "missing the email"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        // Act
        let request = test::TestRequest::post()
            .uri("/subscriptions")
            .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
            .set_payload(invalid_body)
            .to_request();

        let response = test::call_service(&app, request).await;

        // Assert
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

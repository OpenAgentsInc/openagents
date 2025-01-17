use axum::{routing::post, Router};
use axum_test::TestServer;
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

    // Drop table if exists and recreate
    sqlx::query!("DROP TABLE IF EXISTS subscriptions")
        .execute(&connection_pool)
        .await
        .expect("Failed to drop table");

    sqlx::query!(
        r#"
        CREATE TABLE subscriptions (
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

    let app = Router::new()
        .route("/subscriptions", post(subscribe))
        .with_state(connection_pool.clone());

    let server = TestServer::new(app).unwrap();

    let response = server
        .post("/subscriptions")
        .form(&[("name", "le guin"), ("email", "ursula_le_guin@gmail.com")])
        .await;

    assert_eq!(response.status_code(), 200);

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&connection_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
}

#[tokio::test]
async fn subscribe_returns_a_422_when_data_is_missing() {
    // Arrange
    let configuration = get_configuration().expect("Failed to read configuration");
    let connection_pool = PgPool::connect_with(configuration.database.connect_options())
        .await
        .expect("Failed to connect to Postgres");

    let app = Router::new()
        .route("/subscriptions", post(subscribe))
        .with_state(connection_pool.clone());

    let server = TestServer::new(app).unwrap();

    let test_cases = vec![
        (vec![("name", "le guin")], "missing the email"),
        (vec![("email", "ursula_le_guin@gmail.com")], "missing the name"),
        (vec![], "missing both name and email"),
    ];

    for (invalid_form, error_message) in test_cases {
        let response = server.post("/subscriptions").form(&invalid_form).await;

        assert_eq!(
            response.status_code(),
            422,
            "The API did not fail with 422 Unprocessable Entity when the payload was {}.",
            error_message
        );
    }
}
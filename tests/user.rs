use axum::{routing::post, Router};
use axum_test::TestServer;
use dotenvy::dotenv;
use openagents::server::{
    handlers::user::create_user,
    models::user::{CreateUser, User},
};
use serde_json::json;
use sqlx::PgPool;
use tracing::{info, Level};

#[tokio::test]
async fn test_user_creation() {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    // Load environment variables
    dotenv().ok();

    // Set up database connection
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Create router with user creation endpoint
    let app = Router::new()
        .route("/users", post(create_user))
        .with_state(pool.clone());

    // Create test server
    let server = TestServer::new(app).unwrap();

    // Test cases
    let test_cases = vec![
        (
            // Valid user creation
            json!({
                "scramble_id": "test_user_1",
                "metadata": {
                    "display_name": "Test User 1"
                }
            }),
            200,
            Some(json!({
                "scramble_id": "test_user_1",
                "metadata": {
                    "display_name": "Test User 1"
                }
            })),
        ),
        (
            // Duplicate scramble_id
            json!({
                "scramble_id": "test_user_1",
                "metadata": {
                    "display_name": "Duplicate User"
                }
            }),
            409,
            None,
        ),
        (
            // Missing required field
            json!({
                "metadata": {
                    "display_name": "Invalid User"
                }
            }),
            400,
            None,
        ),
    ];

    for (input, expected_status, expected_response) in test_cases {
        info!("\n\nTesting user creation with input: {}", input);
        
        // Make request
        let response = server
            .post("/users")
            .json(&input)
            .await;

        // Assert status code
        assert_eq!(
            response.status_code(),
            expected_status,
            "Status code mismatch for input: {}",
            input
        );

        // For successful creation, verify response
        if expected_status == 200 {
            let user: User = response.json();
            
            // Verify user fields
            assert_eq!(
                user.scramble_id,
                input["scramble_id"].as_str().unwrap(),
                "scramble_id mismatch"
            );
            
            // Verify metadata
            if let Some(metadata) = input.get("metadata") {
                assert_eq!(
                    user.metadata.as_ref().unwrap(),
                    metadata,
                    "metadata mismatch"
                );
            }

            // Verify timestamps exist
            assert!(user.created_at.is_some(), "created_at should be set");
            assert!(user.updated_at.is_some(), "updated_at should be set");

            // Verify user exists in database
            let db_user = sqlx::query_as!(
                User,
                "SELECT * FROM users WHERE scramble_id = $1",
                user.scramble_id
            )
            .fetch_one(&pool)
            .await
            .expect("User should exist in database");

            assert_eq!(user, db_user, "Database user should match response");
        }

        // For error cases, verify error response
        if expected_status != 200 {
            let error: serde_json::Value = response.json();
            assert!(error.get("error").is_some(), "Error response should contain error field");
        }

        // Clean up test data
        if expected_status == 200 {
            sqlx::query!("DELETE FROM users WHERE scramble_id = $1", 
                input["scramble_id"].as_str().unwrap()
            )
            .execute(&pool)
            .await
            .expect("Failed to clean up test user");
        }
    }
}
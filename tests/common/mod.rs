use dotenvy::dotenv;
use sqlx::PgPool;
use tracing::info;

pub async fn setup_test_db() -> PgPool {
    info!("Setting up test database");
    dotenv().ok();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    info!("Connecting to database: {}", database_url);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    info!("Connected to database successfully");

    // Create users table if it doesn't exist
    sqlx::query!(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            scramble_id TEXT NOT NULL UNIQUE,
            metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
            last_login_at TIMESTAMP WITH TIME ZONE,
            created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create users table");

    info!("Users table created/verified");

    // Clean up any existing test data
    sqlx::query!("DELETE FROM users WHERE scramble_id LIKE 'test_%'")
        .execute(&pool)
        .await
        .expect("Failed to clean up existing test data");

    info!("Cleaned up existing test data");

    pool
}
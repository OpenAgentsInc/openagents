use sqlx::PgPool;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tracing::info;

static DB_SETUP: OnceLock<Mutex<()>> = OnceLock::new();

pub async fn setup_test_db() -> PgPool {
    info!("Setting up test database");

    // Use DATABASE_URL from environment, fall back to default for local dev
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());
    info!("Connecting to database: {}", database_url);

    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    info!("Connected to database successfully");

    // Ensure only one test can set up the database at a time
    let lock = DB_SETUP.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock().await;

    // Drop existing sequence and table if they exist
    sqlx::query!("DROP TABLE IF EXISTS users CASCADE")
        .execute(&pool)
        .await
        .expect("Failed to drop table");

    // Create users table with sequence
    sqlx::query!(
        r#"
        CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            scramble_id TEXT,
            github_id BIGINT UNIQUE,
            github_token TEXT,
            metadata JSONB DEFAULT '{}'::jsonb,
            last_login_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP
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
        .expect("Failed to clean up test data");

    info!("Cleaned up existing test data");

    pool
}

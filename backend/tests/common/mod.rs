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

    // Execute all setup in a single transaction
    let mut tx = pool.begin().await.expect("Failed to start transaction");

    // Drop existing sequence and table if they exist
    sqlx::query("DROP TABLE IF EXISTS users CASCADE")
        .execute(&mut *tx)
        .await
        .expect("Failed to drop table");

    // Create users table with sequence
    sqlx::query(
        r#"
        CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            scramble_id TEXT,
            github_id BIGINT,
            github_token TEXT,
            metadata JSONB DEFAULT '{}'::jsonb,
            last_login_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ DEFAULT CURRENT_TIMESTAMP,
            pseudonym TEXT
        )
        "#,
    )
    .execute(&mut *tx)
    .await
    .expect("Failed to create users table");

    // Create indexes
    sqlx::query("CREATE INDEX idx_users_scramble_id ON users(scramble_id)")
        .execute(&mut *tx)
        .await
        .expect("Failed to create scramble_id index");

    sqlx::query("CREATE INDEX idx_users_github_id ON users(github_id)")
        .execute(&mut *tx)
        .await
        .expect("Failed to create github_id index");

    sqlx::query("CREATE INDEX idx_users_pseudonym ON users(pseudonym)")
        .execute(&mut *tx)
        .await
        .expect("Failed to create pseudonym index");

    // Add unique constraint
    sqlx::query("ALTER TABLE users ADD CONSTRAINT users_github_id_key UNIQUE (github_id)")
        .execute(&mut *tx)
        .await
        .expect("Failed to add unique constraint");

    // Add updated_at trigger
    sqlx::query(
        r#"
        CREATE OR REPLACE FUNCTION update_updated_at_column()
        RETURNS TRIGGER AS $$
        BEGIN
            NEW.updated_at = CURRENT_TIMESTAMP;
            RETURN NEW;
        END;
        $$ language 'plpgsql'
        "#,
    )
    .execute(&mut *tx)
    .await
    .expect("Failed to create trigger function");

    // Create trigger
    sqlx::query(
        r#"
        CREATE TRIGGER update_users_updated_at
            BEFORE UPDATE ON users
            FOR EACH ROW
            EXECUTE FUNCTION update_updated_at_column()
        "#,
    )
    .execute(&mut *tx)
    .await
    .expect("Failed to create trigger");

    // Clean up any existing test data
    sqlx::query("DELETE FROM users WHERE scramble_id LIKE 'test_%'")
        .execute(&mut *tx)
        .await
        .expect("Failed to clean up test data");

    // Commit transaction
    tx.commit().await.expect("Failed to commit transaction");

    info!("Test database setup completed successfully");

    pool
}

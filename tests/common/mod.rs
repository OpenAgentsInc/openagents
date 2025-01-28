use sqlx::PgPool;

pub async fn setup_test_db() -> PgPool {
    let pool = PgPool::connect("postgres://postgres:postgres@localhost:5432/test_db")
        .await
        .expect("Failed to connect to test database");

    // Drop and recreate the users table to ensure a clean state
    sqlx::query!("DROP TABLE IF EXISTS users")
        .execute(&pool)
        .await
        .expect("Failed to drop users table");

    sqlx::query!(
        r#"
        CREATE TABLE users (
            id SERIAL PRIMARY KEY,
            scramble_id VARCHAR(255) UNIQUE NOT NULL,
            metadata JSONB DEFAULT '{}'::jsonb,
            last_login_at TIMESTAMP WITH TIME ZONE,
            created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
        )
        "#
    )
    .execute(&pool)
    .await
    .expect("Failed to create users table");

    pool
}
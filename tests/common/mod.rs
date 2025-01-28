use sqlx::PgPool;

pub async fn setup_test_db() -> PgPool {
    let pool = PgPool::connect("postgres://postgres:postgres@localhost:5432/test_db")
        .await
        .expect("Failed to connect to test database");

    sqlx::query!(
        r#"
        CREATE TABLE IF NOT EXISTS users (
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
use dotenvy::dotenv;
use sqlx::PgPool;

pub async fn setup_test_db() -> PgPool {
    // Load environment variables
    dotenv().ok();

    // Set up database connection
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // Clean up any existing test data
    sqlx::query!("DELETE FROM users WHERE scramble_id LIKE 'test_%'")
        .execute(&pool)
        .await
        .expect("Failed to clean up existing test data");

    pool
}
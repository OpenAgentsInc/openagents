use openagents::server::models::user::User;
use sqlx::PgPool;

mod common;
use common::setup_test_db;

#[tokio::test]
async fn test_user_creation() {
    let pool = setup_test_db().await;

    // Create test user
    let user = create_test_user(&pool).await;

    // Verify user was created
    let db_user = sqlx::query_as!(
        User,
        r#"
        SELECT id, scramble_id, github_id, github_token, metadata, last_login_at, created_at, updated_at
        FROM users WHERE scramble_id = $1
        "#,
        user.scramble_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(db_user.scramble_id, user.scramble_id);
}

async fn create_test_user(pool: &PgPool) -> User {
    sqlx::query_as!(
        User,
        r#"
        INSERT INTO users (scramble_id, metadata, github_id, github_token)
        VALUES ($1, $2, $3, $4)
        RETURNING id, scramble_id, github_id, github_token, metadata, last_login_at, created_at, updated_at
        "#,
        Some("test_user"),
        serde_json::json!({}) as _,
        None::<i64>,
        None::<String>
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

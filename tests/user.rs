use openagents::server::models::{timestamp::DateTimeWrapper, user::User};
use sqlx::PgPool;

mod common;
use common::setup_test_db;

#[tokio::test]
async fn test_user_creation() {
    let pool = setup_test_db().await;

    // Create test user
    let user = create_test_user(&pool).await;

    // Verify user was created
    let row = sqlx::query!(
        r#"
        SELECT id, scramble_id, github_id, github_token, metadata,
               created_at, last_login_at, pseudonym
        FROM users
        WHERE scramble_id = $1
        "#,
        user.scramble_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let db_user = User::new(
        row.id,
        row.scramble_id,
        row.github_id,
        row.github_token,
        row.metadata.expect("metadata should never be null"),
        DateTimeWrapper(row.created_at.expect("created_at should never be null")),
        row.last_login_at.map(DateTimeWrapper),
        row.pseudonym,
    );

    assert_eq!(db_user.scramble_id, user.scramble_id);
}

async fn create_test_user(pool: &PgPool) -> User {
    let row = sqlx::query!(
        r#"
        INSERT INTO users (scramble_id, metadata, github_id, github_token, pseudonym)
        VALUES ($1, $2, $3, $4, $1)
        RETURNING id, scramble_id, github_id, github_token, metadata,
                  created_at, last_login_at, pseudonym
        "#,
        Some("test_user"),
        serde_json::json!({}) as _,
        None::<i64>,
        None::<String>
    )
    .fetch_one(pool)
    .await
    .unwrap();

    User::new(
        row.id,
        row.scramble_id,
        row.github_id,
        row.github_token,
        row.metadata.expect("metadata should never be null"),
        DateTimeWrapper(row.created_at.expect("created_at should never be null")),
        row.last_login_at.map(DateTimeWrapper),
        row.pseudonym,
    )
}

use openagents::server::models::{timestamp::DateTimeWrapper, user::User};
use sqlx::PgPool;
use tokio::time::{sleep, Duration};
use tracing::warn;

mod common;
use common::setup_test_db;

const MAX_RETRIES: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);

#[tokio::test]
async fn test_user_creation() {
    let pool = setup_test_db().await;

    // Create test user
    let user = create_test_user(&pool).await;

    // Verify user was created with retries
    let row = retry_db_operation(|| async {
        sqlx::query!(
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
    })
    .await
    .unwrap();

    let db_user = User::builder(row.id)
        .scramble_id(row.scramble_id)
        .github_id(row.github_id)
        .github_token(row.github_token)
        .metadata(row.metadata.expect("metadata should never be null"))
        .created_at(DateTimeWrapper(
            row.created_at.expect("created_at should never be null"),
        ))
        .last_login_at(row.last_login_at.map(DateTimeWrapper))
        .pseudonym(row.pseudonym)
        .build();

    assert_eq!(db_user.scramble_id, user.scramble_id);
}

async fn create_test_user(pool: &PgPool) -> User {
    let row = retry_db_operation(|| async {
        sqlx::query!(
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
    })
    .await
    .unwrap();

    User::builder(row.id)
        .scramble_id(row.scramble_id)
        .github_id(row.github_id)
        .github_token(row.github_token)
        .metadata(row.metadata.expect("metadata should never be null"))
        .created_at(DateTimeWrapper(
            row.created_at.expect("created_at should never be null"),
        ))
        .last_login_at(row.last_login_at.map(DateTimeWrapper))
        .pseudonym(row.pseudonym)
        .build()
}

async fn retry_db_operation<F, Fut, T>(operation: F) -> Result<T, sqlx::Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, sqlx::Error>>,
{
    let mut attempts = 0;
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                if attempts >= MAX_RETRIES {
                    return Err(e);
                }
                warn!(
                    "Database operation failed (attempt {}/{}): {}",
                    attempts, MAX_RETRIES, e
                );
                sleep(RETRY_DELAY).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use openagents::server::models::{timestamp::DateTimeWrapper, User};
    use serde_json::json;

    #[test]
    fn test_user_serialization() {
        let now = Utc::now();
        let db_user = User::builder(1)
            .scramble_id(Some("test_scramble_id".to_string()))
            .github_id(Some(123))
            .github_token(Some("test_token".to_string()))
            .metadata(json!({"key": "value"}))
            .created_at(DateTimeWrapper(now))
            .last_login_at(Some(DateTimeWrapper(now)))
            .pseudonym(Some("test_pseudonym".to_string()))
            .build();

        let serialized = serde_json::to_string(&db_user).unwrap();
        let deserialized: User = serde_json::from_str(&serialized).unwrap();

        assert_eq!(db_user.id, deserialized.id);
        assert_eq!(db_user.scramble_id, deserialized.scramble_id);
        assert_eq!(db_user.github_id, deserialized.github_id);
        assert_eq!(db_user.github_token, deserialized.github_token);
        assert_eq!(db_user.metadata, deserialized.metadata);
        let db_created_at: chrono::DateTime<Utc> = db_user.created_at.into();
        let deserialized_created_at: chrono::DateTime<Utc> = deserialized.created_at.into();
        assert_eq!(db_created_at, deserialized_created_at);
        assert_eq!(db_user.last_login_at, deserialized.last_login_at);
        assert_eq!(db_user.pseudonym, deserialized.pseudonym);
    }

    #[test]
    fn test_user_debug() {
        let now = Utc::now();
        let user = User::builder(1)
            .scramble_id(Some("test_scramble_id".to_string()))
            .github_id(Some(123))
            .github_token(Some("test_token".to_string()))
            .metadata(json!({"key": "value"}))
            .created_at(DateTimeWrapper(now))
            .last_login_at(Some(DateTimeWrapper(now)))
            .pseudonym(Some("test_pseudonym".to_string()))
            .build();

        let debug_string = format!("{:?}", user);
        assert!(debug_string.contains(&user.id.to_string()));
        assert!(debug_string.contains(user.scramble_id.as_ref().unwrap()));
        assert!(debug_string.contains(&user.github_id.unwrap().to_string()));
    }
}

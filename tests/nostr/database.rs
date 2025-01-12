use openagents::configuration;
use openagents::database::{get_connection_pool, migrate_database};

#[tokio::test]
async fn test_get_connection_pool() {
    let config = configuration::get_configuration().expect("Failed to load config");
    let result = get_connection_pool(&config).await;
    assert!(result.is_ok(), "Should connect to test database");
}

#[tokio::test]
async fn test_migrate_database() {
    let config = configuration::get_configuration().expect("Failed to load config");
    let pool = get_connection_pool(&config)
        .await
        .expect("Failed to get pool");
    let result = migrate_database(&pool).await;
    assert!(result.is_ok(), "Should run migrations successfully");
}

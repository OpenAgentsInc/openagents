use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use crate::configuration::{DatabaseSettings, Settings};

pub async fn get_connection_pool(configuration: &Settings) -> Result<PgPool, sqlx::Error> {
    let mut retries = 0;
    let max_retries = configuration.database.max_connection_retries;

    loop {
        match PgPoolOptions::new()
            .max_connections(5)
            .connect_with(configuration.database.with_db())
            .await
        {
            Ok(pool) => return Ok(pool),
            Err(e) => {
                if retries >= max_retries {
                    return Err(e);
                }
                retries += 1;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    }
}

pub async fn migrate_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
}

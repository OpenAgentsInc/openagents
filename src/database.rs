use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use crate::configuration::Settings;

pub async fn get_connection_pool(configuration: &Settings) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect_with(configuration.database.with_db())
        .await
}

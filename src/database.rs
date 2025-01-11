use sqlx::postgres::{PgPoolOptions, PgConnectOptions};
use sqlx::PgPool;
use crate::configuration::Settings;

pub async fn get_connection_pool(configuration: &Settings) -> Result<PgPool, sqlx::Error> {
    let mut retries = 0;
    let max_retries = configuration.database.max_connection_retries;

    loop {
        let connect_options = PgConnectOptions::new()
            .host(&configuration.database.host)
            .port(configuration.database.port)
            .username(&configuration.database.username)
            .password(configuration.database.password.expose_secret())
            .database(&configuration.database.database_name)
            .require_ssl(configuration.database.require_ssl);

        match PgPoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
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
    match sqlx::migrate!("./migrations").run(pool).await {
        Ok(_) => Ok(()),
        Err(e) => Err(sqlx::Error::Migration(e))
    }
}

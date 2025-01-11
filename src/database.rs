use sqlx::postgres::{PgPoolOptions, PgConnectOptions};
use sqlx::{PgPool, postgres::PgSslMode};
use secrecy::ExposeSecret;
use crate::configuration::Settings;
use tracing::error;

pub async fn get_connection_pool(configuration: &Settings) -> Result<PgPool, sqlx::Error> {
    error!("Creating database connection pool...");
    
    let connect_options = PgConnectOptions::new()
        .host(&configuration.database.host)
        .port(configuration.database.port)
        .username(&configuration.database.username)
        .password(configuration.database.password.expose_secret())
        .database(&configuration.database.database_name)
        .ssl_mode(if configuration.database.require_ssl {
            error!("Using SSL mode: REQUIRE");
            PgSslMode::Require
        } else {
            error!("Using SSL mode: PREFER");
            PgSslMode::Prefer
        });

    error!("Attempting database connection to {}:{}", 
           configuration.database.host, 
           configuration.database.port);

    PgPoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options)
        .await
}

pub async fn migrate_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    error!("Running database migrations...");
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| {
            error!("Migration error: {}", e);
            sqlx::Error::Protocol(format!("Migration error: {}", e))
        })
}
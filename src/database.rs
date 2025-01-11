use sqlx::postgres::{PgPoolOptions, PgConnectOptions};
use sqlx::{PgPool, postgres::PgSslMode};
use secrecy::ExposeSecret;
use crate::configuration::Settings;
use tracing::error;
use url::Url;

pub async fn get_connection_pool(configuration: &Settings) -> Result<PgPool, sqlx::Error> {
    error!("Creating database connection pool...");
    
    let connect_options = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        error!("Using DATABASE_URL from environment");
        let url = match Url::parse(&database_url) {
            Ok(url) => {
                error!("Successfully parsed DATABASE_URL");
                url
            },
            Err(e) => {
                error!("Failed to parse DATABASE_URL: {}", e);
                return Err(sqlx::Error::Configuration(e.into()));
            }
        };

        let host = url.host_str().unwrap_or("localhost");
        let port = url.port().unwrap_or(5432);
        let username = url.username();
        let password = url.password().unwrap_or("");
        let database = url.path().trim_start_matches('/');
        
        error!("!!! DATABASE CONNECTION DETAILS !!!");
        error!("  Host: {}", host);
        error!("  Port: {}", port);
        error!("  Username: {}", username);
        error!("  Database Name: {}", database);
        error!("  SSL Mode: REQUIRE (forced for DigitalOcean)");

        PgConnectOptions::new()
            .host(host)
            .port(port)
            .username(username)
            .password(password)
            .database(database)
            .ssl_mode(PgSslMode::Require)
    } else {
        error!("Using configuration file for database connection");
        PgConnectOptions::new()
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
            })
    };

    error!("Attempting to connect to database...");

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
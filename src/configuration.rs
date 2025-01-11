use config::{Config, ConfigError, Environment as ConfigEnvironment, File};
use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use sqlx::ConnectOptions;
use tracing::{info, warn, error};
use url::Url;

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    #[serde(default = "default_admin_token")]
    pub admin_token: String,
}

#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
    #[serde(default = "default_max_retries")]
    pub max_connection_retries: u32,
    #[serde(default = "default_retry_interval")]
    pub retry_interval_secs: u64,
}

fn default_max_retries() -> u32 {
    5 // 5 retries by default
}

fn default_retry_interval() -> u64 {
    5 // 5 seconds between retries
}

fn default_admin_token() -> String {
    "admin-token".to_string() // Default fallback value
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> PgConnectOptions {
        // Log all environment variables (excluding sensitive ones)
        info!("Environment variables present:");
        for (key, value) in std::env::vars() {
            if !key.contains("SECRET") && !key.contains("PASSWORD") && !key.contains("KEY") {
                info!("  {}: {}", key, value);
            } else {
                info!("  {}: [REDACTED]", key);
            }
        }

        // First check for App Platform's DATABASE_URL
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            error!("!!! USING DATABASE_URL FROM ENVIRONMENT !!!");
            error!("!!! THIS IS THE ACTUAL URL WE'RE USING !!!");
            
            // Parse the DATABASE_URL
            let url = match Url::parse(&database_url) {
                Ok(url) => {
                    error!("Successfully parsed DATABASE_URL");
                    url
                },
                Err(e) => {
                    error!("Failed to parse DATABASE_URL: {}", e);
                    error!("DATABASE_URL format should be: postgres://username:password@host:port/database");
                    panic!("Invalid DATABASE_URL format");
                }
            };

            let host = url.host_str().unwrap_or("localhost");
            let port = url.port().unwrap_or(5432);
            let username = url.username();
            let password = url.password().unwrap_or("");
            let database = url.path().trim_start_matches('/');
            
            error!("!!! ACTUAL DATABASE CONNECTION DETAILS !!!");
            error!("  Host: {}", host);
            error!("  Port: {}", port);
            error!("  Username: {}", username);
            error!("  Database Name: {}", database);
            error!("  SSL Mode: REQUIRE (forced for DigitalOcean)");
            
            let mut options = PgConnectOptions::new()
                .host(host)
                .port(port)
                .username(username)
                .password(password)
                .database(database)
                .ssl_mode(PgSslMode::Require)  // Force SSL for DigitalOcean
                .application_name("openagents");

            // Enable detailed logging for troubleshooting
            options = options
                .log_statements(tracing::log::LevelFilter::Debug)
                .log_slow_statements(tracing::log::LevelFilter::Debug, std::time::Duration::from_secs(1))
                .statement_cache_capacity(0);
            
            error!("Database connection options configured:");
            error!("  Statement cache: DISABLED");
            error!("  Slow query logging: ENABLED (1 second threshold)");
            
            return options;
        }

        error!("!!! NO DATABASE_URL FOUND - USING CONFIG FILE !!!");
        error!("Using configuration file database settings");
        
        let ssl_mode = if self.require_ssl {
            error!("SSL is REQUIRED by configuration");
            PgSslMode::Require
        } else {
            error!("SSL is PREFERRED but not required");
            PgSslMode::Prefer
        };

        error!("!!! ACTUAL DATABASE CONNECTION DETAILS FROM CONFIG !!!");
        error!("  Host: {}", self.host);
        error!("  Port: {}", self.port);
        error!("  Username: {}", self.username);
        error!("  Database Name: {}", self.database_name);
        error!("  SSL Mode: {}", if self.require_ssl { "REQUIRE" } else { "PREFER" });
        error!("  Max Retries: {}", self.max_connection_retries);
        error!("  Retry Interval: {}s", self.retry_interval_secs);

        let mut options = PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
            .database(&self.database_name);

        options = options
            .log_statements(tracing::log::LevelFilter::Debug)
            .log_slow_statements(tracing::log::LevelFilter::Debug, std::time::Duration::from_secs(1))
            .statement_cache_capacity(0);

        options
    }
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    error!("!!! STARTING CONFIGURATION LOADING !!!");
    
    let base_path = std::env::current_dir()
        .expect("Failed to determine current directory")
        .join("configuration");

    error!("Configuration directory: {:?}", base_path);

    let environment: AppEnvironment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| {
            error!("APP_ENVIRONMENT not set, defaulting to 'local'");
            "local".into()
        })
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let environment_filename = format!("{}.yaml", environment.as_str());
    
    error!("!!! ENVIRONMENT DETAILS !!!");
    error!("  Current Environment: {}", environment.as_str());
    error!("  Config File: {}", environment_filename);

    let builder = Config::builder()
        .add_source(File::from(base_path.join("base.yaml")))
        .add_source(File::from(base_path.join(&environment_filename)))
        .add_source(
            ConfigEnvironment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        );

    // Override port with $PORT if it exists (App Platform requirement)
    let builder = if let Ok(port) = std::env::var("PORT") {
        error!("Using PORT from environment: {}", port);
        builder
            .set_override("application.port", port)?
            .set_override("application.host", "0.0.0.0")?
    } else {
        error!("No PORT environment variable found, using configuration value");
        builder
    };

    let settings = builder.build()?;
    let settings = settings.try_deserialize::<Settings>()?;

    error!("!!! FINAL CONFIGURATION !!!");
    error!("Application settings:");
    error!("  Host: {}", settings.application.host);
    error!("  Port: {}", settings.application.port);
    
    if let Ok(db_url) = std::env::var("DATABASE_URL") {
        error!("Database configuration source: DATABASE_URL");
        match Url::parse(&db_url) {
            Ok(url) => {
                error!("  Database Host: {}", url.host_str().unwrap_or("unknown"));
                error!("  Database Port: {}", url.port().unwrap_or(5432));
                error!("  Database Name: {}", url.path().trim_start_matches('/'));
                error!("  Username: {}", url.username());
            },
            Err(e) => {
                error!("!!! FAILED TO PARSE DATABASE_URL: {} !!!", e);
            }
        }
    } else {
        error!("Database configuration source: configuration file");
        error!("  Database Host: {}", settings.database.host);
        error!("  Database Port: {}", settings.database.port);
        error!("  Database Name: {}", settings.database.database_name);
        error!("  Username: {}", settings.database.username);
        error!("  SSL Required: {}", settings.database.require_ssl);
        error!("  Max Retries: {}", settings.database.max_connection_retries);
        error!("  Retry Interval: {}s", settings.database.retry_interval_secs);
    }

    Ok(settings)
}

pub enum AppEnvironment {
    Local,
    Production,
}

impl AppEnvironment {
    pub fn as_str(&self) -> &'static str {
        match self {
            AppEnvironment::Local => "local",
            AppEnvironment::Production => "production",
        }
    }
}

impl TryFrom<String> for AppEnvironment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => {
                error!("!!! INVALID ENVIRONMENT SPECIFIED: {} !!!", other);
                Err(format!(
                    "{} is not a supported environment. Use either `local` or `production`.",
                    other
                ))
            }
        }
    }
}
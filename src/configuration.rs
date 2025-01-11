use config::{Config, ConfigError, Environment as ConfigEnvironment, File};
use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use sqlx::ConnectOptions;
use tracing::{info, warn, error};

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
}

fn default_admin_token() -> String {
    "admin-token".to_string()
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> PgConnectOptions {
        error!("!!! CONNECTING TO DATABASE !!!");
        error!("  Host: {}", self.host);
        error!("  Port: {}", self.port);
        error!("  Database: {}", self.database_name);
        error!("  Username: {}", self.username);
        error!("  SSL Required: {}", self.require_ssl);

        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };

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
    error!("!!! LOADING CONFIGURATION !!!");
    
    let base_path = std::env::current_dir()
        .expect("Failed to determine current directory")
        .join("configuration");

    let environment: AppEnvironment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let environment_filename = format!("{}.yaml", environment.as_str());
    
    error!("Environment: {}", environment.as_str());
    error!("Loading config from: {}", environment_filename);

    let settings = Config::builder()
        .add_source(File::from(base_path.join("base.yaml")))
        .add_source(File::from(base_path.join(&environment_filename)))
        .add_source(
            ConfigEnvironment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    let settings = settings.try_deserialize::<Settings>()?;

    error!("!!! CONFIGURATION LOADED !!!");
    error!("Application:");
    error!("  Host: {}", settings.application.host);
    error!("  Port: {}", settings.application.port);
    error!("Database:");
    error!("  Host: {}", settings.database.host);
    error!("  Port: {}", settings.database.port);
    error!("  Name: {}", settings.database.database_name);
    error!("  Username: {}", settings.database.username);
    error!("  SSL Required: {}", settings.database.require_ssl);

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
                error!("Invalid environment: {}", other);
                Err(format!(
                    "{} is not a supported environment. Use either `local` or `production`.",
                    other
                ))
            }
        }
    }
}
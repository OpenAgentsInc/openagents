use config::{Config, ConfigError, Environment as ConfigEnvironment, File};
use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use sqlx::ConnectOptions;
use tracing::info;

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

impl DatabaseSettings {
    pub fn connect_options(&self) -> PgConnectOptions {
        // First check for App Platform's DATABASE_URL
        if std::env::var("DATABASE_URL").is_ok() {
            info!("Using App Platform database environment variables");
            let host = std::env::var("PGHOST").unwrap_or_else(|_| "localhost".to_string());
            let port = std::env::var("PGPORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(5432);
            let username = std::env::var("PGUSER").unwrap_or_else(|_| "postgres".to_string());
            let password = std::env::var("PGPASSWORD").unwrap_or_default();
            let database = std::env::var("PGDATABASE").unwrap_or_else(|_| "postgres".to_string());
            
            info!("Connecting to database at {}:{}", host, port);
            
            return PgConnectOptions::new()
                .host(&host)
                .port(port)
                .username(&username)
                .password(&password)
                .database(&database)
                .ssl_mode(PgSslMode::Require);
        }

        // Fall back to configuration file settings
        info!("Using configuration file database settings");
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

        options = options.log_statements(tracing::log::LevelFilter::Trace);
        options
    }
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    let base_path = std::env::current_dir()
        .expect("Failed to determine current directory")
        .join("configuration");

    let environment: AppEnvironment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let environment_filename = format!("{}.yaml", environment.as_str());

    info!("Loading configuration from {:?}", base_path);
    info!("Environment: {}", environment.as_str());
    info!("Config file: {}", environment_filename);

    let builder = Config::builder()
        .add_source(File::from(base_path.join("base.yaml")))
        .add_source(File::from(base_path.join(environment_filename)))
        .add_source(
            ConfigEnvironment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        );

    // Override port with $PORT if it exists (App Platform requirement)
    let builder = if let Ok(port) = std::env::var("PORT") {
        info!("Using PORT from environment: {}", port);
        builder.set_override("application.port", port)?
    } else {
        builder
    };

    let settings = builder.build()?;
    settings.try_deserialize::<Settings>()
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
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
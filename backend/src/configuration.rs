use config::{Config, ConfigError, Environment as ConfigEnvironment, File};
use secrecy::{ExposeSecret, Secret};
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::postgres::{PgConnectOptions, PgSslMode};
use tracing::{debug, error, info};
use url::Url;

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    #[serde(default)]
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
    #[serde(default)]
    pub username: String,
    #[serde(default = "default_password")]
    pub password: Secret<String>,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub database_name: String,
    #[serde(default = "default_true")]
    pub require_ssl: bool,
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            username: "postgres".to_string(),
            password: Secret::new("postgres".to_string()),
            port: 5432,
            host: "127.0.0.1".to_string(),
            database_name: "openagents".to_string(),
            require_ssl: false,
        }
    }
}

impl DatabaseSettings {
    pub fn connect_options(&self) -> PgConnectOptions {
        // First check for DATABASE_URL
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            info!("Using DATABASE_URL from environment");

            let url = match Url::parse(&database_url) {
                Ok(url) => {
                    debug!("Successfully parsed DATABASE_URL");
                    url
                }
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

            info!("Database connection details:");
            info!("  Host: {}", host);
            info!("  Port: {}", port);
            info!("  Username: {}", username);
            info!("  Database Name: {}", database);
            info!("  SSL Mode: PREFER (auto-detect)");

            return PgConnectOptions::new()
                .host(host)
                .port(port)
                .username(username)
                .password(password)
                .database(database)
                .ssl_mode(PgSslMode::Prefer);
        }

        info!("Using configuration file for database settings");

        let ssl_mode = if self.require_ssl {
            debug!("SSL is required by configuration");
            PgSslMode::Require
        } else {
            debug!("SSL is preferred but not required");
            PgSslMode::Prefer
        };

        debug!("Database connection details:");
        debug!("  Host: {}", self.host);
        debug!("  Port: {}", self.port);
        debug!("  Username: {}", self.username);
        debug!("  Database Name: {}", self.database_name);
        debug!(
            "  SSL Mode: {}",
            if self.require_ssl {
                "REQUIRE"
            } else {
                "PREFER"
            }
        );

        let mut options = PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode);

        // Only set database name if it's not empty
        if !self.database_name.is_empty() {
            options = options.database(&self.database_name);
        }

        options
    }
}

fn default_admin_token() -> String {
    "admin-token".to_string()
}

fn default_password() -> Secret<String> {
    Secret::new("postgres".to_string())
}

fn default_port() -> u16 {
    5432
}

fn default_true() -> bool {
    true
}

pub fn get_configuration() -> Result<Settings, ConfigError> {
    info!("Loading configuration...");

    let base_path = std::env::current_dir()
        .expect("Failed to determine current directory")
        .join("configuration");

    let environment: AppEnvironment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| {
            info!("APP_ENVIRONMENT not set, defaulting to 'local'");
            "local".into()
        })
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT");

    let environment_filename = format!("{}.yaml", environment.as_str());

    info!("Environment: {}", environment.as_str());
    debug!("Loading config from: {}", environment_filename);

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

    debug!("Configuration loaded successfully");
    debug!("Application settings:");
    debug!("  Host: {}", settings.application.host);
    debug!("  Port: {}", settings.application.port);

    // Only log database settings if DATABASE_URL is not present
    if std::env::var("DATABASE_URL").is_err() {
        debug!("Database settings:");
        debug!("  Host: {}", settings.database.host);
        debug!("  Port: {}", settings.database.port);
        debug!("  Name: {}", settings.database.database_name);
        debug!("  Username: {}", settings.database.username);
        debug!("  SSL Required: {}", settings.database.require_ssl);
    } else {
        info!("Using DATABASE_URL for database configuration");
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
                error!("Invalid environment: {}", other);
                Err(format!(
                    "{} is not a supported environment. Use either `local` or `production`.",
                    other
                ))
            }
        }
    }
}

//! The Postgres connection and this service's migrations, in their own
//! schema (`computers`) of the account database.

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_postgres::{Client, Config, NoTls};

/// Migrations, applied in order, each once, under an advisory lock.
const MIGRATIONS: &[(i32, &str)] = &[
    (1, include_str!("../migrations/0001_computers.sql")),
    (2, include_str!("../migrations/0002_turns.sql")),
    (3, include_str!("../migrations/0003_create_requests.sql")),
    (4, include_str!("../migrations/0004_commands.sql")),
    (5, include_str!("../migrations/0005_deployments.sql")),
    (6, include_str!("../migrations/0006_lifecycle_intents.sql")),
    (7, include_str!("../migrations/0007_usage.sql")),
];

#[derive(Debug)]
pub struct DbError(pub String);
impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DbError {}
impl From<tokio_postgres::Error> for DbError {
    fn from(e: tokio_postgres::Error) -> Self {
        // The server's message, never the connection string.
        Self(match e.as_db_error() {
            Some(db) => format!("database: {}", db.message()),
            None => format!("database: {e}"),
        })
    }
}

/// One shared client (queries pipeline over it), reconnected when closed.
#[derive(Clone)]
pub struct Db {
    config: Config,
    client: Arc<RwLock<Arc<Client>>>,
}

async fn open(config: &Config) -> Result<Client, DbError> {
    let (client, connection) = config.connect(NoTls).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute("SET openagents.usage_writer = 'v1'")
        .await?;
    Ok(client)
}

impl Db {
    /// Connect with a libpq string or URL and apply pending migrations.
    pub async fn connect(dsn: &str) -> Result<Self, DbError> {
        let mut config: Config = dsn
            .parse()
            .map_err(|e: tokio_postgres::Error| DbError(format!("connection string: {e}")))?;
        if config.get_application_name().is_none() {
            config.application_name("oa-computers");
        }
        config.connect_timeout(std::time::Duration::from_secs(10));
        let client = open(&config).await?;
        let db = Self {
            config,
            client: Arc::new(RwLock::new(Arc::new(client))),
        };
        db.migrate().await?;
        Ok(db)
    }

    /// The client, reconnecting first if the connection dropped.
    pub async fn client(&self) -> Result<Arc<Client>, DbError> {
        {
            let c = self.client.read().await;
            if !c.is_closed() {
                return Ok(c.clone());
            }
        }
        let mut c = self.client.write().await;
        if c.is_closed() {
            *c = Arc::new(open(&self.config).await?);
        }
        Ok(c.clone())
    }

    /// A separate connection for an admission transaction and its locks.
    pub(crate) async fn independent(&self) -> Result<Client, DbError> {
        open(&self.config).await
    }

    async fn migrate(&self) -> Result<(), DbError> {
        // Its own connection: the session lock and transactions stay there.
        let mut client = open(&self.config).await?;
        client
            .batch_execute(
                "SELECT pg_advisory_lock(hashtext('oa-computers-migrate'));
                 CREATE SCHEMA IF NOT EXISTS computers;
                 CREATE TABLE IF NOT EXISTS computers.migrations (
                     version integer PRIMARY KEY,
                     applied_at timestamptz NOT NULL DEFAULT now()
                 );",
            )
            .await?;
        for (version, sql) in MIGRATIONS {
            let tx = client.transaction().await?;
            let applied = tx
                .query_opt(
                    "SELECT 1 FROM computers.migrations WHERE version = $1",
                    &[version],
                )
                .await?
                .is_some();
            if !applied {
                tx.batch_execute(sql).await?;
                tx.execute(
                    "INSERT INTO computers.migrations (version) VALUES ($1)",
                    &[version],
                )
                .await?;
            }
            tx.commit().await?;
        }
        client
            .batch_execute("SELECT pg_advisory_unlock(hashtext('oa-computers-migrate'))")
            .await?;
        Ok(())
    }

    /// For tests: a fresh database on the server `admin_dsn` reaches.
    pub async fn scratch(admin_dsn: &str) -> Result<(Self, String), DbError> {
        let admin: Config = admin_dsn
            .parse()
            .map_err(|e: tokio_postgres::Error| DbError(format!("connection string: {e}")))?;
        let client = open(&admin).await?;
        let mut bytes = [0_u8; 6];
        getrandom::fill(&mut bytes).map_err(|e| DbError(e.to_string()))?;
        let name = format!(
            "oa_computers_t_{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        client
            .batch_execute(&format!("CREATE DATABASE {name}"))
            .await?;
        let dsn = format!("{admin_dsn} dbname={name}");
        Ok((Self::connect(&dsn).await?, dsn))
    }
}

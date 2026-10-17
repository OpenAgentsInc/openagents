use tokio_postgres::{Client, NoTls};

#[derive(Clone)]
pub struct Database {
    config: tokio_postgres::Config,
}

impl Database {
    pub fn new(dsn: &str) -> Result<Self, tokio_postgres::Error> {
        let mut config: tokio_postgres::Config = dsn.parse()?;
        config.connect_timeout(std::time::Duration::from_secs(5));
        Ok(Self { config })
    }

    pub async fn connect(&self) -> Result<Client, tokio_postgres::Error> {
        let (client, connection) = self.config.connect(NoTls).await?;
        tokio::spawn(async move {
            if connection.await.is_err() {
                tracing::warn!("Postgres connection closed");
            }
        });
        Ok(client)
    }

    pub async fn migrate(&self) -> Result<(), tokio_postgres::Error> {
        let client = self.connect().await?;
        client
            .batch_execute("BEGIN; SELECT pg_advisory_xact_lock(11842);")
            .await?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS coder_sync_schema (version INTEGER PRIMARY KEY)",
            )
            .await?;
        let versions = client
            .query("SELECT version FROM coder_sync_schema", &[])
            .await?
            .iter()
            .map(|r| r.get::<_, i32>(0))
            .collect::<std::collections::HashSet<_>>();
        for (version, sql) in [
            (1, include_str!("../../migrations/0001_rooms.sql")),
            (2, include_str!("../../migrations/0002_chat_protocol.sql")),
            (3, include_str!("../../migrations/0003_registry.sql")),
            (4, include_str!("../../migrations/0004_device.sql")),
            (5, include_str!("../../migrations/0005_blobs.sql")),
            (6, include_str!("../../migrations/0006_push_grants.sql")),
        ] {
            // Re-running ALTER TABLE, including IF NOT EXISTS, takes schema
            // locks that can deadlock with live rooms during a rollout.
            if !versions.contains(&version) {
                client.batch_execute(sql).await?;
            }
        }
        client.batch_execute("COMMIT").await
    }

    pub async fn healthy(&self) -> bool {
        match self.connect().await {
            Ok(client) => client.simple_query("SELECT 1").await.is_ok(),
            Err(_) => false,
        }
    }
}

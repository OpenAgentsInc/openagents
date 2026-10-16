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
            .batch_execute(include_str!("../../migrations/0001_rooms.sql"))
            .await?;
        client
            .batch_execute(include_str!("../../migrations/0002_chat_protocol.sql"))
            .await?;
        client.batch_execute("COMMIT").await
    }

    pub async fn healthy(&self) -> bool {
        match self.connect().await {
            Ok(client) => client.simple_query("SELECT 1").await.is_ok(),
            Err(_) => false,
        }
    }
}

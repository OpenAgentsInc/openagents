use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls};

#[derive(Clone)]
pub struct RuntimeDb {
    client: Arc<Mutex<Client>>,
}

impl RuntimeDb {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .context("connect to postgres")?;

        tokio::spawn(async move {
            if let Err(error) = connection.await {
                tracing::error!(reason = %error, "runtime postgres connection error");
            }
        });

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub fn client(&self) -> Arc<Mutex<Client>> {
        self.client.clone()
    }
}

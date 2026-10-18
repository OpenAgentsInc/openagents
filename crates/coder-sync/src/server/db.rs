use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_postgres::{Client, NoTls};

/// Connections kept open per service instance (#12120). Opening one costs a
/// Cloud SQL handshake and SCRAM authentication: 1.4 to 3.7 s against the
/// production `db-f1-micro`, ten times a query. Opened per job, that cost
/// queued every registry hello behind other devices' pushes past the
/// client's 20 s deadline. The cap stays well under that instance's
/// `max_connections` (25), leaving room for migrations and operators.
pub const POOL_SIZE: usize = 10;
/// A kept connection idle this long is closed rather than reused.
const MAX_IDLE: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct Database {
    config: tokio_postgres::Config,
    idle: Arc<Mutex<Vec<(Client, Instant)>>>,
    slots: Arc<Semaphore>,
}

/// A connection of the pool; it goes back when dropped, unless it closed.
pub struct Pooled {
    client: Option<Client>,
    idle: Arc<Mutex<Vec<(Client, Instant)>>>,
    _slot: OwnedSemaphorePermit,
}

impl Deref for Pooled {
    type Target = Client;
    fn deref(&self) -> &Client {
        self.client
            .as_ref()
            .expect("a pooled connection is present until drop")
    }
}

impl DerefMut for Pooled {
    fn deref_mut(&mut self) -> &mut Client {
        self.client
            .as_mut()
            .expect("a pooled connection is present until drop")
    }
}

impl Drop for Pooled {
    fn drop(&mut self) {
        // A transaction dropped without commit has queued its ROLLBACK on
        // this connection, ahead of the next user's statements.
        if let Some(client) = self.client.take()
            && !client.is_closed()
        {
            self.idle
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((client, Instant::now()));
        }
    }
}

impl Database {
    pub fn new(dsn: &str) -> Result<Self, tokio_postgres::Error> {
        Self::with_pool_size(dsn, POOL_SIZE)
    }

    pub fn with_pool_size(dsn: &str, size: usize) -> Result<Self, tokio_postgres::Error> {
        let mut config: tokio_postgres::Config = dsn.parse()?;
        config.connect_timeout(Duration::from_secs(5));
        Ok(Self {
            config,
            idle: Arc::default(),
            slots: Arc::new(Semaphore::new(size.max(1))),
        })
    }

    /// A connection: a kept one when one is idle, else a new one. At most
    /// the pool's size are out at once; more callers wait their turn.
    pub async fn connect(&self) -> Result<Pooled, tokio_postgres::Error> {
        let slot = Arc::clone(&self.slots)
            .acquire_owned()
            .await
            .expect("the pool's semaphore is never closed");
        let kept = {
            let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
            idle.retain(|(client, at)| !client.is_closed() && at.elapsed() < MAX_IDLE);
            idle.pop().map(|(client, _)| client)
        };
        let client = match kept {
            Some(client) => client,
            None => self.open().await?,
        };
        Ok(Pooled {
            client: Some(client),
            idle: Arc::clone(&self.idle),
            _slot: slot,
        })
    }

    /// A connection of its own, outside the pool (migrations hold a
    /// transaction across several statements by hand).
    async fn open(&self) -> Result<Client, tokio_postgres::Error> {
        let (client, connection) = self.config.connect(NoTls).await?;
        tokio::spawn(async move {
            if connection.await.is_err() {
                tracing::warn!("Postgres connection closed");
            }
        });
        Ok(client)
    }

    pub async fn migrate(&self) -> Result<(), tokio_postgres::Error> {
        let client = self.open().await?;
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
            (7, include_str!("../../migrations/0007_device_proofs.sql")),
            (
                8,
                include_str!("../../migrations/0008_owner_notifications.sql"),
            ),
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

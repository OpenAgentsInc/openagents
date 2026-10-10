//! Bounded PostgreSQL connections. Use a local socket or an authenticated database proxy.
//!
//! This module does not terminate TLS. Deployments must supply a protected local
//! connection (for example the Cloud SQL proxy); DSNs are never included in errors.
use crate::{ActorError, Result};
use std::{
    ops::{Deref, DerefMut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_postgres::{Client, Config, NoTls};

#[derive(Clone)]
pub struct Pool {
    inner: Arc<Inner>,
}
struct Inner {
    config: Config,
    idle: Mutex<Vec<Client>>,
    permits: Arc<Semaphore>,
    waiting: AtomicUsize,
    opened: AtomicUsize,
    max: usize,
}
#[derive(Clone, Debug, serde::Serialize)]
pub struct PoolStats {
    pub max: usize,
    pub active: usize,
    pub idle: usize,
    pub waiting: usize,
    pub opened: usize,
}
impl Pool {
    pub fn new(dsn: &str, max: usize) -> Result<Self> {
        if !(1..=128).contains(&max) {
            return Err(ActorError::new(
                "bad_args",
                "Choose between 1 and 128 database connections.",
            ));
        }
        let config: Config = dsn.parse().map_err(|_| {
            ActorError::new("bad_args", "The database connection settings are invalid.")
        })?;
        Ok(Self {
            inner: Arc::new(Inner {
                config,
                idle: Mutex::new(Vec::new()),
                permits: Arc::new(Semaphore::new(max)),
                waiting: AtomicUsize::new(0),
                opened: AtomicUsize::new(0),
                max,
            }),
        })
    }
    pub(crate) fn config(&self) -> Config {
        self.inner.config.clone()
    }
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            max: self.inner.max,
            active: self.inner.max - self.inner.permits.available_permits(),
            idle: self
                .inner
                .idle
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .len(),
            waiting: self.inner.waiting.load(Ordering::Relaxed),
            opened: self.inner.opened.load(Ordering::Relaxed),
        }
    }
    pub async fn acquire(&self) -> Result<Connection> {
        let old = self.inner.waiting.fetch_add(1, Ordering::AcqRel);
        let waiting = Waiting(&self.inner.waiting);
        if old >= 64 {
            return Err(ActorError::retry(
                "busy",
                "Too many requests are waiting. Try again shortly.",
            ));
        }
        let permit = tokio::time::timeout(
            Duration::from_secs(2),
            self.inner.permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| ActorError::retry("busy", "Storage is busy. Try again shortly."))?
        .map_err(|_| ActorError::retry("storage", "Storage is shutting down."))?;
        drop(waiting);
        let existing = {
            let mut idle = self.inner.idle.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                match idle.pop() {
                    Some(client) if !client.is_closed() => break Some(client),
                    Some(_) => continue,
                    None => break None,
                }
            }
        };
        let client = if let Some(client) = existing {
            client
        } else {
            tokio::time::timeout(Duration::from_secs(5),async {
                let (client, connection) = self.inner.config.connect(NoTls).await?;
                let mut driver = SetupDriver(Some(tokio::spawn(async move { let _ = connection.await; })));
                client.batch_execute("SET statement_timeout = '5s'; SET lock_timeout = '2s'; SET idle_in_transaction_session_timeout = '2s'").await?;
                let version: String = client.query_one("SHOW server_version_num", &[]).await?.get(0);
                if version.parse::<u32>().unwrap_or(0) >= 170000 {
                    client.batch_execute("SET transaction_timeout = '2s'").await?;
                }
                self.inner.opened.fetch_add(1, Ordering::Relaxed);
                driver.0.take(); // A fully configured client keeps its driver alive.
                Ok::<_,ActorError>(client)
            }).await.map_err(|_|ActorError::retry("storage","Storage did not respond in time."))??
        };
        Ok(Connection {
            client: Some(client),
            inner: self.inner.clone(),
            _permit: permit,
            reusable: true,
        })
    }
}
struct SetupDriver(Option<tokio::task::JoinHandle<()>>);
impl Drop for SetupDriver {
    fn drop(&mut self) {
        if let Some(driver) = self.0.take() {
            driver.abort();
        }
    }
}
struct Waiting<'a>(&'a AtomicUsize);
impl Drop for Waiting<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}
/// A checked-out connection owns its permit until the client is returned or dropped.
pub struct Connection {
    client: Option<Client>,
    inner: Arc<Inner>,
    _permit: OwnedSemaphorePermit,
    reusable: bool,
}
impl Connection {
    pub fn discard(&mut self) {
        self.reusable = false;
    }
}
impl Deref for Connection {
    type Target = Client;
    fn deref(&self) -> &Client {
        self.client.as_ref().expect("checked-out client")
    }
}
impl DerefMut for Connection {
    fn deref_mut(&mut self) -> &mut Client {
        self.client.as_mut().expect("checked-out client")
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            if self.reusable && !client.is_closed() {
                self.inner
                    .idle
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(client);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_configuration_without_echoing_it() {
        assert!(Pool::new("host=localhost", 0).is_err());
        assert!(Pool::new("host=localhost", 129).is_err());
        let error = match Pool::new("password='secret", 1) {
            Ok(_) => panic!("invalid settings accepted"),
            Err(e) => e,
        };
        assert!(!error.to_string().contains("secret"));
    }
    #[test]
    fn wait_counter_is_released_on_drop() {
        let n = AtomicUsize::new(1);
        {
            let _guard = Waiting(&n);
        }
        assert_eq!(n.load(Ordering::Relaxed), 0);
    }
}

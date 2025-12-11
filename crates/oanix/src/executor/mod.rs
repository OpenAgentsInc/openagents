//! Network executors for OANIX capability services.
//!
//! This module provides executors that bridge OANIX capability services
//! (HttpFs, WsFs, NostrFs) to actual network I/O.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐                     ┌──────────────────────────┐
//! │   OANIX      │◄──── Arc<*Fs> ────►│    ExecutorManager       │
//! │  (Sync)      │                     │       (Async/Tokio)      │
//! │              │                     │                          │
//! │ HttpFs       │                     │  HttpExecutor            │
//! │ WsFs         │                     │  WsConnector             │
//! │ NostrFs      │                     │  NostrRelayConnector     │
//! └──────────────┘                     └──────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use oanix::{HttpFs, WsFs, NostrFs};
//! use oanix::executor::{ExecutorManager, ExecutorConfig};
//! use std::sync::Arc;
//!
//! let http_fs = Arc::new(HttpFs::new());
//! let ws_fs = Arc::new(WsFs::new());
//!
//! let mut executor = ExecutorManager::new(ExecutorConfig::default())?;
//! executor.attach_http(http_fs);
//! executor.attach_ws(ws_fs);
//! executor.start()?;
//!
//! // ... use OANIX with real network I/O ...
//!
//! executor.shutdown()?;
//! ```

mod config;
mod error;
mod http;
mod ws;

#[cfg(feature = "nostr")]
mod nostr;

pub use config::{ExecutorConfig, ExecutorConfigBuilder, RetryPolicy};
pub use error::ExecutorError;
pub use http::HttpExecutor;
pub use ws::WsConnector;

#[cfg(feature = "nostr")]
pub use nostr::NostrRelayConnector;

use crate::{HttpFs, WsFs};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[cfg(feature = "nostr")]
use crate::NostrFs;

/// Manages all network executors for OANIX capability services.
///
/// The ExecutorManager owns a tokio runtime and coordinates the lifecycle
/// of all attached executors. It provides a unified interface for starting,
/// stopping, and monitoring executor health.
pub struct ExecutorManager {
    /// Configuration for all executors
    config: ExecutorConfig,
    /// Tokio runtime for async operations
    runtime: Runtime,
    /// Broadcast channel for shutdown signal
    shutdown_tx: broadcast::Sender<()>,
    /// Handle to HTTP executor task
    http_handle: Option<JoinHandle<()>>,
    /// Handle to WebSocket connector task
    ws_handle: Option<JoinHandle<()>>,
    /// Handle to Nostr relay connector task
    #[cfg(feature = "nostr")]
    nostr_handle: Option<JoinHandle<()>>,
    /// Attached HttpFs (if any)
    http_fs: Option<Arc<HttpFs>>,
    /// Attached WsFs (if any)
    ws_fs: Option<Arc<WsFs>>,
    /// Attached NostrFs (if any)
    #[cfg(feature = "nostr")]
    nostr_fs: Option<Arc<NostrFs>>,
    /// Whether the executor has been started
    started: bool,
}

impl ExecutorManager {
    /// Create a new executor manager with the given configuration.
    ///
    /// This creates a new tokio runtime for running async operations.
    pub fn new(config: ExecutorConfig) -> Result<Self, ExecutorError> {
        let runtime = Runtime::new()
            .map_err(|e| ExecutorError::Runtime(format!("Failed to create runtime: {}", e)))?;

        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            config,
            runtime,
            shutdown_tx,
            http_handle: None,
            ws_handle: None,
            #[cfg(feature = "nostr")]
            nostr_handle: None,
            http_fs: None,
            ws_fs: None,
            #[cfg(feature = "nostr")]
            nostr_fs: None,
            started: false,
        })
    }

    /// Attach an HttpFs for HTTP request execution.
    ///
    /// The executor will poll this HttpFs for pending requests and
    /// execute them using reqwest.
    pub fn attach_http(&mut self, http_fs: Arc<HttpFs>) {
        self.http_fs = Some(http_fs);
    }

    /// Attach a WsFs for WebSocket connection management.
    ///
    /// The executor will manage WebSocket connections for this WsFs,
    /// routing messages between the filesystem buffers and actual sockets.
    pub fn attach_ws(&mut self, ws_fs: Arc<WsFs>) {
        self.ws_fs = Some(ws_fs);
    }

    /// Attach a NostrFs for Nostr relay connectivity.
    ///
    /// The executor will connect to relays and route events between
    /// the NostrFs outbox/inbox and actual relay connections.
    #[cfg(feature = "nostr")]
    pub fn attach_nostr(&mut self, nostr_fs: Arc<NostrFs>) {
        self.nostr_fs = Some(nostr_fs);
    }

    /// Start all attached executors.
    ///
    /// This spawns async tasks for each attached service. The tasks will
    /// run until `shutdown()` is called.
    pub fn start(&mut self) -> Result<(), ExecutorError> {
        if self.started {
            return Err(ExecutorError::Runtime("Already started".to_string()));
        }

        // Start HTTP executor if attached
        if let Some(http_fs) = &self.http_fs {
            let executor = HttpExecutor::new(
                Arc::clone(http_fs),
                self.config.clone(),
                self.shutdown_tx.subscribe(),
            );

            let handle = self.runtime.spawn(async move {
                executor.run().await;
            });

            self.http_handle = Some(handle);
        }

        // Start WebSocket connector if attached
        if let Some(ws_fs) = &self.ws_fs {
            let connector = WsConnector::new(
                Arc::clone(ws_fs),
                self.config.clone(),
                self.shutdown_tx.subscribe(),
            );

            let handle = self.runtime.spawn(async move {
                connector.run().await;
            });

            self.ws_handle = Some(handle);
        }

        // Start Nostr relay connector if attached
        #[cfg(feature = "nostr")]
        if let Some(nostr_fs) = &self.nostr_fs {
            let ws_fs = self.ws_fs.clone();
            let connector = NostrRelayConnector::new(
                Arc::clone(nostr_fs),
                ws_fs,
                self.config.clone(),
                self.shutdown_tx.subscribe(),
            );

            let handle = self.runtime.spawn(async move {
                connector.run().await;
            });

            self.nostr_handle = Some(handle);
        }

        self.started = true;
        Ok(())
    }

    /// Gracefully shutdown all executors.
    ///
    /// This sends a shutdown signal to all running executors and waits
    /// for them to complete. The tokio runtime is then shut down.
    pub fn shutdown(self) -> Result<(), ExecutorError> {
        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Wait for all handles to complete
        self.runtime.block_on(async {
            if let Some(handle) = self.http_handle {
                let _ = handle.await;
            }
            if let Some(handle) = self.ws_handle {
                let _ = handle.await;
            }
            #[cfg(feature = "nostr")]
            if let Some(handle) = self.nostr_handle {
                let _ = handle.await;
            }
        });

        Ok(())
    }

    /// Check if the executor manager has been started.
    pub fn is_started(&self) -> bool {
        self.started
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Run an async future on the executor's runtime.
    ///
    /// This is useful for tests that need to run async code using the
    /// executor's runtime instead of creating a new one.
    pub fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.runtime.block_on(future)
    }

    /// Check if HttpFs is attached.
    pub fn has_http(&self) -> bool {
        self.http_fs.is_some()
    }

    /// Check if WsFs is attached.
    pub fn has_ws(&self) -> bool {
        self.ws_fs.is_some()
    }

    /// Check if NostrFs is attached.
    #[cfg(feature = "nostr")]
    pub fn has_nostr(&self) -> bool {
        self.nostr_fs.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_manager_creation() {
        let config = ExecutorConfig::default();
        let manager = ExecutorManager::new(config).unwrap();
        assert!(!manager.is_started());
        assert!(!manager.has_http());
        assert!(!manager.has_ws());
    }

    #[test]
    fn test_attach_services() {
        let config = ExecutorConfig::default();
        let mut manager = ExecutorManager::new(config).unwrap();

        let http_fs = Arc::new(HttpFs::new());
        let ws_fs = Arc::new(WsFs::new());

        manager.attach_http(http_fs);
        manager.attach_ws(ws_fs);

        assert!(manager.has_http());
        assert!(manager.has_ws());
    }

    #[test]
    fn test_cannot_start_twice() {
        let config = ExecutorConfig::default();
        let mut manager = ExecutorManager::new(config).unwrap();

        manager.start().unwrap();
        assert!(manager.is_started());

        let result = manager.start();
        assert!(matches!(result, Err(ExecutorError::Runtime(_))));
    }

    #[test]
    fn test_start_and_shutdown() {
        let config = ExecutorConfig::default();
        let mut manager = ExecutorManager::new(config).unwrap();

        let http_fs = Arc::new(HttpFs::new());
        manager.attach_http(http_fs);

        manager.start().unwrap();
        assert!(manager.is_started());

        manager.shutdown().unwrap();
    }
}

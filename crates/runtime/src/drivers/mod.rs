//! Driver interfaces for external event sources.

use crate::envelope::Envelope;
use crate::error::Result;
use crate::types::AgentId;
use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

/// Envelope routed to a specific agent.
#[derive(Clone, Debug)]
pub struct RoutedEnvelope {
    /// Target agent id.
    pub agent_id: AgentId,
    /// Envelope payload.
    pub envelope: Envelope,
}

/// Channel used by drivers to deliver envelopes.
pub type EnvelopeSink = mpsc::Sender<RoutedEnvelope>;

/// Handle to a running driver task.
pub struct DriverHandle {
    /// Unique driver instance id.
    pub id: String,
    stop_tx: oneshot::Sender<()>,
    task: JoinHandle<Result<()>>,
}

impl DriverHandle {
    /// Request the driver to stop and await shutdown.
    pub async fn stop(self) -> Result<()> {
        let _ = self.stop_tx.send(());
        match self.task.await {
            Ok(result) => result,
            Err(err) => Err(err.to_string().into()),
        }
    }
}

/// Driver trait for external event sources.
#[async_trait]
pub trait Driver: Send + Sync {
    /// Driver name for logging/metrics.
    fn name(&self) -> &str;

    /// Start the driver, sending envelopes to the provided sink.
    async fn start(&self, sink: EnvelopeSink) -> Result<DriverHandle>;

    /// Stop the driver gracefully.
    async fn stop(&self, handle: DriverHandle) -> Result<()> {
        handle.stop().await
    }
}

pub mod nostr;

pub use nostr::{NostrDriver, NostrDriverConfig, NostrPublishRequest};

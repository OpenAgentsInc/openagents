use anyhow::Result;
use async_trait::async_trait;
use foyer::{BlockEngineBuilder, DeviceBuilder, FsDeviceBuilder, HybridCache, HybridCacheBuilder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile;
use tokio::sync::mpsc;

use crate::{Example, Prediction};

type CacheKey = Vec<(String, Value)>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CallResult {
    pub prompt: String,
    pub prediction: Prediction,
}

#[async_trait]
pub trait Cache: Send + Sync {
    async fn new() -> Self;
    async fn get(&self, key: Example) -> Result<Option<Prediction>>;
    async fn insert(&mut self, key: Example, rx: mpsc::Receiver<CallResult>) -> Result<()>;
    async fn get_history(&self, n: usize) -> Result<Vec<CallResult>>;
}

#[derive(Clone)]
pub struct ResponseCache {
    handler: HybridCache<CacheKey, CallResult>,
    window_size: usize,
    history_window: Vec<CallResult>,
}

#[async_trait]
impl Cache for ResponseCache {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();

        let device = FsDeviceBuilder::new(dir.path())
            .with_capacity(1024 * 1024 * 1024)
            .build()
            .unwrap();

        let hybrid: HybridCache<CacheKey, CallResult> = HybridCacheBuilder::new()
            .memory(256 * 1024 * 1024)
            .storage()
            .with_engine_config(BlockEngineBuilder::new(device))
            .build()
            .await
            .unwrap();
        Self {
            handler: hybrid,
            window_size: 100,
            history_window: Vec::new(),
        }
    }

    async fn get(&self, key: Example) -> Result<Option<Prediction>> {
        let key = key.into_iter().collect::<CacheKey>();

        let value = self.handler.get(&key).await?.map(|v| v.value().clone());

        Ok(value.map(|entry| entry.prediction))
    }

    async fn insert(&mut self, key: Example, mut rx: mpsc::Receiver<CallResult>) -> Result<()> {
        let key = key.into_iter().collect::<CacheKey>();
        let value = rx.recv().await.unwrap();

        self.history_window.insert(0, value.clone());
        if self.history_window.len() > self.window_size {
            self.history_window.pop();
        }
        self.handler.insert(key, value.clone());

        Ok(())
    }

    async fn get_history(&self, n: usize) -> Result<Vec<CallResult>> {
        let actual_n = n.min(self.history_window.len());
        Ok(self.history_window[..actual_n].to_vec())
    }
}

//! FX conversion utilities for USD <-> sats settlement.

use crate::types::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// FX source configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FxSource {
    /// Fixed rate (sats per USD).
    Fixed {
        /// Satoshis per USD.
        sats_per_usd: u64,
    },
    /// Read from /wallet/fx via a wallet provider.
    Wallet,
    /// Fetch from an external oracle.
    Oracle {
        /// URL returning JSON with `sats_per_usd`.
        url: String,
    },
}

/// FX rate snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FxRateSnapshot {
    /// Satoshis per USD.
    pub sats_per_usd: u64,
    /// Timestamp when the rate was observed.
    pub updated_at: Timestamp,
}

/// FX provider trait for wallet-backed rates.
pub trait FxRateProvider: Send + Sync {
    /// Return the current FX rate snapshot.
    fn fx_rate(&self) -> Result<FxRateSnapshot, FxError>;
}

/// FX errors.
#[derive(Debug, thiserror::Error)]
pub enum FxError {
    /// FX data unavailable.
    #[error("fx rate unavailable")]
    Unavailable,
    /// Wallet source missing.
    #[error("wallet fx provider not configured")]
    WalletUnavailable,
    /// Oracle fetch failed.
    #[error("oracle error: {0}")]
    Oracle(String),
    /// Wallet error propagated through FX adapter.
    #[error("wallet error: {0}")]
    Wallet(String),
    /// Invalid oracle response.
    #[error("invalid fx response")]
    InvalidResponse,
}

#[cfg(not(target_arch = "wasm32"))]
use reqwest::Client;

/// FX rate cache with optional wallet and oracle support.
#[cfg(not(target_arch = "wasm32"))]
pub struct FxRateCache {
    source: FxSource,
    cache_secs: u64,
    wallet: Option<Arc<dyn FxRateProvider>>,
    client: Client,
    runtime: Arc<tokio::runtime::Runtime>,
    cached: Mutex<Option<FxRateSnapshot>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl FxRateCache {
    /// Create a new FX cache.
    pub fn new(
        source: FxSource,
        cache_secs: u64,
        wallet: Option<Arc<dyn FxRateProvider>>,
        runtime: Arc<tokio::runtime::Runtime>,
    ) -> Self {
        Self {
            source,
            cache_secs,
            wallet,
            client: Client::new(),
            runtime,
            cached: Mutex::new(None),
        }
    }

    /// Convert micro-USD to sats (rounded up).
    pub fn usd_to_sats(&self, micro_usd: u64) -> Result<u64, FxError> {
        let sats_per_usd = self.sats_per_usd()?;
        let numerator = u128::from(micro_usd) * u128::from(sats_per_usd);
        let sats = (numerator + 999_999) / 1_000_000;
        Ok(sats as u64)
    }

    /// Convert sats to micro-USD (rounded down).
    pub fn sats_to_usd(&self, sats: u64) -> Result<u64, FxError> {
        let sats_per_usd = self.sats_per_usd()?;
        if sats_per_usd == 0 {
            return Err(FxError::InvalidResponse);
        }
        let numerator = u128::from(sats) * 1_000_000u128;
        let micro_usd = numerator / u128::from(sats_per_usd);
        Ok(micro_usd as u64)
    }

    /// Return the latest FX snapshot (cached).
    pub fn snapshot(&self) -> Result<FxRateSnapshot, FxError> {
        let now = Timestamp::now();
        if self.cache_secs > 0 {
            if let Ok(guard) = self.cached.lock() {
                if let Some(snapshot) = guard.as_ref() {
                    let age_ms = now.as_millis().saturating_sub(snapshot.updated_at.as_millis());
                    if age_ms <= self.cache_secs.saturating_mul(1000) {
                        return Ok(snapshot.clone());
                    }
                }
            }
        }

        let snapshot = match &self.source {
            FxSource::Fixed { sats_per_usd } => {
                if *sats_per_usd == 0 {
                    return Err(FxError::InvalidResponse);
                }
                FxRateSnapshot {
                    sats_per_usd: *sats_per_usd,
                    updated_at: now,
                }
            }
            FxSource::Wallet => {
                let wallet = self.wallet.as_ref().ok_or(FxError::WalletUnavailable)?;
                wallet.fx_rate()?
            }
            FxSource::Oracle { url } => self.fetch_oracle(url)?,
        };

        if let Ok(mut guard) = self.cached.lock() {
            *guard = Some(snapshot.clone());
        }
        Ok(snapshot)
    }

    fn sats_per_usd(&self) -> Result<u64, FxError> {
        Ok(self.snapshot()?.sats_per_usd)
    }

    fn fetch_oracle(&self, url: &str) -> Result<FxRateSnapshot, FxError> {
        let client = self.client.clone();
        let url = url.to_string();
        let value = self
            .runtime
            .block_on(async move {
                client
                    .get(url)
                    .send()
                    .await
                    .map_err(|err| FxError::Oracle(err.to_string()))?
                    .json::<Value>()
                    .await
                    .map_err(|err| FxError::Oracle(err.to_string()))
            })?;

        let sats_per_usd = parse_sats_per_usd(&value)?;
        if sats_per_usd == 0 {
            return Err(FxError::InvalidResponse);
        }
        Ok(FxRateSnapshot {
            sats_per_usd,
            updated_at: Timestamp::now(),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_sats_per_usd(value: &Value) -> Result<u64, FxError> {
    match value.get("sats_per_usd") {
        Some(Value::Number(num)) => num
            .as_u64()
            .ok_or(FxError::InvalidResponse),
        Some(Value::String(s)) => s.parse::<u64>().map_err(|_| FxError::InvalidResponse),
        _ => Err(FxError::InvalidResponse),
    }
}

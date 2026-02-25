use std::time::Instant;

use autopilot_app::{DvmHistorySnapshot, DvmProviderStatus, PylonStatus};
use pylon::PylonConfig;
use pylon::db::{PylonDb, jobs::JobStatus};
use pylon::provider::{ProviderError, PylonProvider};

use crate::identity_domain::{load_or_init_identity, pylon_identity_exists};

pub(crate) struct InProcessPylon {
    provider: Option<PylonProvider>,
    started_at: Option<Instant>,
    last_error: Option<String>,
}

impl InProcessPylon {
    pub(crate) fn new() -> Self {
        Self {
            provider: None,
            started_at: None,
            last_error: None,
        }
    }
}

pub(crate) fn pylon_status_error(err: impl Into<String>) -> PylonStatus {
    PylonStatus {
        last_error: Some(err.into()),
        ..PylonStatus::default()
    }
}

pub(crate) fn dvm_provider_status_error(err: impl Into<String>) -> DvmProviderStatus {
    DvmProviderStatus {
        last_error: Some(err.into()),
        ..DvmProviderStatus::default()
    }
}

pub(crate) async fn init_pylon_identity(
    state: &mut InProcessPylon,
    config: &PylonConfig,
) -> PylonStatus {
    match load_or_init_identity(config) {
        Ok(_) => state.last_error = None,
        Err(err) => state.last_error = Some(err.to_string()),
    }
    refresh_pylon_status(state, config).await
}

pub(crate) async fn start_pylon_in_process(
    state: &mut InProcessPylon,
    config: &PylonConfig,
) -> PylonStatus {
    if let Some(provider) = state.provider.as_ref() {
        let provider_status = provider.status().await;
        if provider_status.running {
            state.last_error = None;
            return refresh_pylon_status(state, config).await;
        }
    }

    let identity = match load_or_init_identity(config) {
        Ok(identity) => identity,
        Err(err) => {
            state.last_error = Some(err.to_string());
            return refresh_pylon_status(state, config).await;
        }
    };

    let mut provider = match state.provider.take() {
        Some(provider) => provider,
        None => match PylonProvider::new(config.clone()).await {
            Ok(provider) => provider,
            Err(err) => {
                state.last_error = Some(err.to_string());
                return refresh_pylon_status(state, config).await;
            }
        },
    };

    if let Err(err) = provider.init_with_identity(identity).await {
        state.last_error = Some(err.to_string());
        state.provider = None;
        state.started_at = None;
        return refresh_pylon_status(state, config).await;
    }

    let provider_status = provider.status().await;
    if provider_status.backends.is_empty() && provider_status.agent_backends.is_empty() {
        state.last_error =
            Some("No provider backends detected (inference or Codex agent).".to_string());
        state.provider = None;
        state.started_at = None;
        return refresh_pylon_status(state, config).await;
    }

    match provider.start().await {
        Ok(()) | Err(ProviderError::AlreadyRunning) => {
            if state.started_at.is_none() {
                state.started_at = Some(Instant::now());
            }
            state.last_error = None;
            state.provider = Some(provider);
        }
        Err(err) => {
            state.last_error = Some(err.to_string());
            state.provider = None;
            state.started_at = None;
        }
    }

    refresh_pylon_status(state, config).await
}

pub(crate) async fn stop_pylon_in_process(
    state: &mut InProcessPylon,
    config: &PylonConfig,
) -> PylonStatus {
    if let Some(provider) = state.provider.as_mut() {
        match provider.stop().await {
            Ok(()) | Err(ProviderError::NotRunning) => {
                state.started_at = None;
                state.last_error = None;
            }
            Err(err) => {
                state.last_error = Some(err.to_string());
            }
        }
    } else {
        state.started_at = None;
    }

    refresh_pylon_status(state, config).await
}

pub(crate) async fn refresh_pylon_status(
    state: &mut InProcessPylon,
    config: &PylonConfig,
) -> PylonStatus {
    let identity_exists = pylon_identity_exists(config);
    let (running, jobs_completed, earnings_msats) = if let Some(provider) = state.provider.as_ref()
    {
        let provider_status = provider.status().await;
        (
            provider_status.running,
            provider_status.jobs_processed,
            provider_status.total_earnings_msats,
        )
    } else {
        (false, 0, 0)
    };

    if running && state.started_at.is_none() {
        state.started_at = Some(Instant::now());
    }
    if !running {
        state.started_at = None;
    }

    PylonStatus {
        running,
        pid: None,
        uptime_secs: state.started_at.as_ref().map(|t| t.elapsed().as_secs()),
        provider_active: Some(running),
        host_active: Some(false),
        jobs_completed,
        earnings_msats,
        identity_exists,
        last_error: state.last_error.clone(),
    }
}

pub(crate) async fn fetch_dvm_provider_status(
    state: &mut InProcessPylon,
    config: &PylonConfig,
) -> DvmProviderStatus {
    let provider_status = if let Some(provider) = state.provider.as_ref() {
        Some(provider.status().await)
    } else {
        None
    };
    let running = provider_status
        .as_ref()
        .map(|status| status.running)
        .unwrap_or(false);
    let agent_backends = provider_status
        .as_ref()
        .map(|status| status.agent_backends.clone())
        .unwrap_or_default();
    let supported_bazaar_kinds = provider_status
        .as_ref()
        .map(|status| status.supported_bazaar_kinds.clone())
        .unwrap_or_default();

    DvmProviderStatus {
        running,
        provider_active: Some(running),
        host_active: Some(false),
        min_price_msats: config.min_price_msats,
        require_payment: config.require_payment,
        default_model: config.default_model.clone(),
        backend_preference: config.backend_preference.clone(),
        agent_backends,
        supported_bazaar_kinds,
        network: config.network.clone(),
        enable_payments: config.enable_payments,
        last_error: state.last_error.clone(),
    }
}

pub(crate) fn fetch_dvm_history() -> DvmHistorySnapshot {
    let mut snapshot = DvmHistorySnapshot::default();

    let config = match PylonConfig::load() {
        Ok(config) => config,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load Pylon config: {err}"));
            return snapshot;
        }
    };

    let data_dir = match config.data_path() {
        Ok(path) => path,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to resolve Pylon data dir: {err}"));
            return snapshot;
        }
    };

    let path = data_dir.join("pylon.db");

    let db = match PylonDb::open(path) {
        Ok(db) => db,
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to open Pylon DB: {err}"));
            return snapshot;
        }
    };

    match db.get_earnings_summary() {
        Ok(summary) => {
            snapshot.summary.total_msats = summary.total_msats;
            snapshot.summary.total_sats = summary.total_sats;
            snapshot.summary.job_count = summary.job_count;
            let mut sources = summary
                .by_source
                .into_iter()
                .collect::<Vec<(String, u64)>>();
            sources.sort_by(|a, b| a.0.cmp(&b.0));
            snapshot.summary.by_source = sources;
        }
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load earnings summary: {err}"));
        }
    }

    match db.count_jobs_by_status() {
        Ok(counts) => {
            let mut status_counts = counts
                .into_iter()
                .map(|(status, count)| (status.as_str().to_string(), count))
                .collect::<Vec<_>>();
            status_counts.sort_by(|a, b| a.0.cmp(&b.0));
            snapshot.status_counts = status_counts;
        }
        Err(err) => {
            snapshot.last_error = Some(format!("Failed to load job counts: {err}"));
        }
    }

    let mut jobs = Vec::new();
    for status in [
        JobStatus::Completed,
        JobStatus::Failed,
        JobStatus::Processing,
        JobStatus::Pending,
    ] {
        if let Ok(list) = db.list_jobs_by_status(status, 25) {
            jobs.extend(list);
        }
    }
    jobs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    jobs.truncate(25);
    snapshot.jobs = jobs
        .into_iter()
        .map(|job| autopilot_app::DvmJobSummary {
            id: job.id,
            status: job.status.as_str().to_string(),
            kind: job.kind,
            price_msats: job.price_msats,
            created_at: job.created_at,
        })
        .collect();

    snapshot
}

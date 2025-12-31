//! Domain events for the compute provider
//!
//! These events represent state changes in the application and can be used
//! for logging, UI updates, and persistence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Events that occur in the compute provider domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    // Identity events
    /// A new identity was created (seed phrase generated)
    IdentityCreated {
        npub: String,
        timestamp: DateTime<Utc>,
    },
    /// An existing identity was loaded from storage
    IdentityLoaded {
        npub: String,
        timestamp: DateTime<Utc>,
    },

    // Online status events
    /// Provider went online and is accepting jobs
    WentOnline {
        timestamp: DateTime<Utc>,
        relays: Vec<String>,
    },
    /// Provider went offline
    WentOffline { timestamp: DateTime<Utc> },

    // Job events
    /// A new job request was received
    JobReceived {
        job_id: String,
        kind: u16,
        customer_pubkey: String,
        timestamp: DateTime<Utc>,
    },
    /// Job processing started
    JobStarted {
        job_id: String,
        model: String,
        timestamp: DateTime<Utc>,
    },
    /// Job processing progress update
    JobProgress {
        job_id: String,
        progress: f32,
        timestamp: DateTime<Utc>,
    },
    /// Job completed successfully
    JobCompleted {
        job_id: String,
        amount_msats: Option<u64>,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    /// Job failed with an error
    JobFailed {
        job_id: String,
        error: String,
        timestamp: DateTime<Utc>,
    },

    // Payment events
    /// Payment was received for a job
    PaymentReceived {
        job_id: String,
        amount_msats: u64,
        timestamp: DateTime<Utc>,
    },
    /// Invoice was created for a job
    InvoiceCreated {
        job_id: String,
        bolt11: String,
        amount_msats: u64,
        timestamp: DateTime<Utc>,
    },

    // Network events
    /// Connected to a Nostr relay
    RelayConnected {
        url: String,
        timestamp: DateTime<Utc>,
    },
    /// Disconnected from a Nostr relay
    RelayDisconnected {
        url: String,
        reason: Option<String>,
        timestamp: DateTime<Utc>,
    },

    // Backend events
    /// A backend became available
    BackendAvailable {
        backend_id: String,
        timestamp: DateTime<Utc>,
    },
    /// A backend became unavailable
    BackendUnavailable {
        backend_id: String,
        timestamp: DateTime<Utc>,
    },
    /// Backends were registered after detection
    BackendsRegistered {
        backend_ids: Vec<String>,
        timestamp: DateTime<Utc>,
    },
    /// Available models were refreshed from backends
    ModelsRefreshed {
        backend_id: String,
        models: Vec<String>,
        timestamp: DateTime<Utc>,
    },

    // Wallet events
    /// Wallet balance was updated
    BalanceUpdated {
        balance_sats: u64,
        timestamp: DateTime<Utc>,
    },
}

impl DomainEvent {
    /// Get the timestamp of the event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            DomainEvent::IdentityCreated { timestamp, .. } => *timestamp,
            DomainEvent::IdentityLoaded { timestamp, .. } => *timestamp,
            DomainEvent::WentOnline { timestamp, .. } => *timestamp,
            DomainEvent::WentOffline { timestamp, .. } => *timestamp,
            DomainEvent::JobReceived { timestamp, .. } => *timestamp,
            DomainEvent::JobStarted { timestamp, .. } => *timestamp,
            DomainEvent::JobProgress { timestamp, .. } => *timestamp,
            DomainEvent::JobCompleted { timestamp, .. } => *timestamp,
            DomainEvent::JobFailed { timestamp, .. } => *timestamp,
            DomainEvent::PaymentReceived { timestamp, .. } => *timestamp,
            DomainEvent::InvoiceCreated { timestamp, .. } => *timestamp,
            DomainEvent::RelayConnected { timestamp, .. } => *timestamp,
            DomainEvent::RelayDisconnected { timestamp, .. } => *timestamp,
            DomainEvent::BackendAvailable { timestamp, .. } => *timestamp,
            DomainEvent::BackendUnavailable { timestamp, .. } => *timestamp,
            DomainEvent::BackendsRegistered { timestamp, .. } => *timestamp,
            DomainEvent::ModelsRefreshed { timestamp, .. } => *timestamp,
            DomainEvent::BalanceUpdated { timestamp, .. } => *timestamp,
        }
    }

    /// Get a short description of the event for logging
    pub fn description(&self) -> String {
        match self {
            DomainEvent::IdentityCreated { npub, .. } => {
                format!("Identity created: {}", truncate_npub(npub))
            }
            DomainEvent::IdentityLoaded { npub, .. } => {
                format!("Identity loaded: {}", truncate_npub(npub))
            }
            DomainEvent::WentOnline { relays, .. } => {
                format!("Went online ({} relays)", relays.len())
            }
            DomainEvent::WentOffline { .. } => "Went offline".to_string(),
            DomainEvent::JobReceived { job_id, kind, .. } => {
                format!(
                    "Job received: {} (kind {})",
                    &job_id[..8.min(job_id.len())],
                    kind
                )
            }
            DomainEvent::JobStarted { job_id, model, .. } => {
                format!(
                    "Job started: {} ({})",
                    &job_id[..8.min(job_id.len())],
                    model
                )
            }
            DomainEvent::JobProgress {
                job_id, progress, ..
            } => {
                format!(
                    "Job progress: {} ({:.0}%)",
                    &job_id[..8.min(job_id.len())],
                    progress * 100.0
                )
            }
            DomainEvent::JobCompleted {
                job_id,
                amount_msats,
                ..
            } => {
                let amt = amount_msats
                    .map(|a| format!(" ({} sats)", a / 1000))
                    .unwrap_or_default();
                format!("Job completed: {}{}", &job_id[..8.min(job_id.len())], amt)
            }
            DomainEvent::JobFailed { job_id, error, .. } => {
                format!("Job failed: {} - {}", &job_id[..8.min(job_id.len())], error)
            }
            DomainEvent::PaymentReceived { amount_msats, .. } => {
                format!("Payment received: {} sats", amount_msats / 1000)
            }
            DomainEvent::InvoiceCreated { amount_msats, .. } => {
                format!("Invoice created: {} sats", amount_msats / 1000)
            }
            DomainEvent::RelayConnected { url, .. } => format!("Relay connected: {}", url),
            DomainEvent::RelayDisconnected { url, reason, .. } => {
                let r = reason
                    .as_ref()
                    .map(|s| format!(" ({})", s))
                    .unwrap_or_default();
                format!("Relay disconnected: {}{}", url, r)
            }
            DomainEvent::BackendAvailable { backend_id, .. } => {
                format!("Backend available: {}", backend_id)
            }
            DomainEvent::BackendUnavailable { backend_id, .. } => {
                format!("Backend unavailable: {}", backend_id)
            }
            DomainEvent::BackendsRegistered { backend_ids, .. } => {
                format!("Backends registered: {}", backend_ids.join(", "))
            }
            DomainEvent::ModelsRefreshed {
                backend_id, models, ..
            } => {
                format!(
                    "Models refreshed ({backend_id}): {} available",
                    models.len()
                )
            }
            DomainEvent::BalanceUpdated { balance_sats, .. } => {
                format!("Balance: {} sats", balance_sats)
            }
        }
    }
}

fn truncate_npub(npub: &str) -> String {
    if npub.len() > 16 {
        format!("{}...{}", &npub[..12], &npub[npub.len() - 4..])
    } else {
        npub.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_description() {
        let event = DomainEvent::WentOnline {
            timestamp: Utc::now(),
            relays: vec![
                "wss://relay1.com".to_string(),
                "wss://relay2.com".to_string(),
            ],
        };
        assert_eq!(event.description(), "Went online (2 relays)");
    }

    #[test]
    fn test_truncate_npub() {
        let npub = "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
        let truncated = truncate_npub(npub);
        assert!(truncated.starts_with("npub1"));
        assert!(truncated.contains("..."));
    }
}

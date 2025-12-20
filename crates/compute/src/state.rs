//! Reactive application state using Signal<T>

use crate::domain::{DomainEvent, EarningsTracker, Job, UnifiedIdentity};
use coder_ui_runtime::Signal;
use std::sync::Arc;

/// Model information from Ollama
#[derive(Debug, Clone)]
pub struct OllamaModel {
    /// Model name (e.g., "llama3:8b")
    pub name: String,
    /// Model size (e.g., "4.7 GB")
    pub size: String,
    /// Quantization level (e.g., "Q4_0")
    pub quantization: Option<String>,
    /// Whether this model is selected for serving
    pub selected: bool,
}

/// Reactive application state
pub struct AppState {
    // Identity
    /// The user's unified identity (Nostr + Spark)
    pub identity: Signal<Option<Arc<UnifiedIdentity>>>,
    /// Whether to show backup screen
    pub show_backup: Signal<bool>,
    /// Whether the seed has been backed up
    pub is_backed_up: Signal<bool>,

    // Online status
    /// Whether the provider is online and accepting jobs
    pub is_online: Signal<bool>,

    // Wallet
    /// Current wallet balance in sats
    pub balance_sats: Signal<u64>,
    /// Spark address for receiving payments
    pub spark_address: Signal<String>,

    // Earnings
    /// Earnings tracking (today/week/all-time)
    pub earnings: Signal<EarningsTracker>,

    // Models
    /// Available Ollama models
    pub available_models: Signal<Vec<OllamaModel>>,
    /// Currently selected model for serving
    pub selected_model: Signal<Option<String>>,
    /// Whether Ollama is available
    pub ollama_available: Signal<bool>,

    // Network
    /// Connected Nostr relays
    pub connected_relays: Signal<Vec<String>>,
    /// Number of pending jobs
    pub pending_jobs: Signal<u32>,

    // Jobs
    /// Currently active jobs
    pub active_jobs: Signal<Vec<Job>>,
    /// Recently completed jobs (for display)
    pub completed_jobs: Signal<Vec<Job>>,

    // Events
    /// Recent domain events for the event log
    pub event_log: Signal<Vec<DomainEvent>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    /// Create a new application state with default values
    pub fn new() -> Self {
        Self {
            identity: Signal::new(None),
            show_backup: Signal::new(false),
            is_backed_up: Signal::new(false),
            is_online: Signal::new(false),
            balance_sats: Signal::new(0),
            spark_address: Signal::new(String::new()),
            earnings: Signal::new(EarningsTracker::new()),
            available_models: Signal::new(Vec::new()),
            selected_model: Signal::new(None),
            ollama_available: Signal::new(false),
            connected_relays: Signal::new(Vec::new()),
            pending_jobs: Signal::new(0),
            active_jobs: Signal::new(Vec::new()),
            completed_jobs: Signal::new(Vec::new()),
            event_log: Signal::new(Vec::new()),
        }
    }

    /// Set the identity
    pub fn set_identity(&self, identity: UnifiedIdentity) {
        // Update spark_address signal with the public key
        let spark_pubkey = identity.spark_public_key_hex();
        self.spark_address.set(spark_pubkey);

        self.identity.set(Some(Arc::new(identity)));
    }

    /// Clear the identity
    pub fn clear_identity(&self) {
        self.identity.set(None);
        self.spark_address.set(String::new());
    }

    /// Show backup screen
    pub fn show_backup_screen(&self) {
        self.show_backup.set(true);
    }

    /// Hide backup screen
    pub fn hide_backup_screen(&self) {
        self.show_backup.set(false);
    }

    /// Mark seed as backed up
    pub fn mark_backed_up(&self) {
        self.is_backed_up.set(true);
        self.show_backup.set(false);
    }

    /// Toggle online status
    pub fn toggle_online(&self) {
        let current = self.is_online.get();
        self.is_online.set(!current);
    }

    /// Go online
    pub fn go_online(&self) {
        self.is_online.set(true);
    }

    /// Go offline
    pub fn go_offline(&self) {
        self.is_online.set(false);
    }

    /// Add a domain event to the log
    pub fn log_event(&self, event: DomainEvent) {
        self.event_log.update(|events| {
            events.push(event);
            // Keep only the last 100 events
            if events.len() > 100 {
                events.remove(0);
            }
        });
    }

    /// Add an active job
    pub fn add_job(&self, job: Job) {
        self.active_jobs.update(|jobs| {
            jobs.push(job);
        });
        self.pending_jobs.update(|count| *count += 1);
    }

    /// Update a job's status
    pub fn update_job<F>(&self, job_id: &str, f: F)
    where
        F: FnOnce(&mut Job),
    {
        self.active_jobs.update(|jobs| {
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                f(job);
            }
        });
    }

    /// Complete a job (move from active to completed)
    pub fn complete_job(&self, job_id: &str) {
        let mut completed_job = None;

        self.active_jobs.update(|jobs| {
            if let Some(pos) = jobs.iter().position(|j| j.id == job_id) {
                completed_job = Some(jobs.remove(pos));
            }
        });

        if let Some(job) = completed_job {
            self.completed_jobs.update(|jobs| {
                jobs.insert(0, job);
                // Keep only last 50 completed jobs
                if jobs.len() > 50 {
                    jobs.pop();
                }
            });
            self.pending_jobs.update(|count| {
                if *count > 0 {
                    *count -= 1;
                }
            });
        }
    }

    /// Record a payment
    pub fn record_payment(&self, amount_msats: u64) {
        self.earnings.update(|e| {
            e.record_payment(amount_msats);
        });
        self.balance_sats.update(|b| {
            *b += amount_msats / 1000;
        });
    }

    /// Set available models
    pub fn set_models(&self, models: Vec<OllamaModel>) {
        self.available_models.set(models);
    }

    /// Toggle model selection
    pub fn toggle_model(&self, model_name: &str) {
        self.available_models.update(|models| {
            if let Some(model) = models.iter_mut().find(|m| m.name == model_name) {
                model.selected = !model.selected;
            }
        });

        // Update selected model
        let selected = self.available_models.get().iter().find(|m| m.selected).map(|m| m.name.clone());
        self.selected_model.set(selected);
    }

    /// Add a connected relay
    pub fn add_relay(&self, url: String) {
        self.connected_relays.update(|relays| {
            if !relays.contains(&url) {
                relays.push(url);
            }
        });
    }

    /// Remove a disconnected relay
    pub fn remove_relay(&self, url: &str) {
        self.connected_relays.update(|relays| {
            relays.retain(|r| r != url);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_default() {
        let state = AppState::new();
        assert!(!state.show_backup.get());
        assert!(!state.is_backed_up.get());
        assert!(!state.is_online.get());
        assert_eq!(state.balance_sats.get(), 0);
    }

    #[test]
    fn test_toggle_online() {
        let state = AppState::new();
        assert!(!state.is_online.get());

        state.toggle_online();
        assert!(state.is_online.get());

        state.toggle_online();
        assert!(!state.is_online.get());
    }

    #[test]
    fn test_record_payment() {
        let state = AppState::new();

        state.record_payment(100_000); // 100 sats
        assert_eq!(state.balance_sats.get(), 100);
        assert_eq!(state.earnings.get().today_sats, 100);
    }
}

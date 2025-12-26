//! Compute Buyer Implementation
//!
//! The ComputeBuyer manages requesting compute jobs from the NIP-90 marketplace.

use std::collections::HashMap;
use std::time::Duration;

use compute::domain::{
    PriceBook, Quote, RepoIndexRequest, RepoIndexResult, SandboxRunRequest, SandboxRunResult,
};
use nostr::nip90::{JobRequest, KIND_JOB_REPO_INDEX, KIND_JOB_SANDBOX_RUN};

use super::{
    BidStrategy, ComputeBuyerError, JobOutcome, JobResultData, JobStatus, PendingJob, ProviderBid,
};

/// Configuration for the compute buyer
#[derive(Debug, Clone)]
pub struct ComputeBuyerConfig {
    /// Maximum time to wait for bids (seconds)
    pub bid_timeout_secs: u32,
    /// Maximum time to wait for job completion (seconds)
    pub job_timeout_secs: u32,
    /// Bid selection strategy
    pub bid_strategy: BidStrategy,
    /// Price book for estimating costs
    pub price_book: PriceBook,
    /// Maximum percentage over quote to accept
    pub max_price_premium_pct: u8,
    /// Preferred relays for job requests
    pub preferred_relays: Vec<String>,
}

impl Default for ComputeBuyerConfig {
    fn default() -> Self {
        Self {
            bid_timeout_secs: 30,
            job_timeout_secs: 600, // 10 minutes
            bid_strategy: BidStrategy::LowestPrice,
            price_book: PriceBook::new(),
            max_price_premium_pct: 20,
            preferred_relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://relay.nostr.band".to_string(),
                "wss://nos.lol".to_string(),
            ],
        }
    }
}

impl ComputeBuyerConfig {
    /// Create a new config with custom bid timeout
    pub fn with_bid_timeout(mut self, secs: u32) -> Self {
        self.bid_timeout_secs = secs;
        self
    }

    /// Create a new config with custom job timeout
    pub fn with_job_timeout(mut self, secs: u32) -> Self {
        self.job_timeout_secs = secs;
        self
    }

    /// Set bid strategy
    pub fn with_strategy(mut self, strategy: BidStrategy) -> Self {
        self.bid_strategy = strategy;
        self
    }

    /// Set preferred relays
    pub fn with_relays(mut self, relays: Vec<String>) -> Self {
        self.preferred_relays = relays;
        self
    }
}

/// Compute buyer for requesting jobs from the NIP-90 marketplace
pub struct ComputeBuyer {
    /// Configuration
    config: ComputeBuyerConfig,
    /// Pending jobs by request ID
    pending_jobs: HashMap<String, PendingJob>,
}

impl ComputeBuyer {
    /// Create a new compute buyer
    pub fn new(config: ComputeBuyerConfig) -> Self {
        Self {
            config,
            pending_jobs: HashMap::new(),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(ComputeBuyerConfig::default())
    }

    /// Get a quote for a sandbox run
    pub fn quote_sandbox_run(&self, request: &SandboxRunRequest) -> Quote {
        let max_time = request.limits.max_time_secs as f64;
        let max_memory_gb_mins =
            (request.limits.max_memory_mb as f64 / 1024.0) * (max_time / 60.0);

        self.config
            .price_book
            .sandbox_run
            .calculate(max_time, max_memory_gb_mins);

        compute::domain::quote_sandbox_run(
            &self.config.price_book.sandbox_run,
            request.limits.max_time_secs,
            request.limits.max_memory_mb,
        )
    }

    /// Get a quote for repo indexing
    pub fn quote_repo_index(
        &self,
        estimated_tokens: u64,
        estimated_files: u32,
        include_embeddings: bool,
    ) -> Quote {
        compute::domain::quote_repo_index(
            &self.config.price_book.repo_index,
            estimated_tokens,
            estimated_files,
            include_embeddings,
        )
    }

    /// Calculate maximum price we'll pay (quote + premium)
    pub fn max_price(&self, quote: &Quote) -> u64 {
        let premium = (quote.price_sats as f64 * (self.config.max_price_premium_pct as f64 / 100.0))
            as u64;
        quote.price_sats + premium
    }

    /// Submit a sandbox run request
    ///
    /// Returns the request ID for tracking
    pub fn submit_sandbox_run(
        &mut self,
        request: &SandboxRunRequest,
    ) -> Result<String, ComputeBuyerError> {
        let job_request = request.to_job_request()?;
        let quote = self.quote_sandbox_run(request);
        let max_price = self.max_price(&quote);

        // Generate a request ID (in production this would be the event ID)
        let request_id = format!("sandbox-{}", std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos());

        let pending = PendingJob::new(&request_id, KIND_JOB_SANDBOX_RUN, max_price);
        self.pending_jobs.insert(request_id.clone(), pending);

        Ok(request_id)
    }

    /// Submit a repo index request
    ///
    /// Returns the request ID for tracking
    pub fn submit_repo_index(
        &mut self,
        request: &RepoIndexRequest,
        estimated_tokens: u64,
        estimated_files: u32,
    ) -> Result<String, ComputeBuyerError> {
        let job_request = request.to_job_request()?;
        let include_embeddings = request
            .index_types
            .iter()
            .any(|t| matches!(t, compute::domain::IndexType::Embeddings | compute::domain::IndexType::All));

        let quote = self.quote_repo_index(estimated_tokens, estimated_files, include_embeddings);
        let max_price = self.max_price(&quote);

        let request_id = format!("index-{}", std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos());

        let pending = PendingJob::new(&request_id, KIND_JOB_REPO_INDEX, max_price);
        self.pending_jobs.insert(request_id.clone(), pending);

        Ok(request_id)
    }

    /// Process an incoming bid for a pending job
    pub fn process_bid(&mut self, request_id: &str, bid: ProviderBid) -> bool {
        if let Some(job) = self.pending_jobs.get_mut(request_id) {
            if matches!(job.status, JobStatus::WaitingForBids) {
                job.add_bid(bid);
                return true;
            }
        }
        false
    }

    /// Get pending job by ID
    pub fn get_pending_job(&self, request_id: &str) -> Option<&PendingJob> {
        self.pending_jobs.get(request_id)
    }

    /// Get pending job mutably
    pub fn get_pending_job_mut(&mut self, request_id: &str) -> Option<&mut PendingJob> {
        self.pending_jobs.get_mut(request_id)
    }

    /// Select a provider for a job
    pub fn select_provider(&mut self, request_id: &str) -> Option<String> {
        let strategy = self.config.bid_strategy.clone();
        if let Some(job) = self.pending_jobs.get_mut(request_id) {
            if let Some(bid) = job.select_provider(&strategy) {
                return Some(bid.provider_pubkey.clone());
            }
        }
        None
    }

    /// Check if a job has timed out waiting for bids
    pub fn is_bid_timeout(&self, request_id: &str) -> bool {
        if let Some(job) = self.pending_jobs.get(request_id) {
            if matches!(job.status, JobStatus::WaitingForBids) {
                return job.elapsed_secs() > self.config.bid_timeout_secs as u64;
            }
        }
        false
    }

    /// Check if a job has timed out during execution
    pub fn is_job_timeout(&self, request_id: &str) -> bool {
        if let Some(job) = self.pending_jobs.get(request_id) {
            if matches!(
                job.status,
                JobStatus::Accepted { .. } | JobStatus::Processing { .. }
            ) {
                return job.elapsed_secs() > self.config.job_timeout_secs as u64;
            }
        }
        false
    }

    /// Mark job as completed
    pub fn complete_job(&mut self, request_id: &str, provider: String, amount: u64) {
        if let Some(job) = self.pending_jobs.get_mut(request_id) {
            job.status = JobStatus::Completed { provider, amount };
        }
    }

    /// Mark job as failed
    pub fn fail_job(&mut self, request_id: &str, reason: String) {
        if let Some(job) = self.pending_jobs.get_mut(request_id) {
            job.status = JobStatus::Failed { reason };
        }
    }

    /// Remove a completed or failed job
    pub fn remove_job(&mut self, request_id: &str) -> Option<PendingJob> {
        self.pending_jobs.remove(request_id)
    }

    /// Get all pending job IDs
    pub fn pending_job_ids(&self) -> Vec<String> {
        self.pending_jobs.keys().cloned().collect()
    }

    /// Get count of pending jobs
    pub fn pending_count(&self) -> usize {
        self.pending_jobs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compute::domain::{IndexType, ResourceLimits};

    #[test]
    fn test_default_config() {
        let config = ComputeBuyerConfig::default();
        assert_eq!(config.bid_timeout_secs, 30);
        assert_eq!(config.job_timeout_secs, 600);
        assert_eq!(config.max_price_premium_pct, 20);
    }

    #[test]
    fn test_config_builder() {
        let config = ComputeBuyerConfig::default()
            .with_bid_timeout(60)
            .with_job_timeout(300)
            .with_strategy(BidStrategy::BestValue);

        assert_eq!(config.bid_timeout_secs, 60);
        assert_eq!(config.job_timeout_secs, 300);
        assert!(matches!(config.bid_strategy, BidStrategy::BestValue));
    }

    #[test]
    fn test_quote_sandbox_run() {
        let buyer = ComputeBuyer::default_config();
        let request = SandboxRunRequest::new("https://github.com/test/repo", "main")
            .with_limits(ResourceLimits::default_basic());

        let quote = buyer.quote_sandbox_run(&request);
        assert!(quote.price_sats > 0);
        assert_eq!(quote.job_type, "sandbox_run");
    }

    #[test]
    fn test_quote_repo_index() {
        let buyer = ComputeBuyer::default_config();
        let quote = buyer.quote_repo_index(10_000, 100, true);

        assert!(quote.price_sats > 0);
        assert_eq!(quote.job_type, "repo_index");
    }

    #[test]
    fn test_max_price_calculation() {
        let buyer = ComputeBuyer::default_config();
        let quote = compute::domain::Quote::new(1000, "test");

        // 20% premium
        let max = buyer.max_price(&quote);
        assert_eq!(max, 1200);
    }

    #[test]
    fn test_submit_sandbox_run() {
        let mut buyer = ComputeBuyer::default_config();
        let request = SandboxRunRequest::new("https://github.com/test/repo", "main")
            .add_command("cargo test");

        let request_id = buyer.submit_sandbox_run(&request).unwrap();
        assert!(request_id.starts_with("sandbox-"));
        assert_eq!(buyer.pending_count(), 1);
    }

    #[test]
    fn test_submit_repo_index() {
        let mut buyer = ComputeBuyer::default_config();
        let request =
            RepoIndexRequest::new("https://github.com/test/repo", "main").all_indexes();

        let request_id = buyer.submit_repo_index(&request, 10_000, 100).unwrap();
        assert!(request_id.starts_with("index-"));
        assert_eq!(buyer.pending_count(), 1);
    }

    #[test]
    fn test_process_bid() {
        let mut buyer = ComputeBuyer::default_config();
        let request = SandboxRunRequest::new("https://github.com/test/repo", "main");
        let request_id = buyer.submit_sandbox_run(&request).unwrap();

        // Bid within max_price (quote ~350 + 20% premium = ~420)
        let bid = ProviderBid::new("provider-1", 300);
        assert!(buyer.process_bid(&request_id, bid));

        let job = buyer.get_pending_job(&request_id).unwrap();
        assert!(job.has_bids());
    }

    #[test]
    fn test_select_provider() {
        let mut buyer = ComputeBuyer::default_config();
        let request = SandboxRunRequest::new("https://github.com/test/repo", "main");
        let request_id = buyer.submit_sandbox_run(&request).unwrap();

        // Bids within max_price
        buyer.process_bid(&request_id, ProviderBid::new("provider-a", 350));
        buyer.process_bid(&request_id, ProviderBid::new("provider-b", 250));

        let selected = buyer.select_provider(&request_id).unwrap();
        assert_eq!(selected, "provider-b"); // Lowest price
    }

    #[test]
    fn test_job_lifecycle() {
        let mut buyer = ComputeBuyer::default_config();
        let request = SandboxRunRequest::new("https://github.com/test/repo", "main");
        let request_id = buyer.submit_sandbox_run(&request).unwrap();

        // Add bid within max_price and select
        buyer.process_bid(&request_id, ProviderBid::new("provider", 300));
        buyer.select_provider(&request_id);

        // Complete
        buyer.complete_job(&request_id, "provider".to_string(), 500);

        let job = buyer.get_pending_job(&request_id).unwrap();
        assert!(matches!(job.status, JobStatus::Completed { .. }));

        // Remove
        let removed = buyer.remove_job(&request_id).unwrap();
        assert_eq!(buyer.pending_count(), 0);
    }
}

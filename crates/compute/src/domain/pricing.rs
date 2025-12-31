//! Pricing module for compute jobs
//!
//! Calculates costs for SandboxRun and RepoIndex jobs based on resource usage.

use serde::{Deserialize, Serialize};

/// Price in satoshis
pub type Sats = u64;

/// Price book for compute jobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceBook {
    /// Pricing for sandbox runs
    pub sandbox_run: SandboxRunPricing,
    /// Pricing for repo indexing
    pub repo_index: RepoIndexPricing,
}

impl Default for PriceBook {
    fn default() -> Self {
        Self {
            sandbox_run: SandboxRunPricing::default(),
            repo_index: RepoIndexPricing::default(),
        }
    }
}

impl PriceBook {
    /// Create a new price book with default pricing
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate price for a sandbox run
    pub fn price_sandbox_run(&self, cpu_secs: f64, memory_gb_mins: f64) -> Sats {
        self.sandbox_run.calculate(cpu_secs, memory_gb_mins)
    }

    /// Calculate price for repo indexing
    pub fn price_repo_index(&self, tokens: u64, files: u32, embeddings: bool) -> Sats {
        self.repo_index.calculate(tokens, files, embeddings)
    }
}

/// Pricing configuration for sandbox runs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRunPricing {
    /// Base fee in sats
    pub base_fee: Sats,
    /// Price per CPU-second in sats
    pub per_cpu_sec: f64,
    /// Price per GB-minute of memory in sats
    pub per_gb_min: f64,
    /// Maximum charge (cap) in sats
    pub max_charge: Sats,
    /// Minimum charge in sats
    pub min_charge: Sats,
}

impl Default for SandboxRunPricing {
    fn default() -> Self {
        Self {
            base_fee: 200,      // 200 sats base
            per_cpu_sec: 0.5,   // 0.5 sats per CPU-second
            per_gb_min: 0.05,   // 0.05 sats per GB-minute
            max_charge: 20_000, // 20k sats max
            min_charge: 100,    // 100 sats min
        }
    }
}

impl SandboxRunPricing {
    /// Calculate the price for a sandbox run
    pub fn calculate(&self, cpu_secs: f64, memory_gb_mins: f64) -> Sats {
        let cpu_cost = (cpu_secs * self.per_cpu_sec) as Sats;
        let memory_cost = (memory_gb_mins * self.per_gb_min) as Sats;

        let total = self.base_fee + cpu_cost + memory_cost;

        // Apply min/max bounds
        total.max(self.min_charge).min(self.max_charge)
    }

    /// Create custom pricing
    pub fn custom(
        base_fee: Sats,
        per_cpu_sec: f64,
        per_gb_min: f64,
        min_charge: Sats,
        max_charge: Sats,
    ) -> Self {
        Self {
            base_fee,
            per_cpu_sec,
            per_gb_min,
            max_charge,
            min_charge,
        }
    }

    /// Budget-friendly pricing (lower rates)
    pub fn budget() -> Self {
        Self {
            base_fee: 100,
            per_cpu_sec: 0.25,
            per_gb_min: 0.02,
            max_charge: 10_000,
            min_charge: 50,
        }
    }

    /// Premium pricing (higher rates, higher limits)
    pub fn premium() -> Self {
        Self {
            base_fee: 500,
            per_cpu_sec: 1.0,
            per_gb_min: 0.1,
            max_charge: 50_000,
            min_charge: 200,
        }
    }
}

/// Pricing configuration for repo indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoIndexPricing {
    /// Price per 1000 tokens for embeddings in sats
    pub per_1k_tokens: f64,
    /// Price per file for symbol extraction in sats
    pub per_file: f64,
    /// Base fee for embeddings generation in sats
    pub embedding_base_fee: Sats,
    /// Maximum charge (cap) in sats
    pub max_charge: Sats,
    /// Minimum charge in sats
    pub min_charge: Sats,
}

impl Default for RepoIndexPricing {
    fn default() -> Self {
        Self {
            per_1k_tokens: 8.0,      // 8 sats per 1k tokens
            per_file: 1.0,           // 1 sat per file
            embedding_base_fee: 100, // 100 sats for embedding jobs
            max_charge: 100_000,     // 100k sats max
            min_charge: 50,          // 50 sats min
        }
    }
}

impl RepoIndexPricing {
    /// Calculate the price for repo indexing
    pub fn calculate(&self, tokens: u64, files: u32, embeddings: bool) -> Sats {
        let token_cost = ((tokens as f64 / 1000.0) * self.per_1k_tokens) as Sats;
        let file_cost = (files as f64 * self.per_file) as Sats;

        let base = if embeddings {
            self.embedding_base_fee
        } else {
            0
        };

        let total = base + token_cost + file_cost;

        // Apply min/max bounds
        total.max(self.min_charge).min(self.max_charge)
    }

    /// Create custom pricing
    pub fn custom(
        per_1k_tokens: f64,
        per_file: f64,
        embedding_base_fee: Sats,
        min_charge: Sats,
        max_charge: Sats,
    ) -> Self {
        Self {
            per_1k_tokens,
            per_file,
            embedding_base_fee,
            max_charge,
            min_charge,
        }
    }

    /// Budget-friendly pricing
    pub fn budget() -> Self {
        Self {
            per_1k_tokens: 4.0,
            per_file: 0.5,
            embedding_base_fee: 50,
            max_charge: 50_000,
            min_charge: 25,
        }
    }

    /// Premium pricing with higher token rates
    pub fn premium() -> Self {
        Self {
            per_1k_tokens: 16.0,
            per_file: 2.0,
            embedding_base_fee: 200,
            max_charge: 200_000,
            min_charge: 100,
        }
    }
}

/// Quote for a compute job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    /// Quoted price in sats
    pub price_sats: Sats,
    /// Job type description
    pub job_type: String,
    /// Breakdown of costs
    pub breakdown: Vec<(String, Sats)>,
    /// Time estimate in seconds
    pub estimated_time_secs: Option<u32>,
    /// Quote validity in seconds
    pub valid_for_secs: u32,
}

impl Quote {
    /// Create a new quote
    pub fn new(price_sats: Sats, job_type: impl Into<String>) -> Self {
        Self {
            price_sats,
            job_type: job_type.into(),
            breakdown: Vec::new(),
            estimated_time_secs: None,
            valid_for_secs: 300, // 5 minute default validity
        }
    }

    /// Add a cost breakdown item
    pub fn add_breakdown(mut self, item: impl Into<String>, amount: Sats) -> Self {
        self.breakdown.push((item.into(), amount));
        self
    }

    /// Set estimated time
    pub fn with_estimated_time(mut self, secs: u32) -> Self {
        self.estimated_time_secs = Some(secs);
        self
    }

    /// Set quote validity period
    pub fn valid_for(mut self, secs: u32) -> Self {
        self.valid_for_secs = secs;
        self
    }
}

/// Generate a quote for a sandbox run
pub fn quote_sandbox_run(
    pricing: &SandboxRunPricing,
    max_time_secs: u32,
    max_memory_mb: u32,
) -> Quote {
    // Estimate based on max resources (actual may be less)
    let estimated_cpu_secs = max_time_secs as f64;
    let estimated_memory_gb_mins = (max_memory_mb as f64 / 1024.0) * (max_time_secs as f64 / 60.0);

    let cpu_cost = (estimated_cpu_secs * pricing.per_cpu_sec) as Sats;
    let memory_cost = (estimated_memory_gb_mins * pricing.per_gb_min) as Sats;
    let total = pricing.calculate(estimated_cpu_secs, estimated_memory_gb_mins);

    Quote::new(total, "sandbox_run")
        .add_breakdown("Base fee", pricing.base_fee)
        .add_breakdown(format!("CPU ({:.0} secs)", estimated_cpu_secs), cpu_cost)
        .add_breakdown(
            format!("Memory ({:.1} GB-mins)", estimated_memory_gb_mins),
            memory_cost,
        )
        .with_estimated_time(max_time_secs)
}

/// Generate a quote for repo indexing
pub fn quote_repo_index(
    pricing: &RepoIndexPricing,
    estimated_tokens: u64,
    estimated_files: u32,
    include_embeddings: bool,
) -> Quote {
    let token_cost = ((estimated_tokens as f64 / 1000.0) * pricing.per_1k_tokens) as Sats;
    let file_cost = (estimated_files as f64 * pricing.per_file) as Sats;
    let total = pricing.calculate(estimated_tokens, estimated_files, include_embeddings);

    let mut quote = Quote::new(total, "repo_index")
        .add_breakdown(
            format!("Tokens ({} K)", estimated_tokens / 1000),
            token_cost,
        )
        .add_breakdown(format!("Files ({})", estimated_files), file_cost);

    if include_embeddings {
        quote = quote.add_breakdown("Embedding fee", pricing.embedding_base_fee);
    }

    // Rough estimate: 100 files/sec for symbols, 10 files/sec for embeddings
    let time_estimate = if include_embeddings {
        (estimated_files / 10).max(5)
    } else {
        (estimated_files / 100).max(1)
    };
    quote.with_estimated_time(time_estimate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_run_pricing_default() {
        let pricing = SandboxRunPricing::default();

        // 60 seconds of CPU, 1 GB for 1 minute
        let price = pricing.calculate(60.0, 1.0);

        // 200 base + 30 (60 * 0.5) + 0 (1.0 * 0.05 rounds down) = 230
        assert!(price >= pricing.min_charge);
        assert!(price <= pricing.max_charge);
    }

    #[test]
    fn test_sandbox_run_pricing_min_max() {
        let pricing = SandboxRunPricing::default();

        // Very small job - total is base_fee (200) + tiny cpu/mem costs
        // Since base_fee (200) > min_charge (100), we get base_fee as floor
        let price = pricing.calculate(0.1, 0.01);
        assert!(price >= pricing.min_charge);
        assert!(price >= pricing.base_fee);

        // Very large job - should hit maximum
        // 100k cpu_secs * 0.5 = 50k sats, well above max_charge of 20k
        let price = pricing.calculate(100000.0, 100000.0);
        assert_eq!(price, pricing.max_charge);
    }

    #[test]
    fn test_repo_index_pricing_default() {
        let pricing = RepoIndexPricing::default();

        // 10k tokens, 50 files, with embeddings
        let price = pricing.calculate(10_000, 50, true);

        // 100 base + 80 (10 * 8) + 50 (50 * 1) = 230
        assert!(price >= pricing.min_charge);
        assert!(price <= pricing.max_charge);
    }

    #[test]
    fn test_repo_index_pricing_no_embeddings() {
        let pricing = RepoIndexPricing::default();

        // Without embeddings, no base fee
        let with = pricing.calculate(10_000, 50, true);
        let without = pricing.calculate(10_000, 50, false);

        assert!(with > without);
        assert_eq!(with - without, pricing.embedding_base_fee);
    }

    #[test]
    fn test_price_book() {
        let book = PriceBook::new();

        let sandbox_price = book.price_sandbox_run(60.0, 1.0);
        let index_price = book.price_repo_index(10_000, 50, true);

        assert!(sandbox_price > 0);
        assert!(index_price > 0);
    }

    #[test]
    fn test_quote_sandbox_run() {
        let pricing = SandboxRunPricing::default();
        let quote = quote_sandbox_run(&pricing, 300, 2048);

        assert!(!quote.breakdown.is_empty());
        assert!(quote.estimated_time_secs.is_some());
        assert_eq!(quote.job_type, "sandbox_run");
    }

    #[test]
    fn test_quote_repo_index() {
        let pricing = RepoIndexPricing::default();
        let quote = quote_repo_index(&pricing, 50_000, 200, true);

        assert!(!quote.breakdown.is_empty());
        assert!(quote.estimated_time_secs.is_some());
        assert_eq!(quote.job_type, "repo_index");
    }

    #[test]
    fn test_budget_vs_premium_pricing() {
        let budget_sandbox = SandboxRunPricing::budget();
        let premium_sandbox = SandboxRunPricing::premium();

        let budget_price = budget_sandbox.calculate(60.0, 1.0);
        let premium_price = premium_sandbox.calculate(60.0, 1.0);

        assert!(premium_price > budget_price);

        let budget_index = RepoIndexPricing::budget();
        let premium_index = RepoIndexPricing::premium();

        let budget_price = budget_index.calculate(10_000, 50, true);
        let premium_price = premium_index.calculate(10_000, 50, true);

        assert!(premium_price > budget_price);
    }
}

# Integrations

The agent-orchestrator integrates with OpenAgents-specific infrastructure for directives, issue tracking, trajectory logging, and marketplace functionality.

## Directives Integration

Load and inject active directives into agent context. When DSPy is configured,
status, priority, and semantic matching use learned classifiers with a fallback
to frontmatter heuristics.

### DirectiveContext

```rust
pub struct DirectiveContext {
    pub active_directives: Vec<DirectiveSummary>,
    pub current_directive: Option<String>,
}

pub struct DirectiveSummary {
    pub id: String,           // e.g., "d-022"
    pub title: String,
    pub status: DirectiveStatus,
    pub priority: DirectivePriority,
    pub created: Option<String>,
    pub updated: Option<String>,
    pub file_path: PathBuf,
}
```

### Loading Directives

```rust
use agent_orchestrator::integrations::DirectiveContext;

let ctx = DirectiveContext::load("/path/to/workspace").await?;

// Active directives
for directive in &ctx.active_directives {
    println!("{}: {}", directive.id, directive.title);
}

// Find related directive (DSPy semantic match when available)
if let Some(d) = ctx.find_related(&["agent", "orchestration"]) {
    println!("Related: {}", d.id);
}

// Format for injection
let context_text = ctx.format_for_context();
```

### DirectiveInjectionConfig

```rust
pub struct DirectiveInjectionConfig {
    pub include_active: bool,
    pub include_related: bool,
    pub max_directives: usize,
    pub priority_filter: Option<DirectivePriority>,
}
```

```rust
let config = DirectiveInjectionConfig::new()
    .with_max(5)
    .with_priority(DirectivePriority::High);

let context_text = ctx.format_with_config(&config);
```

## Autopilot Integration

Hooks for issue claim/complete workflows.

### AutopilotIntegration

```rust
use agent_orchestrator::integrations::AutopilotIntegration;

let autopilot = AutopilotIntegration::new(issue_store, "session-123".to_string());

// Claim next issue (DSPy selection when available)
let issue = autopilot.claim_next(Some("claude"));

// Complete or block the current issue
autopilot.complete_current();
autopilot.block_current("Waiting for dependency");
```

### IssueClaimHook

Automatically claims issues when work begins.

```rust
let hook = IssueClaimHook::new(autopilot.clone());
hooks.register(hook);
```

Behavior:
- Intercepts issue-related tool calls
- Claims issue on first interaction
- Tracks claimed issues per session

### IssueCompleteHook

Marks issues complete when work finishes.

```rust
let hook = IssueCompleteHook::new(autopilot.clone());
hooks.register(hook);
```

Behavior:
- Detects completion signals
- Updates issue status
- Records session in issue metadata

## Trajectory Integration

APM metrics and action logging for performance tracking.

### ApmTracker

```rust
use agent_orchestrator::integrations::ApmTracker;

let tracker = ApmTracker::new();

// Record action
tracker.record(ActionMetric {
    action_type: ActionType::ToolCall,
    tool_name: Some("read".to_string()),
    duration_ms: 150,
    tokens_in: 100,
    tokens_out: 500,
    timestamp: Utc::now(),
});

// Get APM (Actions Per Minute)
let apm = tracker.calculate_apm();
println!("Current APM: {:.1}", apm);

// Get snapshot
let snapshot = tracker.snapshot();
println!("Total actions: {}", snapshot.action_count);
println!("Avg latency: {}ms", snapshot.avg_latency_ms);
```

### TrajectoryLogger

```rust
use agent_orchestrator::integrations::TrajectoryLogger;

let logger = TrajectoryLogger::new("/path/to/logs");

// Log session start
logger.session_start("session-123", "claude-sonnet-4").await?;

// Log tool call
logger.tool_call("read", &params, &output).await?;

// Log thinking
logger.thinking("Analyzing the codebase structure...").await?;

// Log session end
logger.session_end(apm_snapshot).await?;
```

### TrajectoryLoggerHook

Automatically logs all tool executions.

```rust
let hook = TrajectoryLoggerHook::new(logger.clone());
hooks.register(hook);
```

## Marketplace Integration

Skill licensing and usage tracking.

### SkillLicenseInfo

```rust
pub struct SkillLicenseInfo {
    pub skill_id: String,
    pub license_id: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub capabilities: Vec<String>,
    pub usage_limit: Option<u64>,
    pub usage_count: u64,
}
```

### MarketplaceIntegration

```rust
use agent_orchestrator::integrations::MarketplaceIntegration;

let marketplace = MarketplaceIntegration::new(license_store, usage_tracker);

// Check license
if marketplace.has_valid_license("web-scraper") {
    // Use skill
}

// Record usage
marketplace.record_usage("web-scraper", 100, 500)?; // tokens in/out

// Get usage summary
let usage = marketplace.get_usage_summary("web-scraper")?;
println!("Calls: {}, Cost: {} sats", usage.call_count, usage.total_cost);
```

### SkillLicenseHook

Validates licenses before skill execution.

```rust
let hook = SkillLicenseHook::new(license_store.clone())
    .map_tool("scrape_url", "web-scraper")
    .map_tool("parse_pdf", "document-parser");
    
hooks.register(hook);
```

Behavior:
- Checks license validity before tool call
- Blocks if license expired or missing
- Tracks capability usage

### SkillUsageHook

Tracks usage and calculates costs.

```rust
let hook = SkillUsageHook::new(usage_tracker.clone(), pricing.clone());
hooks.register(hook);
```

### SkillPricing

```rust
pub enum SkillPricing {
    Free,
    PerCall { sats: u64 },
    PerToken { input_sats: u64, output_sats: u64 },
    Hybrid { base_sats: u64, per_token_sats: u64 },
}
```

## Integration Composition

Combine integrations for full functionality:

```rust
// Create integrations
let directives = DirectiveContext::load(&workspace).await?;
let autopilot = AutopilotIntegration::new(issue_store);
let trajectory = TrajectoryLogger::new(&log_dir);
let marketplace = MarketplaceIntegration::new(licenses, usage);

// Create hook manager
let mut hooks = HookManager::new();

// Register directive injection
hooks.register(DirectiveInjectionHook::new(&workspace)
    .with_config(DirectiveInjectionConfig::default()));

// Register autopilot hooks
hooks.register(IssueClaimHook::new(autopilot.clone()));
hooks.register(IssueCompleteHook::new(autopilot.clone()));

// Register trajectory logging
hooks.register(TrajectoryLoggerHook::new(trajectory.clone()));

// Register marketplace hooks
hooks.register(SkillLicenseHook::new(marketplace.licenses())
    .map_tool("scrape_url", "web-scraper"));
hooks.register(SkillUsageHook::new(marketplace.usage(), pricing));
```

## Storage Traits

Integrations use trait abstractions for storage:

### IssueStore

```rust
pub trait IssueStore: Send + Sync {
    fn get_next_ready(&self, agent: &str) -> Result<Option<Issue>>;
    fn claim(&self, number: u32, session: &str) -> Result<()>;
    fn complete(&self, number: u32) -> Result<()>;
    fn block(&self, number: u32, reason: &str) -> Result<()>;
}
```

### LicenseStore

```rust
pub trait LicenseStore: Send + Sync {
    fn get(&self, skill_id: &str) -> Option<SkillLicenseInfo>;
    fn list(&self) -> Vec<SkillLicenseInfo>;
    fn revoke(&self, license_id: &str) -> bool;
}
```

### UsageTracker

```rust
pub trait UsageTracker: Send + Sync {
    fn record(&self, skill_id: &str, tokens_in: u64, tokens_out: u64, cost: u64);
    fn get_usage(&self, skill_id: &str) -> Option<UsageSummary>;
    fn total_cost(&self) -> u64;
}
```

Implement these traits to connect to your storage backend (SQLite, Redis, etc.).

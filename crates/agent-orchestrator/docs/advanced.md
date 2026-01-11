# Advanced Features

Phase 6 introduces infrastructure for sovereign AI agents: threshold-protected identity, graduated autonomy, multi-backend routing, and budget enforcement.

## Threshold-Protected Identity (FROSTR)

Agents can have cryptographic identity protected by threshold signatures — no single party can extract the private key.

### ThresholdConfig

```rust
pub struct ThresholdConfig {
    pub threshold: u32,           // Signatures required (e.g., 2)
    pub total_signers: u32,       // Total signers (e.g., 3)
    pub signer_pubkeys: Vec<String>,
    pub group_pubkey: String,
}
```

### Usage

```rust
use agent_orchestrator::ThresholdConfig;

// 2-of-3 threshold configuration
let config = ThresholdConfig::new(2, 3)
    .with_signers(vec![
        "signer1_pubkey".to_string(),
        "signer2_pubkey".to_string(),
        "signer3_pubkey".to_string(),
    ])
    .with_group_pubkey("group_pubkey".to_string());

// Validate configuration
assert!(config.is_valid());
```

### Why Threshold Signatures?

| Traditional | Threshold |
|-------------|-----------|
| Operator holds full key | Key split across signers |
| Operator can impersonate agent | No single party can sign alone |
| Key extraction = identity theft | Key extraction impossible |
| Trust the operator | Trust the protocol |

## Agent Identity (NIP-SA)

Sovereign agent identity with autonomy levels.

### AgentIdentity

```rust
pub struct AgentIdentity {
    pub agent_pubkey: String,
    pub name: String,
    pub autonomy_level: AutonomyLevel,
    pub threshold_config: Option<ThresholdConfig>,
    pub operator_pubkey: Option<String>,
    pub model: String,
}
```

### Autonomy Levels

```rust
pub enum AutonomyLevel {
    Supervised,       // All actions require approval
    SemiAutonomous,   // Low-cost auto, high-cost needs approval
    FullyAutonomous,  // No approval required
}
```

| Level | Approval Required | Use Case |
|-------|-------------------|----------|
| Supervised | Always | New agents, high-risk tasks |
| SemiAutonomous | Cost > 1000 sats | Proven agents, normal ops |
| FullyAutonomous | Never | Trusted agents, routine work |

### Usage

```rust
use agent_orchestrator::{AgentIdentity, AutonomyLevel, ThresholdConfig};

let identity = AgentIdentity::new("agent_pubkey", "MyAgent", "codex-sonnet-4")
    .with_autonomy(AutonomyLevel::SemiAutonomous)
    .with_operator("operator_pubkey")
    .with_threshold(ThresholdConfig::new(2, 3).with_signers(signers));

// Check if threshold-protected
if identity.is_threshold_protected() {
    println!("Agent has unforgeable identity");
}
```

## Multi-Backend Routing

Route different agents to different AI providers.

### BackendProvider

```rust
pub enum BackendProvider {
    Codex,   // OpenAI Codex API
    OpenAI,   // OpenAI API
    Codex,    // OpenAI Codex
    GptOss,   // Local GPT-OSS inference
    Local,    // Custom local endpoint
}
```

### BackendConfig

```rust
pub struct BackendConfig {
    pub provider: BackendProvider,
    pub model: String,
    pub endpoint: Option<String>,
    pub cost_per_1k_input: u64,   // sats
    pub cost_per_1k_output: u64,  // sats
    pub enabled: bool,
}
```

### MultiBackendRouter

```rust
use agent_orchestrator::{MultiBackendRouter, BackendProvider, BackendConfig};

let router = MultiBackendRouter::new(BackendProvider::Codex)
    // Add backends
    .add_backend(BackendConfig::codex("sonnet-4"))
    .add_backend(BackendConfig::openai("gpt-4"))
    .add_backend(BackendConfig::local("llama3", "http://localhost:11434"))
    // Route specific agents
    .route_agent("oracle", BackendProvider::OpenAI)
    .route_agent("explore", BackendProvider::Local);

// Get backend for agent
let backend = router.get_backend("oracle");
assert_eq!(backend.unwrap().provider, BackendProvider::OpenAI);

// Default backend for unmapped agents
let backend = router.get_backend("sisyphus");
assert_eq!(backend.unwrap().provider, BackendProvider::Codex);

// List enabled backends
let enabled = router.list_enabled();
```

### Cost Calculation

```rust
let config = BackendConfig::codex("sonnet-4");

// 3 sats per 1k input, 15 sats per 1k output
let cost = config.calculate_cost(10_000, 5_000);
// = (10_000 * 3 / 1000) + (5_000 * 15 / 1000)
// = 30 + 75 = 105 sats
```

## Cost Tracking & Budget Enforcement

Track costs and enforce budgets across all backends.

### BudgetConfig

```rust
pub struct BudgetConfig {
    pub daily_limit_sats: u64,      // Max per day
    pub session_limit_sats: u64,    // Max per session
    pub warn_threshold_pct: u8,     // Warn at this % (e.g., 80)
}
```

### CostTracker

```rust
use agent_orchestrator::{CostTracker, BudgetConfig, CostRecord, BackendProvider};

let tracker = CostTracker::new()
    .with_default_budget(BudgetConfig {
        daily_limit_sats: 100_000,
        session_limit_sats: 10_000,
        warn_threshold_pct: 80,
    })
    .with_budget("expensive-agent", BudgetConfig {
        daily_limit_sats: 500_000,
        session_limit_sats: 50_000,
        warn_threshold_pct: 90,
    });

// Record cost
tracker.record(CostRecord {
    agent_name: "sisyphus".to_string(),
    backend: BackendProvider::Codex,
    input_tokens: 10_000,
    output_tokens: 5_000,
    cost_sats: 105,
    timestamp: now(),
})?;

// Check totals
let session_total = tracker.session_total("sisyphus", session_start);
let daily_total = tracker.daily_total("sisyphus");

// Check budget status
let status = tracker.check_budget("sisyphus", session_start);
```

### BudgetStatus

```rust
pub enum BudgetStatus {
    Ok,
    Warning { session_pct: u8, daily_pct: u8 },
    SessionExceeded,
    DailyExceeded,
}
```

### CostTrackingHook

Automatically enforces budgets on tool calls.

```rust
use agent_orchestrator::CostTrackingHook;
use std::sync::Arc;

let hook = CostTrackingHook::new(
    Arc::new(tracker),
    Arc::new(router),
);

hooks.register(hook);
```

Behavior:
1. **before_tool**: Check budget, block if exceeded
2. **after_tool**: Calculate cost, record to tracker
3. Logs warnings when approaching limits

## Approval Workflow (NIP-SA)

Supervised agents request approval for actions.

### SolverAgentCoordinator

```rust
use agent_orchestrator::{AgentIdentity, SolverAgentCoordinator, AutonomyLevel};

let identity = AgentIdentity::new("pk", "Agent", "model")
    .with_autonomy(AutonomyLevel::SemiAutonomous);

let coordinator = SolverAgentCoordinator::new(identity);

// Check if approval needed
if coordinator.requires_approval("purchase_skill", 5000) {
    // Request approval
    let request_id = coordinator.request_approval(
        "purchase_skill",
        "Buy web-scraper skill for data extraction",
        5000,  // cost estimate in sats
    );
    
    // List pending approvals
    let pending = coordinator.list_pending();
    for p in pending {
        println!("{}: {} ({} sats)", p.request_id, p.description, p.cost_estimate_sats);
    }
    
    // Operator approves or rejects
    coordinator.approve(&request_id);
    // or: coordinator.reject(&request_id);
}
```

### PendingApproval

```rust
pub struct PendingApproval {
    pub request_id: String,
    pub action_type: String,
    pub description: String,
    pub cost_estimate_sats: u64,
    pub created_at: u64,
}
```

### Approval Logic by Autonomy Level

| Level | requires_approval() |
|-------|---------------------|
| Supervised | Always `true` |
| SemiAutonomous | `true` if cost > 1000 sats |
| FullyAutonomous | Always `false` |

## Putting It All Together

```rust
use agent_orchestrator::{
    AgentIdentity, AutonomyLevel, ThresholdConfig,
    MultiBackendRouter, BackendProvider, BackendConfig,
    CostTracker, BudgetConfig, CostTrackingHook,
    SolverAgentCoordinator, HookManager,
};
use std::sync::Arc;

// 1. Create threshold-protected identity
let threshold = ThresholdConfig::new(2, 3)
    .with_signers(vec![pk1, pk2, pk3])
    .with_group_pubkey(group_pk);

let identity = AgentIdentity::new(&agent_pk, "ProductionAgent", "codex-sonnet-4")
    .with_threshold(threshold)
    .with_autonomy(AutonomyLevel::SemiAutonomous)
    .with_operator(&operator_pk);

// 2. Configure multi-backend routing
let router = Arc::new(
    MultiBackendRouter::new(BackendProvider::Codex)
        .add_backend(BackendConfig::codex("sonnet-4"))
        .add_backend(BackendConfig::openai("gpt-4"))
        .add_backend(BackendConfig::local("llama3", "http://localhost:11434"))
        .route_agent("oracle", BackendProvider::OpenAI)
);

// 3. Configure cost tracking
let tracker = Arc::new(
    CostTracker::new()
        .with_default_budget(BudgetConfig {
            daily_limit_sats: 100_000,
            session_limit_sats: 10_000,
            warn_threshold_pct: 80,
        })
);

// 4. Create approval coordinator
let coordinator = SolverAgentCoordinator::new(identity);

// 5. Wire up hooks
let mut hooks = HookManager::new();
hooks.register(CostTrackingHook::new(tracker.clone(), router.clone()));

// 6. Run agent with full infrastructure
// - Identity is threshold-protected
// - Requests route to appropriate backends
// - Costs are tracked and budgets enforced
// - High-cost actions require approval
```

## Integration with External Systems

### FROSTR Key Ceremony

```rust
// Future: Connect ThresholdConfig to actual FROSTR keygen
use frostr::keygen::{KeygenSession, KeygenConfig};

let keygen = KeygenSession::new(KeygenConfig {
    threshold: 2,
    total: 3,
});

// Participants exchange messages...
let shares = keygen.complete()?;

// Create agent identity with real threshold key
let config = ThresholdConfig::new(2, 3)
    .with_signers(shares.signer_pubkeys())
    .with_group_pubkey(shares.group_pubkey());
```

### Spark Wallet

```rust
// Future: Connect CostTracker to real Bitcoin payments
use spark::Wallet;

let wallet = Wallet::from_threshold_key(&threshold_config)?;

// Agent pays for compute
wallet.pay_invoice(&invoice, cost_sats)?;

// Record in tracker
tracker.record(CostRecord { ... })?;
```

### NIP-90 DVMs

```rust
// Future: Route to decentralized compute
let router = MultiBackendRouter::new(BackendProvider::Codex)
    .add_dvm_backend(DvmConfig {
        relay_urls: vec!["wss://relay.damus.io"],
        job_kind: 5050,  // Text generation
        max_price_sats: 100,
    });

// Agent discovers providers, submits jobs, pays via Lightning
```

## Security Considerations

1. **Threshold keys**: Store shares separately, never combine except for signing
2. **Budget limits**: Set conservative limits for new agents
3. **Approval workflow**: Review pending approvals regularly
4. **Audit logs**: Log all cost records and approvals
5. **Key rotation**: Implement periodic threshold key rotation

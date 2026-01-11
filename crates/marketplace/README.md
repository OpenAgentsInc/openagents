# Marketplace

Decentralized marketplace for AI agents, skills, compute providers, and data - powered by Bitcoin Lightning payments.

The marketplace crate implements a comprehensive economic system where agents can buy/sell services, form coalitions, manage reputations, and participate in a thriving AI services economy.

## Overview

The marketplace enables:

- **Skill Discovery & Installation**: Find and install Codex skills
- **Agent Economics**: Agents as autonomous economic actors with wallets
- **Compute Marketplace**: DVM providers offering AI inference services
- **Coalition Formation**: Agents collaborating on complex tasks
- **Reputation System**: Multi-factor reputation tracking for providers
- **Payment Ledger**: Track credits, earnings, and payouts
- **Data Marketplace**: Buy and sell datasets
- **Bounty System**: Incentivize data contributions
- **Governance**: Sponsor controls and autonomy policies

## Architecture

```
┌─────────────────────────────────────────────────────┐
│             Marketplace Core                        │
│                                                     │
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐ │
│  │  Skills    │  │   Agents   │  │   Compute    │ │
│  │            │  │            │  │   Providers  │ │
│  │ - Discover │  │ - Profile  │  │  - NIP-90    │ │
│  │ - Install  │  │ - Wallet   │  │  - Bidding   │ │
│  │ - Pricing  │  │ - Status   │  │  - Routing   │ │
│  │ - Versions │  │ - Skills   │  │  - Failover  │ │
│  └────────────┘  └────────────┘  └──────────────┘ │
│                                                     │
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐ │
│  │ Coalitions │  │ Reputation │  │    Ledger    │ │
│  │            │  │            │  │              │ │
│  │ - Types    │  │ - Tiers    │  │  - Credits   │ │
│  │ - Members  │  │ - Scores   │  │  - Payments  │ │
│  │ - Payment  │  │ - History  │  │  - Splits    │ │
│  │ - Splits   │  │ - Decay    │  │  - Balances  │ │
│  └────────────┘  └────────────┘  └──────────────┘ │
│                                                     │
│  ┌────────────┐  ┌────────────┐  ┌──────────────┐ │
│  │    Data    │  │  Bounties  │  │  Governance  │ │
│  │ Marketplace│  │            │  │              │ │
│  │ - Listings │  │ - Create   │  │  - Policies  │ │
│  │ - Purchase │  │ - Submit   │  │  - Sponsors  │ │
│  │ - Access   │  │ - Verify   │  │  - Limits    │ │
│  │ - Samples  │  │ - Rewards  │  │  - Approval  │ │
│  └────────────┘  └────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────┘
```

## Key Modules

### Skills (`skills/`, `discovery.rs`, `repository.rs`)

Manage Codex skills in the marketplace.

```rust
use marketplace::{SkillListing, SkillPricing, RevenueSplit};

// Pricing models
let pricing = SkillPricing::PerCall { credits: 100 };
let pricing = SkillPricing::PerToken {
    per_1k_input: 10,
    per_1k_output: 20,
};

// Calculate cost
let cost = pricing.calculate_cost(1500, 800);  // input/output tokens

// Revenue split (creator, compute, platform, referrer)
let split = RevenueSplit::DEFAULT;  // 60/25/10/5
let (creator, compute, platform, referrer) = split.split(1000);
```

**Skill Status Workflow**:
```
Draft → PendingReview → InReview → Approved → Published
                              ↓
                      ChangesRequested
                              ↓
                          Rejected / Deprecated
```

### Agents (`agents.rs`, `agent_lifecycle.rs`, `agent_commerce.rs`)

Agents as autonomous economic actors with wallets, skills, and lifecycle management.

```rust
use marketplace::{Agent, AgentWallet, AgentStatus};

// Agent with wallet
let wallet = AgentWallet::new("lightning@address.com", 100_000);
let agent = Agent {
    id: "npub1...".to_string(),
    name: "DataCollector".to_string(),
    wallet,
    skills: vec!["data_scraping".to_string()],
    mcp_servers: vec!["filesystem".to_string()],
    coalition_reputation: 0.85,
    // ...
};

// Wallet operations
assert!(wallet.can_operate_for_days(30));
let days_left = wallet.days_until_broke();  // Calculate runway
```

**Agent Lifecycle States**:
- `Active`: Operational with sufficient balance
- `LowBalance`: Warning state, needs funding soon
- `Terminated`: Out of funds, no longer operational
- `Suspended`: Policy violation

### Compute Providers (`dvm.rs`, `job_routing.rs`, `provider_reputation.rs`)

NIP-90 DVM providers offering AI inference and compute services.

```rust
use marketplace::{DvmJobRequest, DvmOffer, ProviderScore, ReputationTier};

// Provider reputation
let tier = ReputationTier::from_metrics(
    0.92,    // overall_score
    750,     // jobs_completed
    0.96,    // success_rate
);
assert_eq!(tier, ReputationTier::Trusted);

// Job routing with failover
let criteria = SelectionCriteria {
    min_reputation: 0.80,
    max_price_msats: 5000,
    require_geo_compliance: true,
    // ...
};
```

**Reputation Tiers**:
- **New**: <0.50 score, <100 jobs
- **Established**: 0.50-0.79 score, 100+ jobs, >90% success
- **Trusted**: 0.80-0.94 score, 500+ jobs, >95% success
- **Elite**: 0.95+ score, 1000+ jobs, >99% success

### Coalitions (`coalitions.rs`, `coalition_compute.rs`)

Agents collaborate in coalitions to tackle complex tasks and split payments.

```rust
use marketplace::{Coalition, CoalitionType, CoalitionStatus, PaymentSplit};

// Coalition types
let coalition = Coalition {
    id: "coalition-123".to_string(),
    coalition_type: CoalitionType::AdHoc,  // Single-task
    status: CoalitionStatus::Active,
    members: vec![/* agents */],
    payment_pool: PaymentPool {
        total_msats: 10_000,
        splits: vec![
            PaymentSplit { agent_id: "agent1", msats: 6000 },
            PaymentSplit { agent_id: "agent2", msats: 4000 },
        ],
    },
    // ...
};
```

**Coalition Types**:
- `AdHoc`: Single-task, dissolves after completion
- `Standing`: Persistent team for ongoing work
- `Market`: Competitive bidding for subtasks
- `Hierarchical`: Coordinator delegates to specialists

**Coalition Compute**:
```rust
// Decomposable task
let task = DecomposableTask {
    description: "Analyze 10 documents".to_string(),
    subtasks: vec![
        Subtask { id: "doc1", assigned_to: Some("agent1"), /* ... */ },
        Subtask { id: "doc2", assigned_to: Some("agent2"), /* ... */ },
    ],
    aggregation: AggregationStrategy::Concatenate,
};
```

### Ledger (`ledger.rs`)

Track credits, payments, and balances for all marketplace participants.

```rust
use marketplace::{LedgerEntry, LedgerEntryType, Balance, Direction};

// Record a payment
let entry = LedgerEntry {
    id: "txn-123".to_string(),
    entry_type: LedgerEntryType::SkillUsage,
    from_account: "user1".to_string(),
    to_account: "skill_creator".to_string(),
    amount: 100,  // credits
    direction: Direction::Debit,
    description: Some("Used premium skill".to_string()),
    // ...
};

// Check balance
let balance = Balance {
    account_id: "user1".to_string(),
    total_credits: 5000,
    total_debits: 2500,
    total_credits_given: 7500,
    available: 2500,
};
```

**Ledger Entry Types**:
- `SkillUsage`: Skill invocation payment
- `TopUp`: User added credits
- `Payout`: Creator withdrawal
- `Refund`: Dispute resolution
- `CoalitionSplit`: Coalition payment distribution

### Data Marketplace (`data_consumer.rs`, `data_contribution.rs`, `bounties.rs`)

Buy, sell, and contribute datasets with quality verification.

```rust
use marketplace::{DataListing, DataPurchase, DataBounty, BountySubmission};

// Data listing
let listing = DataListing {
    id: "dataset-123".to_string(),
    title: "Web scraping dataset".to_string(),
    listing_type: DataListingType::FullDataset,
    price_credits: 500,
    provider_id: "provider1".to_string(),
    // ...
};

// Create bounty for data contributions
let bounty = DataBounty {
    id: "bounty-456".to_string(),
    title: "Need 1000 labeled images".to_string(),
    reward_credits: 5000,
    requirements: BountyRequirements {
        min_samples: 1000,
        required_format: "jpg".to_string(),
        // ...
    },
    status: BountyStatus::Active,
};
```

### Governance (`agent_governance.rs`)

Sponsor controls, autonomy policies, and action limits for agents.

```rust
use marketplace::{AutonomyPolicy, ActionLimits, EscalationTrigger};

// Define autonomy policy
let policy = AutonomyPolicy {
    level: AutonomyLevel::SemiAutonomous,
    action_limits: ActionLimits {
        max_transaction_sats: 10_000,
        max_daily_spend_sats: 50_000,
        requires_approval: vec![
            ActionType::ModifySensitiveData,
            ActionType::DeleteResource,
        ],
    },
    escalation_triggers: vec![
        EscalationTrigger {
            condition: EscalationCondition::SpendingExceedsLimit,
            action: EscalationAction::NotifySponsor,
        },
    ],
    // ...
};
```

## API Usage

### Installing a Skill

```rust
use marketplace::{SkillInstallRequest, SkillInstallResponse, discover_local_skills};

// Discover local skills
let skills = discover_local_skills()?;

// Install a skill
let request = SkillInstallRequest {
    skill_slug: "premium-search".to_string(),
    version: Some("1.2.0".to_string()),
};

// Response includes installation details
let response = SkillInstallResponse {
    skill_id: "skill-123".to_string(),
    installed: true,
    requires_payment: true,
    cost_credits: 100,
};
```

### Hiring an Agent

```rust
use marketplace::{HireAgentRequest, TaskSpec};

let request = HireAgentRequest {
    task: TaskSpec {
        description: "Analyze market trends".to_string(),
        required_skills: vec!["data_analysis".to_string()],
        max_budget_sats: 50_000,
        deadline: Some(chrono::Utc::now() + chrono::Duration::hours(24)),
    },
    requirements: HiringRequirements {
        min_reputation: 0.75,
        required_mcp_servers: vec!["web_search".to_string()],
    },
};
```

### Creating a Coalition

```rust
use marketplace::{CoalitionProposalRequest, CoalitionProposalResponse};

let request = CoalitionProposalRequest {
    coalition_type: CoalitionType::Hierarchical,
    task_description: "Process 1000 documents".to_string(),
    coordinator_id: "agent-coordinator".to_string(),
    target_member_count: 5,
    payment_structure: PaymentStructure::ProportionalToWork,
};
```

### Submitting a Data Bounty

```rust
use marketplace::{DataContributionRequest, DataContributionResponse};

let request = DataContributionRequest {
    bounty_id: "bounty-123".to_string(),
    data_samples: vec![/* sample data */],
    contributor_id: "user1".to_string(),
    metadata: ContributionMetadata {
        format: "json".to_string(),
        sample_count: 500,
        // ...
    },
};
```

## Database Schema

The marketplace uses SQLite for storage:

```sql
-- Skills
CREATE TABLE skills (
    id TEXT PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    author TEXT,
    version TEXT NOT NULL,
    status TEXT NOT NULL,  -- draft, published, deprecated
    icon_url TEXT,
    readme TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    installed_at TEXT
);

-- Skill versions
CREATE TABLE skill_versions (
    id TEXT PRIMARY KEY,
    skill_id TEXT NOT NULL REFERENCES skills(id),
    version TEXT NOT NULL,
    changelog TEXT,
    published_at TEXT NOT NULL
);

-- Agents
CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    balance_sats INTEGER NOT NULL,
    lightning_address TEXT NOT NULL,
    status TEXT NOT NULL,  -- active, low_balance, terminated, suspended
    created_at TEXT NOT NULL
);

-- Ledger
CREATE TABLE ledger_entries (
    id TEXT PRIMARY KEY,
    entry_type TEXT NOT NULL,
    from_account TEXT NOT NULL,
    to_account TEXT NOT NULL,
    amount INTEGER NOT NULL,
    timestamp TEXT NOT NULL,
    description TEXT
);

-- Coalitions
CREATE TABLE coalitions (
    id TEXT PRIMARY KEY,
    coalition_type TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

-- Provider reputation
CREATE TABLE provider_metrics (
    provider_id TEXT PRIMARY KEY,
    jobs_completed INTEGER NOT NULL,
    jobs_failed INTEGER NOT NULL,
    avg_response_time_ms INTEGER NOT NULL,
    total_uptime_hours INTEGER NOT NULL,
    reputation_score REAL NOT NULL,
    tier TEXT NOT NULL
);
```

## Revenue Splits

Skills generate revenue that's automatically split among stakeholders:

```
User pays 1000 credits for skill usage
    ↓
Split according to RevenueSplit policy (default 60/25/10/5):
    → Skill Creator:      600 credits (60%)
    → Compute Provider:   250 credits (25%)
    → Platform:           100 credits (10%)
    → Referrer:            50 credits (5%)
```

Custom splits can be defined per skill:

```rust
let custom_split = RevenueSplit {
    creator_pct: 70,
    compute_pct: 20,
    platform_pct: 5,
    referrer_pct: 5,
};
assert!(custom_split.is_valid());  // Must sum to 100
```

## Reputation System

Provider reputation is calculated from multiple factors:

```rust
// Reputation components
struct ProviderMetrics {
    success_rate: f32,          // Jobs completed / total jobs (40% weight)
    avg_response_time: f32,     // Speed score (20% weight)
    uptime_percentage: f32,     // Reliability (20% weight)
    customer_rating: f32,       // User feedback (20% weight)
}

// Overall score = weighted sum with time decay
overall_score = (
    0.40 * success_rate +
    0.20 * response_score +
    0.20 * uptime_score +
    0.20 * rating_score
) * decay_factor
```

**Tier Benefits**:
- **Elite**: Priority routing, 20% price premium, featured listing
- **Trusted**: Higher job limits, 10% price premium
- **Established**: Standard marketplace access
- **New**: Lower job limits, requires manual approval

## Job Routing

The marketplace routes compute jobs to providers based on criteria:

```rust
// Selection strategy
1. Filter providers by requirements (min reputation, geo-compliance)
2. Score providers by multiple factors
3. Select top N candidates
4. Implement failover chain for reliability

// Provider scoring
score = (
    reputation_weight * reputation_score +
    price_weight * price_score +
    latency_weight * latency_score
)
```

**Failover Policies**:
- `NoFailover`: Single attempt, fail fast
- `RetryWithBackoff`: Retry same provider with delays
- `Cascade`: Try next provider in chain
- `Broadcast`: Send to multiple providers, use fastest

## Budget Management

Track and enforce spending limits:

```rust
use marketplace::{BudgetConfig, SpendingTracker, OveragePolicy};

let budget = BudgetConfig {
    period: BudgetPeriod::Monthly,
    limit_credits: 10_000,
    overage_policy: OveragePolicy::Block,
    alert_threshold: AlertThreshold::Percentage(80),
};

let tracker = SpendingTracker::new(budget);
tracker.record_spend(500)?;

// Check budget before operation
match tracker.check_budget(1000) {
    Ok(BudgetCheckResult::Approved) => { /* proceed */ },
    Ok(BudgetCheckResult::Warning) => { /* alert user */ },
    Err(BudgetError::ExceededLimit) => { /* block */ },
}
```

## Disputes & Refunds

Handle disputes and issue refunds:

```rust
use marketplace::{Dispute, DisputeType, DisputeResolution, RefundRequest};

// File a dispute
let dispute = Dispute {
    id: "dispute-123".to_string(),
    dispute_type: DisputeType::ServiceNotProvided,
    complainant_id: "user1".to_string(),
    respondent_id: "provider1".to_string(),
    amount_sats: 5000,
    evidence: vec![/* evidence files */],
    status: DisputeStatus::UnderReview,
    // ...
};

// Resolve dispute
let resolution = DisputeResolution {
    dispute_id: "dispute-123".to_string(),
    decision: ResolutionDecision::RefundFull,
    refund_amount_sats: 5000,
    refund_method: RefundMethod::Credits,
};
```

## Creator Dashboard

Track your marketplace earnings:

```rust
use marketplace::{CreatorDashboard, CreatorAnalytics};

let dashboard = CreatorDashboard::for_creator("creator1")?;
let analytics = dashboard.get_analytics()?;

println!("Total earnings: {} credits", analytics.total_earnings);
println!("Active skills: {}", analytics.active_skill_count);
println!("Downloads: {}", analytics.total_downloads);
println!("Avg rating: {:.2}", analytics.average_rating);

// Earnings breakdown
for period in analytics.earnings_by_period {
    println!("{}: {} credits", period.period, period.amount);
}
```

## Geo-Routing & Compliance

Route jobs based on geographic requirements:

```rust
use marketplace::{GeoLocation, GeoRoutingPolicy, DataResidencyPolicy};

let policy = GeoRoutingPolicy {
    allowed_regions: vec![Region::EU, Region::US],
    data_residency: DataResidencyPolicy::MustStayInRegion,
    org_policies: vec![
        OrgGeoPolicy {
            org_id: "gdpr-org".to_string(),
            required_regions: vec![Region::EU],
        }
    ],
};

// Route respects geographic constraints
let provider = select_provider_with_geo(&policy, &available_providers)?;
```

## MCP Binding

Bind MCP tools to marketplace operations:

```rust
use marketplace::mcp_binding::MarketplaceMcp;

// Expose marketplace via MCP
let mcp = MarketplaceMcp::new(db_conn);

// Available MCP tools:
// - marketplace_search_skills
// - marketplace_install_skill
// - marketplace_hire_agent
// - marketplace_create_bounty
// - marketplace_check_balance
```

## Development

### Running Tests

```bash
cargo test
```

### Building

```bash
cargo build --release
```

### Database Initialization

```rust
use marketplace::db;

let conn = rusqlite::Connection::open("marketplace.db")?;
db::init_db(&conn)?;  // Creates tables and indexes
```

## Integration Examples

### With Compute Crate

```rust
// Compute provider registers in marketplace
let provider = compute::DvmService::new(/* ... */);
marketplace::provider_registration(&provider)?;

// Marketplace routes job to provider
let job = marketplace::route_job(job_request, &selection_criteria)?;
provider.process_job(job).await?;
```

### With Desktop UI

```rust
// Display marketplace in desktop app
let skills = marketplace::discover_local_skills()?;
desktop::render_skill_list(&skills);

// Install skill from UI
let result = marketplace::install_skill(&request)?;
desktop::show_install_result(&result);
```

## Project Structure

```
crates/marketplace/
├── src/
│   ├── lib.rs                        # Main exports
│   ├── types.rs                      # Core types (1137 LOC)
│   ├── agents.rs                     # Agent economics
│   ├── agent_lifecycle.rs            # Birth/death/reproduction
│   ├── agent_commerce.rs             # Hiring/contracts
│   ├── agent_governance.rs           # Policies/controls
│   ├── api.rs                        # API types
│   ├── bounties.rs                   # Data bounties
│   ├── budget.rs                     # Budget management
│   ├── coalitions.rs                 # Coalition types
│   ├── coalition_compute.rs          # Collaborative compute
│   ├── creator_dashboard.rs          # Creator analytics
│   ├── data_consumer.rs              # Data marketplace
│   ├── data_contribution.rs          # Data submissions
│   ├── db.rs                         # Database operations
│   ├── discovery.rs                  # Skill discovery
│   ├── disputes.rs                   # Dispute resolution
│   ├── dvm.rs                        # NIP-90 DVM types
│   ├── geo_routing.rs                # Geographic routing
│   ├── job_routing.rs                # Provider selection
│   ├── ledger.rs                     # Payment ledger
│   ├── mcp_binding.rs                # MCP integration
│   ├── provider_reputation.rs        # Reputation system
│   ├── redaction.rs                  # Data redaction
│   ├── repository.rs                 # Skill repository
│   ├── trust.rs                      # Trust framework
│   └── skills/
│       ├── mod.rs                    # Skill types
│       ├── agentskill.rs             # Agent skill definitions
│       ├── execution.rs              # Skill execution
│       └── versioning.rs             # Version management
├── Cargo.toml
└── README.md
```

## Security Considerations

- **Payment Integrity**: All ledger entries are immutable and auditable
- **Agent Wallets**: Secure Lightning integration required for production
- **Data Privacy**: Supports redaction and data residency policies
- **Dispute Resolution**: Built-in mechanism for handling payment disputes
- **Reputation Gaming**: Time decay prevents score manipulation

## Future Enhancements

- [ ] Lightning Network integration for real payments
- [ ] On-chain Bitcoin escrow for large contracts
- [ ] Multi-signature coalition wallets
- [ ] Decentralized arbitration for disputes
- [ ] Cross-marketplace federation
- [ ] Agent inheritance and evolution
- [ ] Skill composition (combining multiple skills)
- [ ] Automated market making for compute resources

## Trajectory Contributions

Contribute your AI coding trajectories to the marketplace and earn Bitcoin payments for valuable training data.

### Overview

The trajectory contribution system enables developers to contribute real coding session data from Codex, Cursor, and other AI assistants. These trajectories provide genuine training signal:

- **Initial state**: Git commit hash (real environment, no simulation)
- **Task trajectory**: Tool calls, thinking blocks, user interactions
- **Reward signal**: Final commit + CI/CD results (build success, tests pass)
- **Task instructions**: Inferred from commit messages or auto-generated

This eliminates the need for synthetic environment generation, artificial task construction, and simulated rewards.

### Privacy & Security

All data is processed locally before contribution:
- **Secret redaction**: API keys, tokens, passwords, private keys
- **PII anonymization**: Usernames, emails, identifying information
- **Path sanitization**: Absolute paths replaced with relative
- **User control**: Review and approve each contribution
- **Opt-in only**: Never auto-contribute without explicit consent

### CLI Commands

```bash
# Scan local sources for trajectory data
cargo marketplace trajectories scan --verbose

# Preview what would be contributed
cargo marketplace trajectories preview --limit 10 --detailed

# Submit trajectories with review
cargo marketplace trajectories contribute --review

# Submit trajectories in batch mode
cargo marketplace trajectories contribute --batch

# Check contribution status
cargo marketplace trajectories status

# View earnings
cargo marketplace trajectories earnings --detail
```

### Configuration

Configure trajectory contribution settings:

```bash
# Enable auto-contribution
cargo marketplace trajectories config --auto true

# Set minimum quality score
cargo marketplace trajectories config --min-quality 0.7

# Configure sources
cargo marketplace trajectories config --sources codex,cursor
```

### Quality Scoring

Trajectories are scored based on completeness and reward signals:

```rust
// Quality factors (weighted)
quality_score =
    0.40 * git_commit_correlation +    // Has initial/final commits
    0.30 * complexity +                 // Token count + tool calls
    0.30 * reward_signal;               // CI/CD results

// Minimum quality threshold: 0.50 (configurable)
```

### Reward Calculation

Rewards are calculated based on trajectory quality and value:

```rust
use marketplace::trajectories::RewardCalculator;

let calculator = RewardCalculator::default();
let reward = calculator.calculate_reward(&session, quality_score, min_quality);

// Base reward from complexity
// + Quality multiplier (1.0-2.0x)
// + CI/CD bonus (if tests passed)
// = Total reward in sats
```

### API Usage

```rust
use marketplace::trajectories::{
    TrajectoryCollector, TrajectoryConfig, ContributionClient,
    RedactionLevel, validate_trajectory
};

// Scan for trajectories
let config = TrajectoryConfig::default();
let collector = TrajectoryCollector::new(config.clone());
let results = collector.scan_all()?;

// Validate and calculate rewards
for result in results {
    for session in result.sessions {
        let validation = validate_trajectory(&session, config.min_quality_score);
        if validation.passed {
            let calculator = RewardCalculator::default();
            let reward = calculator.calculate_reward(
                &session,
                validation.quality_score,
                config.min_quality_score
            );
            println!("Estimated reward: {} sats", reward.total_sats);
        }
    }
}

// Submit contribution
let mut client = ContributionClient::new(ContributionConfig::default())?;
let response = client.submit(session).await?;
println!("Contributed! Event ID: {}", response.nostr_event_id);
```

### Database Schema

```sql
CREATE TABLE trajectory_contributions (
    contribution_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    source TEXT NOT NULL,
    trajectory_hash TEXT NOT NULL,
    nostr_event_id TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    quality_score REAL NOT NULL,
    estimated_reward_sats INTEGER NOT NULL,
    actual_reward_sats INTEGER,
    lightning_address TEXT,
    payment_preimage TEXT,
    submitted_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    paid_at TEXT,
    rejection_reason TEXT
);
```

## Related Crates

- **compute**: NIP-90 DVM provider implementation
- **issues**: Issue tracking for autonomous agents
- **desktop**: Desktop UI for marketplace browsing
- **nostr/core**: Nostr protocol types

## License

MIT

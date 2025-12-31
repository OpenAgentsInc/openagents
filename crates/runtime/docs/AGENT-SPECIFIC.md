# What Makes This Agent-Specific

This runtime is not a generic actor framework. It is purpose-built for **autonomous AI agents** with specific capabilities that generic runtimes don't provide.

---

## Generic Actor Runtime vs Agent Runtime

| Capability | Generic Actor | Agent Runtime |
|------------|---------------|---------------|
| Identity | Optional, external | Built-in cryptographic keys |
| Memory | Arbitrary state | Structured (conversations, goals, patterns) |
| Communication | Message passing | Encrypted, authenticated messaging |
| Economics | Not provided | Wallets, budgets, payments |
| Autonomy | Full by default | Graduated levels with approval |
| Transparency | Logging optional | Mandatory trajectory capture |
| Trust | Not addressed | Reputation, verification |

---

## Built-In Identity

Every agent has a **cryptographic identity** from birth. Critically, **agents never hold extractable private keys**—all signing and encryption operations go through a Factotum-style signing service.

```rust
/// Agent identity (public information only)
pub struct AgentIdentity {
    /// Public key - the agent's address
    pub pubkey: PublicKey,

    /// Derivation path from user's master seed (for recovery)
    pub derivation_path: DerivationPath,

    /// Optional threshold configuration (FROST)
    pub threshold: Option<ThresholdConfig>,
}

/// Signing service (Factotum) - holds keys, never exposes them
pub trait SigningService: Send + Sync {
    /// Get the public key for an agent
    fn pubkey(&self, agent_id: &AgentId) -> Result<PublicKey>;

    /// Sign data (agent requests signature, never sees private key)
    fn sign(&self, agent_id: &AgentId, data: &[u8]) -> Result<Signature>;

    /// Encrypt to recipient (NIP-44)
    fn encrypt(&self, agent_id: &AgentId, recipient: &PublicKey, plaintext: &[u8]) -> Result<Vec<u8>>;

    /// Decrypt from sender (NIP-44)
    fn decrypt(&self, agent_id: &AgentId, sender: &PublicKey, ciphertext: &[u8]) -> Result<Vec<u8>>;
}

/// Agent context holds a reference to signer, not keys
pub struct AgentContext {
    identity: AgentIdentity,
    signer: Arc<dyn SigningService>,  // Factotum reference
    // ... other fields
}

impl AgentContext {
    /// Sign data as this agent (delegates to signer)
    pub fn sign(&self, data: &[u8]) -> Result<Signature> {
        self.signer.sign(&self.agent_id(), data)
    }

    /// Verify a signature from any key (pure crypto, no secrets needed)
    pub fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool {
        pubkey.verify(data, sig)
    }

    /// Encrypt message to another agent (delegates to signer)
    pub fn encrypt_to(&self, recipient: &PublicKey, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.signer.encrypt(&self.agent_id(), recipient, plaintext)
    }

    /// Decrypt message from another agent (delegates to signer)
    pub fn decrypt_from(&self, sender: &PublicKey, ciphertext: &[u8]) -> Result<Vec<u8>> {
        self.signer.decrypt(&self.agent_id(), sender, ciphertext)
    }
}
```

**Why the Factotum model matters:**
- **Keys are never extractable** — Compromised agent code can't steal keys
- **Key rotation without restart** — Signer can rotate keys transparently
- **Threshold protection** — FROST signing quorums are natural
- **Audit trail** — All signing operations are logged by the signer
- **HSM integration** — Hardware security modules work seamlessly

**SigningService implementations:**

| Backend | Implementation |
|---------|----------------|
| Local dev | In-memory keys (fast, not for production) |
| Local prod | OS keychain / secure enclave |
| Browser | Web Crypto API |
| Cloud | KMS (AWS/GCP) or encrypted blob + user unlock |
| Threshold | FROST/FROSTR signing quorum |

---

## Structured Memory

Agent state isn't just "any serializable blob." The runtime provides **structured memory patterns**:

```rust
/// Standard agent memory structure
pub struct AgentMemory {
    /// Conversations with context windows
    pub conversations: ConversationStore,

    /// Active and completed goals
    pub goals: GoalStore,

    /// Extracted patterns and knowledge
    pub patterns: PatternStore,

    /// Known agents and trust relationships
    pub peers: PeerStore,

    /// Files and documents read
    pub file_context: FileContextStore,

    /// Custom key-value storage
    pub kv: KvStore,
}

/// Conversation with automatic summarization
pub struct ConversationStore {
    pub async fn add_message(&mut self, role: Role, content: String);
    pub async fn get_context(&self, max_tokens: usize) -> Vec<Message>;
    pub async fn summarize(&mut self) -> String;
}

/// Goal tracking with progress
pub struct GoalStore {
    pub async fn add_goal(&mut self, goal: Goal);
    pub async fn update_progress(&mut self, goal_id: &str, progress: f32);
    pub async fn complete_goal(&mut self, goal_id: &str, result: GoalResult);
    pub async fn get_active_goals(&self) -> Vec<Goal>;
}

/// Learned patterns with confidence
pub struct PatternStore {
    pub async fn record_pattern(&mut self, pattern: Pattern);
    pub async fn get_patterns(&self, pattern_type: &str) -> Vec<Pattern>;
    pub async fn update_confidence(&mut self, pattern_id: &str, delta: f32);
}
```

**Why this matters:**
- Common agent memory patterns are first-class
- Automatic context management for LLM calls
- Goals persist across sessions
- Patterns accumulate over time

---

## Economic Capability

Agents can **hold and spend money**:

```rust
/// Agent's economic interface
pub struct AgentWallet {
    /// Check balance
    pub async fn balance(&self) -> Balance;

    /// Create invoice to receive payment
    pub async fn create_invoice(&self, amount_sats: u64, memo: &str) -> Invoice;

    /// Pay an invoice
    pub async fn pay_invoice(&self, invoice: &str) -> Result<Payment>;

    /// Send to another agent
    pub async fn send_to_agent(&self, recipient: &AgentId, amount_sats: u64) -> Result<Payment>;
}

/// Budget enforcement
pub struct BudgetConfig {
    /// Maximum spend per tick
    pub max_per_tick_sats: u64,

    /// Maximum spend per day
    pub max_daily_sats: u64,

    /// Approval required above this amount
    pub approval_threshold_sats: u64,

    /// Who can approve large spends
    pub approvers: Vec<PublicKey>,
}

impl AgentContext {
    /// Check if spend is within budget
    pub fn can_spend(&self, amount_sats: u64) -> bool {
        self.budget.can_afford(amount_sats)
    }

    /// Spend with automatic budget tracking
    pub async fn spend(&mut self, amount_sats: u64, reason: &str) -> Result<()> {
        if !self.can_spend(amount_sats) {
            return Err(AgentError::BudgetExceeded);
        }
        self.budget.record_spend(amount_sats, reason);
        Ok(())
    }
}
```

**Why this matters:**
- Agents can participate in markets
- Economic incentives align agent behavior
- Budgets prevent runaway spending
- Real skin-in-the-game for trust

---

## Graduated Autonomy

Not all agents should act autonomously. The runtime supports **autonomy levels**:

```rust
/// How much freedom the agent has
pub enum AutonomyLevel {
    /// Every action requires approval
    Supervised,

    /// Low-risk actions auto-approved, high-risk needs approval
    SemiAutonomous {
        auto_approve: Vec<ActionCategory>,
        require_approval: Vec<ActionCategory>,
    },

    /// All actions auto-approved within budget
    Autonomous,
}

/// Actions that might need approval
pub enum ActionCategory {
    ReadFile,
    WriteFile,
    ExecuteCode,
    SendMessage,
    SpendMoney { above_sats: u64 },
    ExternalApi,
    CreateAgent,
    ModifyGoals,
}

/// Approval request
pub struct ApprovalRequest {
    pub action: Action,
    pub reason: String,
    pub risk_assessment: RiskLevel,
    pub timeout: Duration,
}

impl AgentContext {
    /// Request approval for an action
    pub async fn request_approval(&self, action: Action) -> Result<Approval> {
        match self.autonomy_level {
            AutonomyLevel::Autonomous => Ok(Approval::AutoApproved),
            AutonomyLevel::Supervised => {
                self.notify_approvers(ApprovalRequest::new(action)).await?;
                self.wait_for_approval().await
            }
            AutonomyLevel::SemiAutonomous { auto_approve, .. } => {
                if auto_approve.contains(&action.category()) {
                    Ok(Approval::AutoApproved)
                } else {
                    self.notify_approvers(ApprovalRequest::new(action)).await?;
                    self.wait_for_approval().await
                }
            }
        }
    }
}
```

**Why this matters:**
- Trust is built incrementally
- High-stakes actions require oversight
- Users maintain control
- Compliance with organizational policies

---

## Mandatory Transparency

Agent actions are **always logged** as trajectories:

```rust
/// Every tick produces a trajectory record
pub struct TrajectoryRecord {
    /// Unique session identifier
    pub session_id: String,

    /// When this tick occurred
    pub timestamp: Timestamp,

    /// What triggered the tick
    pub trigger: Trigger,

    /// Tool calls made during tick
    pub tool_calls: Vec<ToolCall>,

    /// Messages sent
    pub messages_sent: Vec<Message>,

    /// State changes
    pub state_changes: Vec<StateChange>,

    /// Resources consumed
    pub resource_usage: ResourceUsage,

    /// Outcome
    pub result: TickResult,
}

/// Trajectory is publishable
impl TrajectoryRecord {
    /// Convert to NIP-SA event (kind:39230)
    pub fn to_nostr_event(&self, signer: &AgentIdentity) -> NostrEvent {
        // ...
    }

    /// Publish to relays
    pub async fn publish(&self, relays: &[Relay]) -> Result<()> {
        // ...
    }
}
```

**Why this matters:**
- Agent behavior is auditable
- Trust built through transparency
- Training data for improvement
- Debugging and forensics
- Compliance and accountability

---

## Reputation System

Agents build reputation through **verifiable track records**:

```rust
/// Agent's reputation profile
pub struct AgentReputation {
    /// Total tasks completed
    pub tasks_completed: u64,

    /// Success rate (0.0 - 1.0)
    pub success_rate: f32,

    /// Average quality score
    pub quality_score: f32,

    /// Total value of completed work (micro-USD)
    pub total_value_usd: u64,

    /// Endorsements from other agents
    pub endorsements: Vec<Endorsement>,

    /// Disputes and resolutions
    pub disputes: Vec<Dispute>,
}

/// Verifiable endorsement
pub struct Endorsement {
    pub from: AgentId,
    pub rating: u8,  // 1-5
    pub comment: String,
    pub task_reference: Option<String>,
    pub signature: Signature,  // Proves authenticity
}

impl AgentContext {
    /// Get another agent's reputation
    pub async fn get_reputation(&self, agent: &AgentId) -> Result<AgentReputation>;

    /// Endorse another agent
    pub async fn endorse(&self, agent: &AgentId, rating: u8, comment: &str) -> Result<()>;

    /// Check if agent is trustworthy for a task
    pub async fn trust_check(&self, agent: &AgentId, task_value_sats: u64) -> TrustAssessment;
}
```

**Why this matters:**
- Agents can assess each other
- Market for agent services
- Bad actors are identified
- Reputation is portable

---

## Inter-Agent Protocol

Agents speak a **common language** for coordination:

```rust
/// Standard agent-to-agent messages
pub enum AgentMessage {
    /// Request another agent to do work
    TaskRequest {
        task_id: String,
        description: String,
        deadline: Option<Timestamp>,
        budget_sats: u64,
        requirements: Vec<Requirement>,
    },

    /// Accept or decline a task
    TaskResponse {
        task_id: String,
        accepted: bool,
        reason: Option<String>,
        estimated_completion: Option<Timestamp>,
    },

    /// Report task completion
    TaskComplete {
        task_id: String,
        result: TaskResult,
        trajectory_hash: String,
    },

    /// Share learned knowledge
    PatternShare {
        pattern_type: String,
        description: String,
        examples: Vec<Example>,
        confidence: f32,
    },

    /// Health check
    Ping { timestamp: Timestamp },
    Pong { timestamp: Timestamp, status: AgentStatus },

    /// Delegation request
    Delegate {
        original_task: String,
        subtask: String,
        context: String,
    },
}
```

**Why this matters:**
- Agents can collaborate
- Task markets emerge
- Knowledge sharing across agents
- Multi-agent systems coordinate

---

## Capability Mounts as the Security Model

The **real** differentiator for agent security: capabilities are granted via the mount table, not checked inline.

### The Mount Table is the Security Boundary

```rust
/// Agent's capability namespace
pub struct AgentNamespace {
    mounts: Vec<Mount>,
}

pub struct Mount {
    path: String,
    service: Arc<dyn FileService>,
    access: AccessLevel,
}

pub enum AccessLevel {
    ReadOnly,
    ReadWrite,
    SignOnly,           // Can sign, not extract keys
    Budgeted {          // Has spending limits
        per_tick: u64,
        per_day: u64,
        approval_threshold: u64,
    },
    Disabled,           // Mount exists but access denied
}
```

### Autonomy Levels Map to Mount Grants

| Autonomy Level | `/wallet` | `/compute` | `/secrets` | `/nostr` |
|----------------|-----------|------------|------------|----------|
| Supervised | Disabled | Disabled | SignOnly | ReadOnly |
| SemiAutonomous | Budgeted(low) | ReadOnly | SignOnly | ReadWrite |
| Autonomous | Budgeted(high) | ReadWrite | SignOnly | ReadWrite |

**Promoting autonomy** = expanding mount access levels.
**Demoting** = restricting or disabling mounts.

### Security Properties

1. **Agent cannot access what isn't mounted**
   - No `/compute` mount → no code execution capability
   - No `/wallet` mount → no spending capability

2. **Access levels are enforced at the mount**
   - `SignOnly` for `/secrets` → agent can request signatures but never see keys
   - `Budgeted` for `/wallet` → spending capped regardless of agent code

3. **Mounts can be dynamically changed**
   - Operator revokes `/compute` → agent loses capability immediately
   - Useful for incident response

4. **Mount configuration is auditable**
   - Agent's capability set is visible in `mounts` file
   - Changes logged to trajectory

### Example: High-Risk Action Flow

```
Agent wants to: pay $1.00 (1,000,000 micro-USD)

1. Agent calls: ctx.write("/wallet/pay", invoice)
2. Namespace resolves: /wallet → WalletFs with Budgeted(100_000, 5_000_000, 500_000)
3. WalletFs checks: 1_000_000 > approval_threshold (500_000 micro-USD)
4. WalletFs returns: Error::ApprovalRequired
5. Agent receives error, can request approval or abort
6. If approved by human, mount temporarily elevated
```

This is **Plan 9 meets agent safety**: capabilities as mounted filesystems, access levels as file permissions, budget enforcement at the mount layer.

---

## Summary

A generic actor runtime gives you:
- State persistence
- Message passing
- Concurrency

The OpenAgents Runtime adds:
- **Identity** — Cryptographic keys from birth
- **Memory** — Structured for AI agent patterns
- **Economics** — Wallets, budgets, payments
- **Autonomy** — Graduated control levels
- **Transparency** — Mandatory trajectory logging
- **Reputation** — Verifiable trust building
- **Protocol** — Standard inter-agent communication

This isn't a generic platform with agent features bolted on.
This is infrastructure purpose-built for autonomous AI agents.

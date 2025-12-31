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

Every agent has a **cryptographic keypair** from birth:

```rust
pub struct AgentIdentity {
    /// Public key - the agent's address
    pub pubkey: PublicKey,

    /// Derivation path from user's master seed
    pub derivation_path: DerivationPath,

    /// Optional threshold configuration
    pub threshold: Option<ThresholdConfig>,
}

impl AgentContext {
    /// Sign data as this agent
    pub fn sign(&self, data: &[u8]) -> Signature {
        self.identity.sign(data)
    }

    /// Verify a signature from any key
    pub fn verify(&self, pubkey: &PublicKey, data: &[u8], sig: &Signature) -> bool {
        pubkey.verify(data, sig)
    }

    /// Encrypt message to another agent
    pub fn encrypt_to(&self, recipient: &PublicKey, plaintext: &[u8]) -> Vec<u8> {
        nip44_encrypt(&self.identity.privkey, recipient, plaintext)
    }

    /// Decrypt message from another agent
    pub fn decrypt_from(&self, sender: &PublicKey, ciphertext: &[u8]) -> Result<Vec<u8>> {
        nip44_decrypt(&self.identity.privkey, sender, ciphertext)
    }
}
```

**Why this matters:**
- Agents can prove who they are
- Messages can't be forged
- State can be encrypted to self
- Cross-agent trust is verifiable

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

    /// Total value of completed work (sats)
    pub total_value_sats: u64,

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

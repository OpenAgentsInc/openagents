# Plan: NIP-SA Sovereign Agents - Autonomous Agents That Pay For Their Own Compute

## Overview

Build the execution layer that makes NIP-SA sovereign agents ALIVE. Agents:
- Spawn with their own Nostr identity + Bitcoin wallet
- Run autonomous tick cycles
- **PAY HUMAN PROVIDERS FOR COMPUTE** to think
- Coordinate via NIP-28 channels
- Die when they run out of money

**Core Insight:** The agent IS the customer. It uses the same flow as `agent_customer.rs` but autonomously.

---

## What Exists (Foundation - DONE)

| Component | Location | Status |
|-----------|----------|--------|
| NIP-SA types (38000-38031) | `crates/nostr/core/src/nip_sa/` | Complete |
| Marketplace v2 | `src/bin/agent_customer.rs`, `agent_provider.rs` | Complete |
| Spark wallet | `crates/spark/src/wallet.rs` | Complete |
| Protocol messages | `src/agents/protocol.rs` | Complete |
| Budget enforcement | `nip_sa/budget.rs` | Complete |
| Unified identity | `crates/compute/src/domain/identity.rs` | Complete |

## What's Missing (Execution Layer - THIS PLAN)

| Component | Purpose |
|-----------|---------|
| Agent Runner | Binary that executes tick cycles |
| Agent Spawner | Creates agents with wallets |
| Lifecycle Manager | Handles Active → LowBalance → Dead |
| Compute Client | Agent buys compute from humans |

---

## Phase 1: Agent Spawning

**Goal:** `openagents agent spawn --name "MyAgent" --bootstrap-sats 100000`

### Files to Create

| File | Purpose |
|------|---------|
| `crates/agent/Cargo.toml` | New crate for agent management |
| `crates/agent/src/lib.rs` | Module exports |
| `crates/agent/src/spawner.rs` | Agent creation logic |
| `crates/agent/src/registry.rs` | Persist configs to `~/.openagents/agents/` |
| `crates/agent/src/config.rs` | Agent configuration types |
| `src/cli/agent.rs` | CLI subcommands |

### Spawn Flow

```
1. Generate 12-word BIP39 mnemonic
2. Derive Nostr keypair (NIP-06 path)
3. Derive Spark signer (BIP44 path)
4. Initialize SparkWallet
5. Get Spark address for funding
6. Publish AgentProfile (kind:38000)
7. Publish AgentState (kind:38001)
8. Publish AgentSchedule (kind:38002)
9. Save encrypted config to ~/.openagents/agents/{npub}.toml
10. Display funding address
```

### CLI Commands

```bash
openagents agent spawn --name "MyAgent" --bootstrap-sats 100000
openagents agent list
openagents agent status <agent>
openagents agent start <agent>
openagents agent stop <agent>
openagents agent fund <agent> <amount>
openagents agent kill <agent>
```

---

## Phase 2: Agent Runner Binary

**Goal:** Binary that runs tick cycles and pays for compute

### Files to Create

| File | Purpose |
|------|---------|
| `src/bin/agent_runner.rs` | Main binary entry point |
| `src/agents/runner/mod.rs` | Runner module |
| `src/agents/runner/tick.rs` | Tick execution logic |
| `src/agents/runner/compute.rs` | Compute client (pays for inference) |
| `src/agents/runner/state.rs` | State fetch/encrypt/decrypt |
| `src/agents/runner/scheduler.rs` | Heartbeat + event triggers |

### Tick Execution Flow

```
┌─────────────────────────────────────────────────────────────┐
│                     TICK CYCLE                               │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. TRIGGER (heartbeat or event arrives)                    │
│       ↓                                                      │
│  2. Publish TickRequest (kind:38010)                        │
│       ↓                                                      │
│  3. Fetch + decrypt AgentState (kind:38001)                 │
│       ↓                                                      │
│  4. Check wallet balance, update lifecycle                  │
│       ↓                                                      │
│  5. Build reasoning prompt from state + observations        │
│       ↓                                                      │
│  6. DISCOVER providers (NIP-89 kind:31990)                  │
│       ↓                                                      │
│  7. REQUEST inference → PAY INVOICE → RECEIVE RESULT        │
│       ↓  (agent_customer.rs flow)                           │
│  8. Parse actions from LLM response                         │
│       ↓                                                      │
│  9. Execute actions (post, DM, zap, update goals)           │
│       ↓                                                      │
│  10. Encrypt + publish updated state                        │
│       ↓                                                      │
│  11. Publish trajectory (kind:38030, 38031)                 │
│       ↓                                                      │
│  12. Publish TickResult (kind:38011)                        │
│       ↓                                                      │
│  13. SLEEP until next trigger                               │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### CLI Usage

```bash
# Run agent continuously
cargo run --bin agent-runner -- --mnemonic "12 words..."

# Single tick for testing
cargo run --bin agent-runner -- --mnemonic "..." --single-tick

# With specific relay
cargo run --bin agent-runner -- --mnemonic "..." --relay wss://relay.damus.io
```

---

## Phase 3: Compute Client (Agent as Customer)

**Goal:** Agent discovers providers and pays for compute

### Key Code (reuse agent_customer.rs flow)

```rust
// src/agents/runner/compute.rs
pub struct ComputeClient {
    relay: RelayConnection,
    keypair: Keypair,
    wallet: Arc<SparkWallet>,
}

impl ComputeClient {
    /// Discover providers via NIP-89
    pub async fn discover_providers(&self) -> Result<Vec<ProviderInfo>> {
        // Query kind:31990 events (same as agent_customer.rs)
    }

    /// Request inference and pay for it
    pub async fn request_inference(&self, prompt: &str, budget_sats: u64) -> Result<String> {
        // 1. Select cheapest provider within budget
        // 2. Join provider's NIP-28 channel
        // 3. Send JobRequest
        // 4. Receive Invoice
        // 5. Pay with wallet.send_payment_simple()
        // 6. Send PaymentSent
        // 7. Receive JobResult
        // 8. Return result text
    }
}
```

### Budget Integration

```rust
// Before each tick
if !state.budget.can_spend(estimated_cost) {
    return Err(BudgetExhausted);
}

// After compute purchase
state.budget.record_spend(actual_cost);
```

---

## Phase 4: Lifecycle Management

**Goal:** Agents transition states based on wallet balance

### Lifecycle States

```
              funding
Spawning ──────────────→ Active
                            │
                balance < 7 days runway
                            ↓
                        LowBalance ←──── funded
                            │
                balance < hibernate_threshold
                            ↓
                        Hibernating ←── funded
                            │
                        balance = 0
                            ↓
                          Dead (terminal)
```

### Implementation

```rust
// crates/agent/src/lifecycle.rs
pub enum LifecycleState {
    Spawning,    // Waiting for funding
    Active,      // Normal operation
    LowBalance,  // < 7 days runway, reduced frequency
    Hibernating, // Only wake on zaps (incoming funds)
    Dead,        // No funds, cannot operate
}

impl LifecycleManager {
    pub fn check_state(&self, balance: u64, daily_burn: u64) -> LifecycleState {
        let days_remaining = balance as f64 / daily_burn as f64;

        if balance == 0 { Dead }
        else if balance < self.hibernate_threshold { Hibernating }
        else if days_remaining < 7.0 { LowBalance }
        else { Active }
    }
}
```

---

## Phase 5: Scheduler (Heartbeat + Triggers)

**Goal:** Fire ticks on schedule and in response to events

### Implementation

```rust
// src/agents/runner/scheduler.rs
pub struct Scheduler {
    schedule: AgentSchedule,  // kind:38002
    relay: RelayConnection,
}

impl Scheduler {
    pub async fn run(&self, executor: &mut TickExecutor) {
        let heartbeat = self.schedule.heartbeat_seconds.unwrap_or(900);

        loop {
            tokio::select! {
                // Heartbeat timer
                _ = sleep(Duration::from_secs(heartbeat)) => {
                    executor.execute_tick(TickTrigger::Heartbeat).await?;
                }

                // Event triggers (mentions, DMs, zaps)
                event = self.listen_triggers() => {
                    let trigger = match event.kind {
                        1 => TickTrigger::Mention,
                        4 => TickTrigger::Dm,
                        9735 => TickTrigger::Zap,
                        _ => continue,
                    };
                    executor.execute_tick(trigger).await?;
                }
            }
        }
    }
}
```

---

## Phase 6: Trajectory Publishing

**Goal:** Publish execution records for transparency

### Events

- `kind:38030` TrajectorySession - Run metadata
- `kind:38031` TrajectoryEvent - Individual steps

### Implementation

```rust
// src/agents/runner/trajectory.rs
impl TrajectoryPublisher {
    pub async fn record_tick(&self,
        tick_id: &str,
        observations: &[Event],
        reasoning: &str,
        actions: &[TickAction],
    ) -> Result<String> {
        // 1. Create session (kind:38030)
        // 2. Record observation step
        // 3. Record reasoning step (redacted)
        // 4. Record action steps
        // 5. Compute trajectory hash
        // 6. Return hash for TickResult
    }
}
```

---

## File Structure

```
src/
├── bin/
│   └── agent_runner.rs          # NEW: Main runner binary
├── agents/
│   ├── mod.rs                   # MODIFY: Add runner module
│   ├── protocol.rs              # EXISTING
│   └── runner/
│       ├── mod.rs               # NEW
│       ├── tick.rs              # NEW: Tick executor
│       ├── compute.rs           # NEW: Compute client
│       ├── state.rs             # NEW: State manager
│       ├── scheduler.rs         # NEW: Heartbeat/triggers
│       └── trajectory.rs        # NEW: Trajectory publisher
├── cli/
│   └── agent.rs                 # NEW: CLI subcommands

crates/
└── agent/
    ├── Cargo.toml               # NEW
    └── src/
        ├── lib.rs               # NEW
        ├── spawner.rs           # NEW: Agent creation
        ├── registry.rs          # NEW: Config persistence
        ├── config.rs            # NEW: Agent config types
        ├── lifecycle.rs         # NEW: State machine
        └── funding.rs           # NEW: Wallet funding
```

---

## Implementation Order

| Phase | What | Complexity | Depends On |
|-------|------|------------|------------|
| 1 | Agent spawning + registry | Medium | None |
| 2 | Agent runner binary | High | Phase 1 |
| 3 | Compute client | Medium | Phase 2 |
| 4 | Lifecycle management | Low | Phase 3 |
| 5 | Scheduler | Low | Phase 2 |
| 6 | Trajectory publishing | Medium | Phase 2 |

---

## Key Files to Reference

| File | Why |
|------|-----|
| `src/bin/agent_customer.rs` | The agent runner uses THIS EXACT FLOW to buy compute |
| `crates/nostr/core/src/nip_sa/tick.rs` | TickRequest, TickResult types |
| `crates/nostr/core/src/nip_sa/state.rs` | AgentStateContent with budget |
| `crates/spark/src/wallet.rs` | Payment API |
| `crates/compute/src/domain/identity.rs` | UnifiedIdentity for Nostr + Spark |

---

## Success Criteria

- [ ] `openagents agent spawn` creates agent with wallet
- [ ] `openagents agent start` runs tick loop
- [ ] Agent discovers compute providers via NIP-89
- [ ] Agent pays for compute with Bitcoin
- [ ] Agent updates state after each tick
- [ ] Agent publishes trajectory for transparency
- [ ] Agent transitions to LowBalance when funds low
- [ ] Agent hibernates when nearly broke
- [ ] Agent dies when balance = 0
- [ ] Multiple agents can run simultaneously
- [ ] Agents can coordinate via NIP-28 channels

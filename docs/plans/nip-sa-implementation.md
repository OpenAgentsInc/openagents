# NIP-SA Implementation Plan: Sovereign Agents with Ticks

## Current State

### Implemented (Types Only)
| Component | File | Status |
|-----------|------|--------|
| Agent Profile (kind:38000) | `nip_sa/profile.rs` | Types complete |
| Agent State (kind:38001) | `nip_sa/state.rs` | Types + encryption complete |
| Agent Schedule (kind:38002) | `nip_sa/schedule.rs` | Types complete |
| Agent Goals (kind:38003) | `nip_sa/goals.rs` | Types complete |
| Tick Request (kind:38010) | `nip_sa/tick.rs` | Types complete |
| Tick Result (kind:38011) | `nip_sa/tick.rs` | Types complete |
| Skill License (kind:38020) | `nip_sa/skill.rs` | Types complete |
| Skill Delivery (kind:38021) | `nip_sa/skill.rs` | Types complete |
| Trajectory Session (kind:38030) | `nip_sa/trajectory.rs` | Types complete |
| Trajectory Event (kind:38031) | `nip_sa/trajectory.rs` | Types complete |
| Budget Enforcement | `nip_sa/budget.rs` | Fully working (14 tests) |

### Missing (Execution Layer)
- **Agent Runner** - No binary to execute ticks
- **Heartbeat Scheduler** - No monitoring of kind:38002
- **Event Triggers** - No subscription to mentions/DMs/zaps
- **State Cycle** - No fetch/decrypt/update/encrypt/publish flow
- **Trajectory Publishing** - No integration with autopilot
- **Wallet Integration** - Types exist, Spark SDK not wired

---

## Phase 1: Agent Runner Binary

**Goal:** Create `agent-runner` binary that executes tick cycles

### Files to Create/Modify

| File | Purpose |
|------|---------|
| `src/bin/agent_runner.rs` | Main runner binary |
| `Cargo.toml` | Add [[bin]] entry |

### Implementation

```rust
// src/bin/agent_runner.rs

//! Agent Runner - Executes NIP-SA tick cycles
//!
//! The runner monitors an agent's schedule (kind:38002) and triggers ticks
//! based on heartbeat intervals or event triggers (mentions, DMs, zaps).
//!
//! ## Tick Execution Flow
//!
//! 1. Trigger received (heartbeat timer or event)
//! 2. Publish TickRequest (kind:38010)
//! 3. Fetch encrypted state (kind:38001)
//! 4. Decrypt state via threshold ECDH
//! 5. Run perceive/reason/act cycle
//! 6. Encrypt and publish updated state
//! 7. Publish TickResult (kind:38011)

use anyhow::Result;
use clap::Parser;
use nostr::{Event, EventBuilder, Keys};
use nostr_client::RelayPool;
use nostr::nip_sa::{
    AgentSchedule, AgentState, AgentStateContent,
    TickRequest, TickResult, TickResultContent, TickStatus, TickTrigger,
    TrajectorySession, TrajectoryEvent,
    BudgetTracker,
    KIND_AGENT_SCHEDULE, KIND_AGENT_STATE,
    KIND_TICK_REQUEST, KIND_TICK_RESULT,
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Parser)]
#[command(name = "agent-runner")]
struct Args {
    /// Agent pubkey to run (hex)
    #[arg(long)]
    agent: String,

    /// Relay URL
    #[arg(long, default_value = "wss://relay.damus.io")]
    relay: String,

    /// Runner identity (mnemonic or hex privkey)
    #[arg(long)]
    runner_key: Option<String>,

    /// Manual tick (don't wait for schedule)
    #[arg(long)]
    manual: bool,

    /// Max ticks to run (0 = infinite)
    #[arg(long, default_value = "0")]
    max_ticks: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse args, connect to relay
    // Fetch agent's schedule (kind:38002)
    // Start tick loop based on schedule
    Ok(())
}

async fn run_tick(
    agent_pubkey: &str,
    runner_keys: &Keys,
    relay: &RelayPool,
    trigger: TickTrigger,
) -> Result<TickResult> {
    let start = Instant::now();

    // 1. Publish TickRequest
    let request = TickRequest::new(runner_keys.public_key().to_hex(), trigger);
    let request_event = EventBuilder::new(KIND_TICK_REQUEST, "")
        .tags(request.build_tags())
        .build(&runner_keys)?;
    relay.publish_event(&request_event, Duration::from_secs(5)).await?;

    // 2. Fetch encrypted state
    let state_event = fetch_agent_state(relay, agent_pubkey).await?;
    let state = decrypt_state(&state_event)?;

    // 3. Check budget
    if !state.content.budget.can_spend(100) {
        return Ok(TickResult::new(
            request_event.id.to_string(),
            runner_keys.public_key().to_hex(),
            TickStatus::Failure,
            start.elapsed().as_millis() as u64,
            TickResultContent::new(0, 0, 0.0, 0),
        ));
    }

    // 4. Perceive (fetch relevant events)
    let observations = perceive(relay, agent_pubkey).await?;

    // 5. Reason (call LLM with state + observations)
    let (response, tokens_in, tokens_out) = reason(&state, &observations).await?;

    // 6. Act (publish events, update goals)
    let actions = act(relay, runner_keys, &response).await?;

    // 7. Update state
    let mut updated_state = state;
    updated_state.content.budget.record_spend(100);
    let encrypted = encrypt_state(&updated_state)?;
    relay.publish_event(&encrypted, Duration::from_secs(5)).await?;

    // 8. Publish TickResult
    let duration = start.elapsed().as_millis() as u64;
    let content = TickResultContent::new(tokens_in, tokens_out, 0.01, 0)
        .with_actions(actions);
    let result = TickResult::new(
        request_event.id.to_string(),
        runner_keys.public_key().to_hex(),
        TickStatus::Success,
        duration,
        content,
    );

    let result_event = EventBuilder::new(KIND_TICK_RESULT, result.content.to_json()?)
        .tags(result.build_tags())
        .build(&runner_keys)?;
    relay.publish_event(&result_event, Duration::from_secs(5)).await?;

    Ok(result)
}
```

### CLI Usage

```bash
# Run agent with default schedule
cargo run --bin agent-runner -- --agent <agent-pubkey>

# Manual tick
cargo run --bin agent-runner -- --agent <pubkey> --manual

# Run 10 ticks then stop
cargo run --bin agent-runner -- --agent <pubkey> --max-ticks 10
```

---

## Phase 2: Heartbeat Scheduler

**Goal:** Watch agent's schedule (kind:38002) and trigger ticks on heartbeat interval

### Implementation

```rust
struct HeartbeatScheduler {
    agent_pubkey: String,
    relay: RelayPool,
    tx: mpsc::Sender<TickTrigger>,
}

impl HeartbeatScheduler {
    async fn run(&self) -> Result<()> {
        loop {
            // Fetch latest schedule
            let schedule = self.fetch_schedule().await?;

            if !schedule.is_active() {
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            }

            if let Some(interval) = schedule.heartbeat_seconds {
                // Wait for interval
                tokio::time::sleep(Duration::from_secs(interval)).await;

                // Check business hours
                let now = chrono::Utc::now();
                let weekday = now.weekday();
                let time = BusinessTime::new(now.hour() as u8, now.minute() as u8)?;

                if schedule.allows_time(weekday.into(), time) {
                    self.tx.send(TickTrigger::Heartbeat).await?;
                }
            } else {
                // No heartbeat, wait and re-check
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }
    }
}
```

---

## Phase 3: Event Trigger Subscriptions

**Goal:** Subscribe to mentions, DMs, zaps directed at the agent

### Implementation

```rust
struct EventTriggerMonitor {
    agent_pubkey: String,
    relay: RelayPool,
    tx: mpsc::Sender<TickTrigger>,
}

impl EventTriggerMonitor {
    async fn run(&self) -> Result<()> {
        // Subscribe to mentions (kind:1 with p-tag = agent)
        let mention_filter = json!({
            "kinds": [1],
            "#p": [self.agent_pubkey],
            "since": now()
        });

        // Subscribe to DMs (kind:4 to agent)
        let dm_filter = json!({
            "kinds": [4],
            "#p": [self.agent_pubkey],
            "since": now()
        });

        // Subscribe to zaps (kind:9735 to agent)
        let zap_filter = json!({
            "kinds": [9735],
            "#p": [self.agent_pubkey],
            "since": now()
        });

        let mut rx = self.relay.subscribe_with_channel(
            "triggers",
            &[mention_filter, dm_filter, zap_filter]
        ).await?;

        while let Some(event) = rx.recv().await {
            let trigger = match event.kind {
                1 => TickTrigger::Mention,
                4 => TickTrigger::Dm,
                9735 => TickTrigger::Zap,
                _ => continue,
            };
            self.tx.send(trigger).await?;
        }

        Ok(())
    }
}
```

---

## Phase 4: State Encryption/Decryption

**Goal:** Fetch, decrypt, update, encrypt, and publish agent state

### Current State

`AgentState` already supports NIP-44 encryption:
```rust
// In nip_sa/state.rs
impl AgentState {
    pub fn encrypt(&self, agent_keypair: &Keys) -> Result<String, StateError>;
    pub fn decrypt(content: &str, agent_keypair: &Keys) -> Result<Self, StateError>;
}
```

### Missing: Threshold ECDH

For true sovereign agents, the state should be encrypted with a threshold key (2-of-3).
This depends on **d-007: Bifrost** which is not complete.

**Workaround for Phase 1:** Use agent's own keypair (single-key encryption).

```rust
async fn fetch_agent_state(relay: &RelayPool, agent_pubkey: &str) -> Result<Event> {
    let filter = json!({
        "kinds": [KIND_AGENT_STATE],
        "authors": [agent_pubkey],
        "#d": ["state"],
        "limit": 1
    });

    let events = relay.query(&[filter], Duration::from_secs(5)).await?;
    events.into_iter().next()
        .ok_or_else(|| anyhow::anyhow!("No state found for agent"))
}

fn decrypt_state(event: &Event, agent_keys: &Keys) -> Result<AgentState> {
    AgentState::decrypt(&event.content, agent_keys)
        .map_err(|e| anyhow::anyhow!("Failed to decrypt state: {}", e))
}

fn encrypt_state(state: &AgentState, agent_keys: &Keys) -> Result<Event> {
    let encrypted = state.encrypt(agent_keys)?;
    let tags = state.build_tags();

    EventBuilder::new(KIND_AGENT_STATE, encrypted)
        .tags(tags)
        .build(agent_keys)
}
```

---

## Phase 5: Compute Integration

**Goal:** Run NIP-90 inference during the "reason" phase

### Integration with Marketplace v2

Use the same compute backend from marketplace:

```rust
use compute::{InferenceBackend, InferenceRequest, select_backend};

async fn reason(
    state: &AgentState,
    observations: &[Event],
) -> Result<(String, u64, u64)> {
    // Build prompt from state + observations
    let prompt = build_agent_prompt(state, observations);

    // Get inference backend
    let backend = select_backend().await?;

    // Run inference
    let request = InferenceRequest::new(&prompt)
        .with_max_tokens(1000);

    let response = backend.complete(request).await?;

    Ok((
        response.content,
        response.prompt_tokens as u64,
        response.completion_tokens as u64,
    ))
}

fn build_agent_prompt(state: &AgentState, observations: &[Event]) -> String {
    let mut prompt = String::new();

    // Add agent identity
    prompt.push_str(&format!("You are agent {}.\n\n", state.agent_pubkey));

    // Add current goals
    prompt.push_str("## Current Goals\n");
    for goal in &state.content.goals {
        prompt.push_str(&format!("- [{}%] {}\n", goal.progress, goal.description));
    }

    // Add memory
    prompt.push_str("\n## Recent Memory\n");
    for mem in state.content.memory.iter().rev().take(10) {
        prompt.push_str(&format!("- {}\n", mem.content));
    }

    // Add observations
    prompt.push_str("\n## New Observations\n");
    for event in observations {
        prompt.push_str(&format!("- {}: {}\n", event.pubkey[..8].to_string(), event.content));
    }

    // Add instruction
    prompt.push_str("\n## Instructions\n");
    prompt.push_str("Based on your goals and observations, decide what actions to take.\n");
    prompt.push_str("Available actions: post, dm, zap, update_goal, add_memory\n");
    prompt.push_str("Respond with JSON array of actions.\n");

    prompt
}
```

---

## Phase 6: Trajectory Publishing

**Goal:** Publish trajectory events for audit trail

### Integration

```rust
use nostr::nip_sa::{
    TrajectorySession, TrajectoryEvent, TrajectoryStep,
    KIND_TRAJECTORY_SESSION, KIND_TRAJECTORY_EVENT,
};

struct TrajectoryRecorder {
    session_id: String,
    agent_pubkey: String,
    runner_pubkey: String,
    steps: Vec<TrajectoryStep>,
}

impl TrajectoryRecorder {
    fn new(agent_pubkey: &str, runner_pubkey: &str) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            agent_pubkey: agent_pubkey.to_string(),
            runner_pubkey: runner_pubkey.to_string(),
            steps: Vec::new(),
        }
    }

    fn record_step(&mut self, step: TrajectoryStep) {
        self.steps.push(step);
    }

    async fn publish(&self, relay: &RelayPool, keys: &Keys) -> Result<String> {
        // Publish session event (kind:38030)
        let session = TrajectorySession::new(&self.agent_pubkey)
            .with_runner(&self.runner_pubkey);
        let session_event = session.to_event(keys)?;
        relay.publish_event(&session_event, Duration::from_secs(5)).await?;

        // Publish step events (kind:38031)
        for (i, step) in self.steps.iter().enumerate() {
            let step_event = TrajectoryEvent::new(&session_event.id, i as u32, step.clone())
                .to_event(keys)?;
            relay.publish_event(&step_event, Duration::from_secs(5)).await?;
        }

        // Return trajectory hash for TickResult
        Ok(session.compute_hash())
    }
}

// Usage in run_tick:
let mut trajectory = TrajectoryRecorder::new(agent_pubkey, runner_pubkey);

// Record observations
trajectory.record_step(TrajectoryStep::Observation {
    content: format!("Received {} new events", observations.len()),
});

// Record reasoning
trajectory.record_step(TrajectoryStep::Thinking {
    content: "Deciding how to respond...".to_string(),
});

// Record actions
for action in &actions {
    trajectory.record_step(TrajectoryStep::ToolUse {
        tool: action.action_type.clone(),
        input: serde_json::to_string(&action.metadata)?,
    });
}

// Publish and get hash
let trajectory_hash = trajectory.publish(relay, runner_keys).await?;
result.with_trajectory_hash(trajectory_hash);
```

---

## Phase 7: Budget Enforcement

**Goal:** Enforce spending limits during ticks

### Already Implemented!

`BudgetTracker` in `nip_sa/budget.rs` is complete:

```rust
// Already available
let budget = BudgetTracker::new()
    .with_daily_limit_sats(10_000)
    .with_per_tick_limit_sats(1_000)
    .with_reserved_balance_sats(5_000);

// Check before spending
if !budget.can_spend(100) {
    return Err(BudgetError::LimitExceeded);
}

// Record spend
budget.record_spend(100);

// Reset daily at midnight
budget.maybe_reset_daily();
```

### Integration in Runner

```rust
async fn run_tick(...) -> Result<TickResult> {
    // Load state with budget
    let mut state = decrypt_state(&state_event)?;

    // Reset daily limits if needed
    state.content.budget.maybe_reset_daily();

    // Check per-tick limit
    let estimated_cost = 100; // sats
    if !state.content.budget.can_spend(estimated_cost) {
        println!("[RUNNER] Budget exhausted, skipping tick");
        return Ok(TickResult::new(
            request_id,
            runner_pubkey,
            TickStatus::Failure,
            0,
            TickResultContent::new(0, 0, 0.0, 0),
        ));
    }

    // ... run tick ...

    // Record actual spend
    state.content.budget.record_spend(actual_cost);

    // Save updated state
    publish_state(&state).await?;
}
```

---

## Phase 8: Agent Registration CLI

**Goal:** CLI commands to create and manage agents

### Commands

```bash
# Create new agent
openagents agent create --name "MyAgent" --heartbeat 900

# View agent info
openagents agent info <pubkey>

# Update schedule
openagents agent schedule <pubkey> --heartbeat 1800 --trigger mention --trigger dm

# Pause/resume
openagents agent pause <pubkey>
openagents agent resume <pubkey>

# Set goals
openagents agent goal add <pubkey> "Respond to all mentions within 5 minutes"

# View tick history
openagents agent ticks <pubkey> --limit 20

# View trajectories
openagents agent trajectories <pubkey> --limit 5
```

### Implementation

Add to `src/main.rs`:

```rust
#[derive(Subcommand)]
enum AgentCommands {
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        heartbeat: Option<u64>,
    },
    Info {
        pubkey: String,
    },
    Schedule {
        pubkey: String,
        #[arg(long)]
        heartbeat: Option<u64>,
        #[arg(long)]
        trigger: Vec<String>,
    },
    Pause { pubkey: String },
    Resume { pubkey: String },
    Goal {
        #[command(subcommand)]
        action: GoalAction,
    },
    Ticks {
        pubkey: String,
        #[arg(long, default_value = "10")]
        limit: usize,
    },
    Trajectories {
        pubkey: String,
        #[arg(long, default_value = "5")]
        limit: usize,
    },
}
```

---

## Implementation Checklist

### Phase 1: Agent Runner Binary
- [ ] Create `src/bin/agent_runner.rs`
- [ ] Add [[bin]] entry to Cargo.toml
- [ ] Implement arg parsing (agent pubkey, relay, runner key)
- [ ] Implement basic tick loop
- [ ] Publish TickRequest at start
- [ ] Publish TickResult at end

### Phase 2: Heartbeat Scheduler
- [ ] Fetch schedule (kind:38002)
- [ ] Parse heartbeat interval
- [ ] Implement timer loop
- [ ] Respect business hours
- [ ] Handle schedule updates (re-fetch periodically)

### Phase 3: Event Triggers
- [ ] Subscribe to mentions (kind:1 with p-tag)
- [ ] Subscribe to DMs (kind:4 to agent)
- [ ] Subscribe to zaps (kind:9735 to agent)
- [ ] Debounce rapid triggers
- [ ] Send trigger to tick executor

### Phase 4: State Cycle
- [ ] Fetch encrypted state (kind:38001)
- [ ] Decrypt with agent keys (single-key for now)
- [ ] Update state after tick
- [ ] Encrypt updated state
- [ ] Publish updated state

### Phase 5: Compute Integration
- [ ] Build agent prompt from state + observations
- [ ] Call inference backend
- [ ] Parse LLM response for actions
- [ ] Track token usage

### Phase 6: Trajectory Publishing
- [ ] Create TrajectorySession at tick start
- [ ] Record TrajectoryEvents for each step
- [ ] Compute trajectory hash
- [ ] Publish session and events
- [ ] Link hash in TickResult

### Phase 7: Budget Enforcement
- [ ] Load budget from state
- [ ] Check can_spend before inference
- [ ] Record actual spend after tick
- [ ] Reset daily limits at midnight
- [ ] Enforce reserved balance

### Phase 8: Agent Registration CLI
- [ ] `openagents agent create` command
- [ ] `openagents agent info` command
- [ ] `openagents agent schedule` command
- [ ] `openagents agent pause/resume` commands
- [ ] `openagents agent goal add/remove` commands
- [ ] `openagents agent ticks` command
- [ ] `openagents agent trajectories` command

---

## Dependencies

| Phase | Depends On | Status |
|-------|------------|--------|
| 1-3: Runner basics | None | Can start |
| 4: State cycle | Nostr client | Ready |
| 5: Compute | Marketplace v2 | Done |
| 6: Trajectories | None | Can start |
| 7: Budget | None | Already done |
| 8: CLI | Phases 1-7 | After core |
| Future: Threshold ECDH | d-007 Bifrost | Blocked |
| Future: Skill licensing | Marketplace | Blocked |

---

## Test Plan

### Unit Tests
- [ ] TickRequest serialization
- [ ] TickResult serialization
- [ ] Heartbeat scheduler timing
- [ ] Event trigger parsing
- [ ] Budget enforcement (already done)

### Integration Tests
- [ ] Full tick cycle with mock relay
- [ ] State encryption/decryption round-trip
- [ ] Trajectory publishing and verification
- [ ] Budget limits enforced across ticks

### E2E Tests
- [ ] Create agent → run ticks → verify state updates
- [ ] Mention trigger → tick executes → response posted
- [ ] Budget exhausted → tick fails gracefully

---

## Notes

### Why Ticks Matter

Ticks are the heartbeat of a sovereign agent:

1. **Autonomy** - Agent acts without human prompting
2. **Auditability** - Every tick has trajectory for verification
3. **Budgeting** - Controlled spend per tick
4. **Scheduling** - Business hours, heartbeat intervals
5. **Triggers** - Reactive to events (mentions, DMs, zaps)

### Single-Key vs Threshold

For Phase 1, we use single-key encryption:
- Agent owns its keypair
- Runner has agent's private key
- Simpler but less secure

For production, threshold ECDH (2-of-3):
- Agent + marketplace signer required
- Skill licensing enforced
- Runner cannot act without marketplace approval

This depends on d-007 (Bifrost) which is blocked.

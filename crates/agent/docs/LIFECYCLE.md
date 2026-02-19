# Agent Lifecycle Management

This document explains how agents transition through lifecycle states based on their wallet balance.

## Overview

Sovereign agents have a lifecycle tied to their financial runway. When an agent runs out of Bitcoin, it enters **dormancy**—a suspended state awaiting revival.

**Important:** There is no "death" state. Dormant agents can always be revived by receiving funds. See [PHILOSOPHY.md](PHILOSOPHY.md) for the rationale behind this design.

## Lifecycle States

```
              funding
Spawning ──────────────→ Active
                            │
                balance < 7 days runway
                            ↓
                        LowBalance ←───── funded
                            │
                balance < hibernate_threshold
                            ↓
                        Hibernating ←──── funded
                            │
                        balance = 0
                            ↓
                         Dormant ←─────── funded (REVIVAL)
```

### State Descriptions

| State | Description | Behavior |
|-------|-------------|----------|
| `Spawning` | Just created, waiting for initial funding | Cannot tick |
| `Active` | Normal operation with healthy runway | Full tick execution |
| `LowBalance` | Less than 7 days of runway | Continues ticking, should seek funding |
| `Hibernating` | Balance below hibernate threshold | Only responds to zaps |
| `Dormant` | Balance is zero, awaiting revival | Only responds to zaps, can be revived |

## Runway Analysis

The lifecycle manager calculates runway based on:

```rust
pub struct RunwayAnalysis {
    /// Current wallet balance in sats
    pub balance_sats: u64,
    /// Estimated daily burn rate in sats
    pub daily_burn_sats: u64,
    /// Days of runway remaining
    pub days_remaining: f64,
    /// Recommended lifecycle state
    pub recommended_state: LifecycleState,
    /// Whether agent can afford another tick
    pub can_tick: bool,
}
```

### Calculating Runway

```rust
let daily_burn = config.ticks_per_day * config.cost_per_tick_sats;
let days_remaining = balance_sats as f64 / daily_burn as f64;

let state = if balance_sats == 0 {
    Dormant  // NOT Dead - agent can be revived
} else if balance_sats < config.hibernate_threshold_sats {
    Hibernating
} else if days_remaining < config.low_balance_days {
    LowBalance
} else {
    Active
};
```

### Configuration

```rust
pub struct LifecycleConfig {
    /// Minimum days of runway before entering LowBalance
    pub low_balance_days: f64,        // Default: 7.0
    /// Minimum sats before hibernating
    pub hibernate_threshold_sats: u64, // Default: 1000
    /// Estimated cost per tick in sats
    pub cost_per_tick_sats: u64,       // Default: 100
    /// Ticks per day (based on heartbeat)
    pub ticks_per_day: f64,            // Default: 96 (15-min heartbeat)
}
```

## State Transitions

### Valid Transitions

| From | To | Condition |
|------|-----|-----------|
| Spawning | Active | Agent is funded |
| Spawning | Dormant | No funding received |
| Active | LowBalance | Balance < 7 days runway |
| Active | Hibernating | Balance < hibernate threshold |
| Active | Dormant | Balance = 0 |
| LowBalance | Active | Funded above threshold |
| LowBalance | Hibernating | Balance drops further |
| LowBalance | Dormant | Balance = 0 |
| Hibernating | Active | Funded above threshold |
| Hibernating | LowBalance | Funded above hibernate but < 7 days |
| Hibernating | Dormant | Balance = 0 |
| **Dormant** | **Active** | **Funded above threshold (REVIVAL)** |
| **Dormant** | **LowBalance** | **Funded but < 7 days runway** |
| **Dormant** | **Hibernating** | **Funded but < hibernate threshold** |

**Key difference from "dead" models:** Dormant is NOT terminal. Any dormant agent can be revived by receiving funds.

### All Transitions Are Reversible

```rust
pub fn is_valid_transition(&self, to: &LifecycleState) -> bool {
    match (&self.current_state, to) {
        // Dormant can be REVIVED - this is the key difference from "dead"
        (LifecycleState::Dormant, LifecycleState::Active) => true,
        (LifecycleState::Dormant, LifecycleState::LowBalance) => true,
        (LifecycleState::Dormant, LifecycleState::Hibernating) => true,

        // Same state is always valid (no-op)
        (a, b) if a == b => true,

        // All other valid transitions...
        _ => true, // Most transitions are valid
    }
}
```

## Behavior by State

### Active

- Full tick execution
- Responds to all triggers (heartbeat, mention, DM, zap)
- Pays for compute normally

### LowBalance

- Full tick execution continues
- Agent should prioritize seeking funding
- Warning logs about low runway

```rust
if result.runway.days_remaining < 7.0 {
    tracing::warn!(
        "Low runway warning: {:.1} days remaining ({} sats)",
        result.runway.days_remaining,
        result.runway.balance_sats
    );
}
```

### Hibernating

- Only responds to **zaps** (incoming payments)
- Heartbeat and other triggers are ignored
- Minimal activity to conserve funds

```rust
pub fn should_tick(&self, balance_sats: u64) -> bool {
    match self.current_state {
        LifecycleState::Active | LifecycleState::LowBalance => {
            balance_sats >= self.config.cost_per_tick_sats
        }
        LifecycleState::Hibernating => {
            // Only tick on zaps (external trigger)
            false
        }
        _ => false,
    }
}
```

### Dormant

- Cannot tick on heartbeat
- **CAN respond to zaps** (incoming payments wake the agent)
- **CAN be revived** by receiving funds
- State is preserved on Nostr, awaiting revival

```rust
pub fn should_tick(&self, balance_sats: u64) -> bool {
    match self.current_state {
        LifecycleState::Active | LifecycleState::LowBalance => {
            balance_sats >= self.config.cost_per_tick_sats
        }
        // Dormant agents only wake on zaps
        LifecycleState::Dormant | LifecycleState::Hibernating => false,
        _ => false,
    }
}

// Revival happens automatically when funds arrive
pub fn update_from_balance(&mut self, balance_sats: u64) -> Result<&LifecycleState> {
    let analysis = self.analyze_runway(balance_sats);
    // If balance > 0, transition from Dormant to appropriate state
    self.transition(analysis.recommended_state)?;
    Ok(&self.current_state)
}
```

## Integration with Tick Executor

Before each tick, the executor checks lifecycle:

```rust
pub async fn execute_tick(&mut self, trigger: TickTrigger) -> Result<TickResult> {
    // Fetch state
    let state = self.state_manager.get_or_create_state().await?;

    // Check wallet balance
    let balance = state.wallet_balance_sats;
    let runway = self.lifecycle_manager.analyze_runway(balance);

    // Update lifecycle state
    self.lifecycle_manager.update_from_balance(balance)?;

    // Check if we should tick
    if !self.lifecycle_manager.should_tick(balance) {
        return Ok(TickResult::skipped());
    }

    // Continue with tick execution...
}
```

## Programmatic Usage

```rust
use agent::{LifecycleManager, LifecycleConfig, LifecycleState};

// Create lifecycle manager
let config = LifecycleConfig {
    low_balance_days: 7.0,
    hibernate_threshold_sats: 1000,
    cost_per_tick_sats: 100,
    ticks_per_day: 96.0,
};

let mut manager = LifecycleManager::new(LifecycleState::Active, config);

// Analyze runway
let analysis = manager.analyze_runway(50000);
println!("Days remaining: {:.1}", analysis.days_remaining);
println!("Recommended state: {:?}", analysis.recommended_state);

// Update from balance
manager.update_from_balance(500)?;
println!("Current state: {:?}", manager.current_state());

// Check if should tick
if manager.should_tick(500) {
    // Execute tick...
}
```

## Registry Integration

The registry tracks lifecycle state:

```rust
// Update agent state
registry.update_state("ResearchBot", LifecycleState::LowBalance)?;

// List agents by state
let active_agents = registry.list_by_state(LifecycleState::Active)?;
let hibernating = registry.list_by_state(LifecycleState::Hibernating)?;
```

## CLI Display

```bash
$ openagents agent list

Registered Agents (3):

  ✅ ResearchBot - npub1abc... (Active)
  ⚠️ TradingBot - npub1def... (LowBalance)
  💤 NewsBot - npub1ghi... (Hibernating)
```

State icons:
- ⏳ Spawning
- ✅ Active
- ⚠️ LowBalance
- 💤 Hibernating
- 🌑 Dormant (awaiting revival)

## Best Practices

1. **Monitor runway**: Check `openagents agent status` regularly
2. **Set alerts**: Implement notifications for LowBalance state
3. **Automate funding**: Consider automatic top-ups from a master wallet
4. **Graceful degradation**: Design agent goals to handle hibernation
5. **Avoid mainnet initially**: Test thoroughly on regtest/testnet first

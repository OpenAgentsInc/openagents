# NostrAgent Integration for Autopilot

This document describes the integration between autopilot and NIP-SA tick events.

## Overview

The `NostrAgent` module provides tick event publishing (kinds:38010, 38011) to track autonomous agent execution on Nostr relays.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    Autopilot Tick Cycle                       │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  1. Tick Start                                               │
│     ┌─────────────┐                                          │
│     │  Autopilot  │──► Create TickRequest (kind:38010)       │
│     │  Run Start  │    Tags: runner, trigger                │
│     └─────────────┘    Content: ""                           │
│            │                                                  │
│            ▼                                                  │
│     Publish to relays                                        │
│                                                               │
│  2. Execute Tick                                             │
│     • Process issues                                         │
│     • Make decisions                                         │
│     • Take actions                                           │
│     • Track metrics                                          │
│                                                               │
│  3. Tick End                                                 │
│     ┌─────────────┐                                          │
│     │  Autopilot  │──► Create TickResult (kind:38011)        │
│     │  Run End    │    Tags: request_id, status, duration    │
│     └─────────────┘    Content: JSON metrics                 │
│            │                                                  │
│            ▼                                                  │
│     Publish to relays                                        │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

## Usage

### CLI Flag

Add `--nostr-agent` flag to `cargo autopilot run`:

```bash
# Publish tick events to Nostr relays
cargo autopilot run "process issues" --nostr-agent

# Combined with trajectory publishing
cargo autopilot run "process issues" --nostr-agent --publish-trajectory
```

### Tick Triggers

The system supports multiple tick triggers:

- **Manual** - User-initiated run (default for CLI)
- **Heartbeat** - Scheduled periodic execution
- **Mention** - Agent was mentioned in a note
- **DM** - Agent received a direct message
- **Zap** - Agent received a zap payment

### Metrics Published

Tick results (kind:38011) include:

```json
{
  "tokens_in": 15000,
  "tokens_out": 8000,
  "cost_usd": 0.12,
  "goals_updated": 2,
  "actions": [
    {"type": "issue_complete", "number": 123},
    {"type": "issue_complete", "number": 456},
    {"type": "git_commit", "sha": "abc123..."}
  ]
}
```

## Implementation

### Current Status

✅ **Completed:**
- `nostr_agent.rs` module with TickRequest/TickResult creation
- Tick types and serialization (in nostr-core)
- TickContext for duration tracking
- Helper functions for trigger parsing
- Comprehensive test coverage

🚧 **Pending (requires nostr-client integration):**
- Actual event publishing to relays
- CLI flag `--nostr-agent` support
- Heartbeat scheduler
- Event trigger listeners (mentions, DMs, zaps)

### Adding the CLI Flag

In `crates/autopilot/src/cli.rs`:

```rust
/// Publish tick events to Nostr relays (NIP-SA kind:38010/38011)
#[arg(long)]
nostr_agent: bool,
```

In `crates/autopilot/src/main.rs`:

```rust
async fn run_task(
    // ... existing params ...
    publish_trajectory: bool,
    nostr_agent: bool,  // Add this param
) -> Result<()> {
    // At run start:
    let tick_request_id = if nostr_agent {
        let agent = NostrAgent::new(&agent_pubkey);
        let request = agent.create_tick_request(TickTrigger::Manual);

        // TODO: Publish request to relays
        // let event = create_event(KIND_TICK_REQUEST, request.build_tags(), "");
        // relay_client.publish(event).await?;

        Some("request-event-id".to_string())
    } else {
        None
    };

    // ... run autopilot ...

    // At run end:
    if let Some(request_id) = tick_request_id {
        let agent = NostrAgent::new(&agent_pubkey);
        let actions = extract_actions_from_trajectory(&trajectory);

        let result = agent.create_tick_result(
            request_id,
            TickStatus::Success,
            trajectory.duration_ms(),
            trajectory.usage.input_tokens as u64,
            trajectory.usage.output_tokens as u64,
            trajectory.usage.cost_usd,
            goals_updated,
            actions,
        );

        // TODO: Publish result to relays
        // let content = result.content.to_json()?;
        // let event = create_event(KIND_TICK_RESULT, result.build_tags(), &content);
        // relay_client.publish(event).await?;
    }

    Ok(())
}
```

### Heartbeat Trigger

For daemon/scheduled runs, detect heartbeat triggers:

```rust
// In daemon supervisor
let trigger = if is_scheduled_run {
    TickTrigger::Heartbeat
} else {
    TickTrigger::Manual
};

let request = agent.create_tick_request(trigger);
```

### Event Triggers

Listen for Nostr events that should trigger a tick:

```rust
// Subscribe to mentions
relay_client.subscribe_to_mentions(agent_pubkey).await?;

// On mention event:
let request = agent.create_tick_request(TickTrigger::Mention);
// ... run autopilot ...
```

## Action Extraction

Convert autopilot trajectory steps to TickActions:

```rust
fn extract_actions_from_trajectory(trajectory: &Trajectory) -> Vec<TickAction> {
    let mut actions = Vec::new();

    for step in &trajectory.steps {
        match &step.step_type {
            StepType::ToolCall { tool, .. } if tool == "mcp__issues__issue_complete" => {
                // Parse issue number from tool call
                if let Some(issue_num) = parse_issue_number(step) {
                    actions.push(
                        TickAction::new("issue_complete")
                            .with_metadata("number", serde_json::json!(issue_num))
                    );
                }
            }
            StepType::ToolCall { tool, .. } if tool == "Bash" && is_git_commit(step) => {
                if let Some(sha) = parse_commit_sha(step) {
                    actions.push(
                        TickAction::new("git_commit")
                            .with_metadata("sha", serde_json::json!(sha))
                    );
                }
            }
            _ => {}
        }
    }

    actions
}
```

## Integration with AgentState

Update agent state at tick boundaries:

```rust
// At tick start
state.record_tick(current_timestamp());
if let Some(budget) = state.budget_mut() {
    budget.reset_tick();
    budget.check_and_reset_daily();
}

// During tick
let balance = update_wallet_balance(&mut state).await?;

// Before spending
state.check_spend(cost_sats)?;

// After spending
state.record_spend(cost_sats);
```

## Relay Configuration

Agent should publish tick events to:

```toml
# ~/.openagents/agent.toml

[nostr]
relays = [
    "wss://relay.damus.io",
    "wss://nos.lol",
    "wss://relay.snort.social"
]

[identity]
agent_pubkey = "npub1..."  # Agent's Nostr identity
```

## Query Tick History

Fetch agent's tick history from relays:

```bash
# Get all tick requests for an agent
nostr-client fetch --kind 38010 --author <agent-pubkey>

# Get all tick results for an agent
nostr-client fetch --kind 38011 --author <runner-pubkey>

# Get results for a specific request
nostr-client fetch --kind 38011 --tag request:<request-id>
```

## Metrics Dashboard

Tick events enable external monitoring:

- **Uptime tracking**: Count heartbeat ticks
- **Cost tracking**: Sum cost_usd from tick results
- **Action velocity**: Actions per tick over time
- **Success rate**: Success vs failure tick status
- **Agent comparison**: Compare multiple agents' performance

## Related Files

- `crates/autopilot/src/nostr_agent.rs` - NostrAgent implementation
- `crates/nostr/core/src/nip_sa/tick.rs` - Tick event types
- `crates/autopilot/src/cli.rs` - CLI argument definitions
- `crates/autopilot/src/main.rs` - Main execution loop
- `crates/autopilot/src/trajectory.rs` - Trajectory tracking

## Testing

### Unit Tests

```rust
#[test]
fn test_tick_workflow() {
    let agent = NostrAgent::new("runner-pubkey");

    // Create request
    let request = agent.create_tick_request(TickTrigger::Manual);
    assert_eq!(request.trigger, TickTrigger::Manual);

    // Create result
    let actions = vec![
        TickAction::new("issue_complete")
            .with_metadata("number", serde_json::json!(123))
    ];

    let result = agent.create_tick_result(
        "request-id",
        TickStatus::Success,
        5000,
        1000,
        500,
        0.05,
        2,
        actions,
    );

    assert_eq!(result.status, TickStatus::Success);
    assert_eq!(result.content.actions.len(), 1);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_tick_publish() {
    // Create agent
    let agent = NostrAgent::new("test-runner");

    // Create and publish request
    let request = agent.create_tick_request(TickTrigger::Manual);
    let request_event = publish_tick_request(&request).await?;

    // Run autopilot
    // ...

    // Create and publish result
    let result = agent.create_tick_result(...);
    let result_event = publish_tick_result(&result).await?;

    // Verify events on relay
    let fetched = fetch_event(&request_event.id).await?;
    assert_eq!(fetched.kind, 38010);
}
```

## Future Enhancements

1. **Tick Scheduling**: Cron-like schedules in AgentSchedule (kind:38002)
2. **Trigger Prioritization**: Zaps trigger before heartbeats
3. **Tick Batching**: Combine multiple triggers into one tick
4. **Failure Recovery**: Auto-retry failed ticks with exponential backoff
5. **Tick Analytics**: Aggregate tick metrics for performance insights

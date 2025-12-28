# Autopilot + Agent-Orchestrator Integration Analysis

**Date:** December 24, 2025  
**Status:** Analysis Complete  
**Next Steps:** Implementation Ready

## Executive Summary

The `autopilot` and `agent-orchestrator` crates have complementary capabilities that can be unified to create a powerful sovereign agent execution framework. This document analyzes the integration points, gaps, and provides a concrete implementation roadmap.

## Current State

### Autopilot Capabilities
| Capability | Status | Notes |
|------------|--------|-------|
| Trajectory logging | ✅ Complete | JSON + rlog streaming |
| APM tracking | ✅ Complete | Real-time + historical |
| Issue management | ✅ Complete | Via `issues` crate |
| Claude SDK integration | ✅ Complete | Full message processing |
| Codex SDK integration | ✅ Complete | ThreadEvent processing |
| NIP-SA trajectory publishing | ✅ Complete | kinds 39230/39231 |
| Daemon supervision | ✅ Complete | Crash recovery, memory management |
| Cost tracking | 🟡 Partial | Basic via SDK, not unified |

### Agent-Orchestrator Capabilities
| Capability | Status | Notes |
|------------|--------|-------|
| Agent registry | ✅ Complete | 7 builtin agents |
| Hook system | ✅ Complete | Session, tool, context, todo hooks |
| Background tasks | ✅ Complete | Parallel session management |
| AutopilotIntegration | ✅ Complete | Issue claim/complete workflow |
| Multi-backend routing | ✅ Complete | Claude, OpenAI, Codex, GPT-OSS, Local |
| Cost tracking | ✅ Complete | Budget enforcement per agent |
| FROSTR bridge | ✅ Complete | Real threshold keygen |
| Spark bridge | ✅ Complete | PaymentProvider trait |
| NIP-SA identity | ✅ Complete | AgentIdentity with autonomy levels |

## Integration Architecture

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                              Unified Autopilot                                  │
├────────────────────────────────────────────────────────────────────────────────┤
│                                                                                 │
│  ┌─────────────────────────────────────────────────────────────────────────┐   │
│  │                        agent-orchestrator                                │   │
│  │                                                                          │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │   │
│  │  │AgentRegistry │  │ HookManager  │  │  Background  │                  │   │
│  │  │ (7 agents)   │  │ (lifecycle)  │  │   Tasks      │                  │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                  │   │
│  │                                                                          │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │   │
│  │  │ CostTracker  │  │MultiBackend  │  │  FROSTR/     │                  │   │
│  │  │   (budget)   │  │   Router     │  │   Spark      │                  │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────────┘   │
│                                    ▲                                            │
│                                    │                                            │
│  ┌─────────────────────────────────┴───────────────────────────────────────┐   │
│  │                           autopilot                                      │   │
│  │                                                                          │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │   │
│  │  │ Trajectory   │  │    APM       │  │   Daemon     │                  │   │
│  │  │  Collector   │  │  Tracking    │  │  Supervisor  │                  │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                  │   │
│  │                                                                          │   │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │   │
│  │  │   Claude     │  │    Codex     │  │   NIP-SA     │                  │   │
│  │  │     SDK      │  │     SDK      │  │  Publisher   │                  │   │
│  │  └──────────────┘  └──────────────┘  └──────────────┘                  │   │
│  └─────────────────────────────────────────────────────────────────────────┘   │
│                                                                                 │
└────────────────────────────────────────────────────────────────────────────────┘
```

## Integration Points

### 1. Issue Store Adapter

**Current State:** `agent-orchestrator` defines `IssueStore` trait, `autopilot` uses `issues` crate directly.

**Integration:**
```rust
// crates/autopilot/src/orchestrator_bridge.rs
use agent_orchestrator::{IssueStore, IssueInfo, IssueStatus};
use issues::{db, issue};

/// Adapts the `issues` crate to agent-orchestrator's IssueStore trait
pub struct IssuesDbStore {
    conn: rusqlite::Connection,
}

impl IssueStore for IssuesDbStore {
    fn claim_issue(&self, issue_id: &str, run_id: &str) -> Result<bool, String> {
        let number: i32 = issue_id.parse().map_err(|e| format!("{}", e))?;
        issue::claim_issue(&self.conn, number, run_id)
            .map(|_| true)
            .map_err(|e| e.to_string())
    }
    
    fn complete_issue(&self, issue_id: &str) -> Result<bool, String> {
        let number: i32 = issue_id.parse().map_err(|e| format!("{}", e))?;
        issue::complete_issue(&self.conn, number)
            .map(|_| true)
            .map_err(|e| e.to_string())
    }
    // ... other methods
}
```

### 2. Unified Cost Tracking

**Current State:** Autopilot tracks cost via SDK usage metrics, orchestrator has `CostTracker` with budget enforcement.

**Integration:**
```rust
// Bridge TrajectoryCollector cost tracking to CostTracker
impl TrajectoryCollector {
    pub fn with_cost_tracker(mut self, tracker: Arc<CostTracker>) -> Self {
        self.cost_tracker = Some(tracker);
        self
    }
    
    fn record_cost(&self, agent: &str, input: u64, output: u64) {
        if let Some(tracker) = &self.cost_tracker {
            let backend = self.router.get_backend(agent).unwrap();
            let record = CostRecord {
                agent_name: agent.to_string(),
                backend: backend.provider,
                input_tokens: input,
                output_tokens: output,
                cost_sats: backend.calculate_cost(input, output),
                timestamp: now(),
            };
            tracker.record(record).ok();
        }
    }
}
```

### 3. Hook Integration

**Current State:** Autopilot has `on_session_id` callback, orchestrator has full hook system.

**Integration:**
```rust
// Use orchestrator hooks in autopilot main loop
pub async fn run_with_hooks(
    prompt: &str,
    hooks: &HookManager,
    collector: &mut TrajectoryCollector,
) -> Result<()> {
    // Inject context via hooks
    let mut ctx = ContextBuilder::new();
    hooks.inject_context(&mut ctx).await;
    
    // Session start hook
    hooks.on_session(&SessionEvent::Created { 
        session_id: collector.trajectory().session_id.clone() 
    }).await;
    
    // Tool execution with hooks
    for tool_call in pending_tools {
        let result = hooks.before_tool(&mut tool_call).await;
        if matches!(result, HookResult::Block { .. }) {
            continue;
        }
        
        let output = execute_tool(tool_call).await;
        hooks.after_tool(&tool_call, &mut output).await;
    }
}
```

### 4. Agent Identity + Trajectory

**Current State:** Autopilot's `TrajectoryPublisher` and orchestrator's `AgentIdentity` are separate.

**Integration:**
```rust
// Unified agent with identity and trajectory
pub struct SovereignAgent {
    identity: AgentIdentity,
    collector: TrajectoryCollector,
    publisher: TrajectoryPublisher,
    coordinator: SolverAgentCoordinator,
}

impl SovereignAgent {
    pub fn new(identity: AgentIdentity) -> Self {
        let session_id = generate_session_id();
        let tick_id = format!("tick-{}", uuid::Uuid::new_v4());
        
        Self {
            identity: identity.clone(),
            collector: TrajectoryCollector::new(
                String::new(), // prompt set later
                identity.model.clone(),
                std::env::current_dir().unwrap().display().to_string(),
                get_repo_sha(),
                get_branch(),
            ),
            publisher: TrajectoryPublisher::new(&session_id, &tick_id),
            coordinator: SolverAgentCoordinator::new(identity),
        }
    }
    
    /// Execute with approval workflow for supervised agents
    pub async fn execute_with_approval(&self, action: &str, cost: u64) -> Result<()> {
        if self.coordinator.requires_approval(action, cost) {
            let request_id = self.coordinator.request_approval(action, "...", cost);
            // Wait for operator approval via Nostr event
            self.wait_for_approval(&request_id).await?;
        }
        // Execute action
        Ok(())
    }
}
```

### 5. Payment Integration

**Current State:** Orchestrator has `PaymentProvider` trait and `SparkPaymentProvider`, autopilot has no payment.

**Integration:**
```rust
// Pay for compute on issue completion
pub async fn complete_issue_with_payment(
    issue: &IssueInfo,
    payment_provider: &dyn PaymentProvider,
    bounty_sats: u64,
) -> Result<()> {
    // Complete the issue
    store.complete_issue(&issue.id)?;
    
    // Pay bounty if configured
    if bounty_sats > 0 {
        let invoice = create_invoice(bounty_sats)?;
        payment_provider.pay_invoice(&invoice).await?;
    }
    
    Ok(())
}
```

## Implementation Phases

### Phase 1: Dependency Wiring (1 day)
- [ ] Add `agent-orchestrator` as dependency in `autopilot/Cargo.toml`
- [ ] Create `IssuesDbStore` adapter
- [ ] Wire `AutopilotIntegration` into main.rs

### Phase 2: Hook Integration (2 days)
- [ ] Create `HookManager` in autopilot initialization
- [ ] Add `ContextInjectionHook` for AGENTS.md, directives
- [ ] Add `CostTrackingHook` for budget enforcement
- [ ] Add `TodoHook` for task completion enforcement

### Phase 3: Unified Cost Tracking (1 day)
- [ ] Bridge `TrajectoryCollector` to `CostTracker`
- [ ] Add budget checks before agent execution
- [ ] Display cost warnings in CLI output

### Phase 4: Agent Identity (2 days)
- [ ] Create `SovereignAgent` struct combining identity + trajectory
- [ ] Integrate FROSTR for threshold-protected identity
- [ ] Add approval workflow for supervised agents

### Phase 5: Payment Integration (2 days)
- [ ] Wire `SparkPaymentProvider` to autopilot
- [ ] Add issue bounty payment on completion
- [ ] Track payment history in trajectory

### Phase 6: NIP-SA Publishing (1 day)
- [ ] Publish trajectory events to Nostr relays
- [ ] Sign events with agent identity
- [ ] Add trajectory verification endpoint

## Code Changes Summary

| File | Change |
|------|--------|
| `crates/autopilot/Cargo.toml` | Add `agent-orchestrator` dependency |
| `crates/autopilot/src/orchestrator_bridge.rs` | New: IssuesDbStore adapter |
| `crates/autopilot/src/lib.rs` | Export orchestrator bridge |
| `crates/autopilot/src/main.rs` | Wire hooks, cost tracking |
| `crates/autopilot/src/cli.rs` | Add `--budget` flag using orchestrator |
| `crates/agent-orchestrator/src/integrations/autopilot.rs` | Add `IssuesDbStore` support |

## Benefits

1. **Unified Cost Tracking** - Single source of truth for agent costs across all backends
2. **Budget Enforcement** - Prevent runaway costs with session/daily limits
3. **Sovereign Identity** - Agents own their keys via FROSTR
4. **Transparent Trajectories** - All work published to Nostr for verification
5. **Payment Integration** - Automatic bounty payment on issue completion
6. **Hook Extensibility** - Custom behavior injection without modifying core code

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing autopilot workflows | Feature-flag new integration, gradual rollout |
| FROSTR key management complexity | Start with single-key mode, threshold optional |
| Nostr relay reliability | Local trajectory persistence as primary, Nostr as secondary |
| Payment integration security | Require operator approval for payments > threshold |

## Dependencies

```toml
# crates/autopilot/Cargo.toml additions
[dependencies]
agent-orchestrator = { path = "../agent-orchestrator", features = ["full"] }

[features]
orchestrator = ["agent-orchestrator"]
sovereign = ["orchestrator", "agent-orchestrator/frostr", "agent-orchestrator/spark"]
```

## Conclusion

The integration is well-defined with clear boundaries. Agent-orchestrator provides the control plane (hooks, budget, identity), while autopilot provides the execution plane (trajectory, SDK integration, daemon). Unifying them creates a complete sovereign agent framework.

**Recommended Start:** Phase 1 (dependency wiring) + Phase 2 (hook integration) provide immediate value with minimal risk.

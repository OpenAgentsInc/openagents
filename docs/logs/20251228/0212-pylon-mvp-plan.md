# Plan: Pylon MVP - Unified Local Runtime

## Overview

Build the Pylon MVP: a single self-daemonizing binary that runs both **Host Mode** (your agents) and **Provider Mode** (earn sats) with SQLite persistence.

**User Requirements:**
- Both modes (host + provider)
- Self-daemonizing (`pylon start -d` backgrounds, `pylon stop` kills)
- All agent actions (post, DM, zap)

---

## Current State

| Component | Status | Location |
|-----------|--------|----------|
| Provider mode | 70% (foreground only) | `crates/pylon/src/` |
| Agent spawning | 95% | `crates/agent/src/` |
| Tick execution | 95% (actions stubbed) | `src/agents/runner/` |
| Compute client | 100% | `src/agents/runner/compute.rs` |
| Agent runner | 90% (not integrated) | `src/bin/agent_runner.rs` |
| Daemon mode | 0% | Missing |
| SQLite persistence | 0% | Missing |

---

## Phase 1: Self-Daemonizing Infrastructure

### Files to Create

| File | Purpose |
|------|---------|
| `crates/pylon/src/daemon/mod.rs` | Module exports |
| `crates/pylon/src/daemon/pid.rs` | PID file at `~/.pylon/pylon.pid` |
| `crates/pylon/src/daemon/process.rs` | fork()/setsid() daemonization |
| `crates/pylon/src/daemon/control.rs` | Unix socket IPC at `~/.pylon/control.sock` |

### Files to Modify

**`crates/pylon/src/cli/start.rs`** (line 106-125)
- Replace TODO with actual daemonization
- Add `-d/--daemon` flag (default behavior)
- Add `--mode` flag: "host", "provider", "both"
- Write PID file after fork
- Open control socket

**`crates/pylon/src/cli/stop.rs`** (currently stub)
- Read PID from file
- Send SIGTERM for graceful shutdown
- Wait with timeout, fallback to SIGKILL
- Remove PID file

**`crates/pylon/Cargo.toml`**
```toml
nix = { version = "0.27", features = ["signal", "process"] }
```

---

## Phase 2: Complete Agent Actions

### Files to Modify

**`src/agents/runner/tick.rs`** - Replace TODOs:

Line 528 - Post action:
```rust
TickAction::Post { content } => {
    let template = EventTemplate {
        kind: 1,
        content: content.clone(),
        tags: vec![],
        created_at: now(),
    };
    let event = finalize_event(&template, identity.private_key_bytes())?;
    self.relay.publish_event(&event, Duration::from_secs(10)).await?;
}
```

Line 537 - DM action (NIP-04):
```rust
TickAction::DirectMessage { recipient, content } => {
    let encrypted = nostr::nip04::encrypt(
        identity.private_key_bytes(),
        &recipient_pubkey,
        content,
    )?;
    // Build kind:4 event with p-tag
}
```

Line 546 - Zap action (NIP-57):
```rust
TickAction::Zap { target, amount_sats } => {
    let zap_request = build_zap_request(...)?;
    let invoice = request_zap_invoice(&lnurl, &zap_request, amount_sats * 1000).await?;
    self.wallet.send_payment_simple(&invoice, None).await?;
}
```

### Files to Create

| File | Purpose |
|------|---------|
| `src/agents/runner/actions.rs` | Helper functions for zap flow |

---

## Phase 3: SQLite Persistence

### Files to Create

| File | Purpose |
|------|---------|
| `crates/pylon/src/db/mod.rs` | Database connection, migrations |
| `crates/pylon/src/db/jobs.rs` | Job CRUD operations |
| `crates/pylon/src/db/earnings.rs` | Earnings tracking |
| `crates/pylon/src/db/agents.rs` | Agent state persistence |

### Schema

```sql
-- Jobs (provider mode)
CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    kind INTEGER NOT NULL,
    customer_pubkey TEXT NOT NULL,
    status TEXT NOT NULL,
    price_msats INTEGER NOT NULL,
    started_at INTEGER NOT NULL,
    completed_at INTEGER
);

-- Earnings
CREATE TABLE earnings (
    id TEXT PRIMARY KEY,
    job_id TEXT REFERENCES jobs(id),
    amount_msats INTEGER NOT NULL,
    earned_at INTEGER NOT NULL
);

-- Agent state (host mode)
CREATE TABLE agent_state (
    agent_npub TEXT PRIMARY KEY,
    lifecycle_state TEXT NOT NULL,
    balance_sats INTEGER NOT NULL,
    tick_count INTEGER NOT NULL,
    last_tick_at INTEGER
);
```

### Files to Modify

**`crates/pylon/Cargo.toml`**
```toml
rusqlite = { version = "0.31", features = ["bundled"] }
```

**`crates/pylon/src/provider.rs`**
- Inject `PylonDb` for job/earnings persistence
- Record job on receive, update on complete
- Record earning on payment confirmation

---

## Phase 4: Unified CLI & Both Modes

### Files to Modify

**`crates/pylon/src/cli/start.rs`**
```rust
async fn run_both_modes(args: StartArgs) -> Result<()> {
    let provider_handle = tokio::spawn(run_provider_loop());
    let host_handle = tokio::spawn(run_host_loop());

    tokio::select! {
        _ = provider_handle => { ... }
        _ = host_handle => { ... }
        _ = tokio::signal::ctrl_c() => { ... }
    }
}

async fn run_host_loop() -> Result<()> {
    let registry = AgentRegistry::new()?;
    let agents = registry.list_by_state(LifecycleState::Active)?;

    for agent in agents {
        tokio::spawn(run_agent(&agent));
    }
}
```

**`crates/pylon/src/cli/mod.rs`**
- Add `Agent` subcommand with spawn/list/status/fund/delete
- Add `Earnings` command for provider stats

### Files to Create

| File | Purpose |
|------|---------|
| `crates/pylon/src/cli/earnings.rs` | Show earnings breakdown |
| `crates/pylon/src/cli/agent.rs` | Agent management commands |

---

## Phase 5: Testing & Polish

### Files to Create

| File | Purpose |
|------|---------|
| `crates/pylon/tests/daemon.rs` | Daemon start/stop tests |
| `crates/pylon/tests/integration.rs` | Full workflow tests |

### Manual Test Checklist

- [ ] `pylon init` creates identity
- [ ] `pylon start -d` backgrounds and writes PID
- [ ] `pylon status` shows running daemon
- [ ] `pylon stop` terminates cleanly
- [ ] `pylon agent spawn --name test` creates agent
- [ ] Agent tick posts to Nostr
- [ ] Provider processes NIP-90 jobs
- [ ] `pylon earnings` shows stats
- [ ] Both modes run simultaneously

---

## Implementation Order

```
Phase 1 (Daemon)────────────────────────┐
                                        │
Phase 2 (Actions)───────────────────────┼──> Phase 4 (Unified CLI)
                                        │          │
Phase 3 (SQLite)────────────────────────┘          v
                                            Phase 5 (Testing)
```

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| 1. Daemon | Medium | None |
| 2. Actions | Low | None |
| 3. SQLite | Medium | None |
| 4. Unified CLI | Medium | Phases 1-3 |
| 5. Testing | Low | Phase 4 |

---

## Key Files Summary

| File | Changes |
|------|---------|
| `crates/pylon/src/cli/start.rs` | Daemonization, mode selection |
| `crates/pylon/src/cli/stop.rs` | Actual stop logic |
| `src/agents/runner/tick.rs:528,537,546` | Complete action TODOs |
| `crates/pylon/src/daemon/*` | New daemon infrastructure |
| `crates/pylon/src/db/*` | New SQLite persistence |
| `crates/pylon/src/cli/agent.rs` | Agent subcommands |

---

## Success Criteria

1. `pylon start -d --mode both` runs in background
2. `pylon stop` cleanly terminates
3. Agents execute ticks and post to Nostr
4. Provider earns sats from NIP-90 jobs
5. `pylon earnings` shows persistent stats
6. `pylon status` shows both modes running

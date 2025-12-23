# Plan: Unify All OpenAgents Binaries into Single `openagents` Binary

## Decisions Made
- **Daemon**: Becomes `openagents daemon` subcommand (not separate binary)
- **GitAfter**: Unified into single binary
- **Binary name**: `openagents`

## Current State (7 Binaries to Eliminate)

| Crate | Binary Name | Purpose |
|-------|-------------|---------|
| `crates/wallet/` | `wallet` | Nostr identity + Bitcoin payments (CLI + GUI) |
| `crates/marketplace/` | `marketplace` | Compute/skills/data marketplace (CLI + GUI stub) |
| `crates/autopilot/` | `autopilot` | Autonomous task runner (CLI) |
| `crates/autopilot/` | `autopilotd` | Autopilot daemon/supervisor |
| `crates/autopilot-gui/` | `autopilot-gui` | Visual autopilot interface |
| `crates/desktop/` | `openagents-desktop` | Desktop shell (currently minimal) |
| `crates/gitafter/` | `gitafter` | Git collaboration (NIP-34) |

## Goal

Create ONE unified `openagents` binary that:
1. Launches GUI by default (tabbed view: Wallet, Marketplace, Autopilot, GitAfter)
2. Provides CLI subcommands for all functionality
3. Uses single wry/tao window with Actix server

## Architecture

**Key Decision**: Create NEW top-level `src/` at workspace root (not modify crates/desktop).

```
openagents/                      # Workspace root
├── Cargo.toml                   # Add [[bin]] name = "openagents"
├── src/                         # NEW - Unified binary source
│   ├── main.rs                  # Entry point (CLI + GUI)
│   ├── cli/                     # CLI subcommands
│   │   ├── mod.rs
│   │   ├── wallet.rs            # openagents wallet ...
│   │   ├── marketplace.rs       # openagents marketplace ...
│   │   ├── autopilot.rs         # openagents autopilot ...
│   │   ├── gitafter.rs          # openagents gitafter ...
│   │   └── daemon.rs            # openagents daemon ...
│   │
│   └── gui/                     # Unified GUI
│       ├── mod.rs               # GUI entry point
│       ├── app.rs               # Window creation (wry/tao)
│       ├── server.rs            # Unified Actix server
│       ├── state.rs             # UnifiedAppState
│       ├── ws.rs                # Single WsBroadcaster
│       ├── routes/              # Route mounting
│       │   ├── mod.rs           # /wallet/*, /marketplace/*, /autopilot/*, /git/*
│       │   ├── wallet.rs
│       │   ├── marketplace.rs
│       │   ├── autopilot.rs
│       │   ├── gitafter.rs
│       │   └── daemon.rs
│       └── views/               # Maud templates
│           ├── mod.rs
│           └── layout.rs        # Tabbed layout with navigation
│
├── crates/                      # Existing crates become LIBRARIES ONLY
│   ├── wallet/                  # Remove [[bin]], keep [lib]
│   ├── marketplace/             # Remove [[bin]], keep [lib]
│   ├── autopilot/               # Remove both [[bin]], keep [lib]
│   ├── autopilot-gui/           # Remove [[bin]], export views
│   ├── desktop/                 # DEPRECATED (functionality merged)
│   └── gitafter/                # Remove [[bin]], keep [lib]
```

## Unified State

```rust
pub struct UnifiedAppState {
    pub identity: Option<Arc<UnifiedIdentity>>,  // Shared identity from wallet
    pub broadcaster: Arc<WsBroadcaster>,          // Single WS broadcaster
    pub nostr_client: Arc<NostrClient>,           // Shared relay connections
    pub active_tab: RwLock<Tab>,                  // UI state
    pub daemon: Option<Arc<RwLock<DaemonState>>>, // Daemon status
}
```

## CLI Commands

```bash
openagents                              # Launch GUI (default)
openagents gui                          # Launch GUI (explicit)

openagents wallet init|import|whoami|balance|send|receive|...
openagents marketplace compute|skills|data|trajectories ...
openagents autopilot run|metrics|analyze|issue|benchmark ...
openagents gitafter repo|issues|patches|...
openagents daemon start|stop|status|restart-worker
```

## Implementation Phases

### Phase 1: Prepare Crates as Libraries (Non-breaking)
- [ ] Add `[lib]` sections to all crates if missing
- [ ] Add `pub use` exports for key types in each lib.rs
- [ ] Ensure `cli`, `gui`, `core` modules are public
- [ ] Consolidate WsBroadcaster to `crates/ui/` or shared location

### Phase 2: Create Unified Binary (Additive)
- [ ] Create `src/` directory at workspace root
- [ ] Create `src/main.rs` with clap CLI structure
- [ ] Create `src/gui/` module with app.rs, server.rs, state.rs, ws.rs
- [ ] Create `src/cli/` module with wallet.rs, marketplace.rs, autopilot.rs, gitafter.rs, daemon.rs
- [ ] Add to workspace Cargo.toml: `[[bin]] name = "openagents"`
- [ ] Wire route mounting: `/wallet/*`, `/marketplace/*`, `/autopilot/*`, `/git/*`, `/daemon/*`
- [ ] Create tabbed layout in `src/gui/views/layout.rs`

### Phase 3: Test Parallel Running
- [ ] Build and test: `cargo build --bin openagents`
- [ ] Test CLI: `./target/debug/openagents wallet whoami`
- [ ] Test GUI: `./target/debug/openagents` (default)
- [ ] Verify all routes work
- [ ] Verify all CLI commands work

### Phase 4: Remove Old Binaries (Breaking)
- [ ] Remove `[[bin]]` from `crates/wallet/Cargo.toml`
- [ ] Remove `[[bin]]` from `crates/marketplace/Cargo.toml`
- [ ] Remove both `[[bin]]` from `crates/autopilot/Cargo.toml`
- [ ] Remove `[[bin]]` from `crates/autopilot-gui/Cargo.toml`
- [ ] Remove `[[bin]]` from `crates/desktop/Cargo.toml`
- [ ] Remove `[[bin]]` from `crates/gitafter/Cargo.toml`
- [ ] Update `default-members` in workspace Cargo.toml

### Phase 5: Cleanup
- [ ] Deprecate `crates/desktop/` (functionality merged)
- [ ] Update CLAUDE.md with new `openagents` commands
- [ ] Update systemd files to use `openagents daemon`
- [ ] Update any scripts referencing old binaries

## Files to CREATE

| Path | Purpose |
|------|---------|
| `src/main.rs` | Unified entry point with clap CLI |
| `src/gui/mod.rs` | GUI module root |
| `src/gui/app.rs` | Window creation (wry/tao) - copy pattern from wallet/gui/app.rs |
| `src/gui/server.rs` | Unified Actix server |
| `src/gui/state.rs` | UnifiedAppState definition |
| `src/gui/ws.rs` | WsBroadcaster (move from desktop/ws.rs) |
| `src/gui/routes/mod.rs` | Route configuration |
| `src/gui/routes/wallet.rs` | `/wallet/*` routes |
| `src/gui/routes/marketplace.rs` | `/marketplace/*` routes |
| `src/gui/routes/autopilot.rs` | `/autopilot/*` routes |
| `src/gui/routes/gitafter.rs` | `/git/*` routes |
| `src/gui/routes/daemon.rs` | `/daemon/*` routes |
| `src/gui/views/mod.rs` | View exports |
| `src/gui/views/layout.rs` | Tabbed layout with navigation |
| `src/cli/mod.rs` | CLI module root |
| `src/cli/wallet.rs` | Wallet CLI commands (wrap wallet::cli) |
| `src/cli/marketplace.rs` | Marketplace CLI commands |
| `src/cli/autopilot.rs` | Autopilot CLI commands |
| `src/cli/gitafter.rs` | GitAfter CLI commands |
| `src/cli/daemon.rs` | Daemon CLI commands |

## Files to MODIFY

| Path | Change |
|------|--------|
| `Cargo.toml` | Add `[[bin]]`, `[package]`, `[dependencies]` for unified binary |
| `crates/wallet/Cargo.toml` | Remove `[[bin]]` section |
| `crates/wallet/src/lib.rs` | Add `pub mod cli; pub mod gui; pub mod core;` |
| `crates/marketplace/Cargo.toml` | Remove `[[bin]]` section |
| `crates/autopilot/Cargo.toml` | Remove both `[[bin]]` sections |
| `crates/autopilot/src/lib.rs` | Expose daemon, run, metrics, etc. publicly |
| `crates/autopilot-gui/Cargo.toml` | Remove `[[bin]]` section |
| `crates/desktop/Cargo.toml` | Remove `[[bin]]` section |
| `crates/gitafter/Cargo.toml` | Remove `[[bin]]` section |
| `CLAUDE.md` | Update with `openagents` command patterns |

## Critical Reference Files

| File | Use For |
|------|---------|
| `crates/wallet/src/gui/app.rs` | Window creation pattern (wry/tao) |
| `crates/gitafter/src/server.rs` | Most complete AppState with NostrClient |
| `crates/desktop/src/ws.rs` | WsBroadcaster implementation |
| `crates/autopilot/src/main.rs` | Largest CLI structure to integrate |
| `crates/autopilot/src/daemon/supervisor.rs` | Daemon logic |

## Navigation UI

```
┌─────────────────────────────────────────────────────────────────────┐
│  [Wallet]  [Marketplace]  [Autopilot]  [GitAfter]  [Daemon]  [⚙]   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                        Active Tab Content                           │
│                                                                     │
├─────────────────────────────────────────────────────────────────────┤
│  Status: ● 3 relays | Balance: 50,000 sats | Daemon: running       │
└─────────────────────────────────────────────────────────────────────┘
```

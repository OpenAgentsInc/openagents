# Autopilot Architecture Migration Plan

## Overview

This document outlines the migration from the legacy `crates/autopilot/` monolith to the new
modular architecture in `apps/autopilot-desktop/` + supporting crates.

## Current State

### Legacy Architecture (`crates/autopilot/`)
- **180 Rust files**, ~32k lines
- Monolithic binary with embedded UI, state, integrations
- **Manual layout** throughout (`y +=` pattern, ~150+ instances)
- Tightly coupled: UI ↔ state ↔ integrations
- 27+ modal dialogs, chat rendering, sidebars, overlays

```
crates/autopilot/
├── src/
│   ├── app/
│   │   ├── ui/                    # 51 files, ~15k lines (manual layout)
│   │   │   ├── rendering/
│   │   │   │   ├── modals/        # 27 modal dialogs
│   │   │   │   ├── chat*.rs       # Chat rendering
│   │   │   │   ├── layouts.rs     # Manual layout
│   │   │   │   └── ...
│   │   │   └── theme.rs
│   │   ├── agents/                # Agent backends (Codex, etc.)
│   │   ├── autopilot/             # Core handler, DSPy callbacks
│   │   ├── catalog/               # Skills, agents, MCP, hooks
│   │   ├── chat/                  # Message state, layout
│   │   ├── config/                # Settings, keybindings
│   │   ├── permissions/           # Permission rules, history
│   │   ├── session/               # Session state, persistence
│   │   ├── tools/                 # Tool parsing, visualization
│   │   └── *.rs                   # Integrations (wallet, pylon, nostr, etc.)
│   └── main.rs
└── Cargo.toml
```

### New Architecture (`apps/autopilot-desktop/` + crates)

Clean separation of concerns with proper layering:

```
apps/autopilot-desktop/       # Desktop app shell (1.3k lines)
├── src/
│   ├── main.rs                    # Window, event loop, Codex client
│   └── full_auto.rs               # Full-auto mode logic
└── Cargo.toml

crates/autopilot_app/              # App logic layer (~230 lines)
├── src/
│   ├── lib.rs                     # App, Workspace, Session handles
│   └── replay.rs                  # Event recording/replay
└── Cargo.toml

crates/autopilot_ui/               # UI layer (~1.9k lines)
├── src/
│   └── lib.rs                     # MinimalRoot, panes, layout engine
└── Cargo.toml

crates/autopilot-core/             # Core execution logic
└── ...                            # Replay, verification, etc.
```

**Key differences:**
| Aspect | Legacy | New |
|--------|--------|-----|
| Layout | Manual `y +=` | Layout engine (Taffy) |
| Architecture | Monolith | Layered (app/ui/core) |
| State | Embedded in UI | Separate `autopilot_app` |
| Modals | 27 hardcoded | Pane-based system |
| Integrations | Inline | Via typed events |

## Migration Strategy

### Guiding Principles

1. **Incremental**: Migrate feature-by-feature, not big-bang
2. **Test parity**: Each migrated feature must work in new app before removing from legacy
3. **No regression**: Legacy app remains functional until full migration
4. **Clean interfaces**: Use `autopilot_app` events, not direct coupling

### Phase 1: Foundation (Current State) ✓

- [x] Desktop shell with wgpu/winit
- [x] Codex client integration
- [x] Basic pane system (Chat, Events, Identity)
- [x] Event-driven architecture (`AppEvent`, `UserAction`)
- [x] Full-auto mode
- [x] Layout engine usage in `autopilot_ui`

### Phase 2: Core Chat Features

**Goal**: Migrate essential chat functionality to new architecture.

#### 2.1 Message Rendering
- [ ] Migrate `ThreadView` usage from legacy to `autopilot_ui`
- [ ] Port markdown rendering with syntax highlighting
- [ ] Port code block copy/actions
- [ ] Port streaming message display

#### 2.2 Tool Call Display
- [ ] Terminal tool calls (existing in `autopilot_ui`)
- [ ] Diff/patch visualization
- [ ] Search results display
- [ ] File operation visualization

#### 2.3 Input Handling
- [ ] Multi-line composer with history
- [ ] Mode switching (normal/plan)
- [ ] Keyboard shortcuts parity

**Acceptance**: Can have full coding session in new app.

### Phase 3: Session Management

**Goal**: Full session lifecycle in new architecture.

#### 3.1 Session State
- [ ] Port session persistence from `crates/autopilot/src/app/session/`
- [ ] Session list/history in pane
- [ ] Session forking/branching

#### 3.2 Workspace Management
- [ ] Workspace detection and switching
- [ ] Recent workspaces
- [ ] Git integration status

**Acceptance**: Can resume sessions, switch workspaces.

### Phase 4: Permissions & Security

**Goal**: Migrate permission system.

#### 4.1 Permission Rules
- [ ] Port `permissions/rules.rs` logic
- [ ] Permission rule editor pane
- [ ] Permission history viewer

#### 4.2 Approval Flow
- [ ] Tool approval UI
- [ ] Batch approval
- [ ] Permission presets

**Acceptance**: Full permission control in new app.

### Phase 5: Configuration & Settings

**Goal**: Migrate config UI.

#### 5.1 Settings
- [ ] Port `config/settings.rs`
- [ ] Settings pane with categories
- [ ] Keybinding editor

#### 5.2 Model Selection
- [ ] Model picker (exists in `autopilot_ui`)
- [ ] Model presets
- [ ] API key management

**Acceptance**: All user preferences configurable.

### Phase 6: Integrations

**Goal**: Migrate service integrations as panes.

#### 6.1 High Priority
- [ ] Wallet (Spark) - port from `modals/spark_wallet.rs`
- [ ] Gateway status - port from `modals/gateway.rs`
- [ ] Pylon earnings/jobs - port from `modals/pylon_*.rs`

#### 6.2 Medium Priority
- [ ] DSPy visualization - port from `modals/dspy.rs`
- [ ] RLM trace viewer - port from `modals/rlm*.rs`
- [ ] Issues/PR integration - port from `modals/issues.rs`

#### 6.3 Lower Priority
- [ ] NIP-28/NIP-90 (Nostr) - port from `modals/nip*.rs`
- [ ] Nexus - port from `modals/nexus.rs`
- [ ] DVM - port from `modals/dvm.rs`
- [ ] ChainViz - port from `modals/chainviz.rs`

**Acceptance**: All integrations accessible as panes.

### Phase 7: Advanced Features

**Goal**: Port remaining advanced features.

#### 7.1 Agent Management
- [ ] Agent list/backends - port from `modals/agent_*.rs`
- [ ] MCP config - port from `modals/mcp_config.rs`
- [ ] Skill list - port from `modals/skill_list.rs`

#### 7.2 Bootloader & Onboarding
- [ ] Port bootloader flow from `modals/bootloader.rs`
- [ ] First-run experience
- [ ] Help system - port from `modals/help.rs`

**Acceptance**: Full feature parity with legacy.

### Phase 8: Deprecation & Cleanup

**Goal**: Remove legacy code.

1. [ ] Mark `crates/autopilot/` as deprecated
2. [ ] Redirect `autopilot` binary to new app
3. [ ] Remove legacy UI code
4. [ ] Archive or delete `crates/autopilot/`
5. [ ] Update documentation

## File Mapping

### Modals → Panes

| Legacy Modal | New Pane | Priority |
|--------------|----------|----------|
| `help.rs` | Help pane | P1 |
| `spark_wallet.rs` | Wallet pane | P1 |
| `session_list.rs` | Sessions pane | P1 |
| `config.rs` | Settings pane | P2 |
| `gateway.rs` | Gateway pane | P2 |
| `pylon_*.rs` | Pylon pane | P2 |
| `dspy.rs` | DSPy pane | P3 |
| `rlm*.rs` | RLM pane | P3 |
| `issues.rs` | Issues pane | P3 |
| `agent_*.rs` | Agents pane | P3 |
| `tool_list.rs` | Tools pane | P3 |
| `nip*.rs` | Nostr pane | P4 |
| `nexus.rs` | Nexus pane | P4 |
| `dvm.rs` | DVM pane | P4 |
| `chainviz.rs` | ChainViz pane | P4 |

### State → autopilot_app

| Legacy State | New Location |
|--------------|--------------|
| `session/state.rs` | `autopilot_app` + DB |
| `permissions/state.rs` | `autopilot_app` |
| `config/state.rs` | `autopilot_app` |
| `chat/state.rs` | `autopilot_ui` (view) + `autopilot_app` (data) |
| `tools/state.rs` | `autopilot_app` |

## Technical Considerations

### Event-Driven Migration

All features should communicate via `AppEvent` and `UserAction`:

```rust
// Define new events as needed
pub enum AppEvent {
    // Existing
    WorkspaceOpened { ... },
    SessionStarted { ... },
    UserActionDispatched { ... },
    AppServerEvent { ... },
    
    // Add as migrating
    PermissionRequested { ... },
    SettingsChanged { ... },
    WalletUpdated { ... },
    // ...
}
```

### Pane Architecture

New features should be panes, not modals:

```rust
pub enum PaneKind {
    Chat,
    Events,
    Identity,
    // Add as migrating
    Sessions,
    Settings,
    Wallet,
    Permissions,
    // ...
}
```

### Layout Engine

All new UI must use layout engine:

```rust
// ✓ Good: Use LayoutEngine
let engine = LayoutEngine::new();
let root = engine.request_node(style);
engine.compute_layout(root, available);
let bounds = engine.layout(root);

// ✗ Bad: Manual layout
let mut y = bounds.origin.y;
y += height + GAP;
```

## Success Metrics

1. **Feature parity**: All legacy features work in new app
2. **No manual layout**: Zero `y +=` patterns in new code
3. **Clean architecture**: Clear app/ui/core separation
4. **Test coverage**: Unit tests for migrated state logic
5. **Performance**: Equal or better render performance

## Timeline Estimates

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| Phase 2 (Chat) | 2-3 weeks | None |
| Phase 3 (Sessions) | 1-2 weeks | Phase 2 |
| Phase 4 (Permissions) | 1 week | Phase 2 |
| Phase 5 (Config) | 1 week | None |
| Phase 6 (Integrations) | 2-3 weeks | Phases 2-5 |
| Phase 7 (Advanced) | 2 weeks | Phases 2-6 |
| Phase 8 (Cleanup) | 1 week | All phases |

**Total: 10-14 weeks**

## Open Questions

1. **CLI compatibility**: Should `autopilot` CLI commands work with new app?
2. **Data migration**: How to migrate existing session data?
3. **Plugin system**: Should panes be pluggable?
4. **Theming**: Port theme system or redesign?

## Related Documents

- [Unified Layout Engine Plan](./unified-layout-engine-plan.md)
- [ADR-0002: Verified Patch Bundle](./adr/ADR-0002-verified-patch-bundle.md)
- [ADR-0003: Replay](./adr/ADR-0003-replay.md)

---

## Changelog

- 2026-01-28: Initial migration plan created

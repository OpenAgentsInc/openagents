# Migration Documentation

> **Purpose:** Complete documentation for migrating OpenAgents Commander from TypeScript/Bun to Rust/GPUI
> **Status:** Planning & Design Phase
> **Created:** 2025-12-09

---

## Overview

This directory contains comprehensive documentation for migrating the OpenAgents Commander codebase from TypeScript/Bun/Effect to Rust with the GPUI framework. The migration will preserve all functionality while gaining performance, memory safety, and better resource management for desktop application that manages swarm compute and AI agents.

---

## Documentation Index

### Core Planning Documents

#### 1. [Rust Migration Plan](./rust-migration-plan.md)
**Status:** Planning Document | **Priority:** P0

Complete migration roadmap covering:
- Executive summary and timeline (6-12 months)
- Current architecture analysis (~113k lines TypeScript)
- Phase-by-phase migration strategy (8 phases)
- GPUI framework evaluation and decision
- Risk mitigation strategies
- Success criteria

**Key Decisions:**
- ✅ GPUI chosen for UI framework (GPU-accelerated, Rust-native)
- ✅ Parallel development strategy (TypeScript + Rust coexist)
- ✅ Phased approach: Core → UI → Agents → HillClimber → Integration

#### 2. [GPUI Complete Guide](./gpui-complete-guide.md)
**Status:** Reference Guide | **Priority:** P0

Comprehensive GPUI framework documentation:
- Architecture (Entity system, Views, Elements)
- Core concepts (App, Context, Render trait)
- State management with Entity<T>
- Styling system (Tailwind-like API)
- Events and interactions (keyboard-first)
- Async support with integrated executor
- Platform integration (clipboard, menus, dialogs)
- Migration patterns from Effuse
- Best practices and performance tips

**Based on:** Analysis of `/Users/christopherdavid/code/zed/crates/gpui`

#### 3. [Effuse to GPUI Components](./effuse-to-gpui-components.md)
**Status:** Component Inventory & Migration Guide | **Priority:** P0

Component-by-component migration guide:
- Complete inventory (15 core components + 5 complex systems)
- Architecture comparison (Effuse vs GPUI)
- Migration patterns with examples
- Complexity ratings and priorities
- Common challenges and solutions
- Performance considerations

**Component Priorities:**
- P0: MC Tasks, TB Controls, TB Output, TB Results
- P1: APM Widget, TB Learning
- P2: Trajectory components, HF components
- P3: ATIF components
- P4: Three.js backgrounds

#### 4. [Effect Patterns in Rust](./effect-patterns-in-rust.md)
**Status:** Analysis & Recommendations | **Priority:** P0

Comprehensive analysis of functional programming in Rust:
- Rust functional capabilities (closures, iterators, zero-cost abstractions)
- Effect pattern analysis (Effect<T,E,R>, Context, Layers, Streams)
- What to port vs what to replace
- Recommended architecture patterns
- Implementation examples
- Performance considerations
- Decision matrix

**Key Recommendation:** Use native Rust idioms (`async/await`, `Result<T,E>`, traits) instead of porting Effect-TS patterns.

#### 5. [Foundation Model Bridge - Rust Plan](./fm-bridge-rust-plan.md)
**Status:** Implementation Ready | **Priority:** P0 (First Crate)

Complete plan for first Rust crate:
- API surface (types, client, streaming, errors)
- Implementation timeline (2 weeks / 8 phases)
- Testing strategy (unit, integration, property-based)
- Performance targets (latency, throughput, resource usage)
- Migration examples from TypeScript
- Success criteria

**Why First:** Standalone crate, no dependencies, critical for Terminal-Bench

---

## Example Implementations

The `examples/` directory contains standalone Rust implementations of key Effuse components:

### [apm-view.rs](./examples/apm-view.rs)
APM Widget implementation showing:
- Entity<State> for reactive state
- Compact/expanded view toggle
- Color-coded APM metrics
- Event handling with cx.listener()
- Conditional rendering with .when()
- WebSocket subscription pattern

**Complexity:** Medium | **LOC:** ~300

### [mc-tasks-view.rs](./examples/mc-tasks-view.rs)
MechaCoder Tasks component showing:
- Complex table rendering
- Priority badges and type labels
- Async task loading with cx.spawn()
- Service trait pattern
- Error handling
- Collapsible UI

**Complexity:** High | **LOC:** ~450

### [log-output-view.rs](./examples/log-output-view.rs)
Terminal output component showing:
- Virtualized list rendering
- Auto-scroll implementation
- Log filtering
- ANSI color mapping
- Stream subscription
- Performance optimization

**Complexity:** Medium | **LOC:** ~350

---

## Migration Timeline

### Phase 0: Foundation (Weeks 1-4)
- **Deliverable:** GPUI POC, Effect patterns decided
- **Status:** ✅ Planning complete

### Phase 1: Core Infrastructure (Weeks 5-12)
- **Deliverable:** Effect-like primitives, platform abstractions
- **Status:** 🔜 Ready to start
- **First Crate:** `fm-bridge` (2 weeks)

### Phase 2: UI Framework (Weeks 13-20)
- **Deliverable:** All Effuse components ported to GPUI
- **Status:** 📋 Planned

### Phase 3: Agent System (Weeks 21-32)
- **Deliverable:** MechaCoder in Rust
- **Status:** 📋 Planned

### Phase 4: HillClimber (Weeks 33-40)
- **Deliverable:** FM training system
- **Status:** 📋 Planned

### Phase 5: Desktop App (Weeks 41-48)
- **Deliverable:** Complete application
- **Status:** 📋 Planned

### Phase 6: Polish (Weeks 49-52)
- **Deliverable:** Production ready
- **Status:** 📋 Planned

**Total:** 52 weeks (1 year) | **Accelerated:** 32-36 weeks (with parallelization)

---

## Key Technologies

### Rust Ecosystem

| Category | Technology | Purpose |
|----------|-----------|---------|
| **UI Framework** | GPUI | GPU-accelerated native UI |
| **Async Runtime** | Tokio | Async executor, tasks, timers |
| **HTTP Client** | reqwest | HTTP/WebSocket client |
| **Serialization** | serde | JSON, TOML, etc. |
| **Database** | rusqlite, sqlx | SQLite + async SQL |
| **Error Handling** | thiserror, anyhow | Library + app errors |
| **CLI** | clap | Argument parsing |
| **Streaming** | tokio-stream | Async streams |

### Keep As-Is

- Swift Bridge (Foundation Models HTTP API)
- SQLite database schema
- JSONL file formats
- WebSocket protocols

---

## Architecture Decisions

### ✅ Decided

1. **GPUI for UI** - GPU-accelerated, Rust-native, proven in Zed
2. **Native Rust patterns** - async/await, Result<T,E>, traits over Effect-TS
3. **Builder pattern** - Dependency injection via builders
4. **Trait-based services** - Zero-cost abstraction with generics
5. **FM Bridge first** - Standalone crate, clear API, well-tested

### 🔄 To Decide

1. HTTP server framework (for HUD WebSocket)
2. State persistence approach
3. Cross-platform testing strategy
4. Distribution packaging (macOS, Windows, Linux)

---

## Component Inventory

### Effuse Components (15 total)

| Component | Priority | Complexity | Status |
|-----------|----------|------------|--------|
| MC Tasks | P0 | High | 📝 Example written |
| TB Controls | P0 | Medium | 📋 Planned |
| TB Output | P0 | Medium | 📝 Example written |
| TB Results | P0 | High | 📋 Planned |
| APM Widget | P1 | Medium | 📝 Example written |
| TB Learning | P1 | Medium | 📋 Planned |
| Category Tree | P2 | High | 📋 Planned |
| Trajectory Pane | P2 | Medium | 📋 Planned |
| Others | P2-P4 | Varies | 📋 Planned |

### Complex Systems

| System | Components | Complexity | Status |
|--------|-----------|------------|--------|
| TB Command Center | 7 files | Very High | 📋 Planned |
| Agent Graph | 6 files | High | 📋 Planned |
| TestGen Graph | 5 files | High | 📋 Planned |

---

## Getting Started

### For Developers

1. **Read planning docs** in order:
   - Start with [rust-migration-plan.md](./rust-migration-plan.md)
   - Then [gpui-complete-guide.md](./gpui-complete-guide.md)
   - Then [effect-patterns-in-rust.md](./effect-patterns-in-rust.md)

2. **Review examples:**
   - [apm-view.rs](./examples/apm-view.rs) - Simple component
   - [mc-tasks-view.rs](./examples/mc-tasks-view.rs) - Complex component
   - [log-output-view.rs](./examples/log-output-view.rs) - Virtualized list

3. **Start with FM Bridge:**
   - Read [fm-bridge-rust-plan.md](./fm-bridge-rust-plan.md)
   - Clone Zed repo for GPUI reference: `git clone https://github.com/zed-industries/zed ~/code/zed`
   - Set up Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

### For Reviewers

1. Check [rust-migration-plan.md](./rust-migration-plan.md) for overall strategy
2. Review [effuse-to-gpui-components.md](./effuse-to-gpui-components.md) for component priorities
3. Validate [fm-bridge-rust-plan.md](./fm-bridge-rust-plan.md) API design

---

## Success Criteria

### Functional Parity
- [ ] All TypeScript features work in Rust
- [ ] All tests pass
- [ ] Performance >= TypeScript version
- [ ] Memory usage <= TypeScript version

### Code Quality
- [ ] Zero unsafe Rust (or minimal, well-documented)
- [ ] >80% test coverage
- [ ] Documentation complete
- [ ] Linting passing (rustfmt, clippy)

### User Experience
- [ ] Desktop app works identically
- [ ] No functionality regressions
- [ ] Startup time <= TypeScript version
- [ ] UI responsiveness >= TypeScript version

---

## Resources

### Documentation
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [GPUI Website](https://www.gpui.rs/)
- [Zed Editor Source](https://github.com/zed-industries/zed)

### Community
- Rust Discord: [discord.gg/rust-lang](https://discord.gg/rust-lang)
- Zed Discord: [discord.gg/zed](https://discord.gg/zed)

### Tools
- [rustup](https://rustup.rs/) - Rust toolchain installer
- [cargo](https://doc.rust-lang.org/cargo/) - Rust package manager
- [clippy](https://github.com/rust-lang/rust-clippy) - Rust linter
- [rustfmt](https://github.com/rust-lang/rustfmt) - Rust formatter

---

## Next Steps

1. **Team Review** - Review and approve migration plan
2. **Environment Setup** - Install Rust, clone Zed, set up tooling
3. **Start FM Bridge** - First Rust crate (2 weeks)
4. **GPUI POC** - Port one Effuse component to validate approach
5. **Iterate** - Refine patterns based on learnings

---

**Last Updated:** 2025-12-09
**Maintained By:** OpenAgents
**Status:** Ready for Review

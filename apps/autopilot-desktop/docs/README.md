# Autopilot Documentation

This directory contains comprehensive documentation for the Autopilot project.
The Rust backend now lives in `apps/autopilot-desktop/src-tauri/src` within the
Tauri crate at `apps/autopilot-desktop/src-tauri`.

## Main Documentation

### [ARCHITECTURE.md](./autopilot/ARCHITECTURE.md)
High-level architecture overview:
- System architecture and layers
- Core concepts (unified agents, ACP protocol)
- Event flow (unified-event + ui-event)
- Key components (backend and frontend)
- Current implementation status
- File structure

### [IMPLEMENTATION.md](./autopilot/IMPLEMENTATION.md)
Detailed implementation guide:
- Backend implementation details
- Frontend implementation details
- Event flow examples (UI tree + patches)
- Session ID management
- Known issues and future improvements

### [API.md](./autopilot/API.md)
API reference:
- Tauri commands (parameters, returns, examples)
- Tauri events (unified-event + ui-event types)
- TypeScript types
- Usage examples
- Error handling

## Historical/Reference Documentation

### [ACP_ASSESSMENT.md](./acp/ACP_ASSESSMENT.md)
Initial assessment of ACP integration:
- Comparison of current vs ACP approach
- Integration strategy options
- Implementation status (Phase 1 completed)
- Technical details

### [CODEX_ACP_ARCHITECTURE.md](./codex/CODEX_ACP_ARCHITECTURE.md)
Analysis of codex-acp architecture:
- How codex-acp works (doesn't use codex app-server)
- Architecture implications
- Dual connection explanation

### [ACP_EVENT_COMPARISON.md](./acp/ACP_EVENT_COMPARISON.md)
Comparison of events between codex app-server and ACP:
- Event type mapping
- Missing events analysis
- Extension requirements

### [ACP_EXTENSIONS_EXPLANATION.md](./acp/ACP_EXTENSIONS_EXPLANATION.md)
Explanation of ACP extensions:
- How extensions work
- Codex-specific extensions
- Custom notification handling

### [CODEX_ACP_FORK_PLAN.md](./codex/CODEX_ACP_FORK_PLAN.md)
Plan for maintaining a fork of codex-acp:
- Rationale for forking
- Extension requirements
- Maintenance strategy

## Quick Start

1. **New to the project?** Start with [ARCHITECTURE.md](./autopilot/ARCHITECTURE.md)
2. **Want to understand implementation?** Read [IMPLEMENTATION.md](./autopilot/IMPLEMENTATION.md)
3. **Need API reference?** Check [API.md](./autopilot/API.md)
4. **Working on ACP integration?** See [ACP_ASSESSMENT.md](./acp/ACP_ASSESSMENT.md)

## Documentation Status

- ✅ **ARCHITECTURE.md**: Complete and up-to-date
- ✅ **IMPLEMENTATION.md**: Updated for Effuse UITree + signature-driven UI
- ✅ **API.md**: Updated for UI patch events + signature registry
- 📝 **Historical docs**: Reference only, may be outdated

## Contributing

When updating documentation:
1. Update the relevant main doc (ARCHITECTURE, IMPLEMENTATION, or API)
2. Keep historical docs for reference but mark as outdated if needed
3. Update this README if adding new docs
4. Add ADRs to the root `docs/adr/` directory

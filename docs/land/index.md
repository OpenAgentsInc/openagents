# Land Architecture Documentation Index

This directory contains comprehensive documentation of architectural patterns and implementation strategies extracted from the Land code editor project, applicable to OpenAgents' Effect-TS migration.

## Overview

Land is a production-ready code editor built with Rust (Tauri) and TypeScript (Effect-TS) that achieves:
- **10x smaller bundle size** than Electron alternatives
- **4x better memory usage** than VS Code
- **Sub-millisecond latency** for streaming operations
- **95%+ VS Code extension compatibility**

## Documentation Structure

### 1. [README.md](./README.md)
High-level overview of the Land project, its architecture, and relevance to OpenAgents.

### 2. [Lessons for OpenAgents](./lessons.md)
Key architectural insights and patterns that OpenAgents should adopt, organized by implementation priority.

### Pattern Documentation

#### 3. [IPC Patterns](./ipc-patterns.md)
Type-safe cross-boundary communication patterns:
- Tauri command wrapping with Effect
- gRPC for bidirectional communication
- Type generation from Rust to TypeScript
- Error propagation across boundaries

#### 4. [Service Layer Architecture](./service-layer.md)
Effect-based service patterns:
- Service definition and composition
- Layer-based dependency injection
- VS Code workbench service reimplementation
- Testing strategies with mock layers

#### 5. [Resource Management](./resource-management.md)
Automatic cleanup and lifecycle patterns:
- AcquireRelease for event listeners
- Process lifecycle management
- File handle and watcher resources
- Memory management strategies

#### 6. [Error Handling](./error-handling.md)
Comprehensive error strategies:
- Tagged error hierarchies
- Retry and circuit breaker patterns
- Fallback strategies
- Error context enrichment

#### 7. [Streaming Patterns](./streaming-patterns.md)
Reactive programming with Effect streams:
- Replacing polling with streams
- Backpressure handling
- WebSocket and file streaming
- Real-time data processing

### 8. [Implementation Guide](./implementation-guide.md)
Practical roadmap for adopting Land patterns in OpenAgents:
- Four-phase implementation plan
- Migration strategies
- Testing approaches
- Common pitfalls to avoid

## Quick Start for OpenAgents Team

1. **Start Here**: Read [lessons.md](./lessons.md) for key takeaways
2. **Understand Patterns**: Review individual pattern documents based on Phase 3.5 priorities
3. **Implementation**: Follow the [implementation-guide.md](./implementation-guide.md)
4. **Reference**: Use pattern documents for specific examples during implementation

## Key Patterns to Implement First

Based on issue #1244 (Phase 3.5), prioritize:

1. **Type-Safe IPC** ([ipc-patterns.md](./ipc-patterns.md))
   - Wrap all Tauri commands
   - Add tagged errors
   - Consider ts-rs for type generation

2. **Service Layer** ([service-layer.md](./service-layer.md))
   - Convert to Effect.Service pattern
   - Implement Layer composition
   - Add dependency injection

3. **Resource Management** ([resource-management.md](./resource-management.md))
   - Use acquireRelease for all listeners
   - Implement process cleanup
   - Manage connection lifecycles

## Architecture Diagrams

### System Overview
```
┌─────────────┐     Tauri IPC      ┌─────────────┐
│   Wind 🍃   │ <----------------> │ Mountain ⛰️  │
│  (UI Layer) │                    │  (Backend)  │
└─────────────┘                    └─────────────┘
       │                                  │
       │                                  │ gRPC
       │                                  │
       v                                  v
┌─────────────┐                    ┌─────────────┐
│   Sky 🌌    │                    │  Cocoon 🦋  │
│(Components) │                    │(Extensions) │
└─────────────┘                    └─────────────┘
```

### Effect Service Architecture
```
Application Layer
    │
    ├── FileServiceLive ──────┐
    ├── SessionServiceLive ───┤
    ├── AgentServiceLive ─────┼── Composed via Layer.mergeAll
    ├── SyncServiceLive ──────┤
    └── ConfigServiceLive ────┘
```

## Performance Benchmarks

| Metric | Traditional | With Effect |
|--------|-------------|-------------|
| Bundle Size | 250MB | 25MB |
| Memory Usage | 400MB | 100MB |
| Message Latency | 25ms | <1ms |
| Startup Time | 3s | <1s |

## Resources

- **Land Repository**: https://github.com/CodeEditorLand/Land
- **Effect Documentation**: https://effect.website/
- **OpenAgents Effect Guide**: [docs/effect/README.md](../effect/README.md)

## Contributing

When adding new Land patterns:
1. Create a new markdown file in this directory
2. Update this index with a link and description
3. Add practical examples relevant to OpenAgents
4. Include implementation notes and gotchas
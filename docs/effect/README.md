# Effect-TS Integration Guide

This guide provides a comprehensive overview of Effect-TS integration in the OpenAgents project, serving as a central reference for all Effect-related documentation, issues, and implementation patterns.

## Overview

OpenAgents is undergoing a systematic migration to Effect-TS, a powerful functional programming library that provides:
- **Type-safe error handling** with tagged errors
- **Resource management** with automatic cleanup
- **Streaming capabilities** with built-in backpressure
- **Structured concurrency** using lightweight fibers
- **Software Transactional Memory (STM)** for atomic state updates

## Current Status

- ✅ **Phase 1**: Mobile session sync with Confect foundation (Completed)
- ✅ **Phase 2**: APM & Authentication services (Completed)
- ✅ **Phase 3**: Full Confect integration - Effect-TS + Convex unification (Completed)
- 🚧 **Phase 4**: Comprehensive testing coverage (In Progress)
- 📋 **Next**: Agent orchestration using Effect's actor model

## Documentation

### Research Documents

#### 1. [Implementing Effect for Tauri Streaming](../research/effect/implementing-effect-for-tauri-streaming.md)
**Purpose**: Replace 50ms polling architecture with real-time Effect-based streaming  
**Key Topics**:
- TauriEventService implementation with automatic cleanup
- ClaudeStreamingService for message queue management
- React hooks integration (useClaudeStreaming)
- Circuit breaker and retry patterns
- 4-week migration strategy

#### 2. [Tauri Effect Streaming Guide](../research/effect/tauri-effect-streaming.md)
**Purpose**: Comprehensive guide on Effect's streaming architecture for Tauri  
**Key Topics**:
- Pull-based streams with automatic backpressure
- Stream, Queue, and Channel primitives
- Fiber-based concurrency patterns
- Testing with TestClock utilities
- Performance characteristics and RxJS comparison

#### 3. [Effect-TS Tauri Architecture Patterns](../research/effect/tauri-effect.md)
**Purpose**: Production patterns from Land code editor implementation  
**Key Topics**:
- Service composition for Tauri commands
- STM for coordinated state management
- Offline-capable sync patterns
- Multi-window coordination
- Cache invalidation strategies

## Key Issues & Pull Requests

### Main Integration Issue
- **[#1234](https://github.com/openagents/openagents/issues/1234)** - Add Effect (OPEN)
  - Comprehensive integration guide with 4-phase migration plan
  - Covers fundamentals: lazy values, type-safe errors, structured concurrency
  - Bundle overhead: ~25KB compressed

### Phase 1: Foundation
- **[#1237](https://github.com/openagents/openagents/pull/1237)** - Mobile Session Sync (MERGED)
  - Implemented Confect dependency (Effect-TS + Convex)
  - Tagged errors: `MobileSyncError`, `SessionValidationError`, `ProcessingTimeoutError`
  - Replaced Promises with `Effect.gen` patterns
  - 388+ lines simplified with Effect patterns

### Phase 2: Core Services
- **[#1239](https://github.com/openagents/openagents/pull/1239)** - APM & Auth Services (MERGED)
  - Cross-platform Storage Service (localStorage/SecureStore)
  - Effect-based APM tracking (214 → 50 lines)
  - OAuth Authentication with retry patterns
  - Backward-compatible React hooks

### Phase 3: Database Integration
- **[#1241](https://github.com/openagents/openagents/pull/1241)** - Confect Integration (MERGED)
  - Migrated 7 database tables to Effect Schema
  - Converted 15+ Convex functions with Option types
  - Auto-generated OpenAPI documentation
  - End-to-end type safety from DB to React

### Phase 4: Testing
- **[#1243](https://github.com/openagents/openagents/issues/1243)** - Testing Coverage (OPEN)
  - Target: 90%+ unit test coverage
  - Integration testing for cross-service interactions
  - Error scenario and performance testing

### Streaming Architecture
- **[#1159](https://github.com/openagents/openagents/issues/1159)** - Effect-based streaming (CLOSED)
- **[#1164](https://github.com/openagents/openagents/pull/1164)** - Real-time streaming fix (MERGED)
  - Replaced polling with Tauri event streaming
  - Reduced latency from 25ms to <1ms
  - Zero CPU usage when idle
  - Key fix: `Effect.forkDaemon` prevents fiber interruption

### Other Notable PRs
- **[#964](https://github.com/openagents/openagents/pull/964)** - Effect MCP server integration
- **[#1039](https://github.com/openagents/openagents/pull/1039)** - Effect-based Psionic features
- **[#981](https://github.com/openagents/openagents/pull/981)** - OpenRouter provider for Effect AI
- **[#1089](https://github.com/openagents/openagents/pull/1089)** - Convex database integration

## Architectural Decisions

### 1. Confect Framework
Using Confect (Effect-TS + Convex) for seamless database integration with:
- Effect Schema validation
- Option types replacing null checks
- Compile-time type safety

### 2. Service Architecture
All Tauri commands wrapped in Effect services:
```typescript
const TauriCommandService = Context.GenericTag<TauriCommandService>("TauriCommandService")
```

### 3. Error Handling
Tagged errors for precise error discrimination:
- `StorageError`, `AuthError`, `APMError`
- `StreamingError`, `ConnectionError`, `MessageParsingError`
- Automatic retry with exponential backoff

### 4. State Management
STM (Software Transactional Memory) for atomic updates:
- Mobile-desktop sync coordination
- Offline-capable state management
- Conflict-free concurrent updates

## Quick Reference

### Common Patterns

**Service Definition**:
```typescript
export class MyService extends Effect.Service<MyService>()('MyService', {
  sync: () => ({ /* methods */ }),
  dependencies: [/* other services */]
}) {}
```

**Error Handling**:
```typescript
Effect.catchTags({
  StorageError: (e) => Effect.fail(new CustomError(e)),
  NetworkError: (e) => Effect.retry(retrySchedule)
})
```

**React Integration**:
```typescript
const useEffectService = () => {
  const runtime = useContext(EffectRuntimeContext)
  // ... hook implementation
}
```

**Streaming**:
```typescript
Stream.fromQueue(queue).pipe(
  Stream.tap(processMessage),
  Stream.runDrain
)
```

## Performance Metrics

| Metric | Before Effect | With Effect |
|--------|--------------|-------------|
| Message Latency | 25ms (polling) | <1ms (streaming) |
| Bundle Size | - | +25KB compressed |
| Code Reduction | - | ~75% in some services |
| CPU Usage (idle) | 5% | <0.1% |

## Next Steps

1. Complete Phase 4 testing coverage
2. Implement agent orchestration patterns
3. Add Effect-based voice recording service
4. Enhance WebSocket handling with Effect streams

## Resources

- [Effect Documentation](https://effect.website/)
- [Confect Documentation](https://github.com/get-convex/confect)
- [Effect Discord](https://discord.gg/effect-ts)
- [OpenAgents Effect Issues](https://github.com/openagents/openagents/issues?q=is%3Aissue+effect)
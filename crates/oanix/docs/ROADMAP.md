# OANIX Roadmap

## Current Status

**Sprint 1: Complete** ✅
- [x] `MemFs` - In-memory filesystem with full read/write support
- [x] `FileService` trait with `Serialize` support
- [x] Browser WASM build with wasm-bindgen
- [x] Namespace Explorer demo (Tokyo Night themed)
- [x] 8 passing unit tests

**Sprint 2: Complete** ✅
- [x] Wasmtime integration with WASI preview1
- [x] Namespace-to-WASI bridge (sync to temp directories)
- [x] `WasiRuntime` with `RunConfig` and `RunResult`
- [x] CLI example (`run_wasi`)
- [x] Test WASI binary (`hello-wasi`)

**Sprint 3: Complete** ✅
- [x] `MapFs` - Static/immutable filesystem (~270 lines)
- [x] `FuncFs` - Computed/dynamic files via closures (~330 lines)
- [x] `CowFs` - Copy-on-Write overlay filesystem (~340 lines)
- [x] 20 new tests (29 total)
- [x] `FsError::ReadOnly` error variant

**Sprint 4: Complete** ✅
- [x] `TaskFs` - Task specification, status, and results (~320 lines)
- [x] `LogsFs` - Structured logging with stdout/stderr/events (~400 lines)
- [x] `WorkspaceFs` - Real filesystem wrapper with path security (~380 lines)
- [x] 30 new unit tests (59 total)
- [x] 4 new integration tests (12 total)
- [x] Complete agent environment demo

**Sprint 5: Complete** ✅
- [x] `NostrFs` - Nostr event signing and NIP-90 DVM capability (~600 lines)
- [x] `WsFs` - WebSocket connection management (~700 lines)
- [x] `HttpFs` - HTTP request/response client (~600 lines)
- [x] 33 new unit tests (92 total)
- [x] 11 new integration tests (23 total)
- [x] Outbox/inbox pattern for all capability services
- [x] Full agent environment with all capabilities

**Sprint 6: Complete** ✅
- [x] `OanixEnv` - Complete environment abstraction (~250 lines)
- [x] `EnvStatus` - Environment lifecycle states (~100 lines)
- [x] `Scheduler` - Priority-based job queue (~400 lines)
- [x] `JobSpec` / `JobKind` - Job specification types (~250 lines)
- [x] 25 new unit tests (117 total)
- [x] 10 new integration tests (33 total)
- [x] Environment lifecycle management
- [x] Priority-based scheduling with concurrency limits

---

**Sprint 7: Complete** ✅
- [x] `ExecutorManager` - Unified async executor manager (~200 lines)
- [x] `ExecutorConfig` - Configuration with timeouts, retry policies (~150 lines)
- [x] `ExecutorError` - Comprehensive error types (~80 lines)
- [x] `HttpExecutor` - Polls HttpFs, executes via reqwest (~230 lines)
- [x] `WsConnector` - Manages WebSocket connections via tokio-tungstenite (~360 lines)
- [x] `NostrRelayConnector` - Bridges NostrFs to relays via NIP-01 (~350 lines)
- [x] `Filter` type - NIP-01 subscription filters for NostrFs (~50 lines)
- [x] Helper methods for HttpFs, WsFs, NostrFs
- [x] 5 new unit tests (122 total)
- [x] `net-executor` feature flag for optional network capabilities
- [x] Exponential backoff retry with configurable policies
- [x] Graceful shutdown via broadcast channels

---

## Future Considerations

### Browser WASI Runtime

Currently Sprint 2 targets native wasmtime. Browser needs different approach:
- Use browser's WebAssembly API
- Implement WASI preview1 in JavaScript
- Consider existing solutions (browser_wasi_shim)

### Persistence

- Save/restore namespaces
- IndexedDB backing for browser MemFs
- SQLite backing for native

### Networking

- 9P protocol for remote mounts
- Distributed namespaces across nodes
- P2P file sharing

### Observability

- OpenTelemetry integration
- Metrics export via `/metrics`
- Distributed tracing

---

## Milestones

| Milestone | Description | Status |
|-----------|-------------|--------|
| **M1** | Run "Hello World" WASI in namespace | ✅ Sprint 2 |
| **M2** | Terminal-Bench task in OANIX env | ✅ Sprint 4 |
| **M3** | Agent with Nostr capability | ✅ Sprint 5 |
| **M4** | Full capability suite (WsFs, HttpFs) | ✅ Sprint 5 |
| **M5** | OanixEnv & Scheduler | ✅ Sprint 6 |
| **M6** | External executors (HTTP, WS, Nostr) | ✅ Sprint 7 |
| **M7** | Browser WASI execution | Future |

---

## Contributing

Focus areas where help is welcome:

1. **FileService implementations** - New filesystem types
2. **WASI compatibility** - Edge cases, preview2 support
3. **Browser runtime** - WASI shim, IndexedDB persistence
4. **Documentation** - Examples, tutorials
5. **Testing** - Property-based tests, fuzzing

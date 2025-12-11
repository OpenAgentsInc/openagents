# Design Decisions

This document captures key design decisions made during OANIX development and their rationale.

---

## Why Plan 9-Style Namespaces?

**Decision:** Model all capabilities as mountable filesystems with per-process namespaces.

**Alternatives considered:**
- Traditional syscall API (like POSIX)
- Capability-based object system (like seL4)
- Message passing (like Erlang)

**Rationale:**
1. **Universal interface**: Files are understood by every language and tool
2. **Composition**: Union mounting allows flexible capability assembly
3. **Debuggability**: `cat /cap/nostr/status` is easier than inspecting opaque objects
4. **WASI alignment**: WASI already uses filesystem APIs for host interaction
5. **Proven design**: Plan 9 demonstrated this scales to distributed systems

**Trade-offs:**
- Some operations feel awkward as files (e.g., RPC)
- Performance overhead for file abstraction layer
- Learning curve for developers unfamiliar with Plan 9

---

## Why WASM/WASI?

**Decision:** Use WebAssembly with WASI as the primary execution format.

**Alternatives considered:**
- Native binaries with sandboxing (seccomp, pledge)
- Containers (Docker, Firecracker)
- Language-specific VMs (V8, JVM)

**Rationale:**
1. **Universal portability**: Same binary runs server, desktop, browser
2. **Strong isolation**: Memory-safe sandbox with no escape hatches
3. **Fast startup**: Sub-millisecond cold start vs seconds for containers
4. **Deterministic**: Reproducible execution for benchmarking and replays
5. **Language agnostic**: Rust, Go, C, AssemblyScript all compile to WASM
6. **Capability model match**: WASI's preopened directories map to namespaces

**Trade-offs:**
- Smaller ecosystem than native
- Some performance overhead (5-20% typical)
- Limited threading support (improving)
- No raw system access (by design)

---

## Why Immutable Namespaces?

**Decision:** Namespaces are immutable after construction via builder.

**Alternatives considered:**
- Mutable namespaces with runtime mount/unmount
- Copy-on-write namespace forking

**Rationale:**
1. **Thread safety**: No locking needed for path resolution
2. **Predictability**: Agent's view doesn't change mid-execution
3. **Simplicity**: Easier to reason about and debug
4. **Security**: Can't dynamically gain capabilities

**Trade-offs:**
- Can't add capabilities during execution
- Must know full namespace at construction time
- Fork/clone patterns require new namespace construction

**Future consideration:** May add `NamespaceBuilder::extend()` for controlled modification.

---

## Why Arc<dyn FileService>?

**Decision:** Mount services as `Arc<dyn FileService>` (trait objects).

**Alternatives considered:**
- Generic `Mount<S: FileService>` with monomorphization
- Enum of known service types

**Rationale:**
1. **Heterogeneous mounts**: Different service types in same namespace
2. **Runtime flexibility**: Services can be constructed dynamically
3. **Simpler API**: No generic parameters on Namespace

**Trade-offs:**
- Dynamic dispatch overhead (minimal in practice)
- Can't use associated types in FileService
- Object safety constraints on trait design

---

## Why Separate FileHandle Trait?

**Decision:** `open()` returns `Box<dyn FileHandle>` rather than using FileService for I/O.

**Alternatives considered:**
- Stateless operations with file descriptors
- FileService methods take path + offset

**Rationale:**
1. **Stateful I/O**: Position tracking, buffering, dirty flags
2. **Resource cleanup**: Drop implementation can flush/release
3. **Familiar pattern**: Matches std::fs::File ergonomics
4. **Concurrent access**: Multiple handles to same file possible

**Trade-offs:**
- More complex implementation
- Handle lifetime management
- Can't easily implement over stateless protocols

---

## Why RwLock for MemFs?

**Decision:** Use `RwLock<MemNode>` for thread-safe mutable storage.

**Alternatives considered:**
- `Mutex` (simpler but more contention)
- Lock-free data structures
- Actor model with message passing

**Rationale:**
1. **Read-heavy workload**: Most operations are reads
2. **Simplicity**: Standard library primitive, well understood
3. **Correctness**: Easier to verify than lock-free code

**Trade-offs:**
- Write starvation possible under heavy read load
- Potential for deadlock if not careful with lock ordering
- Coarse-grained locking (whole tree)

**Future consideration:** Per-directory locking for better concurrency.

---

## Why Not Async FileService?

**Decision:** FileService methods are synchronous.

**Alternatives considered:**
- `async fn open()` returning futures
- Callback-based API

**Rationale:**
1. **WASI compatibility**: WASI filesystem ops are synchronous
2. **Simpler implementation**: No async runtime dependency in trait
3. **Composability**: Can wrap sync in async, harder vice versa
4. **MemFs use case**: In-memory ops are effectively instant

**Trade-offs:**
- Network-backed services may block
- Can't easily parallelize multiple file operations

**Mitigation:** Implementations can use internal async and block at boundary.

---

## Why Browser Feature Flag?

**Decision:** Browser/WASM support behind `--features browser`.

**Alternatives considered:**
- Always include WASM support
- Separate crate for browser

**Rationale:**
1. **Compile time**: WASM deps slow native builds
2. **Binary size**: Don't include unused wasm-bindgen code
3. **Platform differences**: Some code only makes sense in browser
4. **Clean separation**: Easy to see what's browser-specific

**Trade-offs:**
- Feature combinations to test
- Conditional compilation complexity

---

## Why Tokyo Night Theme for Demo?

**Decision:** Use Tokyo Night color scheme for browser demo.

**Rationale:**
1. **Consistency**: Matches user's Neovim/terminal setup (per CLAUDE.md)
2. **Readability**: Good contrast for code/text
3. **Aesthetic**: Modern, professional appearance

---

## Why Not Use web-sys Extensively?

**Decision:** Minimal web-sys usage, mostly wasm-bindgen primitives.

**Alternatives considered:**
- Full web-sys for DOM manipulation
- Framework like Yew or Leptos

**Rationale:**
1. **Simplicity**: Demo is small, plain JS sufficient
2. **Bundle size**: web-sys adds significant WASM bloat
3. **Flexibility**: JS side can use any framework
4. **Learning curve**: Plain JS more accessible

**Trade-offs:**
- Manual DOM manipulation in JS
- Type safety at JS boundary

---

## Open Questions

### Should FileService be async?
For network-backed services, blocking is problematic. Options:
- Add `AsyncFileService` trait
- Use `futures::executor::block_on` internally
- Require all network ops go through `/cap/*` services

### Should Namespace support union mounts?
Plan 9 allows multiple services at same path (reads check all, writes go to first). This enables:
- Overlay filesystems
- Fallback chains
- But adds complexity

### How to handle large files?
Current MemFs loads entire file into memory. Options:
- Streaming FileHandle implementation
- Memory-mapped backing
- Lazy loading with LRU cache

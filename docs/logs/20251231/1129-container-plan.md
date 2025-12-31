# Apple Container Integration Plan

**Goal**: Enable `crates/runtime/` to spawn Apple Containers when running locally on macOS 26+.

---

## Background

### What is Apple Container?

Apple's new open-source container runtime (`github.com/apple/containerization`) that runs on **macOS 26+ (Tahoe) only**. Unlike Docker (shared VM), each container gets its own lightweight Linux VM via Apple's Virtualization.framework.

**Key properties**:
- VM-level isolation (strongest security)
- OCI-compatible (standard container images)
- Native Apple Silicon optimization
- Uses XPC for service communication
- CLI tool: `container`

**Reference implementation**: `/Users/christopherdavid/code/container/`

### Current Runtime Architecture

The runtime (`crates/runtime/`) already has a container abstraction at `/containers` mount with multiple backends:

| Backend | Purpose |
|---------|---------|
| `LocalContainerProvider` | Docker/Podman via `std::process::Command` |
| `DvmContainerProvider` | NIP-90 Nostr compute jobs |
| `CloudflareProvider` | Cloudflare Container Runtime |
| `WasmOpenAgentsContainerProvider` | Browser fetch to API |

**Key file**: `crates/runtime/src/containers.rs` (200KB)

The container system supports:
- Container kinds: `Ephemeral`, `Interactive`, `Build`, `Custom`
- Resource limits: time, memory, disk, CPU, network
- Stdin/stdout/stderr streaming

**Gap**: No macOS-native container backend. `LocalContainerProvider` shells out to Docker, which requires Docker Desktop (another VM layer).

---

## Architecture Design

### New Backend: `AppleContainerProvider`

Add a new provider that calls the `container` CLI when:
1. Platform is macOS
2. macOS version >= 26.0
3. `container` CLI is available and service is running

```
┌─────────────────────────────────────────────────────────────┐
│                    ContainerFs (/containers)                 │
├─────────────────────────────────────────────────────────────┤
│                    ContainerProvider trait                   │
├──────────┬──────────┬──────────┬──────────┬────────────────┤
│  Local   │   DVM    │Cloudflare│  WASM    │ AppleContainer │
│ (Docker) │ (NIP-90) │          │          │   (NEW)        │
└──────────┴──────────┴──────────┴──────────┴────────────────┘
```

### Detection & Fallback Chain

```
1. macOS 26+ && `container system status` succeeds
   → AppleContainerProvider

2. macOS (any) && Docker available
   → LocalContainerProvider (existing)

3. Any platform && DVM relays configured
   → DvmContainerProvider (existing)

4. None available
   → Error or NoopProvider with warning
```

---

## Implementation Details

### 1. Feature Flag

Add `apple-container` feature to `Cargo.toml`:

```toml
[features]
apple-container = []  # macOS 26+ native containers
```

Compile-time gate with `#[cfg(all(target_os = "macos", feature = "apple-container"))]`.

### 2. Version Detection

At runtime, detect macOS 26+:

```
sw_vers -productVersion  # Returns "26.0" or similar
```

Check container service:

```
container system status  # Exit 0 if running
```

### 3. Container Lifecycle Mapping

Map existing `ContainerProvider` trait methods to `container` CLI:

| Runtime Method | Apple Container CLI |
|----------------|---------------------|
| `create()` | `container run --detach` or `container create` |
| `start()` | `container start <id>` |
| `exec()` | `container exec <id> <cmd>` |
| `stop()` | `container stop <id>` |
| `kill()` | `container kill <id>` |
| `remove()` | `container rm <id>` |
| `logs()` | `container logs <id>` |
| `wait()` | `container wait <id>` |

### 4. Resource Limits Mapping

| Runtime Limit | Apple Container Flag |
|---------------|---------------------|
| `max_memory_mb` | `--memory <bytes>` (convert MB → bytes) |
| `max_cpu_cores` | `--cpus <float>` |
| `allow_network` | `--network none` vs `--network default` |
| `max_time_secs` | Implement via timeout wrapper or `container stop` after duration |
| `max_disk_mb` | Not directly supported; use tmpfs size limits |

### 5. Volume Mounts

Map workspace directory:

```
container run -v /host/workspace:/workspace -w /workspace <image> <cmd>
```

For the agent's working directory, mount read-write. For system paths, mount read-only:

```
-v /host/path:/container/path:ro
```

### 6. Network Policy

**Default**: `--network none` for isolation.

**When network needed** (e.g., for package installs):
- Option A: `--network default` (full access)
- Option B: Future Apple Container network policies (not yet available)

**Host service access** (e.g., calling local LLM):
- Container can't reach `localhost`
- Use host gateway: `192.168.64.1`
- May need socat forwarding on host

### 7. Image Management

Use standard OCI images. Default base image with common tools:

```
container pull ghcr.io/openagents/runtime-base:latest
```

Or build custom:

```dockerfile
FROM oven/bun:latest
RUN apt-get update && apt-get install -y git curl
```

Build: `container build -t openagents-sandbox .`

### 8. Stdio Handling

The `container` CLI supports:
- `--interactive` / `-i` for stdin
- `--tty` / `-t` for PTY
- Default streams stdout/stderr

For `Interactive` container kind, use `-it`. For `Ephemeral`, just capture output.

### 9. Error Handling

| CLI Exit Code | Meaning | Runtime Action |
|---------------|---------|----------------|
| 0 | Success | Return output |
| 125 | Container failed to run | `ContainerError::StartFailed` |
| 126 | Command cannot be invoked | `ContainerError::ExecFailed` |
| 127 | Command not found | `ContainerError::CommandNotFound` |
| 137 | SIGKILL (OOM or timeout) | `ContainerError::Killed` |
| Other | Command exit code | Return as process exit code |

---

## File Structure

```
crates/runtime/src/
├── containers.rs              # Existing - add AppleContainerProvider
├── containers/
│   ├── mod.rs                 # Provider trait, detection logic
│   ├── local.rs               # Existing Docker/Podman provider
│   ├── dvm.rs                 # Existing NIP-90 provider
│   ├── cloudflare.rs          # Existing CF provider
│   ├── wasm.rs                # Existing WASM provider
│   └── apple.rs               # NEW: Apple Container provider
```

Or keep flat in `containers.rs` with `#[cfg]` blocks.

---

## Key Considerations

### 1. Cold Start Latency

Apple Container VMs take ~2-3 seconds to start. Mitigations:
- Keep warm container pool for interactive use
- Use `--detach` and reuse containers for multiple commands
- Pre-pull images at startup

### 2. Memory Management

Memory freed inside container isn't released back to macOS. Mitigations:
- Set reasonable `--memory` limits
- Periodically restart long-running containers
- Document this limitation

### 3. No Rust Bindings

Apple Container is Swift-only. Integration options:
- **CLI** (recommended): Shell out to `container` command
- **XPC FFI**: Complex, requires Swift interop, fragile
- **HTTP API**: Not exposed by default

CLI is simplest and matches existing `LocalContainerProvider` pattern.

### 4. macOS 26 Availability

macOS 26 (Tahoe) is in beta as of late 2025. Plan for:
- Feature-flagged so doesn't break older macOS
- Graceful fallback to Docker
- Clear error messages about version requirements

### 5. Rosetta for x86-64 Images

Apple Container supports x86-64 images via Rosetta:

```
container run --arch amd64 <x86-image>
```

Useful for images without arm64 variants.

---

## Integration Points

### With `/claude` Mount

The Claude filesystem (`crates/runtime/src/claude.rs`) already uses container isolation:
- `--network none` for code execution
- Credential injection via proxy
- Repo filtering

Apple Container provides stronger isolation than Docker for these workloads.

### With Agent Execution

When an agent writes to `/containers/run`:
1. Runtime detects best available backend
2. If macOS 26+, use `AppleContainerProvider`
3. Execute command in isolated VM
4. Stream output back to agent
5. Enforce resource limits

---

## Testing Strategy

### Unit Tests

Mock CLI responses for:
- Successful container creation
- Resource limit enforcement
- Error conditions (OOM, timeout, command not found)

### Integration Tests

Require macOS 26 machine:
- End-to-end container lifecycle
- Volume mount verification
- Network isolation verification
- Resource limit enforcement

### CI/CD

- Use `#[cfg_attr(not(target_os = "macos"), ignore)]` for macOS-only tests
- Add macOS 26 runner when available
- Fall back to Docker tests on other platforms

---

## Rollout Plan

### Phase 1: Provider Implementation
- Add `AppleContainerProvider` struct
- Implement `ContainerProvider` trait
- Add version detection logic
- Add feature flag

### Phase 2: Detection Integration
- Wire into backend auto-detection
- Add fallback chain
- Test on macOS 26 beta

### Phase 3: Documentation & Polish
- Document macOS 26 requirement
- Add troubleshooting guide
- Performance benchmarks vs Docker

### Phase 4: Default Backend
- When macOS 26 is stable, make Apple Container the default on macOS
- Keep Docker as explicit fallback option

---

## References

- Apple Container repo: `/Users/christopherdavid/code/container/`
- Apple Container docs: `~/code/container/docs/`
- Backroom planning doc: `/Users/christopherdavid/code/backroom/reference/openagents-docs/claude/plans/containers.md`
- Runtime containers: `/Users/christopherdavid/code/openagents/crates/runtime/src/containers.rs`
- Runtime docs: `/Users/christopherdavid/code/openagents/crates/runtime/docs/CONTAINERS.md`

---

## Log

- 2025-12-31 12:03:44 CST: Started implementation; reviewing plan and current runtime container/claude integration points.
- 2025-12-31 12:31:20 CST: Implemented AppleContainerProvider with macOS 26+ detection (`sw_vers`) and `container system status`, added CLI-based run/exec/file IO, and registered it ahead of Docker in default providers.
- 2025-12-31 12:31:20 CST: Added Claude containerized CLI support (Apple Container/Docker auto-detect, env-configured image/proxy), wired `isolation_mode: container` to wrap local/cloud providers, and updated runtime docs.
- 2025-12-31 12:35:20 CST: Ran `cargo test -p openagents-runtime` (pass; warnings present).

# Container Support for MechaCoder: Technology Assessment

> Research document comparing sandboxing/containerization approaches for MechaCoder on macOS

## Executive Summary

MechaCoder currently runs with full user privileges, which poses security risks when executing arbitrary code from LLM-generated responses. This document evaluates three approaches for adding container/sandbox support:

1. **Seatbelt (sandbox-exec)** - macOS native process sandboxing
2. **Docker** - Traditional Linux containers via Docker Desktop
3. **Apple Container (macOS 26)** - Apple's new OCI-compatible container runtime

### Recommendation: Apple Container

Given that:
- You're already running **macOS 26 (Tahoe)**
- Target users are **developers** (can handle CLI setup)
- **macOS-only** is acceptable

**Apple Container is the recommended approach.** It provides:
- VM-level isolation (strongest security)
- Apple-supported (not deprecated)
- OCI-compatible (standard images)
- No Docker licensing concerns
- Native Apple Silicon optimization

Skip to [Recommended Implementation: Apple Container](#recommended-implementation-apple-container) for the concrete plan.

---

## Current State: MechaCoder Without Sandboxing

MechaCoder today:
- Runs as a normal user process with full filesystem/network access
- Uses `.openagents/project.json` for configuration
- Executes bash commands, file operations, and git operations
- Has no isolation between agent code and host system
- Relies on test verification before commit/push as the only safety gate

**Security concerns:**
- LLM could generate malicious code that executes with user privileges
- No network egress controls (agent could exfiltrate data)
- No filesystem isolation (agent could read/write anywhere user can)
- No resource limits (could consume all CPU/memory)

---

## Option 1: Seatbelt (sandbox-exec)

### Overview

Seatbelt is Apple's process sandboxing mechanism, invoked via `sandbox-exec`. It uses policy files (`.sbpl`) to define what a process can do.

**Current usage in the ecosystem:**
- OpenAI's Codex CLI uses Seatbelt on macOS
- Chrome browser uses Seatbelt extensively
- App Store apps use App Sandbox (related technology)

### How It Works

```bash
# Example invocation
sandbox-exec -f policy.sbpl bun src/agent/do-one-task.ts --dir .
```

A policy file defines allowed operations:
```scheme
(version 1)
(deny default)                           ; deny everything by default
(allow file-read* (subpath "/path/to/workspace"))
(allow file-write* (subpath "/path/to/workspace"))
(allow process-exec)
(allow process-fork)
; ... granular controls for sysctls, IOKit, mach-lookup, etc.
```

### Codex's Seatbelt Implementation

From `codex-rs/core/src/seatbelt_base_policy.sbpl`:
- Starts with `(deny default)` - closed by default
- Allows process execution and forking (for child processes)
- Permits specific sysctls for CPU info, memory, etc.
- Allows IOKit access for power management
- Enables specific mach-lookup services (opendirectoryd)
- Allows POSIX semaphores (for Python multiprocessing)

The Codex CLI dynamically adds workspace-specific rules on top of this base policy.

### Advantages

| Benefit | Details |
|---------|---------|
| **Native to macOS** | No additional runtime dependencies |
| **Zero overhead** | No VM, no daemon, just process-level enforcement |
| **Granular control** | Can allow/deny specific files, network, syscalls |
| **Battle-tested** | Chrome has used it for years; Codex uses it today |
| **Works today** | Available on macOS 12+ |

### Disadvantages

| Concern | Details |
|---------|---------|
| **Deprecated** | Apple deprecated `sandbox-exec` in macOS Sierra (2016) |
| **Undocumented** | No official Apple documentation; policy syntax learned from WebKit/Chrome |
| **Could break anytime** | Apple could remove or change it in future macOS versions |
| **Expertise required** | Policy files are complex; subtle mistakes = security holes |
| **No official support** | Apple won't help if it breaks |
| **macOS only** | Not portable to Linux (though Linux has Landlock/seccomp) |

### Implementation Complexity

**Low-medium**. Would need to:
1. Create a base `.sbpl` policy file
2. Dynamically generate workspace-specific rules
3. Wrap agent execution with `sandbox-exec`
4. Handle policy violations gracefully

Could largely copy Codex's approach since it's open source.

### Risk Assessment

**Medium-High**. The deprecation is the elephant in the room. However:
- It still works 9 years after deprecation
- Chrome still uses it
- Apple's own tools likely depend on it

If Apple breaks it, we'd need to migrate to Apple Container or Docker.

---

## Option 2: Docker

### Overview

Docker provides Linux containers on macOS via Docker Desktop, which runs a Linux VM behind the scenes. Containers provide full process/filesystem/network isolation.

**Current usage in the ecosystem:**
- Codex CLI uses Docker for Linux sandboxing
- Industry standard for CI/CD and development environments
- Well-documented and widely understood

### How It Works

```bash
# Build container image
docker build -t mechacoder .

# Run MechaCoder in container with workspace mounted
docker run -v /path/to/workspace:/workspace mechacoder \
  bun src/agent/do-one-task.ts --dir /workspace
```

The Codex CLI's Docker approach (`codex-cli/scripts/run_in_container.sh`):
1. Builds image with CLI and firewall helpers
2. Bind-mounts workspace at same path inside container
3. Configures iptables for network egress allowlisting
4. Runs CLI inside container with `--full-auto`

### Advantages

| Benefit | Details |
|---------|---------|
| **Fully supported** | Docker is a stable, documented product |
| **Strong isolation** | Containers are separate Linux environments |
| **Portable** | Same approach works on Linux hosts |
| **Network controls** | Can use iptables for fine-grained egress rules |
| **Reproducible** | Container images ensure consistent environments |
| **OCI standard** | Images work with any OCI-compatible runtime |

### Disadvantages

| Concern | Details |
|---------|---------|
| **Requires Docker Desktop** | ~2GB install, licensing considerations for commercial use |
| **Performance overhead** | VM layer adds latency and resource usage |
| **Cold start latency** | Container startup adds seconds per invocation |
| **Filesystem complexity** | Bind mounts can have permission issues on macOS |
| **Resource usage** | Docker Desktop VM consumes RAM even when idle |
| **Distribution burden** | Users must install Docker before MechaCoder |

### Implementation Complexity

**Medium**. Would need to:
1. Create Dockerfile with MechaCoder dependencies (Bun, Effect, etc.)
2. Write container run script with bind mounts
3. Configure network firewall (iptables or similar)
4. Handle container lifecycle (start/stop/cleanup)
5. Map paths correctly between host and container

Could reference Codex's implementation but would need adaptation.

### Cost Considerations

Docker Desktop licensing:
- Free for small businesses (<250 employees, <$10M revenue)
- Paid for larger organizations
- Alternative: Podman, Colima, or Rancher Desktop (free)

---

## Option 3: Apple Container (macOS 26)

### Overview

Apple's new `container` tool, built on the open-source [Containerization](https://github.com/apple/containerization) Swift package, runs each container in its own lightweight Linux VM. It's OCI-compatible and designed for Apple Silicon.

### Requirements

**macOS 26 (Tahoe) required.** The tool relies on new Virtualization and vmnet framework features in macOS 26. It does not work on macOS 15 (Sonoma) or earlier in any meaningful way.

### How It Works

```bash
# Start the container system service
container system start

# Run an OCI image
container run --publish 8080:80 nginx

# Or build and run custom images
container build -t myapp .
container run myapp
```

Key architectural difference: instead of all containers sharing one VM, each container gets its own lightweight VM. This provides VM-level isolation while maintaining container-like ergonomics.

### Technical Details

From the technical overview:
- Uses Virtualization framework for VM management
- vmnet framework for container networking
- XPC for interprocess communication
- Launchd for service management
- Keychain for registry credentials
- Unified logging system

Architecture:
```
container CLI
    └── container-apiserver (launch agent)
        ├── container-core-images (XPC helper for image management)
        ├── container-network-vmnet (XPC helper for networking)
        └── container-runtime-linux (per-container runtime helper)
```

### Advantages

| Benefit | Details |
|---------|---------|
| **Apple-supported** | First-party solution with ongoing development |
| **VM-level isolation** | Each container is a separate VM (stronger than process sandbox) |
| **OCI-compatible** | Uses standard images, works with any registry |
| **Apple Silicon optimized** | Built specifically for M1/M2/M3 chips |
| **Modern approach** | Designed with current security best practices |
| **Open source** | Containerization framework is on GitHub |
| **No Docker license** | Free to use, no licensing concerns |

### Disadvantages

| Concern | Details |
|---------|---------|
| **macOS 26 only** | Won't work on current macOS (15) |
| **Pre-1.0** | API may change; stability not guaranteed |
| **Limited features** | Container-to-host networking not yet implemented |
| **Memory management** | Memory ballooning only partially works |
| **New/unproven** | Less community knowledge and tooling |
| **Adoption timeline** | macOS 26 won't be widely deployed until late 2025+ |

### Current Limitations (per Apple docs)

1. **Container-to-host networking**: Can't reach `localhost` services on host
2. **Memory release**: Memory freed by containers isn't released back to macOS
3. **macOS 15 specific issues**: Network isolation, subnet mismatches, limited functionality

### Implementation Complexity

**Medium-High** for initial implementation, but potentially **Low** long-term:
1. Create OCI image with MechaCoder dependencies
2. Use `container` CLI to run images
3. Configure resource limits (memory, CPU)
4. Handle workspace mounting
5. (Future) Configure network policies when supported

### Timeline Consideration

macOS 26 is in beta, expected release ~Fall 2025. MechaCoder would need to:
- Wait for general availability
- Wait for user adoption of macOS 26
- Support both sandboxed and non-sandboxed modes during transition

---

## Comparative Analysis

### Feature Comparison

| Feature | Seatbelt | Docker | Apple Container |
|---------|----------|--------|-----------------|
| **Isolation level** | Process | Container/VM | VM per container |
| **macOS support** | 12+ | 12+ (via Desktop) | 26+ only |
| **Performance** | Excellent | Good | Good |
| **Startup time** | Instant | Seconds | Seconds |
| **Network control** | Via policy | iptables | TBD |
| **Filesystem isolation** | Policy-based | Full | Full |
| **Memory limits** | No | Yes | Yes |
| **Official support** | Deprecated | Third-party | Apple |
| **Setup required** | None | Docker Desktop | CLI install |
| **Image format** | N/A | OCI | OCI |

### Security Model Comparison

| Aspect | Seatbelt | Docker | Apple Container |
|--------|----------|--------|-----------------|
| **Isolation boundary** | System calls | Linux namespaces + VM | Hardware VM |
| **Escape difficulty** | Medium | Medium-High | Very High |
| **Attack surface** | macOS kernel | Linux kernel in VM | Linux kernel in VM + Hypervisor |
| **Privilege required** | User | User + Docker group | User |

### Developer Experience

| Aspect | Seatbelt | Docker | Apple Container |
|--------|----------|--------|-----------------|
| **Learning curve** | High (undocumented) | Medium (well-known) | Low-Medium (familiar CLI) |
| **Debugging** | Difficult | Good (logs, exec) | Good (logs, exec) |
| **CI integration** | Manual | Excellent | Unknown |
| **Local development** | Transparent | Requires container awareness | Requires container awareness |

---

## Recommended Paths

### Path A: Seatbelt Now, Apple Container Later

**Timeline: Immediate → 2026+**

1. **Phase 1 (Now)**: Implement Seatbelt sandboxing
   - Copy/adapt Codex's policy approach
   - Get immediate security benefits
   - Accept deprecation risk

2. **Phase 2 (Late 2025)**: Prepare Apple Container support
   - Create OCI image for MechaCoder
   - Test on macOS 26 betas
   - Design abstraction layer for swapping backends

3. **Phase 3 (2026)**: Migrate to Apple Container
   - As macOS 26 adoption increases
   - Deprecate Seatbelt path
   - Use OCI images for both macOS and potential Linux support

**Rationale**: Get security benefits immediately while preparing for the supported future.

### Path B: Docker Now and Forever

**Timeline: Immediate → ongoing**

1. **Phase 1 (Now)**: Implement Docker-based sandboxing
   - Create MechaCoder Docker image
   - Write run scripts with proper mounts
   - Configure network egress rules

2. **Phase 2 (Ongoing)**: Maintain Docker approach
   - Works on macOS and Linux
   - Well-understood, documented
   - No migration needed

**Rationale**: Proven, portable, stable. Accept the overhead and distribution complexity.

### Path C: Wait for Apple Container

**Timeline: 2025-2026**

1. **Phase 1 (Now)**: No sandboxing changes
   - Accept current security posture
   - Document risks and mitigations (test-before-commit)

2. **Phase 2 (Fall 2025)**: Implement Apple Container support
   - Wait for macOS 26 GA
   - Build OCI image
   - Implement container-based execution

**Rationale**: Avoid wasted effort on deprecated technology. Accept security risk temporarily.

### Path D: Hybrid Approach

**Timeline: Now → ongoing**

1. **Implement abstraction layer** for sandboxing
   - Define common interface: `Sandbox.run(command, opts)`
   - Support multiple backends: Seatbelt, Docker, Apple Container, None

2. **Implement backends in priority order**:
   - Seatbelt (immediate, macOS)
   - Docker (for users who prefer it / Linux)
   - Apple Container (when macOS 26 is prevalent)

3. **Auto-select best available backend**:
   - macOS 26+ → Apple Container
   - macOS 12-15 → Seatbelt (if available) or Docker
   - Linux → Docker or Landlock/seccomp

**Rationale**: Maximum flexibility, supports all users, future-proof.

---

## Technical Implementation Sketches

### Seatbelt Integration

```typescript
// src/sandbox/seatbelt.ts
import { $ } from "bun";

interface SeatbeltConfig {
  workspaceDir: string;
  allowNetwork: boolean;
  allowedDomains?: string[];
}

function generatePolicy(config: SeatbeltConfig): string {
  return `
(version 1)
(deny default)

; Allow workspace access
(allow file-read* (subpath "${config.workspaceDir}"))
(allow file-write* (subpath "${config.workspaceDir}"))

; Allow process execution
(allow process-exec)
(allow process-fork)
(allow signal (target same-sandbox))

; System reads needed by Bun/Node
(allow file-read*
  (subpath "/usr")
  (subpath "/System")
  (subpath "${process.env.HOME}/.bun"))

${config.allowNetwork ? '; (allow network*)' : '(deny network*)'}
  `.trim();
}

export async function runSandboxed(
  command: string[],
  config: SeatbeltConfig
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const policyPath = `/tmp/mechacoder-${Date.now()}.sbpl`;
  await Bun.write(policyPath, generatePolicy(config));

  try {
    const result = await $`sandbox-exec -f ${policyPath} ${command}`.quiet();
    return {
      exitCode: result.exitCode,
      stdout: result.stdout.toString(),
      stderr: result.stderr.toString(),
    };
  } finally {
    await Bun.file(policyPath).unlink();
  }
}
```

### Docker Integration

```typescript
// src/sandbox/docker.ts
import { $ } from "bun";

interface DockerConfig {
  image: string;
  workspaceDir: string;
  memoryLimit?: string;
  cpuLimit?: number;
  networkMode?: "none" | "host" | "bridge";
}

export async function runInDocker(
  command: string[],
  config: DockerConfig
): Promise<{ exitCode: number; stdout: string; stderr: string }> {
  const args = [
    "docker", "run", "--rm",
    "-v", `${config.workspaceDir}:/workspace`,
    "-w", "/workspace",
    "--memory", config.memoryLimit ?? "4g",
    "--cpus", String(config.cpuLimit ?? 2),
    "--network", config.networkMode ?? "none",
    config.image,
    ...command,
  ];

  const result = await $`${args}`.quiet();
  return {
    exitCode: result.exitCode,
    stdout: result.stdout.toString(),
    stderr: result.stderr.toString(),
  };
}
```

### Abstraction Layer

```typescript
// src/sandbox/index.ts
import type { Effect } from "effect";

interface SandboxBackend {
  name: string;
  available: () => Promise<boolean>;
  run: (command: string[], config: SandboxConfig) => Effect.Effect<RunResult, SandboxError>;
}

interface SandboxConfig {
  workspaceDir: string;
  allowNetwork: boolean;
  memoryLimit?: string;
  timeoutMs?: number;
}

// Auto-detect best available backend
async function detectBackend(): Promise<SandboxBackend> {
  // 1. Check for macOS 26 + Apple Container
  if (await appleContainerAvailable()) {
    return AppleContainerBackend;
  }

  // 2. Check for Seatbelt (macOS)
  if (process.platform === "darwin" && await seatbeltAvailable()) {
    return SeatbeltBackend;
  }

  // 3. Check for Docker
  if (await dockerAvailable()) {
    return DockerBackend;
  }

  // 4. No sandbox available
  return NoSandboxBackend;
}
```

---

## Decision Matrix

Use this to evaluate which path fits your needs:

| If you... | Consider... |
|-----------|-------------|
| Need sandboxing **now** and accept risk | Seatbelt (Path A) |
| Want proven, portable solution | Docker (Path B) |
| Can wait and want Apple-blessed solution | Apple Container (Path C) |
| Need to support diverse environments | Hybrid (Path D) |
| Have limited development resources | Docker (simplest maintenance) |
| Prioritize minimal dependencies | Seatbelt (native, no installs) |
| Target macOS 26+ only | Apple Container |

---

## Open Questions

1. **What's the minimum viable sandbox?** Do we need network egress control, or is filesystem isolation sufficient?

2. **Who are MechaCoder's users?** Developers who already have Docker? Non-technical users who need minimal setup?

3. **What's the deployment timeline?** Can we wait for macOS 26, or do we need security now?

4. **Is cross-platform important?** Should we support Linux, or is macOS-only acceptable?

5. **What about Claude Code's approach?** Should we align with however Claude Code handles sandboxing for consistency?

---

---

## Recommended Implementation: Apple Container

Since you're on macOS 26 and targeting developers, here's the concrete implementation plan:

### Phase 1: Setup & Validation (Day 1)

1. **Verify Apple Container is installed and working**:
   ```bash
   # Check if container CLI is available
   which container

   # Start the system service
   container system start

   # Test with a simple image
   container run --rm alpine echo "Hello from container"
   ```

2. **Create MechaCoder base image** (`Containerfile`):
   ```dockerfile
   FROM oven/bun:latest

   # Install git and common tools
   RUN apt-get update && apt-get install -y git curl

   # Set up workspace directory
   WORKDIR /workspace

   # Copy MechaCoder source (or mount at runtime)
   # COPY . /app

   ENTRYPOINT ["bun"]
   ```

3. **Build and test the image**:
   ```bash
   cd ~/code/openagents
   container build -t mechacoder .
   container run --rm -v $(pwd):/workspace mechacoder --version
   ```

### Phase 2: Integration Layer (Week 1)

Create `src/sandbox/apple-container.ts`:

```typescript
import { Effect, Layer } from "effect";
import { $ } from "bun";

export interface ContainerConfig {
  workspaceDir: string;
  image: string;
  memoryLimit?: string;  // e.g., "4g"
  cpuLimit?: number;     // e.g., 2
  networkEnabled?: boolean;
  timeout?: number;      // ms
}

export class ContainerSandbox extends Effect.Service<ContainerSandbox>()("ContainerSandbox", {
  effect: Effect.gen(function* () {
    // Check if container service is running
    const isAvailable = async (): Promise<boolean> => {
      try {
        const result = await $`container system status`.quiet();
        return result.exitCode === 0;
      } catch {
        return false;
      }
    };

    // Run command in container
    const run = (
      command: string[],
      config: ContainerConfig
    ): Effect.Effect<RunResult, ContainerError> =>
      Effect.tryPromise({
        try: async () => {
          const args = [
            "container", "run", "--rm",
            "-v", `${config.workspaceDir}:/workspace`,
            "-w", "/workspace",
          ];

          if (config.memoryLimit) {
            args.push("--memory", config.memoryLimit);
          }
          if (config.cpuLimit) {
            args.push("--cpus", String(config.cpuLimit));
          }
          if (!config.networkEnabled) {
            args.push("--network", "none");
          }

          args.push(config.image, ...command);

          const result = await $`${args}`
            .timeout(config.timeout ?? 300_000)
            .quiet();

          return {
            exitCode: result.exitCode,
            stdout: result.stdout.toString(),
            stderr: result.stderr.toString(),
          };
        },
        catch: (e) => new ContainerError({ cause: e }),
      });

    return { isAvailable, run };
  }),
}) {}
```

### Phase 3: MechaCoder Integration (Week 2)

Modify `src/agent/do-one-task.ts` to optionally run in container:

```typescript
// Add to .openagents/project.json schema
interface ProjectConfig {
  // ... existing fields ...
  sandbox?: {
    enabled: boolean;
    backend: "apple-container" | "docker" | "seatbelt" | "none";
    image?: string;
    memoryLimit?: string;
    networkEnabled?: boolean;
  };
}

// In agent execution
if (config.sandbox?.enabled && config.sandbox.backend === "apple-container") {
  const sandbox = yield* ContainerSandbox;

  // Run the agent loop inside the container
  const result = yield* sandbox.run(
    ["bun", "src/agent/do-one-task.ts", "--dir", "/workspace"],
    {
      workspaceDir: projectDir,
      image: config.sandbox.image ?? "mechacoder:latest",
      memoryLimit: config.sandbox.memoryLimit ?? "4g",
      networkEnabled: config.sandbox.networkEnabled ?? false,
    }
  );
}
```

### Phase 4: Network Policy (Week 3)

For LLM API access, you'll need selective network egress. Options:

1. **Network enabled + allowlist** (future Apple Container feature):
   ```typescript
   // When Apple Container supports network policies
   networkPolicy: {
     allowEgress: ["api.openai.com", "api.anthropic.com", "openrouter.ai"]
   }
   ```

2. **Workaround: Proxy pattern**:
   ```bash
   # Run a proxy on host that container can reach
   # Container connects to host gateway (192.168.64.1)
   # Proxy forwards to allowed domains only
   ```

3. **Split execution** (recommended for now):
   ```typescript
   // Run code editing/testing in container (no network)
   // Run LLM calls from host (has network)
   // Container only touches filesystem
   ```

### Configuration Example

`.openagents/project.json`:
```json
{
  "version": 1,
  "projectId": "openagents",
  "defaultBranch": "main",
  "testCommands": ["bun test"],
  "allowPush": true,
  "sandbox": {
    "enabled": true,
    "backend": "apple-container",
    "image": "mechacoder:latest",
    "memoryLimit": "8g",
    "networkEnabled": false
  }
}
```

### Known Limitations to Address

1. **Container-to-host networking**: Can't reach `localhost` services yet
   - Workaround: Use host gateway IP (192.168.64.1) with socat forwarding

2. **Memory ballooning**: Memory not released back to macOS
   - Mitigation: Set reasonable memoryLimit, restart containers periodically

3. **Cold start latency**: Container startup adds ~2-3 seconds
   - Consider: Keep a warm container pool for interactive use

### Testing Strategy

1. **Unit tests** for container abstraction layer
2. **Integration tests** with real Apple Container:
   ```bash
   bun test src/sandbox/apple-container.test.ts
   ```
3. **E2E test**: Run full Golden Loop in container:
   ```bash
   container run -v $(pwd):/workspace mechacoder \
     bun src/agent/do-one-task.ts --dir /workspace --dry-run
   ```

### Fallback Strategy

For users not on macOS 26 or without Apple Container:

```typescript
async function detectSandboxBackend(): Promise<SandboxBackend> {
  // 1. Try Apple Container (preferred)
  if (await appleContainerAvailable()) {
    return AppleContainerBackend;
  }

  // 2. Try Seatbelt (macOS fallback)
  if (process.platform === "darwin") {
    return SeatbeltBackend; // Accept deprecation risk
  }

  // 3. Try Docker
  if (await dockerAvailable()) {
    return DockerBackend;
  }

  // 4. No sandbox (warn user)
  console.warn("No sandbox available. Running with full privileges.");
  return NoSandboxBackend;
}
```

---

## Next Steps

1. [ ] Install Apple Container on your macOS 26 system
2. [ ] Verify `container run` works with a test image
3. [ ] Create `Containerfile` for MechaCoder
4. [ ] Build `mechacoder:latest` image
5. [ ] Implement `src/sandbox/apple-container.ts`
6. [ ] Add sandbox config to `ProjectConfig` schema
7. [ ] Test full agent loop in container
8. [ ] Address network access for LLM APIs

---

## References

- Codex container architecture: `/Users/christopherdavid/code/codex/docs/container-architecture.md`
- Codex Seatbelt policy: `/Users/christopherdavid/code/codex/codex-rs/core/src/seatbelt_base_policy.sbpl`
- Codex sandbox docs: `/Users/christopherdavid/code/codex/docs/sandbox.md`
- Apple Container README: `/Users/christopherdavid/code/container/README.md`
- Apple Container technical overview: `/Users/christopherdavid/code/container/docs/technical-overview.md`
- Hacker News discussion on macOS sandboxing: https://news.ycombinator.com/item?id=44283454
- Chrome Seatbelt policies: https://source.chromium.org/chromium/chromium/src/+/main:sandbox/policy/mac/
- Apple Containerization framework: https://github.com/apple/containerization

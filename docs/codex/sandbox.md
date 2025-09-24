# Sandbox System

## Overview

The OpenAI Codex CLI implements a sandboxing system to provide security boundaries when executing commands on behalf of the AI assistant. The sandbox prevents unauthorized file writes, blocks (or restricts) network access, and confines command execution to safe scopes. This page explains how it works, and how to explicitly disable it when your tests or tooling require full access.

## Architecture

### Core Components

1. **Platform-Specific Sandboxes**:
   - **macOS**: Uses Apple Seatbelt (`/usr/bin/sandbox-exec`) with custom policy files
   - **Linux**: Uses Landlock LSM + seccomp filtering via `codex-linux-sandbox` executable  
   - **Windows**: No sandbox support currently

2. **Policy Engine**: 
   - Located in `codex-rs/core/src/safety.rs`
   - Evaluates command safety and approval requirements
   - Manages policy enforcement and escalation

3. **Configuration System**:
   - Sandbox modes defined in `codex-rs/core/src/config_types.rs`
   - Runtime policy resolution in `codex-rs/core/src/config.rs`

### Sandbox Policies

The system defines three primary sandbox policies:

#### ReadOnly
- **Purpose**: Safe exploration and code analysis
- **Permissions**: Full filesystem read access, no write or network
- **Use Cases**: Code review, exploration, documentation generation
- **Implementation**: Blocks all write operations and network access

#### WorkspaceWrite
- **Purpose**: Active development within controlled boundaries
- **Permissions**: 
  - Full filesystem read access
  - Write access to current working directory and configured roots
  - Network access controlled by `network_access` flag
  - Automatic `.git` directory protection (read-only)
- **Configuration**:
  ```toml
  [sandbox_workspace_write]
  writable_roots = ["/custom/path"]
  network_access = false
  exclude_tmpdir_env_var = false  
  exclude_slash_tmp = false
  ```

#### DangerFullAccess
- **Purpose**: Unrestricted system access (discouraged)
- **Permissions**: No sandbox restrictions
- **Use Cases**: System administration, when sandbox interferes with required operations
- **Warning**: Should only be used in trusted environments

### Platform Implementation Details

#### macOS Seatbelt Implementation

Location: `codex-rs/core/src/seatbelt.rs`

**Key Features**:
- Uses Apple's native `sandbox-exec` with custom SBPL (Seatbelt Policy Language) policies
- Policy generation at runtime based on configuration
- Canonical path resolution to handle symlinks (e.g., `/var` vs `/private/var`)
- Dynamic parameter substitution for writable roots

**Policy Structure**:
```sbpl
; Base policy with system permissions
(version 1)
(deny default)
(allow process-exec (literal "/bin/sh"))

; Dynamic file write policies
(allow file-write* 
  (subpath (param "WRITABLE_ROOT_0"))
  (require-not (subpath (param "WRITABLE_ROOT_0_RO_0")))
)

; Network policy (conditional)
(allow network-outbound)
(allow network-inbound) 
(allow system-socket)
```

#### Linux Landlock Implementation

Location: `codex-rs/linux-sandbox/src/landlock.rs`

**Key Features**:
- Modern Linux LSM (Linux Security Module) approach
- Seccomp filtering for system call restrictions  
- JSON-based policy serialization to helper process
- Path-based access controls

**Helper Process**:
- Separate `codex-linux-sandbox` executable
- Receives JSON-serialized policy configuration
- Implements Landlock path permissions and seccomp filtering
- Spawns target command under restricted environment

### Command Execution Flow

1. **Command Input**: AI assistant requests command execution
2. **Safety Assessment**: `safety.rs` evaluates command against known safe lists
3. **Approval Check**: Policy engine determines if user approval required
4. **Sandbox Selection**: Platform-specific sandbox method chosen
5. **Policy Generation**: Runtime policy created based on configuration
6. **Spawning**: Command executed under sandbox via:
   - `spawn_command_under_seatbelt()` (macOS)
   - `spawn_command_under_linux_sandbox()` (Linux)
7. **Monitoring**: Output streaming and timeout enforcement
8. **Cleanup**: Process termination and resource cleanup

### Environment Variables

The sandbox system sets specific environment variables to inform child processes:

- `CODEX_SANDBOX`: Set to sandbox type ("seatbelt" on macOS) when a sandbox is used
- `CODEX_SANDBOX_NETWORK_DISABLED`: Set to "1" when network access is disabled by policy

## Disabling the Sandbox (for Tests)

Some test frameworks, build systems, or integration tests require unrestricted filesystem and/or network access. You can explicitly disable sandboxing without changing any code.

### Fastest option: fully bypass approvals and sandbox

- `--dangerously-bypass-approvals-and-sandbox` (alias: `--yolo`)
  - TUI: `codex --dangerously-bypass-approvals-and-sandbox "run my tests"`
  - Headless: `codex exec --dangerously-bypass-approvals-and-sandbox "run my tests"`
  - Effect: sets `approval_policy = never` and `sandbox_mode = danger-full-access`.
    Execution runs without seatbelt/landlock and with full network; no approval prompts.
  - Trust flow: skipped (overrides are present).

### Alternative: only disable the sandbox

- `--sandbox danger-full-access`
  - TUI: `codex --sandbox danger-full-access "run my tests"`
  - Headless: `codex exec --sandbox danger-full-access "run my tests"`
  - With the default approval policy (`on-request`), commands auto‑run unsandboxed.
  - If you explicitly use `--ask-for-approval unless-trusted`, untrusted commands may still prompt for approval even though the sandbox is disabled.

### Configuration profile recipe

Add a profile to `~/.codex/config.toml` and opt‑in per‑run:

```toml
[profiles.no-sandbox]
sandbox_mode = "danger-full-access"
# Optional: also bypass approvals entirely
# approval_policy = "never"
```

Usage:

```bash
codex -p no-sandbox "run tests"
codex exec -p no-sandbox "run tests"
```

### Behavior under `danger-full-access`

- Platform sandbox is not used (`SandboxType::None` execution path).
- `CODEX_SANDBOX=seatbelt` is not set (macOS Seatbelt runner is not invoked).
- `CODEX_SANDBOX_NETWORK_DISABLED` is not set; full network is available.
- `.git` protections enforced by sandbox policies do not apply (be cautious).

### Trust and onboarding

If you specify `--sandbox`, `--dangerously-bypass-approvals-and-sandbox`, or a config profile that sets either `sandbox_mode` or `approval_policy`, the TUI trust/onboarding screen is skipped.

### Recommendations and cautions

- Prefer using `danger-full-access` only in trusted environments (CI container, local dev machine).
- Keep the default sandbox in day‑to‑day usage; enable bypass only for tests that require it.
- Consider committing a dedicated CI profile (e.g., `-p no-sandbox`) rather than changing global defaults.

**Cons of Removal**:
- Reduces security posture
- AI has full system access
- Potential for unintended system modifications
- Loses protection against malicious or buggy AI commands

### Recommended Implementation

For your use case, **Option 1 (Configuration-Based Bypass)** is recommended:

1. Change default sandbox mode to `DangerFullAccess`
2. Add environment variable override: `CODEX_ENABLE_SANDBOX=true` to re-enable if needed
3. Update documentation to reflect security implications
4. Consider adding warning messages when sandbox is disabled

This approach maintains the sandbox code for users who need it while providing unrestricted access for automation workflows.

## Files to Modify

Key files that would need modification for sandbox removal:

1. `codex-rs/core/src/config.rs` - Default configuration
2. `codex-rs/core/src/config_types.rs` - Type definitions  
3. `codex-rs/core/src/exec.rs` - Execution logic
4. `codex-rs/core/src/safety.rs` - Safety policies
5. `codex-rs/cli/src/main.rs` - CLI argument parsing
6. `codex-rs/common/src/sandbox_mode_cli_arg.rs` - CLI sandbox options
7. Documentation files in `docs/` directory

The sandbox system represents a significant portion of the codebase's security architecture, so any removal should be carefully considered and implemented with appropriate safeguards for different deployment scenarios.

# Bash Tool Error Analysis - Issue #1

## Summary

Investigated high Bash tool error rate (25.0%, 14 failures in 56 calls). Root cause identified: autopilot agents in Docker containers attempting to run `cargo` compilation commands in a runtime-only environment that lacks build toolchain.

## Root Cause

The autopilot Docker container (`docker/autopilot/Dockerfile`) is intentionally designed as a **runtime-only environment**:

- Base image: `debian:trixie-slim`
- No Rust toolchain installed
- No C compiler (gcc/clang)
- No cargo or rustc binaries
- Uses **pre-built binaries** copied from host: `/usr/local/bin/autopilot`, `/usr/local/bin/issues-mcp`

## Error Pattern

All Bash failures follow the same pattern:

1. Agent attempts `cargo build` or `cargo run` command
2. Command fails with "linker `cc` not found" (no C compiler for build scripts)
3. Exit code: 101 (compilation failure)
4. Error type: NonZeroExit

Example from logs:
```
error: linker `cc` not found
  |
  = note: No such file or directory (os error 2)

error: could not compile `proc-macro2` (build script) due to 1 previous error
```

## Why This Happens

The autopilot agent receives the task "Process issues from database" and attempts to:

1. Access issue management system
2. Encounters missing issue_ready tool or wants to compile code
3. Tries `cargo autopilot issue ready` or `cargo build -p issues-mcp`
4. Fails because cargo requires C compiler for proc-macro build scripts

## Solution

**Already implemented and working**: Use pre-built binaries directly instead of cargo commands.

### Correct Usage

```bash
# ✓ CORRECT - Use pre-built binary
/usr/local/bin/autopilot issue ready
/usr/local/bin/autopilot issue claim 1
/usr/local/bin/autopilot issue complete 1

# ✗ WRONG - Attempts compilation
cargo autopilot issue ready
cargo run -p autopilot -- issue ready
```

### Binary Availability

Pre-built binaries in the container:
- `/usr/local/bin/autopilot` - Main autopilot binary (105MB)
- `/usr/local/bin/issues-mcp` - Issue management MCP server

These are built on the host with `cargo build --release` and copied into the image.

## Recommendations

### 1. Agent Prompt Clarification

Add to autopilot system prompt in containerized mode:

```
CRITICAL: You are running in a runtime-only Docker container with NO build tools.

- NEVER use `cargo` commands
- ALWAYS use pre-built binaries directly:
  - /usr/local/bin/autopilot
  - /usr/local/bin/issues-mcp

If you need to compile code, this is a fundamental architectural limitation
and the task cannot be completed in this environment.
```

### 2. Container Build Mode (Optional Future Enhancement)

For tasks requiring compilation, consider:

- Separate "dev" container with full Rust toolchain
- Multi-stage Docker build copying compiled artifacts
- Host-side compilation with mounted target directory

### 3. Error Detection & Recovery

Add logic to detect and prevent cargo command attempts:

```rust
// In autopilot command validation
if command.starts_with("cargo") && !has_rust_toolchain() {
    return Err("Cannot run cargo in runtime-only container. Use pre-built binaries at /usr/local/bin/");
}
```

## Impact

This issue does NOT indicate bugs in the code or tooling. It's an expected behavior mismatch:

- **Container design**: Runtime-only, minimal dependencies, fast startup
- **Agent behavior**: Attempts standard development workflows (compilation, testing)

The 25% error rate will remain high as long as agents attempt cargo commands. The fix is behavioral (prompt engineering) rather than code changes.

## Verification

Confirmed working approach in this session:
```bash
$ /usr/local/bin/autopilot issue ready
→ Next ready issue:
  Number:   #1
  Title:    Bash tool has 25.0% error rate (14 failures in 56 calls)
  ...

$ /usr/local/bin/autopilot issue claim 1
✓ Claimed issue #1
```

No compilation required, zero Bash errors.

## Related

- d-018: Parallel Autopilot Container Isolation - Container architecture decisions
- d-012: No Stubs - Ensures pre-built binaries are production-ready
- Docker file: `docker/autopilot/Dockerfile`
- Entrypoint: `autopilot run --full-auto --with-issues`

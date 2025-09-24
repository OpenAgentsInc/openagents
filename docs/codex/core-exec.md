# Core: Exec and Sandboxing

Location: `codex-rs/core/src/exec.rs`

Runs shell commands on behalf of the agent with platform‑specific sandboxing,
streaming, truncation, and error normalization.

## ExecParams

```rust
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: HashMap<String, String>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}
```

- `timeout_duration()` resolves per‑call timeouts with a conservative default.

## SandboxType

- `None` — direct spawn.
- `MacosSeatbelt` — invokes `spawn_command_under_seatbelt` (see sandbox docs).
- `LinuxSeccomp` — invokes `spawn_command_under_linux_sandbox` helper.

## process_exec_tool_call

- Selects the spawn path for the sandbox type.
- Streams stdout in bounded deltas (see `MAX_EXEC_OUTPUT_DELTAS_PER_CALL`).
- Collects full stdout/stderr for final payload.
- Detects timeouts and maps exit codes/signals to `SandboxErr` when relevant.

## Denial heuristics

- `is_likely_sandbox_denied` — conservatively flags non‑`None` sandboxes except
  for well‑known shell exit 127 (“command not found”).

## Output handling

- Uses lossy UTF‑8 conversion for display channels while preserving raw bytes
  internally for blob hashing where needed.

See also: `docs/systems/sandbox.md` for policy/Seatbelt/Landlock details.


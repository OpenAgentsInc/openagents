# Core: Safety and Trusted Commands

File: `codex-rs/core/src/safety.rs`

Applies high‑level safety policy before executing commands, chooses a platform
sandbox, and determines approval/auto‑approval behavior.

## Highlights

- `get_platform_sandbox()` picks `MacosSeatbelt` or `LinuxSeccomp` at runtime.
- Trusted command set gates `UnlessTrusted` policy.
- Auto‑approval may be granted in specific flows but still honors sandbox.
- Error mapping preserves whether a failure likely originated from sandboxing.


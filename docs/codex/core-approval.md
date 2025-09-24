# Core: Approval Policy

Determines when the agent asks the user before running commands, and how
escalation works.

Files:
- `codex-rs/core/src/config.rs` (Config.approval_policy)
- `codex-rs/core/src/safety.rs` (policy, trusted commands, platform sandbox)

## Modes

- `AskForApproval::OnRequest` — default prompt on untrusted commands.
- `AskForApproval::UnlessTrusted` — auto‑run trusted commands, prompt otherwise.
- `AskForApproval::Never` — run without asking; intended for CI/headless.

See `docs/systems/sandbox.md` for how approvals interact with sandbox selection
and bypass flags.


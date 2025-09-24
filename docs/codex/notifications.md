# Notifications

Configuration options to surface turn completions and approvals to the user via
system notifications or TUI OSC 9 notifications.

File: `codex-rs/core/src/config.rs`

## External notifier

- `notify = ["notify-send", "Codex"]` in `config.toml` (see docs/config.md).
- On turn complete, Codex spawns the command and appends one JSON argument with
  structured metadata about the turn.

## TUI notifications

- `tui_notifications` in config controls whether the TUI sends OSC 9
  notifications when the terminal is not focused.


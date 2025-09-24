# Rate Limits and Usage Snapshots

Explains how Codex surfaces live rate limit information and token accounting so
users can understand when requests are being throttled.

Files:
- `codex-rs/core/src/client.rs` (header parsing and event emission)
- `codex-rs/protocol/src/protocol.rs` (`RateLimitSnapshotEvent`)

## Headers

- `x-codex-primary-used-percent`
- `x-codex-secondary-used-percent`
- `x-codex-primary-over-secondary-limit-percent`
- `x-codex-primary-window-minutes`
- `x-codex-secondary-window-minutes`

These are parsed by `parse_rate_limit_snapshot` and sent as a
`ResponseEvent::RateLimits` before streaming begins.

## Usage in UI

- The TUI can display a small status bar or notifications when usage crosses
  thresholds or when windows reset.


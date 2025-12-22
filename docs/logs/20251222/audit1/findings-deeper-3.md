# Deeper Findings (Pass 3)

## Medium
- D3-M-1 Unified CLI subcommands are partially stubbed and point to outdated binaries. Several commands print "coming soon" or suggest `cargo autopilot` / `cargo daemon`, which is not valid for the unified `openagents` binary. Evidence: `src/cli/autopilot.rs:129`, `src/cli/autopilot.rs:152`, `src/cli/daemon.rs:74`, `src/cli/marketplace.rs:44`, `src/cli/agentgit.rs:24`.
- D3-M-2 `/api/claude/status` holds a `claude_info` read lock across `fetch_usage_limits().await`. The network call can block writers and violates the async best practice of not holding locks across awaits. Evidence: `src/gui/routes/claude.rs:15`, `src/gui/routes/claude.rs:43`.
- D3-M-3 Claude status polling refreshes every 5 seconds and each request performs a live OAuth usage API call with no caching/backoff, which risks rate limits and unnecessary load. Evidence: `crates/ui/src/claude_status.rs:241`, `src/gui/routes/claude.rs:41`.

## Low
- D3-L-1 Wallet/Marketplace/AgentGit/Daemon GUI routes return static "Coming soon" pages, so the unified desktop tabs are placeholders without real wiring. Evidence: `src/gui/routes/wallet.rs:13`, `src/gui/routes/marketplace.rs:14`, `src/gui/routes/agentgit.rs:12`, `src/gui/routes/daemon.rs:9`.
- D3-L-2 Daemon config file parse errors are silently ignored via `unwrap_or_default`, which can mask misconfiguration. Evidence: `src/cli/daemon.rs:51`.
- D3-L-3 Storybook hot-reload uses `unwrap()` on `actix_ws::handle`, which will panic on handshake errors instead of returning an error response. Evidence: `crates/storybook/src/main.rs:145`.

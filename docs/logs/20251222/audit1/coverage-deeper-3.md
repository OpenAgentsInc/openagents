# Deeper Audit Coverage (Pass 3)

Additional systems reviewed in this pass:
- Unified CLI wrappers: `src/cli/autopilot.rs`, `src/cli/daemon.rs`, `src/cli/marketplace.rs`, `src/cli/agentgit.rs`
- Unified GUI non-autopilot routes: `src/gui/routes/claude.rs`, `src/gui/routes/daemon.rs`, `src/gui/routes/marketplace.rs`, `src/gui/routes/wallet.rs`, `src/gui/routes/agentgit.rs`
- Unified GUI views + Claude status component: `src/gui/views/mod.rs`, `crates/ui/src/claude_status.rs`
- Storybook dev server: `crates/storybook/src/main.rs`

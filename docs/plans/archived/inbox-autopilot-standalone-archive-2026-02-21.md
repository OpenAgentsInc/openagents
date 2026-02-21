# Inbox Autopilot Standalone Archive

Status: Archived  
Date: 2026-02-21  
Roadmap issue: OA-RUST-052

## Summary

The standalone `apps/inbox-autopilot/` root was removed after inbox domain + pane migration into `apps/autopilot-desktop/`.

## Why Removed

1. Inbox classification/draft/approval domain logic moved to shared Rust crate `crates/autopilot-inbox-domain/`.
2. Inbox UI workflows moved into WGPUI desktop panes in `crates/autopilot_ui/` and `apps/autopilot-desktop/`.
3. Codex + inbox now run on a unified desktop command/event lane and shared route state.

## Historical Source Recovery

Historical files from the removed app remain recoverable through git history for:

1. `apps/inbox-autopilot/README.md`
2. `apps/inbox-autopilot/daemon/`
3. `apps/inbox-autopilot/docs/`
4. `apps/inbox-autopilot/Inbox Autopilot/` (Swift/Xcode host)

Use git history at or before the OA-RUST-052 removal commit to inspect prior implementation details.

## Active Replacement

Use these active paths instead:

1. `apps/autopilot-desktop/src/inbox_domain.rs`
2. `crates/autopilot-inbox-domain/`
3. `crates/autopilot_ui/src/lib.rs` (Inbox pane system)
4. `apps/autopilot-desktop/docs/migration/INBOX_AUTOPILOT_DOMAIN_MIGRATION.md`

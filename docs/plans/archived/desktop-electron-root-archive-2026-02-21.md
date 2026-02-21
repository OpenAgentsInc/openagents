# Desktop Electron Root Archive

Status: Archived  
Date: 2026-02-21  
Roadmap issue: OA-RUST-053

## Summary

The legacy Electron desktop root `apps/desktop/` was removed after capability audit and migration/de-scope decisions.

## Migration/De-scope Outcome

1. Codex + inbox desktop UX is served by Rust/WGPUI in `apps/autopilot-desktop/`.
2. Legacy Effuse/Electron pane runtime was retired.
3. Legacy local-node/L402 desktop-specific lanes were explicitly de-scoped to Lightning service lanes and tracked by OA-RUST-101/102/103.

## Historical Recovery

Full legacy implementation remains available via git history for:

1. `apps/desktop/src/`
2. `apps/desktop/tests/`
3. `apps/desktop/scripts/`
4. `apps/desktop/lnd/`

## Active Replacements

1. `apps/autopilot-desktop/`
2. `crates/autopilot_ui/`
3. `crates/wgpui/`
4. `apps/lightning-ops/`
5. `apps/lightning-wallet-executor/`
6. `docs/audits/OA-RUST-053-DESKTOP-LEGACY-CAPABILITY-AUDIT-2026-02-21.md`

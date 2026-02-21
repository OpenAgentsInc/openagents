# OA-RUST-053 Legacy Desktop Capability Audit

Status: Completed  
Date: 2026-02-21  
Issue: OA-RUST-053

## Scope

Audit remaining capabilities in legacy `apps/desktop` (Electron + Effect) and decide migrate vs de-scope before root deletion.

## Capability Matrix

| Legacy capability (`apps/desktop`) | Evidence | Disposition | Replacement / owner lane | Status |
| --- | --- | --- | --- | --- |
| Codex desktop UI shell and pane system | `apps/desktop/src/renderer.ts`, `apps/desktop/src/effect/paneLayout.ts` | Migrated | Rust/WGPUI desktop shell in `apps/autopilot-desktop` + `crates/autopilot_ui` (`owner:desktop`) | Complete |
| Desktop auth/session linkage to OpenAgents | `apps/desktop/src/effect/authGateway.ts` | Migrated | WorkOS email-code + desktop runtime auth in `apps/autopilot-desktop/src/runtime_auth.rs` (`owner:desktop`) | Complete |
| Codex worker stream integration | `apps/desktop/src/effect/runtime.ts` | Migrated | Runtime/Codex stream integration in `apps/autopilot-desktop/src/main.rs` (`owner:desktop`) | Complete |
| Inbox workflows | N/A in Electron root; previously split standalone | Migrated | Inbox domain + panes in `crates/autopilot-inbox-domain`, `apps/autopilot-desktop`, `crates/autopilot_ui` (`owner:desktop`) | Complete |
| Local LND runtime manager and wallet orchestration | `apps/desktop/src/main/lndRuntimeManager.ts`, `apps/desktop/src/main/lndWalletManager.ts` | De-scoped from desktop app root | Rust lightning services (`apps/lightning-ops`, `apps/lightning-wallet-executor`) tracked by OA-RUST-101/102 (`owner:infra`) | Accepted de-scope |
| Effect-based L402 executor loop and related smoke tests | `apps/desktop/src/effect/l402Executor.ts`, `apps/desktop/scripts/test-l402-local-node-smoke.mjs` | De-scoped from legacy Electron root | Service-owned execution lanes (lightning services) + future Rust-native desktop integrations (`owner:infra`, `owner:desktop`) | Accepted de-scope |
| Effuse runtime dependencies (`@openagentsinc/effuse*`) | `apps/desktop/package.json` | Retired with root | WGPUI Rust crates (`crates/wgpui`, `crates/autopilot_ui`) | Complete |

## Decision

1. No remaining must-keep capability requires the legacy `apps/desktop` root.
2. Required user-facing functionality (Codex + inbox desktop workflows) is available in `apps/autopilot-desktop`.
3. Legacy Electron desktop root is removed; remaining Lightning-specific desktop behavior is explicitly de-scoped to service lanes and tracked by dedicated OA-RUST issues.

## Verification Notes

1. Build check after removal:
   - `cargo check -p autopilot-desktop`
   - `cargo check -p autopilot_ui`
2. Canonical docs updated to remove `apps/desktop` as active surface.
3. Repo-wide reference scans now treat `apps/desktop` mentions as historical/plan context only.

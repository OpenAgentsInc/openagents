# Autopilot iOS Docs

- **`BUILDING.md`** — Building the app: XCFramework, Rust artifact script, fixing "no XCFramework found", for developers and coding agents.
- **`WGPUI-IOS.md`** — WGPUI on iOS: where code lives, FFI, Swift bridge, what’s rendered, how to change it; reference for coding agents.
- `../../../docs/autopilot/wgpui-ios-cutover-log.md` — WGPUI iOS background cutover implementation log (2026-02-21).
- `ios-codex-first-structure.md` — proposed iOS app structure for Codex-first delivery.
- `codex-connection-roadmap.md` — comprehensive roadmap for connecting iOS to Codex across runtime/desktop/hosted lanes.
- `real-device-codex-handshake-runbook.md` — operator runbook for validating real-device iOS handshake against desktop Codex sync.
- `rust-client-core-integration.md` — deterministic Rust client-core packaging, FFI contract, and CI verification for iOS host integration.

Current iOS default UX target:

- No manual endpoint configuration (defaults to `https://openagents.com`).
- No manual bearer token paste (email-code sign-in issues mobile API token automatically).

Canonical cross-system references:

- `docs/codex/unified-runtime-desktop-plan.md`
- `docs/codex/webapp-sandbox-and-codex-auth-plan.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `proto/README.md`

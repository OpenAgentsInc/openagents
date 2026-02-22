# Autopilot iOS Docs

- **`BUILDING.md`** — Building the app: XCFramework, Rust artifact script, fixing "no XCFramework found", for developers and coding agents.
- **`WGPUI-IOS.md`** — WGPUI on iOS: where code lives, FFI, Swift bridge, what’s rendered, how to change it; reference for coding agents.
- **`IOS-BLACK-SCREEN-TESTFLIGHT.md`** — Black screen on TestFlight (works in dev): architecture, what was tried, root cause analysis, next steps.
- `../../../docs/autopilot/wgpui-ios-cutover-log.md` — WGPUI iOS background cutover implementation log (2026-02-21).
- `ios-codex-first-structure.md` — iOS module boundaries and control/runtime authority rules.
- `codex-connection-roadmap.md` — phased iOS Codex roadmap over Rust control/runtime + Khala.
- `real-device-codex-handshake-runbook.md` — real-device handshake validation between iOS and desktop Codex lanes.
- `rust-client-core-integration.md` — deterministic Rust client-core packaging, FFI contract, and CI verification for iOS host integration.
- `wgpui-codex-ownership-boundaries.md` — current production ownership split for iOS Codex (WGPUI surface, Rust-authoritative lanes, host-adapter boundaries).

Current iOS default UX target:

- No manual endpoint configuration (defaults to `https://openagents.com`).
- No manual bearer token paste (email-code sign-in issues mobile API token automatically).

Canonical cross-system references:

- `docs/codex/unified-runtime-desktop-plan.md`
- `docs/codex/webapp-sandbox-and-codex-auth-plan.md`
- `apps/runtime/docs/RUNTIME_CONTRACT.md`
- `proto/README.md`

# Autopilot iOS Docs

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

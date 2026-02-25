# Autopilot Desktop

Native Rust/WGPUI desktop app for Codex runtime workflows.

## Inbox Domain Integration

Inbox mailbox policy/draft/audit domain logic now lives in shared crate:

- `crates/autopilot-inbox-domain`

Desktop consumes this domain crate directly via:

- `apps/autopilot-desktop/src/inbox_domain.rs`

Migration mapping details:

- `apps/autopilot-desktop/docs/migration/INBOX_AUTOPILOT_DOMAIN_MIGRATION.md`

## WorkOS Email-Code Auth (Runtime Sync)

Desktop runtime sync now uses the same email-code auth flow as iOS.
Login requests call `/api/auth/email` + `/api/auth/verify` with `X-Client: autopilot-desktop`, then runtime worker streams use that bearer token as the authenticated user.
Control service keeps a temporary compatibility alias for legacy `X-Client: openagents-expo` through **June 30, 2026**.

No restart is required after sign-in.

### Runtime Login Pane (Recommended)

1. Launch desktop: `cargo run -p autopilot-desktop`
2. Open the auth pane from hotbar slot `AU` (`Auth`).
3. Enter your email and click `Send code`.
4. Enter the verification code and click `Verify`.
5. Use `Refresh` to inspect auth status or `Logout` to clear auth.

Auth state is persisted at:

- `~/.openagents/autopilot-desktop-runtime-auth.json`

The app automatically uses this saved auth token for runtime worker sync (`/api/runtime/codex/workers*`) unless explicit `OPENAGENTS_RUNTIME_SYNC_*` env overrides are set.

Endpoint resolution (local-first):

- Runtime sync/auth base defaults to `http://127.0.0.1:8787`.
- Override order:
  - `OPENAGENTS_RUNTIME_SYNC_BASE_URL` (runtime-specific)
  - `OPENAGENTS_CONTROL_BASE_URL` (shared control base)
  - `OPENAGENTS_AUTH_BASE_URL` (legacy alias)
  - saved auth-state base URL
  - local default
- CommunityFeed proxy defaults to `http://127.0.0.1:8787/api/communityfeed/api`.
- CommunityFeed overrides:
  - `OPENAGENTS_COMMUNITYFEED_API_BASE`
  - `OA_COMMUNITYFEED_API_BASE` (legacy alias)
  - `OA_API` (appends `/communityfeed/api`)
  - `COMMUNITYFEED_API_BASE` for direct CommunityFeed live API.

### CLI Fallback (Automation / Headless)

Login once via CLI:

```bash
cd /Users/christopherdavid/code/openagents
cargo run -p autopilot-desktop -- auth login --email "<you@domain.com>"
```

Status:

```bash
cargo run -p autopilot-desktop -- auth status
```

Logout:

```bash
cargo run -p autopilot-desktop -- auth logout
```

## Run App

```bash
cd /Users/christopherdavid/code/openagents
cargo run -p autopilot-desktop
```

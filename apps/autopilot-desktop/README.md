# Autopilot Desktop

Native Rust/WGPUI desktop app for Codex runtime workflows.

## WorkOS Email-Code Auth (Runtime Sync)

Desktop runtime sync now uses the same email-code auth flow as iOS.
Login requests call `/api/auth/email` + `/api/auth/verify` with `X-Client: autopilot-desktop`, then runtime worker streams use that bearer token as the authenticated user.

Login once:

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

Auth state is persisted at:

- `~/.openagents/autopilot-desktop-runtime-auth.json`

The app automatically uses this saved auth token for runtime worker sync (`/api/runtime/codex/workers*`) unless explicit `OPENAGENTS_RUNTIME_SYNC_*` env overrides are set.

## Run App

```bash
cd /Users/christopherdavid/code/openagents
cargo run -p autopilot-desktop
```

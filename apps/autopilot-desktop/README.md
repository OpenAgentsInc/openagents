# Autopilot Desktop

Native Rust/WGPUI desktop app for Codex runtime workflows.

## Source Layout

Desktop domain responsibilities are now split by module under `apps/autopilot-desktop/src/`:

- `main.rs`: app bootstrap, window/event-loop wiring, pane orchestration.
- `runtime_auth.rs`: runtime auth persistence and login flow.
- `runtime_codex_proto.rs`: runtime Codex protocol parse/emit helpers.
- `codex_control.rs`: remote control auto-response decisions.
- `identity_domain.rs`: local NIP-06 identity load/init and config normalization.
- `provider_domain.rs`: in-process NIP-90 provider lifecycle + DVM status/history.
- `wallet_domain.rs`: Spark wallet status, invoice, and payment execution.
- `inbox_domain.rs`: inbox bridge to shared domain crate.

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

## Spark Wallet Pane (Local Breez SDK)

Desktop includes a native wallet pane (`WL`) backed by `crates/spark` and the Rust Breez SDK binding.

What it supports:

1. Local balance visibility (Spark/Lightning/On-chain + total sats).
2. Connectivity status from wallet sync checks.
3. Receive flow: create a Lightning/Spark invoice for a sats amount and copy the last invoice.
4. Send flow: pay a pasted BOLT11 invoice or Spark request (optional amount for zero-amount requests).
5. Recent payment history (direction, amount, status, id/timestamp).

Implementation notes:

1. Wallet identity is derived from the local Pylon mnemonic at `~/.openagents/pylon/identity.mnemonic`.
2. Wallet local storage is `~/.openagents/pylon/spark`.
3. If identity is missing, initialize Pylon identity first (`Pylon` pane or `pylon init`).

## Run App

```bash
cd /Users/christopherdavid/code/openagents
cargo run -p autopilot-desktop
```

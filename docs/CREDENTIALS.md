# Credentials In Autopilot Desktop

This document explains how credentials work in `apps/autopilot-desktop`: how they are stored, how users manage them in the UI, and how they are injected into runtime services like Codex and Spark.

## Why This Exists

Autopilot needs API keys and environment variables for different integrations (for example Codex, Spark, and skills like Blink). The credentials system provides one in-app place to manage these values without requiring users to edit shell profiles or repository files.

## Scope

This doc covers the credentials system used by Autopilot Desktop only:

- UI pane: `PaneKind::Credentials`
- State + storage logic: `apps/autopilot-desktop/src/credentials.rs` and `apps/autopilot-desktop/src/app_state.rs`
- Runtime wiring: `sync_credentials_runtime(...)` into Codex and Spark lanes

## Key Concepts

A credential entry has:

- `name`: env-var style key (for example `BLINK_API_KEY`)
- `enabled`: whether it is active for runtime injection
- `secret`: whether it should be treated as sensitive metadata
- `template`: whether it is a built-in slot or user-created
- `scopes`: bitset controlling which runtimes receive it
- `has_value`: whether secure storage (or fallback env) currently has a non-empty value

Only entries that are both `enabled` and in-scope are injected into runtimes.

## Storage Model

### 1) Non-secret metadata file

Credential metadata (name, enabled flag, scopes, etc.) is stored in:

- `~/.openagents/autopilot-credentials-v1.conf`

This file does **not** store secret values.

### 2) Secret values in OS keychain/keyring

Credential values are stored through the Rust `keyring` crate under service:

- `com.openagents.autopilot.credentials`

Per-entry keyring account name is the credential `name` (for example `BLINK_API_KEY`).

### 3) Environment fallback

When reading a credential value, the system falls back to process environment variables (`std::env::var`) if keyring has no value or unusable value.

## Value Resolution Rules (Important)

For each credential name:

1. Read process env fallback value (`ENV_NAME`) if present and non-empty.
2. Try keyring value.
3. If keyring has a non-empty value, use keyring.
4. If keyring has no value/empty value, use env fallback.
5. If keyring read errors but env fallback exists, use env fallback.
6. If neither has usable value, credential is treated as unset.

Practical implication: keyring is preferred, but env still works as a fallback path.

## Name Validation And Normalization

Credential names are normalized to uppercase and must match:

- `[A-Z_][A-Z0-9_]*`

Invalid names are rejected.

Custom entries infer `secret=true` if the name:

- ends with `_KEY` or `_TOKEN`, or
- contains `SECRET` or `PASSWORD`

## Built-in Template Slots

Autopilot ships with template entries:

- `OPENAI_API_KEY`
- `OPENAI_ACCESS_TOKEN`
- `OPENAI_CHATGPT_ACCOUNT_ID`
- `OPENAI_CHATGPT_PLAN_TYPE`
- `OPENAGENTS_SPARK_API_KEY`
- `BLINK_API_KEY`
- `BLINK_API_URL`

Templates cannot be removed; deleting them clears value only.

## Scope Model

Each credential can be assigned to one or more scopes:

- `CODEX`
- `SPARK`
- `SKILLS`
- `GLOBAL`

Implementation bits:

- `CREDENTIAL_SCOPE_CODEX`
- `CREDENTIAL_SCOPE_SPARK`
- `CREDENTIAL_SCOPE_SKILLS`
- `CREDENTIAL_SCOPE_GLOBAL`

### Effective runtime mapping

When syncing runtime environments:

- Codex receives entries scoped to `CODEX` or `SKILLS` or `GLOBAL`
- Spark receives entries scoped to `SPARK` or `GLOBAL`

## Startup Behavior

On app startup:

1. Credentials are loaded from disk (`CredentialsState::load_from_disk()`).
2. Template slots are merged in if missing.
3. `has_value` is computed by trying to resolve each value.
4. `RenderState` calls `sync_credentials_runtime(false)`.

`restart_codex=false` during startup, so env is loaded into config without forced lane restart at that moment.

## Runtime Injection

### Codex lane

Credential pairs are resolved and assigned to:

- `state.codex_lane_config.env`

Codex lane process spawn passes this env vector into app-server via:

- `AppServerConfig { env: config.env.clone(), ... }`

If a credentials action requests restart and env changed, Autopilot restarts Codex lane.

### Spark worker

Credential pairs are sent through:

- `SparkWalletCommand::ConfigureEnv { vars }`

Spark worker stores these as `env_overrides` and then receives a `Refresh` command.

Current Spark credential use from overrides:

- `OPENAGENTS_SPARK_API_KEY`

Spark network selection currently comes from process env (`OPENAGENTS_SPARK_NETWORK`) at startup.

## Credentials Pane Actions And Effects

From the Credentials pane, users can:

- `Add custom`: create new slot (metadata persisted)
- `Save value`: store value in keyring for selected slot
- `Clear value` / `Delete slot`:
  - template slot: clear keyring value only
  - custom slot: remove slot metadata + clear keyring value
- `Enable/Disable slot`
- `Import env`: copy matching process env vars into keyring
- `Reload`: reload metadata from disk
- Toggle scopes (`Codex`, `Spark`, `Skills`, `Global`)
- Select row

Actions that can trigger runtime resync include add/save/delete/toggle/import/reload.
Most credential-mutating actions request Codex restart if effective env changed.

## Import Env: What It Actually Does

`Import env` imports values already present in the **running app process environment**.
It does not parse `.env.local` automatically.

So if you want import to pick up values, launch the app with those env vars set first.

Example:

```bash
export BLINK_API_KEY="blink_..."
cargo autopilot
```

Then use `Import env` in the Credentials pane.

## Security Properties

What is protected:

- Secret values are stored via OS keyring service, not plaintext metadata file.

What is still visible:

- Credential names, enabled flags, and scopes are stored in plaintext metadata file.
- Process environment fallback can still supply secrets if set externally.

Operational recommendations:

- Prefer storing actual values via `Save value` (keyring path).
- Avoid putting long-lived secrets in repo-tracked files.
- Use narrow scopes (do not enable `Global` unless needed).
- Disable unused slots instead of leaving active credentials broad-scoped.

## Error Handling And Diagnostics

Common error classes:

- Invalid name format
- Keyring unavailable / keyring write/read failure
- Metadata parse or schema mismatch
- Runtime injection failures (Codex lane restart/config, Spark worker enqueue)

When an action fails, Autopilot:

- sets pane `load_state` to error
- stores `last_error` for UI display
- emits error logs to stderr in action handlers

## Typical User Workflow (Non-Technical)

1. Open command palette (`K`) and open `Credentials` pane.
2. Select a slot (for example `BLINK_API_KEY`) or add a custom slot.
3. Paste key into `Value` and click `Save value`.
4. Set scopes:
   - skills/Codex-integrations: enable `Skills` (and `Codex` if needed)
   - Spark API key: enable `Spark`
5. Leave `Global` off unless multiple runtimes truly need it.
6. Use the integration; Autopilot will resync/restart runtimes when required.

## Developer Notes

Core files:

- `apps/autopilot-desktop/src/credentials.rs`
- `apps/autopilot-desktop/src/app_state.rs` (`CredentialsState`, `sync_credentials_runtime`)
- `apps/autopilot-desktop/src/input.rs` (`run_credentials_action`)
- `apps/autopilot-desktop/src/codex_lane.rs` (AppServer env wiring)
- `apps/autopilot-desktop/src/spark_wallet.rs` (Spark env overrides)

If adding a new first-class integration:

1. Add template slot(s) to `CREDENTIAL_TEMPLATES` if appropriate.
2. Assign minimal scope defaults.
3. Ensure runtime lane consumes env from scope resolver.
4. Update this document and `.env.example` if there is a user-facing key.

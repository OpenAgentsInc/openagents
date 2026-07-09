# Codex Multi-Account Auth On Managed VMs

Status: SHC MVP runbook

This note defines how OpenAgents Cloud should hold multiple user-connected
ChatGPT/Codex accounts on one trusted managed VM.

## Recommendation

Do not allocate one VM per ChatGPT account by default. Use one persistent,
account-scoped Codex home per connected provider account:

```text
/home/ubuntu/.openagents-codex-accounts/
  provider-account_a/
    auth.json
    config.toml
  provider-account_b/
    auth.json
    config.toml
```

Run Codex with the selected account home:

```bash
CODEX_HOME=/home/ubuntu/.openagents-codex-accounts/provider-account_a codex login status
CODEX_HOME=/home/ubuntu/.openagents-codex-accounts/provider-account_a codex exec ...
```

Codex file-backed credentials live in `$CODEX_HOME/auth.json`; the default
`~/.codex` path is a single active account home. Logging in a second account in
that same home can overwrite the first account. Official Codex docs also note
that `cli_auth_credentials_store = "file"` stores credentials under
`CODEX_HOME`, and that trusted automation can keep ChatGPT-managed
`auth.json` refreshed by running Codex normally and preserving the updated file.

## Account Connection Flow

1. Vortex creates or selects a provider account record.
2. Vortex asks the Cloud broker to start a Codex device-code login for that
   provider account.
3. The broker runs:

   ```bash
   install -d -m 700 /home/ubuntu/.openagents-codex-accounts/provider-account_...
   CODEX_HOME=/home/ubuntu/.openagents-codex-accounts/provider-account_... \
     codex login --device-auth
   ```

   The selected `CODEX_HOME` is not optional. If the broker logs in under the
   VM user's default `/home/ubuntu/.codex` and the control daemon later maps
   `codex-auth://provider-account_...` to the account-scoped home, Vortex will
   mark the account connected while the workroom still reads stale credentials.
   Copying the default `auth.json` into the account home is only an emergency
   repair; normal login must happen directly in the provider-account home.

4. The user completes the login in ChatGPT/OpenAI.
5. The broker verifies:

   ```bash
   CODEX_HOME=/home/ubuntu/.openagents-codex-accounts/provider-account_... \
     codex login status
   ```

6. Vortex records only a provider-account ref and server-side secret ref such
   as `codex-auth://provider-account_...`. It never stores or displays raw
   `auth.json`.

## Run Flow

For each workroom run:

1. Vortex issues a short-lived auth grant for the chosen provider account.
2. `oa-codex-control` resolves the grant through Vortex.
3. The control daemon maps `codex-auth://provider-account_...` to:

   ```text
   $OA_CODEX_AUTH_JSON_ROOT/provider-account_.../auth.json
   ```

4. `oa-workroomd` copies that cache into a session-scoped `CODEX_HOME`, runs
   `codex login status`, runs `codex exec`, and scrubs the session home.
5. The persistent account home remains so Codex can keep refreshing it across
   future runs.

## Rotation

Rotation is a scheduler decision, not a Codex CLI feature.

Vortex should track each connected account with:

```text
provider_account_ref
email or label
plan type
status: connected | stale | revoked | rate_limited | disabled
last_verified_at
last_successful_run_at
last_failure_reason
quota/rate-limit observations when available
```

The runner should choose among eligible accounts by policy:

```text
preferred user account
same org/project account
least recently used healthy account
manual operator-selected fallback
```

Do not run the same `auth.json` concurrently from multiple machines or
independent job streams. If one account needs parallelism, serialize work per
account home or create explicit account-slot concurrency policy after measuring
Codex behavior.

## Refresh And Reauth

For trusted private runners, let Codex refresh ChatGPT-managed auth in place by
running Codex under the persistent account home. If `codex login status`,
`codex exec`, or a Codex backend call returns `401 token_revoked`, mark that
provider account `stale` or `requires_reauth` in Vortex and present a new
device-code login for that account slot only.

Do not fall back to `OPENAI_API_KEY` or `CODEX_API_KEY` for user-connected
Codex workrooms. API-key fallback changes billing, quota, authorization, and
audit semantics.

## When To Use One VM Per Account

Use one VM per account only when a stronger isolation boundary is required:

- separate customers with contractual isolation requirements;
- high-risk accounts that must not share a host;
- provider terms or rate-limit behavior require one machine identity;
- incident response requires quarantining a single account and host together.

The default SHC/GCP managed-node plan is one trusted VM with multiple
account-scoped Codex homes, strict file permissions, short-lived Vortex grants,
per-run session homes, redacted receipts, and no wallet authority.

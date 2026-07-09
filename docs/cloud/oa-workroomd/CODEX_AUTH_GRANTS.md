# oa-workroomd Codex Auth Grants

Status: Cloud MVP scaffold for `CND-046`

`oa-workroomd codex auth` materializes a Vortex-issued ChatGPT/Codex provider
account grant into a per-session `CODEX_HOME`, checks the account with
`codex login status`, and scrubs VM-local auth material after closeout or
failure.

This is a compatibility bridge for the first Codex VM workroom. It is not the
long-term credential broker.

For multi-account ChatGPT/Codex support, the brokered auth cache should come
from an account-scoped `CODEX_HOME`, not from the VM user's default
`~/.codex`. Codex stores file-backed credentials at
`$CODEX_HOME/auth.json`; using one account home per provider account prevents
the normal login flow from overwriting another account.

## Commands

```bash
oa-workroomd codex auth materialize \
  --grant-file ./codex-auth-grant.json \
  --auth-json-file ./brokered-auth-cache.json \
  --state-dir ./workroom-state \
  --json

oa-workroomd codex auth status \
  --codex-bin codex \
  --state-dir ./workroom-state \
  --json

oa-workroomd codex auth scrub \
  --state-dir ./workroom-state \
  --json
```

## Files

| File | Contents |
| --- | --- |
| `codex-auth-state.json` | Grant refs, session `CODEX_HOME` path, auth file digest, login status, receipt refs. |
| `codex-auth/<grant-ref>/auth.json` | VM-local Codex auth cache, written `0600`, deleted by `scrub`. |
| `codex-auth-receipts.jsonl` | Redacted auth materialization/status/scrub receipts. |

The state and receipt files never store raw auth JSON content. The tracked
contract stores only `provider_secret_ref`, `provider_account_ref`, `grant_ref`,
status, reasons, and digests.

## Lifecycle

1. Vortex issues a short-lived `providerAccountAuthGrant`.
2. `oa-codex-control` calls the Vortex grant resolver API with the Cloud runner
   bearer token and resolves the grant into a server-side provider secret ref.
3. The broker maps `codex-auth://<provider-account-ref>` to
   `$OA_CODEX_AUTH_JSON_ROOT/<provider-account-ref>/auth.json` and supplies
   that Codex auth cache to `oa-workroomd` over the approved secret path.
4. `oa-workroomd` writes that auth cache to a session-scoped `CODEX_HOME` with
   owner-only permissions.
5. The runner calls `oa-workroomd codex auth status`; the command runs
   `codex login status` with only that session `CODEX_HOME`.
6. Success/failure is recorded as a redacted receipt.
7. `oa-workroomd codex run` repeats the status check before `codex exec`.
8. On closeout, failure, timeout, or cancellation, `scrub` removes the
   session `CODEX_HOME` and records a cleanup receipt.

See `CODEX_WORKROOM_RUNNER.md` for the CND-045 runner that consumes this auth
state.

## Non-Goals

- No global VM-user Codex login.
- No raw OpenAI API key as the primary path, fallback path, or benchmark
  shortcut for user workrooms.
- No shared `CODEX_HOME` for multiple user accounts.
- No `auth.json` content in tracked files, receipts, logs, screenshots, or
  docs.
- No wallet authority.
- No broad GCP credentials in the workroom.

## Tests

`crates/oa-workroomd/tests/codex_auth.rs` covers:

- materialize/status/scrub lifecycle;
- `0600` auth-file permissions on Unix;
- expired grant refusal before materialization;
- receipt-log redaction for fake auth cache content.

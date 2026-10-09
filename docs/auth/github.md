# GitHub sign-in (P1)

Part of [OpenAgents authentication](README.md). Code: `crates/oa-auth`
(flow, GitHub client, account resolution, fake GitHub, local account
service), `crates/tenancy/src/accounts/identities.rs` (the `github:`
principal and profile records), `crates/openagents-web/src/auth.rs` (the
web routes), and `crates/gateway/src/accounts.rs` (the production account
service routes).

## OAuth Apps

One GitHub OAuth App per deployment, owned by the OpenAgents organization.
Each App's callback is the deployment's `/auth/github/callback`:

| Deployment | Homepage | Callback |
| --- | --- | --- |
| Local | `http://127.0.0.1:4301` | `http://127.0.0.1:4301/auth/github/callback` |
| Staging | `https://onboarding---coder-ezxz4mgdsq-uc.a.run.app` | `https://onboarding---coder-ezxz4mgdsq-uc.a.run.app/auth/github/callback` |
| Production | `https://openagents.com` | `https://openagents.com/auth/github/callback` |

Each App's private file is `{"client_id", "client_secret",
"token_encryption_key"}` (the key is 32 random bytes, base64), mode 0600,
never committed or logged. The owner keeps them at
`~/work/.secrets/github-oauth-{local,staging,production}.json`.

## Configuration

- Web server: `openagents-web --cloud-config CLOUD_JSON --github-oauth
  PRIVATE_JSON [--github-redirect URL]`. It reads only the client id. The
  callback defaults to the Cloud origin plus `/auth/github/callback`.
- Account service (gateway config, `accounts` block):

  ```json
  "accounts": {
    "signup_tenant": "signup",
    "github": {
      "credentials": "/secrets/github-oauth-production.json",
      "redirect_url": "https://openagents.com/auth/github/callback"
    }
  }
  ```

  `signup_tenant` must be set: new accounts get a personal workspace bound
  to it.

## Flow

```
browser            web (openagents-web)            account service (gateway)       GitHub
  | GET /auth/github?return_to=/x |                          |                          |
  |------------------------------>| state, PKCE verifier,    |                          |
  |   303 to GitHub authorize     | flow cookie (10 min)     |                          |
  |<------------------------------|                          |                          |
  | consent ---------------------------------------------------------------------------->|
  | GET /auth/github/callback?code&state <-----------------------------------------------|
  |------------------------------>| check state vs cookie    |                          |
  |                               | POST /v1/sessions/github |                          |
  |                               | {code, code_verifier} -->| token exchange --------->|
  |                               |                          | GET /user, /user/emails >|
  |                               |                          | drop token; find or      |
  |                               |                          | create account; session  |
  |                               |<-- {session, token} -----|                          |
  |  200 "You're signed in", Set-Cookie oa_cloud_session, refresh to /x                  |
```

Authorize parameters: `client_id`, `redirect_uri`, `scope=read:user
user:email`, `state` (16 random bytes), `code_challenge` (S256 of a
32-byte verifier), `response_type=code`.

The account service routes:

- `POST /v1/sessions/github` `{code, code_verifier}` answers `{session,
  token, created}` like `POST /v1/sessions`; `401 sign_in_denied` for a bad,
  reused, or expired code or a wrong verifier; `503 sign_in_unavailable`
  when GitHub or the stores are down.
- `POST /v1/account/identities/github` (session bearer) `{code,
  code_verifier}` links GitHub to the signed-in account; `409
  identity_taken` when that GitHub account belongs to another account,
  `409 provider_linked` when this account already has a different one.

## What we store

On every sign-in, from `/user` and `/user/emails`, in the account store's
`identities.github["<id>"]` record:

| Field | Notes |
| --- | --- |
| `id` | Numeric GitHub user id, the key (`github:<id>` principal) |
| `login`, `node_id` | Login refreshed on every sign-in (renames) |
| `name`, `avatar_url`, `html_url` | New accounts take `name`, else `login`, as their label |
| `email`, `emails[]` | Public email; every address with `verified`, `primary`, `visibility` |
| `company`, `blog`, `location`, `bio`, `twitter_username`, `hireable` | As written on GitHub |
| `kind` | GitHub's `type` (`User`) |
| `public_repos`, `public_gists`, `followers`, `following` | Counts at last sign-in |
| `created_at`, `updated_at` | GitHub's timestamps |
| `linked`, `refreshed` | Ours |

Text is stripped of control characters and bounded (2 KiB per field, 32
emails). The GitHub access token is never stored: the account service uses
it for the two reads and drops it. If a later feature (repository access,
for example) needs a stored token, it is requested with its own scopes,
encrypted with AES-256-GCM under the App file's `token_encryption_key`, and
kept per feature.

## Local testing

Fake GitHub (no network, pick a fake person or cancel on its page):

```sh
cargo run -p openagents-web --example github_auth_fixture -- \
  "$TMPDIR/oa-github-$(date +%s)" 127.0.0.1:4301
```

Real GitHub, with the owner's local OAuth App:

```sh
cargo run -p openagents-web --example github_auth_fixture -- \
  "$TMPDIR/oa-github-$(date +%s)" 127.0.0.1:4301 \
  --github-oauth ~/work/.secrets/github-oauth-local.json
```

Open `http://127.0.0.1:4301`, click **Log in**, then **Continue with
GitHub**. Add `--cloud-build DIR` to also serve the signed-in Cloud pages
the account menu links to. Accounts and sessions are real tenancy stores
under the scratch directory (printed as `stores`).

Tests: `cargo test -p oa-auth` (flow, config, and end to end against the
fake: sign-up, returning user, rename, email-less user, wrong verifier,
reused code, cancel, linking conflict), `cargo test -p tenancy identities`,
and `openagents-web`'s `auth::tests` (header buttons, `/login`, the full
browser trip, state mismatch, cancel, open-redirect attempts).

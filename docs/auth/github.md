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
emails). The sign-in token is never stored: the account service uses it
for the two reads and drops it. Repository access asks for its own token
(below).

## Repository access

Connecting repositories (#11034, `/projects` on the web,
[sidebar](../web/sidebar.md#projects-and-repositories)) is a second trip
through the same OAuth App and callback, started at
`/auth/github/repos?access=private|public` by a signed-in person:

- `access=private` asks for `read:user repo read:org`: private and
  organization repositories. `access=public` asks for `read:user` only,
  which sign-in already granted, so GitHub shows no new permission screen.
  Nothing more is ever asked at sign-in.
- The flow cookie carries the purpose (`.repos` or `.repos-public`). The
  callback, a cross-site request that lacks the `SameSite=Strict` session
  cookie, answers a page that continues same-site to
  `/auth/github/repos/finish`, which checks the state again and hands the
  code and verifier to the account service under the session.
- The account service (`oa_auth::repos`) exchanges the code, reads `/user`
  and its `X-OAuth-Scopes`, refuses a GitHub user other than the one the
  account signs in with (`409 github_other_account`), and keeps the token
  sealed with AES-256-GCM under the App file's `token_encryption_key`,
  bound to the account and GitHub user id, in
  `github-access/<sha256 of account>.json` (mode 0600) beside
  `accounts.json`, with the granted scopes. A GitHub 401 marks it revoked:
  the status reads `reconnect` and projects stay.

Account-service routes (session bearer), served by the gateway and by
`oa_auth::local`:

| Route | Answer |
| --- | --- |
| `GET /v1/account/github` | `{github: {state: none\|connected\|reconnect, login, private}, projects}` |
| `POST /v1/account/github/grant` `{code, code_verifier}` | the status |
| `DELETE /v1/account/github/grant` | the status; the token is forgotten |
| `GET /v1/account/github/repositories` | up to 300, most recently pushed first |
| `POST /v1/account/github/token` | `{token, private}` for this deployment's web server to read GitHub as the person (`/environments`); never stored there |
| `POST /v1/account/projects` `{repository}` | `{project}`: id, name, repository id and full name, default branch, private, created time; one per repository |
| `DELETE /v1/account/projects/{id}` | `{removed}` |

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
reused code, cancel, linking conflict; repository access: public and
private grants, projects, revocation, another GitHub account, the token
only encrypted on disk), `cargo test -p tenancy identities`, and
`openagents-web`'s `auth::tests` (header buttons, `/login`, the full
browser trip, state mismatch, cancel, open-redirect attempts) and
`projects::tests` (Connect GitHub through the site, adding a project, the
sidebar groups and the composer's picker, moving a chat, revocation, and
what signed-out visitors and other accounts see).

To try repository access locally, sign in on the fixture above, then open
**Connect a GitHub repository** at the bottom of the chat list (or
`/projects`).

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
  to it. GitHub sign-in is the only way accounts are made: `POST
  /v1/accounts` (open sign-up, no GitHub) stays refused (`signup_disabled`)
  unless `"open_signup": true`, which only local development sets. A
  staging smoke suite can name `operator_signup_token_env`, an environment
  variable whose token, sent as the bearer, makes one test account.

## Invite-only sign-in

While accounts are opened by invitation (since 2026-10-09, only the owner,
GitHub `AtlantisPleb`, id 14167547), both halves read one list
(`oa_auth::invite`):

```json
"invite_only": {"github": [{"id": 14167547, "login": "AtlantisPleb", "admin": true}]}
```

- Account service: `accounts.invite_only` in the gateway config
  (`INVITE_ONLY_JSON` in `deploy/staging/gateway.sh`). A GitHub sign-in or
  link from anyone else is refused `403 invite_only` right after GitHub
  says who it is, before the stores are opened: no account, no session.
  `GET /v1/account` answers `"admin": true` for an entry marked `admin`;
  the sign-in answer carries `github: {id, login}` and `admin`.
- Web server: `invite_only` in its Cloud config (`openagents.cloud.web-config.v1`).
  `/login` says "Sign-in is invite-only for now." A sign-in the account
  service refuses, or whose answer names a GitHub user not on the web's own
  list (the session is then ended at once), lands on the plain page "Sign-in
  is invite-only for now" with no cookie set.
- An entry with an `id` matches only that id (a renamed login stays in, a
  login someone else takes later doesn't); an entry with only a `login`
  matches it case-insensitively. Absent means anyone with GitHub may sign
  in. Open sign-up (`POST /v1/accounts`) stays off either way.

Tests: `cargo test -p oa-auth` (`invite::tests`, and
`invite_only_lets_in_the_invited_admin_and_refuses_everyone_else_without_an_account`),
`openagents-web`'s `auth::tests::invite_only_*` and
`the_web_servers_own_invite_list_sets_no_cookie_and_ends_the_session`.

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
| `GET /v1/account/github/repositories?page=N` | `{repositories, more, sso_hidden}`: one GitHub page of 30, most recently pushed first (pages 1–20); `more` when GitHub's `Link` names a next page; disabled repositories left out, archived ones marked `archived`; `sso_hidden` when GitHub's `X-GitHub-SSO: partial-results` says an organization hid some; cached per account (below) |
| `POST /v1/account/github/token` | `{token, private}` for this deployment's web server to read GitHub as the person (`/environments`); never stored there |
| `POST /v1/account/projects` `{repository}` | `{project}`: id, name, repository id and full name, default branch, private, created time; one per repository |
| `DELETE /v1/account/projects/{id}` | `{removed}` |

Repository pages are cached per account and grant (`oa_auth::cache`,
#11057): fresh for 5 minutes, kept for an hour and served at once while one
background read refreshes them; a failed refresh keeps the kept page.
Connecting, disconnecting, adding or removing a project, and a GitHub 401
drop the account's pages. While `x-ratelimit-remaining` is under 100, kept
pages are served without reading GitHub again.

The chat composer's repository and branch pickers read GitHub as the
signed-in person when they connected GitHub (the token comes from
`POST /v1/account/github/token` for that request and is not kept), and
without a token otherwise. Reads without a token share GitHub's 60 an hour
for the whole server; past it the composer says "GitHub is limiting requests
without a sign-in. Connect GitHub, or try again later." Branch lists are
cached the same way, per token digest (or shared when read without one).

Every GitHub read (`oa_auth::github`, the composer, the environments studio)
sends `Accept: application/vnd.github+json` and `X-GitHub-Api-Version:
2022-11-28` (`oa_auth::github::API_VERSION`; the fake refuses any other with
400). `oa_auth::github` takes
bodies up to 8 MB (a page of 100 whole repositories is about 600 KB), waits
at most 15 seconds, and tries a dropped connection or a 502/503/504 once more
after half a second. Errors say what GitHub said, with these codes:

| Code | Status | When |
| --- | --- | --- |
| `github_reconnect` | 409 | GitHub answered 401 to the stored token (it is marked revoked) |
| `github_rate_limited` | 429 | 429, or 403 with `x-ratelimit-remaining: 0`, a `Retry-After`, or a rate-limit message (secondary limits) |
| `github_sso_required` | 403 | 403 with `X-GitHub-SSO: required` |
| `github_forbidden` | 403 | any other 403 or 451 (an organization's OAuth App access restrictions), or a disabled repository |
| `repository_not_found` | 404 | 404 |
| `github_error` | 502 | GitHub's own 5xx |
| `github_bad_answer` | 502 | a 2xx that isn't JSON (an HTML outage page), the wrong shape, or a body over 8 MB |
| `github_unavailable` | 503 | no connection or no answer in time: the only "isn't answering" |
| `github_not_configured` | 503 | this server has no GitHub App file or token key |
| `github_access_storage` | 500 | the sealed token or its file couldn't be read or written |

## GitHub App

Repository access can go through a GitHub App instead of the OAuth App's
`repo` scope (#11056). The OAuth App stays for sign-in, and people with an
OAuth grant keep working until they install the App. Code:
`oa_auth::app` (key, JWT, token cache), `oa_auth::repos::installed`,
`oa_auth::repos::broker`, `crates/openagents-web/src/projects`.

**Configuration.** One App per deployment (owner steps in the workspace
`NEEDS_OWNER.md`). Its private file is `{"app_id", "slug", "client_id",
"client_secret", "token_encryption_key", "private_key"}`, where
`private_key` names the `.pem` GitHub generated (relative to the file);
both mode 0600, kept at `~/work/.secrets/github-app-{local,staging,
production}.json` and `.pem`.

- Web server: `--github-app PRIVATE_JSON` (reads the client id and slug).
  With it, `/projects` offers **Install on repositories**.
- Account service: `accounts.github_app: {"credentials": PATH,
  "redirect_url": URL}` beside `accounts.github`.

**Flow.**

1. **Install on repositories** (`/auth/github/install`) authorizes the
   App's own client (PKCE, flow tag `.install`, same callback), then
   `/auth/github/repos/finish` hands the code to `POST
   /v1/account/github/app/grant`. The account service keeps the App's
   user token and refresh token sealed (AES-256-GCM, bound to the account,
   the GitHub user, and which token), refuses another GitHub user (`409
   github_other_account`), and reads `/user/installations`.
2. With no installation yet, the browser goes on to GitHub's
   `https://github.com/apps/<slug>/installations/new`. GitHub's setup URL
   (`/auth/github/setup`) calls `POST /v1/account/github/app/refresh`.
   The `installation_id` GitHub adds is never trusted: installations are
   always read with the person's own user token.
3. Listing reads `/user/installations/{id}/repositories` per installation
   (only repositories the person can open; `Link` paging). Adding a
   project checks `/repos/{owner}/{name}` with the user token and records
   the installation on the project. The user token is renewed with its
   refresh token when it has under a minute left, or once on a 401; a
   refused refresh reads `reconnect`.

**Installation tokens.** The account service signs a JWT with the App's
key (RS256 by `ring`; `iat` 60 seconds back, `exp` 10 minutes on, `iss`
the App id) and asks `POST /app/installations/{id}/access_tokens` for one
repository (`repository_ids`) with contents write, metadata read, and pull
requests write. Tokens live only in memory (`TokenCache`): reused while
under 50 minutes old with at least 5 minutes left, minted once for
concurrent callers, and minted again once when GitHub answers 401. GitHub's
refusals: `404 github_app_not_installed` (installation removed, or the
repository isn't in it) and `403 github_app_suspended`.

**Credential broker.** A machine that fetches or pushes a project's
repository gets a ticket, not a token:

| Route | Answer |
| --- | --- |
| `POST /v1/account/github/broker` (session) `{repository, seconds}` | `{ticket, expires_unix, path}`; `ogb_` tickets last 2 hours by default (5 minutes to a day), kept only as a SHA-256 digest; disconnecting drops them |
| `POST /v1/github/git-credential` (no session) form `ticket, protocol, host, path` | Git's credential lines `username`, `password`, `password_expiry_utc` for `https://github.com/<owner>/<name>` of the ticket's project only; anything else gets an error and no token |

The machine's credential (`OPENAGENTS_GIT_BROKER`, value `<broker URL>
<ticket>`, `oa_auth::repos::broker::credential_value`) feeds
`coder_environment_setup::git_auth_env`, whose helper answers only
`https://github.com` with a path, posts the ticket on standard input with
`curl`, and has no older token to fall back to.

Tests: `cargo test -p oa-auth --test app` (install and listing, another
GitHub account, the broker's host and repository checks, reuse until 50
minutes, a token with under 5 minutes left, twelve concurrent fetches and
one mint, the 401 retry, a suspended installation, an uninstalled
repository, user-token renewal and a refused refresh, a JWT for another
App), `cargo test -p coder-environment-setup --test git_auth` (real Git
and curl against a broker), and `openagents-web`'s
`projects::tests::app`. The fake (`oa_auth::fake::FakeApp`) checks JWTs
against the App's public key and makes its test key with `openssl`.

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
only encrypted on disk; a 250-repository account paged by real-sized
pages and `Link` headers; rate limits, secondary limits, 429, 5xx, an HTML
page, single sign-on, organization restrictions, a retried 502). The fake (`oa_auth::fake`) answers with GitHub's
whole repository objects (`fake::repository`, about 6 KB each), pages
`/user/repos` with `per_page`/`page` and `Link`, sends rate-limit headers,
and fails on request with `Fake::fail` / `Fake::fail_later` and
`fake::Fault`; `fake::busy(n)` is a person with `n` repositories across
two organizations, some private, archived, or disabled. Also
`cargo test -p tenancy identities`, and
`openagents-web`'s `auth::tests` (header buttons, `/login`, the full
browser trip, state mismatch, cancel, open-redirect attempts) and
`projects::tests` (Connect GitHub through the site, adding a project, the
sidebar groups and the composer's picker, moving a chat, revocation, and
what signed-out visitors and other accounts see).

To try repository access locally, sign in on the fixture above, then open
**Connect a GitHub repository** at the bottom of the chat list (or
`/projects`).

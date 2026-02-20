# gogcli: Details and Pros/Cons of Inlining vs Using as Subprocess

This document summarizes the [gogcli](https://github.com/steipete/gogcli) codebase at `~/code/gogcli`, with emphasis on what’s involved in Gmail automation and the tradeoffs between **inlining** that logic into another app (e.g. Inbox Autopilot) vs **using gog as an external binary** (as OpenClaw does).

---

## 1. What gogcli Is

- **gog** (gogcli) is a **Go CLI** for Google Workspace: Gmail, Calendar, Drive, Docs, Slides, Sheets, Contacts, Tasks, Chat, Classroom, Keep, Forms, Apps Script, People, Groups.
- **Features relevant to email automation:** Gmail search/send/drafts/labels/filters/watch (Pub/Sub push), JSON/plain output, multiple accounts, OAuth + optional Workspace service-account (domain-wide delegation), secure token storage (OS keyring or encrypted file), auto-refreshing tokens.
- **Install:** e.g. `brew install steipete/tap/gogcli` or build from source (`make`).
- **Auth model:** User runs `gog auth credentials <client_secret.json>` then `gog auth add <email> --services gmail,...`. Tokens live in a keyring (macOS Keychain, Linux Secret Service, or encrypted file). No built-in OAuth client ID; user supplies their own Desktop OAuth client from GCP Console.

For Gmail **watch** (push) specifically, see gogcli’s `docs/watch.md`: Gmail watch → Pub/Sub topic → push subscription → `gog gmail watch serve` HTTP handler → downstream webhook (e.g. OpenClaw).

---

## 2. Architecture (Relevant to Inlining)

### 2.1 Auth and Token Storage

| Layer | Location | Responsibility |
|-------|----------|----------------|
| **OAuth flow** | `internal/googleauth/` | Authorize: local server or manual/remote redirect; exchange code for refresh token. Uses `golang.org/x/oauth2` + `google.Endpoint`. |
| **Token storage** | `internal/secrets/store.go` | Keyring abstraction: `SetToken(client, email, tok)`, `GetToken`, `DeleteToken`, `ListTokens`, default account. Backends: `keychain`, `file` (encrypted), or `auto`. Uses [99designs/keyring](https://github.com/99designs/keyring). |
| **Credentials (client secret)** | `internal/config/credentials.go` | Read/write OAuth client JSON (`installed` or `web`); paths like `~/.config/gogcli/credentials.json` or `credentials-<client>.json`. |
| **Client resolution** | `internal/authclient/authclient.go` | Resolve which OAuth *client* (credentials file) to use for an account: `--client`, config `account_clients` / `client_domains`, or default. |
| **Token source for API** | `internal/googleapi/client.go` | For a given account + scopes: prefer service-account token source if configured; else load refresh token from keyring, build `oauth2.Config`, return `cfg.TokenSource(ctx, &oauth2.Token{RefreshToken: ...})`. Used by all Google API clients (Gmail, Calendar, etc.). |
| **Service account** | `internal/googleapi/service_account.go` | JWT token source for domain-wide delegation; config paths for per-account service account key JSON. |

So “Gmail auth” in gog is: **credentials.json (client id/secret) + keyring (refresh token per client+email) or service-account key**, then `oauth2.TokenSource` used by the Gmail API client.

### 2.2 Gmail API and Watch

| Piece | Location | Responsibility |
|-------|----------|----------------|
| **Gmail service** | `internal/googleapi/gmail.go` | `NewGmail(ctx, email)` → `optionsForAccount(ServiceGmail, email)` → `gmail.NewService(ctx, opts...)`. Thin wrapper. |
| **Watch start/renew/stop** | `internal/cmd/gmail_watch_cmds.go` | CLI: `watch start` (topic, labels, TTL), `watch status`, `watch renew`, `watch stop`. Calls Gmail API `Users.Watch`, persists state. |
| **Watch state** | `internal/cmd/gmail_watch_state.go` | Per-account JSON file under `~/.config/gogcli/state/gmail-watch/<account>.json`: topic, labels, historyId, expiration, hook url/token, last delivery status. Thread-safe store with `StartHistoryID(pushHistory)` for dedup and history progression. |
| **Watch serve (HTTP)** | `internal/cmd/gmail_watch_server.go` | HTTP server for Pub/Sub push: parse push envelope (base64 payload), decode Gmail push payload (emailAddress, historyId); authorize (OIDC JWT or shared token); call `Users.History.List` from `startID`; fetch message metadata (and optionally body); exclude labels (e.g. SPAM, TRASH); POST JSON to hook URL with Bearer token; update state (historyId, last delivery). Handles stale history (resync via `Messages.List`), duplicate message ID, and hook errors. |
| **Types** | `internal/cmd/gmail_watch_types.go` | State struct, serve config, Pub/Sub envelope, Gmail push payload, hook message/payload. |

Other Gmail command code (search, send, drafts, labels, filters, history, etc.) lives in many more files under `internal/cmd/` (gmail_*.go); for “watch + push → webhook” only the watch-related files and the shared auth/API client matter.

### 2.3 Dependencies (Go)

- **OAuth / Google APIs:** `golang.org/x/oauth2`, `golang.org/x/oauth2/google`, `google.golang.org/api/gmail/v1`, `google.golang.org/api/option`, `google.golang.org/api/idtoken` (OIDC for push).
- **Keyring:** `github.com/99designs/keyring`.
- **CLI:** `github.com/alecthomas/kong`.
- **Config:** JSON5 config, paths via `internal/config/paths.go` (e.g. `Dir()`, `EnsureGmailWatchDir()`, `ClientCredentialsPathFor`, `ServiceAccountPath`).

### 2.4 Scale of Gmail-Only “Watch + Serve” Code (Rough)

- **Watch + serve (no other Gmail features):**
  `gmail_watch_*.go` (cmds, server, state, types, utils, helpers), plus shared auth/config and the Gmail API client. On the order of **~3k+ lines** of production code plus substantial tests (server, state, serve validation, history types, errors, etc.).
- **Auth + secrets (needed for any Gmail use):**
  `googleauth/`, `secrets/`, `config/credentials.go`, `googleapi/client.go`, `authclient/`, `config/paths.go` — cross-used by all services (Gmail, Calendar, Drive, …), so not trivially “Gmail-only.” Inlining “just Gmail watch” still implies either depending on gog’s auth/keyring/config or reimplementing a minimal token store and OAuth flow.

---

## 3. What “Inlining” Could Mean

1. **Vendor/copy a subset of gog into our repo**
   Copy the minimal set of packages needed for “Gmail watch + serve”: auth flow, keyring (or replace with simpler token storage), config paths, client resolution, Gmail API client, watch state, watch server. This implies:
   - Maintaining a fork or copy of that subset (or a thin wrapper around a vendored gog module, if it were library-friendly).
   - Handling keyring dependency (or replacing it with e.g. encrypted file or env-based tokens for server use).
   - Possibly simplifying to a single-account/single-client case and dropping Kong CLI.

2. **Call gog as a library**
   gog is structured as a CLI (Kong commands, `main`); there is no public “library” API. To use it as a library you’d have to refactor entrypoints (e.g. expose `RunGmailWatchServe(config)` and auth helpers) and depend on the full gog module. That’s a larger refactor inside gogcli and ties your app to its full dependency tree and CLI layout.

3. **Reimplement only the bits we need**
   Implement from scratch: OAuth2 (or use a small library), store refresh token (file or keyring), build `oauth2.TokenSource`, call Gmail API `Users.Watch` and `Users.History.List`, run an HTTP server that parses Pub/Sub push and POSTs to a webhook. That’s less code than the full gog watch stack but you take on token refresh, history semantics, dedup, and error handling yourself.

So in practice “inlining” usually means either **copy/vendor a subset** or **reimplement a minimal watch + serve pipeline**; “link gog as a library” only makes sense if gog gains a library API or you do that refactor.

---

## 4. Pros and Cons

### 4.1 Using gog as an External Binary (Current OpenClaw Model)

**Pros:**

- **No duplication:** One place (gog) handles OAuth, keyring, token refresh, Gmail API, watch state, Pub/Sub parsing, history semantics, hook POST. You get fixes and features (e.g. OIDC, exclude labels, history types) by upgrading gog.
- **Battle-tested:** Keyring backends, manual/remote OAuth, service-account path, stale history and resync, duplicate push handling are already implemented and tested.
- **Clear boundary:** Your app only needs to run `gog gmail watch start` / `gog gmail watch serve` with the right flags and maybe parse gog’s output; no Go API surface to keep in sync.
- **Separation of concerns:** Auth and secrets stay in gog’s config/keyring; your app doesn’t need to touch refresh tokens or client secrets (only account email and hook URL/token).
- **Multiple accounts / clients:** If you ever need them, gog already supports multiple accounts and OAuth clients; you just pass `--account` and optionally `--client`.

**Cons:**

- **Deployment:** You must install and maintain the gog binary (and, for setup, gcloud/tailscale etc.). Version drift between your app and gog can cause surprises.
- **Process model:** You spawn and supervise a long-lived process (`gog gmail watch serve`); if you want different lifecycle (e.g. run inside your process, scale per-account in-process), the subprocess model is less natural.
- **No in-process API:** You can’t call “fetch this message” or “start watch” from your app’s code without shelling out; any new use case requires gog to expose a CLI or you to add one.
- **Debugging:** Logs and errors are in gog’s process; you depend on its logging and exit codes.
- **Platform:** Tied to gog’s support (e.g. keyring on macOS/Linux/Windows); if you need a different secret store or auth flow (e.g. headless server with env-only tokens), you’re constrained by gog’s options (e.g. `GOG_KEYRING_BACKEND=file` + `GOG_KEYRING_PASSWORD`).

### 4.2 Inlining (Vendor Subset or Reimplement Minimal Watch + Serve)

**Pros:**

- **Single process / stack:** Watch serve runs inside your app; one binary, one deployment, easier to scale or integrate with your existing server/runtime.
- **Full control:** You can change token storage (e.g. env, vault, your DB), simplify to one account, or add custom logic (filtering, retries, metrics) without touching gog’s code.
- **No external binary:** No need to ship or upgrade gog; fewer moving parts in production.
- **Tailored UX:** Onboarding can be “sign in with Google” in your UI and store tokens your way instead of “run gog auth add on the server.”

**Cons:**

- **Maintenance:** You own the inlined code (or a fork). Security and API changes (Gmail API, OAuth, Pub/Sub) require you to update your copy or reimplementation.
- **Scope creep:** If you only reimplement “watch + serve,” you miss send/drafts/labels/search unless you add them or keep calling gog for those. If you vendor more of gog, you carry more code and dependencies (keyring, Kong, etc.).
- **Correctness and edge cases:** History ID handling, stale resync, duplicate pushes, OIDC verification, exclude labels — gog has already hit many of these; reimplementing risks regressions unless you copy or re-test thoroughly.
- **Auth surface:** You must implement or vendor OAuth (and optionally keyring or equivalent). That’s a non-trivial security surface (redirect URIs, state, token storage).

### 4.3 Summary Table

| Criterion | Use gog as binary | Inline (vendor/reimplement) |
|----------|-------------------|-----------------------------|
| Maintenance burden | Low (upgrade gog) | Higher (you own code) |
| Deployment | Need gog binary (+ gcloud/tailscale for full setup) | Single app binary possible |
| Control over auth/storage | Limited to gog’s keyring/config | Full (env, DB, vault, etc.) |
| Process model | Subprocess | In-process possible |
| Feature set | Full Gmail + other services via CLI | Only what you implement/vendor |
| Correctness / edge cases | Mature in gog | Your responsibility |
| New use cases (e.g. “search from app”) | Need CLI or new gog command | Can add in-process API |

---

## 5. Recommendation (for Inbox Autopilot)

- **If the goal is “Gmail push → our backend” with minimal ongoing work:** Keep **using gog as a binary** (like OpenClaw). You get a stable, tested implementation and clear separation of auth; you pay with process management and deployment of gog (and optionally gcloud/tailscale for setup).
- **If the goal is a single binary, no external tools, or custom token storage / onboarding:** **Inline** by either:
  - **Vendoring a minimal slice of gog** (auth + secrets + config + Gmail client + watch state + watch server) and trimming the rest, or
  - **Reimplementing a minimal path:** OAuth2 refresh token storage (e.g. file or env), `oauth2.TokenSource`, Gmail `Users.Watch` + `Users.History.List`, HTTP handler for Pub/Sub push, and POST to your webhook. That’s on the order of hundreds of lines plus tests, but you must handle history ID, dedup, and errors yourself.
- **If you need both “run watch in-process” and “reuse gog’s logic”:** The clean long-term approach would be for gog to expose a small **library** (e.g. `gogcli/gmailwatch`) with a stable API; then your app could depend on that and avoid reimplementing or forking. Today that doesn’t exist, so inlining means either forking/vendoring or reimplementing.

---

## 6. Key Files in gogcli (Reference)

| Purpose | Path |
|--------|------|
| OAuth flow | `internal/googleauth/oauth_flow.go`, `oauth_flow_manual*.go`, `accounts_server.go` |
| Scopes / services | `internal/googleauth/service.go` |
| Token storage | `internal/secrets/store.go`, keyring backends |
| Client credentials | `internal/config/credentials.go` |
| Config / paths | `internal/config/config.go`, `paths.go`, `clients.go` |
| Client resolution | `internal/authclient/authclient.go` |
| API client + token source | `internal/googleapi/client.go` |
| Service account | `internal/googleapi/service_account.go` |
| Gmail API | `internal/googleapi/gmail.go` |
| Watch commands | `internal/cmd/gmail_watch_cmds.go` |
| Watch HTTP server | `internal/cmd/gmail_watch_server.go` |
| Watch state | `internal/cmd/gmail_watch_state.go` |
| Watch types/config | `internal/cmd/gmail_watch_types.go`, `gmail_watch_utils.go` |
| Watch docs | `docs/watch.md` |
| README / auth | `README.md` (Quick Start, Authentication & Secrets) |

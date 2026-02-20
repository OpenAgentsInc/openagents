# How OpenClaw Authenticates with Gmail for Email Automation

This document summarizes how the [OpenClaw](https://github.com/openclaw/openclaw) project (~/code/openclaw) handles Gmail authentication and email automation. It is intended as a reference for implementing similar functionality (e.g., in Inbox Autopilot).

---

## Overview

OpenClaw **does not implement Gmail OAuth itself**. It delegates all Gmail (and Google API) access to an external CLI tool called **gog** ([gogcli](https://gogcli.sh)). OpenClaw’s role is to:

1. Configure and run `gog` for Gmail watch + push handling.
2. Use **gcloud** for GCP (Pub/Sub topic/subscription, project, APIs).
3. Expose a **webhook** so that when Gmail sends push notifications, OpenClaw can run agents (e.g., summarize email, reply).

So “auth with Gmail” in OpenClaw is really: **auth is handled by gog (gogcli); OpenClaw only passes an account identifier and relies on gog’s stored credentials.**

---

## 1. Gmail OAuth: Handled by gog (gogcli)

- **Tool:** [gog](https://gogcli.sh) (Google Workspace CLI), installed e.g. via `brew install steipete/tap/gogcli`.
- **OAuth flow:** Done entirely inside gog:
  1. User obtains a Google OAuth client secret (e.g. from GCP Console) and passes it to gog:
     - `gog auth credentials /path/to/client_secret.json`
  2. User adds a Gmail account and authorizes scopes:
     - `gog auth add you@gmail.com --services gmail,calendar,drive,contacts,docs,sheets`
  3. User can list accounts: `gog auth list`

- **Where credentials live:** gog stores credentials and tokens in:
  - `~/.config/gogcli/credentials.json` (or `$XDG_CONFIG_HOME/gogcli/credentials.json`)
  - On macOS: `~/Library/Application Support/gogcli/credentials.json`

- **Format:** The credentials file is JSON. OpenClaw only reads it in one place: to **resolve the GCP project id** from the OAuth client id (see below). It does **not** implement token refresh or OAuth flows; gog does that when you run `gog gmail watch start` or `gog gmail watch serve`.

---

## 2. How OpenClaw Uses the “Account”

- **Config:** OpenClaw’s hook config has a Gmail section, including:
  - `hooks.gmail.account`: the **email address** of the Gmail account (e.g. `openclaw@gmail.com`).

- **Runtime:** Whenever OpenClaw runs gog for Gmail, it passes that email as `--account`:
  - `gog gmail watch start --account <email> --label INBOX --topic projects/.../topics/...`
  - `gog gmail watch serve --account <email> --bind 127.0.0.1 --port 8788 ...`

- **Lookup:** gog uses `--account` to look up the correct OAuth tokens in its own credential store. No tokens are stored in OpenClaw config; only the account identifier (email) is.

---

## 3. Resolving GCP Project from gog Credentials

Gmail Watch requires the Pub/Sub topic to live in the **same GCP project as the OAuth client** used by gog. OpenClaw can derive that project without the user passing `--project`:

- **File:** `src/hooks/gmail-setup-utils.ts`
- **Function:** `resolveProjectIdFromGogCredentials()`
- **Logic:**
  1. Read gog credentials from the same paths gog uses (`~/.config/gogcli/credentials.json`, etc.).
  2. Parse JSON and get the OAuth **client id** from `installed.client_id`, `web.client_id`, or `client_id`.
  3. The client id is of the form `<project_number>-...`; extract the numeric prefix.
  4. Call `gcloud projects list --filter projectNumber=<project_number> --format value(projectId)` to get the project id.

So: **Gmail OAuth client id → project number → GCP project id**, used for creating the Pub/Sub topic and subscription.

---

## 4. gcloud Auth (Separate from Gmail OAuth)

OpenClaw uses **gcloud** for GCP operations only (Pub/Sub, enabling APIs), not for Gmail API access:

- **File:** `src/hooks/gmail-setup-utils.ts`
- **Function:** `ensureGcloudAuth()`
  - Runs `gcloud auth list --filter status:ACTIVE`; if no active account, runs `gcloud auth login`.

So there are two separate auth concerns:

| Purpose              | Tool   | Auth mechanism                          |
|----------------------|--------|-----------------------------------------|
| Gmail API (watch, read, send) | gog    | gog’s OAuth (credentials.json + tokens) |
| GCP (Pub/Sub, APIs)  | gcloud | `gcloud auth login`                     |

---

## 5. End-to-End Gmail Automation Flow

1. **One-time setup (user):**
   - Create OAuth client in GCP Console (or use existing).
   - Run `gog auth credentials <client_secret.json>` and `gog auth add <email> --services gmail`.
   - Optionally run `gcloud auth login` and set project if not using auto-resolution.

2. **OpenClaw setup (wizard):**
   - `openclaw webhooks gmail setup --account <email>`
   - This: ensures `gcloud` and `gog` are available, runs `ensureGcloudAuth()`, creates Pub/Sub topic and subscription, configures Tailscale Funnel as push endpoint, runs `gog gmail watch start`, and writes `hooks.gmail` (and related) into OpenClaw config.

3. **At runtime:**
   - **Gateway auto-start:** If `hooks.enabled` and `hooks.gmail.account` are set, the gateway starts `gog gmail watch serve` on boot and renews the watch periodically (see `gmail-watcher.ts`).
   - **Manual:** `openclaw webhooks gmail run` runs `gog gmail watch serve` with the same config and an auto-renew loop.

4. **When mail arrives:**
   - Gmail sends a push to the Pub/Sub push endpoint (e.g. Tailscale Funnel → `gog gmail watch serve`).
   - `gog` validates the push (optional token), fetches message details using **its** Gmail OAuth tokens, and POSTs to OpenClaw’s hook URL (`hooks.gmail.hookUrl`) with a shared **hook token** (`hooks.token`).
   - OpenClaw receives the webhook at `/hooks/gmail`, matches the “gmail” preset mapping, and runs an agent (e.g. with a message template that includes from/subject/snippet/body).

---

## 6. Tokens and Security (Relevant to Auth)

- **hooks.token:** OpenClaw’s webhook secret; sent to the OpenClaw gateway when gog forwards the notification (`--hook-token`). Not a Gmail token.
- **pushToken (gmail.pushToken):** Optional secret for the **Pub/Sub push endpoint** that gog’s serve handler listens on (`--token`). Used so only callers that know the token (or Google) can hit that endpoint.
- **Gmail OAuth tokens:** Stored and refreshed only inside gog’s credential store; never in OpenClaw config.

---

## 7. Key Files in OpenClaw (Reference)

| Area              | File(s) |
|-------------------|--------|
| Gmail hook config | `src/hooks/gmail.ts` (runtime config, build args for gog) |
| Gmail setup/run   | `src/hooks/gmail-ops.ts` (`runGmailSetup`, `runGmailService`) |
| Gmail watcher     | `src/hooks/gmail-watcher.ts` (gateway auto-start of `gog gmail watch serve`) |
| Setup helpers     | `src/hooks/gmail-setup-utils.ts` (gcloud, Tailscale, topic/subscription, `resolveProjectIdFromGogCredentials`) |
| Config schema    | `src/config/zod-schema.hooks.ts`, `src/config/types.hooks.ts` (`HooksGmailConfig`) |
| CLI               | `src/cli/webhooks-cli.ts` (`openclaw webhooks gmail setup` / `run`) |
| gog skill/docs    | `skills/gog/SKILL.md` (gog auth and Gmail commands) |
| User-facing doc  | `docs/automation/gmail-pubsub.md` (prereqs, wizard, one-time setup, troubleshooting) |

---

## 8. Takeaways for Implementing Similar Behavior

1. **Use an existing OAuth-capable Gmail client** (e.g. gog) or implement OAuth 2.0 (client secret, refresh tokens) and store tokens securely; OpenClaw does not implement Gmail OAuth itself.
2. **Account identifier:** OpenClaw only stores the Gmail **account email**; the actual tokens live in gog’s credential store.
3. **GCP and Gmail:** If you use Gmail Watch + Pub/Sub, the topic must be in the same GCP project as the OAuth client; OpenClaw derives that project from gog’s `credentials.json` client id when possible.
4. **Separation of concerns:** gcloud for GCP, gog for Gmail API; two separate auth flows.
5. **Webhook security:** Separate tokens for (a) the push endpoint and (b) the OpenClaw webhook, so you can lock down who can trigger the automation.

For full setup steps (APIs, topic, IAM, Tailscale, etc.), see OpenClaw’s own docs: `docs/automation/gmail-pubsub.md`.

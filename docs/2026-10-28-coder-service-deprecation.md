# Retiring the `coder` Cloud Run service

Written 2026-10-28. Everything here was checked read-only on that date with the
automation account. Nothing was changed. This plan proposes deletions; it makes
none. Each deletion needs the owner's explicit yes, by name.

## What it is and why it goes

`coder` is the Cloud Run service (project `openagentsgemini`, region
`us-central1`) behind production **https://openagents.com** and
**https://new.openagents.com**, through Cloud Run domain mappings. Until about
08:00 CDT today it also answered **api.openagents.com** (see Findings).

It is the old Rust website plus the old account and inference gateway:

- container `web`: `openagents-web`, an axum and maud site with about 640
  routes: pages, docs, blog, chat, the Verse and games, labs, admin, and about
  220 `/v1` API paths;
- container `gateway`: `openagents-stack`, the account service (accounts,
  sessions, API keys, device sign-in, WorkOS and Nostr sign-in) and the
  inference and decision gateway (Vertex, OpenRouter, quota ledger, balances,
  earnings, billing).

The commit "Nuke" (`0b401561a0`, 2026-10-27) deleted its source. Nothing in the
tree can build or redeploy it now. The running web image was built from
`7919650803` (2026-10-26 14:02 CDT). The site is being rebuilt in Coax
("Coaxial", `apps/openagents-web`, spec `coax/web.md`), so `coder` is a frozen
binary that we can't patch and that costs money every hour.

## Findings that matter before any cutover

1. **api.openagents.com is already down.** DNS points at `35.201.75.247`. TCP
   connects but TLS fails, and no forwarding rule or address in the project
   owns that IP. Until 13:00 UTC today it carried about 7,600 requests an hour
   to `coder`, all of them our own pollers (see Traffic). The CLI default
   `https://api.openagents.com/v1`, Pylon earnings and the sync server's
   account API all used it.
2. **sync.openagents.com is down.** Its DNS now points at the Coaxial load
   balancer `136.69.24.60`. That balancer's certificate map covers only
   `openagents.com` and `staging.openagents.com`, so TLS fails. The `coder-sync`
   Cloud Run service no longer exists. The `coder-sync` URL map and the
   `coder-sync-vm` and `coder-sync-run` backends are still there, with no
   forwarding rule in front of them.
3. **Lightning addresses fail.** `/lnurlp/*` and `/.well-known/lnurlp/*`
   answer 503 (68 requests in 7 days).
4. **l402.openagents.com is a stale mapping.** The domain mapping points at
   `l402-aperture`, which is no longer a Cloud Run service. It answers 404.
5. The project page tells agents to call `/v1/projects/coax/tasks`,
   `/v1/projects/coax/tasks/{id}/claim` and `/v1/claims/{id}/renew`. `coder`
   never had those routes, so they answer 404. They belong to the new
   project/Nostr stack.

## The service

| Item | Value |
|---|---|
| Service created | 2026-08-31 by chris@; last deployed by `oa-mvp-automation` |
| Serving revision | `coder-web-2e5a3e54e3-20261026195402` (100%, tag `new`), created 2026-10-26 19:54 UTC |
| Revisions kept | 439, plus 18 traffic tags at 0% (`conservation`, `labs-11874`, `webonly`, `verse-cloud`, `medieval`, `kiki*`, `zinc*`, …) |
| Web image | `us-central1-docker.pkg.dev/openagentsgemini/openagents/openagents-web@sha256:2e5a3e54…` (tag `stg-7919650803`) |
| Gateway image | `…/openagents/openagents-stack@sha256:f378e092…` |
| Size | web 2 vCPU / 1 GiB; gateway 1 vCPU / 512 MiB; concurrency 200; timeout 3600 s |
| Scaling | min 1, max 1; CPU always allocated (`cpu-throttling: false`); gen2; startup boost |
| Runtime account | `157437760789-compute@developer.gserviceaccount.com` |
| Cloud SQL | `openagentsgemini:us-central1:openagents-production-pg` (Postgres 18, `db-custom-1-3840`, 10 GB; databases `openagents`, `lnurl`) |
| Network | Direct VPC egress, `default` / subnet `openagents-web-production`, private ranges only |
| NFS volumes | `10.42.27.2:/srv/accounts/stack` at `/stack` (gateway read-write, web read-only); `10.42.27.2:/srv/accounts/web` at `/state` (web). The address is reserved as `oa-accounts-nfs-production`. The automation account cannot list Filestore, so the backing instance is unconfirmed. |
| Secret file | `openagents-web-kiln-site-key` at `/secrets/kiln/site.key` |

**Web environment variable names:** `OPENAGENTS_WEB_PAY_HOST`, `PUBLIC_ORIGIN`,
`WEB_STATE`, `INVITE_ONLY_JSON`, `OPENAGENTS_WEB_ANALYTICS_BUCKET`,
`STACK_STATE`, `ENVIRONMENTS_MODEL`, `OPENAGENTS_WEB_VAULT_BUCKET`,
`BOAT_API_BASE`, `OPENAGENTS_WEB_MAC_JOBS_ACTORS`,
`OPENAGENTS_WEB_API_OPERATOR_SIGNUP`, `OPENAGENTS_WEB_COMPUTERS_URL`,
`OPENAGENTS_WEB_PRO_TEST_ACCOUNTS`, `OPENAGENTS_WEB_AGENT_ACCOUNTS`,
`OPENAGENTS_WEB_LABS_BUCKET`, `OPENAGENTS_WEB_LABS_SIGNING_ACCOUNT`,
`OPENAGENTS_CLOUD_HOST_FREE_ALLOWANCE`, `OPENAGENTS_WEB_FEATURES`,
`OPENAGENTS_AUTH`, `WORKOS_METHODS`, `OPENAGENTS_KILN_RUNNER`,
`OPENAGENTS_KILN_REPO`, `OPENAGENTS_KILN_SITE_KEY`, `OPENAGENTS_LIVE_GAMES`,
`OPENAGENTS_LIVE_GAMES_RELAY`, `OPENAGENTS_LIVE_GAMES_STORE`,
`OPENAGENTS_FOREMAN_COMMANDS`, `OPENAGENTS_FOREMAN_OWNER`,
`OPENAGENTS_FOREMAN_WORKER_HOST`, `OPENAGENTS_FOREMAN_WORKER_PROJECT`,
`OPENAGENTS_FOREMAN_KILN_STATE`, `OPENAGENTS_PYLON_WORK_PUBLISHERS`,
`OPENAGENTS_MEDIEVAL_RELEASE`, `OPENAGENTS_HAUNT_RELEASE`,
`OPENAGENTS_VERSE_PUBLISH_RUNTIME`, `OPENAGENTS_VERSE_PERSONAL_RUNTIME`.

**Web secrets (Secret Manager names):** `openagents-web-ask-salt`,
`openagents-web-production-github-oauth`, `openagents-web-production-csrf-key`,
`openagents-web-production-byo-keys`, `openagents-web-analytics-key`,
`oa-boat-api-key`, `openagents-vertex-sa-key`, `openagents-work-host-token`,
`openagents-production-pg-dsn`, `oa-computers-token`,
`openagents-resend-api-key`, `openagents-labs-production-pg-url`,
`openagents-web-room-token-key`, `openagents-web-workos-api-key`,
`openagents-web-workos-client-id`, `coder-pool-git-token`,
`openagents-openrouter-api-key`, `openagents-web-kiln-site-key`.

**Gateway environment variable names:** `STACK_STATE`, `PUBLIC_ORIGIN`,
`GATEWAY_HOLD`, `INVITE_ONLY_JSON`, `OPENAGENTS_AUTH`, `WORKOS_METHODS`.
**Gateway secrets:** `openagents-web-production-github-oauth`,
`openagents-gateway-production-admin-token`, `openagents-vertex-sa-key`,
`openagents-openrouter-api-key`, `openagents-vercel-gateway-api-key`,
`openagents-gateway-production-byok-keyring`,
`openagents-gateway-production-store-key`, `openagents-production-pg-dsn`,
`openagents-gateway-production-smoke-signup-token`,
`openagents-web-workos-api-key`, `openagents-web-workos-client-id`.

## Its source

All paths are at `0b401561a0^` (read them with
`git show 0b401561a0^:<path>`).

- Library `crates/openagents-web` (about 620 files). The router is
  `router_with()` in `src/lib.rs` (lines 807 to 1020), which merges about 80
  module `routes()`. `src/api_alias.rs` is the allow-list of `/v1` paths that
  the web proxies to the gateway at `127.0.0.1:8791`. `/api/v1/*` always goes
  there.
- Binary `openagents-web` comes from `crates/openagents-web-site`
  (`src/main.rs`). It is built by `crates/openagents-web/Dockerfile` and
  `cloudbuild.yaml`, along with the wasm games and labs.
- The gateway is `crates/gateway` (bin `gateway`), built into
  `openagents-stack` by `deploy/staging/Dockerfile`.
- Account schema: `crates/tenancy/migrations/0001` to `0005`. Actors:
  `crates/actors/migrations/0001.sql`. Labs:
  `crates/lab-store/migrations/001_spaces.sql`.
- Deploy: `scripts/deploy/web.sh` (`stage`, `promote`, `shift`, `rollback`),
  `deploy/production/web.sh`, `deploy/production/api-url-map.yaml`.
- Storage notes: `docs/deployment/account-storage.md`.

To rebuild the old image for an emergency patch, check out `7919650803` in a
worktree. That is a last resort, not part of this plan.

## What it serves today (probed 2026-10-28)

| Path | Status | What it is |
|---|---|---|
| `/` | 200 | "Coming soon" page, a canvas field (not the Coax build yet) |
| `/llms.txt` | 200 | 22 KB Markdown index of the site |
| `/llms-full.txt` | 200 | 290 KB, every page as Markdown |
| `/index.md`, `/download.md`, `/auth.md`, `/pricing.md` | 200 | Markdown twins (text/markdown) |
| `/docs`, `/docs/*` | 200 | Docs pages (HTML, with a Markdown twin) |
| `/blog` | 200 | Blog index (many `/blog/*` slugs now 404) |
| `/chat` | 200 | Old chat app shell (63 KB) |
| `/login`, `/signup` | 200 | WorkOS sign-in pages |
| `/device` | 303 to `/login` | Device-login approval page |
| `/mcp` | 401 | Keyed MCP server ("Log in to use this server.") |
| `/mcp/docs` | 405 on GET | Public docs MCP server (POST JSON-RPC) |
| `/oauth/authorize` | 400 without params | OAuth 2.1 for MCP clients |
| `/.well-known/oauth-authorization-server` | 200 | OAuth metadata |
| `/.well-known/apple-app-site-association` | 200 | iOS universal links: `/connect`, `/projects/join/*`, `/login/scan` |
| `/.well-known/nostr.json` | 200 | `{"names":{}}` |
| `/.well-known/openid-configuration`, `/.well-known/l402` | 404 | none |
| `/openapi.json`, `/v1/openapi.json`, `/v1/models` | timeout at 40 s | proxied to the gateway, which holds (`GATEWAY_HOLD`); logs show 502s |
| `/v1`, `/v1/health`, `/v1/me`, `/v1/chat/completions` (GET) | 404 | none |
| `/plugins`, `/markdown` | 404 | do not exist |
| `/robots.txt`, `/sitemap.xml` | 200 | crawler files |
| `/cli/install.sh`, `/cli/install.ps1` | 200 | CLI installers; binaries come from `gs://openagentsgemini-cli-releases` |
| `/health` | 200 `ok` | health check |
| `/connect`, `/verse`, `/download`, `/docs/api` | 200 | pages |
| `/projects/coax` | 404 | Coaxial page; lives only in the bucket |

## Traffic (Cloud Run request logs)

Two pulls:

- **A:** every request for 24 hours (2026-10-27 19:23 to 2026-10-28 19:17 UTC),
  186,944 requests.
- **B:** 7 days (2026-10-21 19:00 to 2026-10-28 19:21 UTC), 170,860 requests.
  B leaves out the known internal pollers and requests with no user agent, so
  their counts come from A.

**Internal pollers (from A, per day), almost all of the load:**

| Path | Per day | Caller |
|---|---|---|
| `/v1/agent-assignments` | 65,256 | no user agent, from the owner's home IPv4 |
| `/v1/account` | 26,456 | no user agent, a Google Cloud IP (`35.188.70.206`) via api.openagents.com |
| `/v1/foreman/activity`, `/status`, `/commands` | 24,379 | `openagents-grid-foreman` |
| `/v1/history/sessions/computers/{id}/moves` | 13,799 | no user agent, owner home |
| `/v1/labs/agent/jobs`, `/v1/labs/agent/hosts` | 27,596 | no user agent, owner home |
| `/v1/work-hosts/oa-solver-dispatch-1/claim` | 6,414 | Google Cloud IP `34.61.169.241` |
| `/api/v1/systemone` | 6,390 | `openagents-decision-throughput/1` (a Cloud Run caller) |
| `/computers/seen` | 5,551 | `coder-terminal/0.5.0`, owner home |
| `/api/v1/models` | 1,407 | no user agent |
| `/health` | 1,077 | uptime checks |

Most of these went through api.openagents.com and stopped with it at 13:00 UTC
today. Since then openagents.com gets about 420 requests an hour.

**Top 50 paths over 7 days (B, internal pollers excluded):** count, then path.

```
39324 /v1/verse/studio/session      (owner's browsers, 10 IPs)
 5463 /forge/tenant.openagents/omega (bots, 404)
 4823 /                             (2,316 IPs)
 3886 /suggestions/config.json      (owner)
 2523 /api/nodes/ping
 1941 /pylons/live
 1879 /login
 1859 /.env                         (scanner)
 1713 /a/r                          (analytics beacon)
 1600 /robots.txt
 1161 /gpui/side
 1011 /api/mdk                      (MoneyDevKit pinger)
  734 /api/operator/fleet/state
  696 /proc/self/environ            (scanner)
  695 /v1/home/issues
  694 /a/v                          (analytics beacon)
  566 /chat
  532 /mcp/docs
  521 /api/agents/register
  520 /.env.local                   (scanner)
  515 /app
  494 /v1/threads
  492 /signup
  451 /v1/verse/key
  440 /new-lander/ui.css
  435 /@fs/.env                     (scanner)
  428 /auth/workos/github
  421 /docs
  415 /static/ui.css
  402 /.env.production              (scanner)
  401 /sitemap.xml
  388 /favicon.svg
  372 /v1/work
  350 /__vite_rsc_findSourceMapURL  (scanner)
  337 /api/mdk/api/mdk
  330 /.git/config                  (scanner)
  328 /new-lander/ui.js
  321 /.env.development             (scanner)
  315 /favicon.ico
  311 /api
  292 /forum/t/{id}                 (404)
  284 /v1/work/wrk61a1f3a7254381c6d5017ed2
  284 /api/traces/{id}/blob/00-login-page.png
  284 /api/traces/{id}/blob/session.webm
  283 /static/r.js
  274 /docs/coder
  269 /settings
  256 /environments
  250 /api/public/khala-code/plans
  249 /download
```

**Who (B):**

| Class | Requests | Notes |
|---|---|---|
| Owner (home network `2603:8080:e900:4359::/64`, `72.177.109.202`) | 53,585 | Verse studio, suggestions, smoke runs |
| Scanners (`.env`, `.git`, `wp-*`, `.php`) | 29,837 | all 403/404 |
| Bots, agents and tools (crawlers, curl, SDKs, non-browser user agents) | 37,971 | |
| Other browsers | 49,467 | 5,711 IPs; 2,800 home views from 1,691 IPs |

People who aren't the owner mostly reach the homepage and the docs. In 7 days
the docs (`/docs*`) had 10,690 requests from 2,569 IPs, mostly crawlers. The
"The Verse" iOS app (CFNetwork, one IP) called `/v1/device`, `/v1/threads`,
`/v1/home` and `/v1/work`, about 1,500 times in total. That IP is probably a
tester's build made from the deleted Rust mobile crate.

## Route inventory: keep, move or drop

Traffic is 7 days from pull B unless it says per day.

| Route(s) | Traffic | Decision | Lands in Coaxial as |
|---|---|---|---|
| `/` and Coax assets | 4,823 | **move** (already built) | bucket `openagentsgemini-openagents-web` |
| `/llms.txt`, `/llms-full.txt`, `/index.md` | 147 / 58 / 50 | **keep** | static text in the bucket, generated from the Coax docs |
| `/robots.txt`, `/sitemap.xml` | 2,001 | **keep** | static files in the bucket |
| `/docs`, `/docs/*`, `*.md` twins | 10,690 + 1,348 | **keep, rewritten** | static Markdown and text pages; drop docs for products that are gone |
| `/download`, `/download.md`, `/pricing*`, `/auth.md` | 260 / 72 / few | **keep, rewritten** | static pages; `/pricing` and `/auth.md` must match what exists |
| `/cli/install.sh`, `/cli/install.ps1` | 176 from 53 IPs | **keep** | static scripts in the bucket (binaries already in `gs://openagentsgemini-cli-releases`) |
| `/.well-known/apple-app-site-association`, `assetlinks.json` | 16+ | **keep** if the phone app keeps universal links | static JSON |
| `/.well-known/security.txt` | 86 | **keep** | static |
| `/.well-known/nostr.json` | low | **keep** | static NIP-05 file from the Nostr stack |
| `/.well-known/agent-card.json`, `ai-catalog.json`, `api-catalog`, `mcp*.json`, `openagents.json`, `x402*` | about 400 | **keep only the ones that describe a real API**, else drop | static JSON from the project/Nostr stack |
| `/openapi.json`, `/api/openapi.json` | 194 (45 are 502s) | **move** when the new API exists | static OpenAPI for the project/Nostr API |
| `/blog`, `/blog/*` | 452 (266 are 404s) | **keep** posts that exist | static pages |
| `/mcp/docs` (public docs MCP) | 532 from 11 IPs | **decide**; low use | later: a docs MCP on the new stack, or drop |
| `/mcp`, `/oauth/*`, `/.well-known/oauth-*` | 27 / 22 / 53 | **drop** with the accounts | none |
| `/login`, `/signup`, `/auth/*`, `/device`, `/v1/device/*` | 4,618 / 414 | **drop** (WorkOS, GitHub, device login) | Nostr key sign-in on the new stack, if any |
| `/chat*`, `/app`, `/settings*`, `/environments*`, `/projects/*` | 2,546 / 515 / 269 / 256 | **drop** | none (one conversation lives in the apps) |
| `/v1/projects/*` | 99 | **move** | project/Nostr API (`/v1/projects/coax/tasks`, claims) |
| `/v1/verse/*`, `/verse`, `/new-lander/*`, `/games/*`, `/everglade/*`, `/gpui/*` | 40,564 / 2,829 / 1,161 | **drop** (owner only) or move to Verse hosting | `verse-assets` or the Coax world |
| `/v1/foreman/*`, `/v1/agent-assignments`, `/v1/labs/*`, `/v1/history/*`, `/v1/work*`, `/v1/home/*`, `/computers/seen`, `/suggestions/*`, `/api/operator/*` | about 175,000 a day, owner tools | **drop**; switch the owner's tools off first | none, or the project/Nostr stack |
| `/v1/account`, `/v1/keys`, `/v1/threads`, `/v1/computers`, `/v1/files`, `/v1/connections` | about 26,000 a day (via api.) plus about 700 | **drop**, after exporting accounts | none |
| `/v1/chat/completions`, `/v1/responses`, `/v1/models`, `/api/v1/*`, `/api/v1/systemone`, `/v1/decisions` (gateway) | about 7,800 a day (decision-throughput, models) | **drop**, or move the decision path to its own service | Decisions API owner decides |
| `/v1/earnings/*`, `/v1/billing/*` webhooks, `/api/mdk*` | 12 / few / 1,348 | **drop** after confirming nobody is owed money | MoneyDevKit and Stripe dashboards must stop pointing here |
| `/lnurlp/*` | 68 (all 503) | **drop** here; Lightning addresses move to `lnurl-hooks` | the Lightning host |
| `/pylons/live`, `/api/nodes/ping`, `/api/agents/register` | 1,941 / 2,523 / 521 | **decide** (Pylon heartbeat callers) | the Nostr relay or the project stack |
| `/stats`, `/live`, `/pilot`, `/a/r`, `/a/v` (analytics) | low / 2,407 beacons | **drop** | none |
| `/forge/*`, `/forum/*`, `/api/traces/*`, `/api/public/khala-code/*` | 5,466 (404) / 383 / 568 / 250 | **drop** | none |
| `/plugins`, `/markdown` | 11 / 1 | not routes; nothing to do | none |
| scanner paths | 29,837 | **drop** (bucket 404) | none |
| `/health` | about 1,000 a day | **drop** with the service | the load balancer has its own checks |

## Data and accounts to preserve or export

`coder` (through its gateway) is the authority for OpenAgents accounts today.
Export all of this before anything is deleted, even if the new stack never
imports it.

1. **Postgres `openagents` on `openagents-production-pg`**
   - schema `identity`: accounts, principals, linked identities, sessions,
     device sign-ins, credentials, bearer keys (SHA-256 digests), provider
     keys, GitHub access, projects, project members, WorkOS users;
   - schema `workspace`: workspaces, memberships, invitations, WorkOS orgs;
   - schema `money`: named invoices, published earnings, receipt key;
   - schemas `audit` and `actor`.
   Export: `gcloud sql export sql` to a new private bucket.
   The same instance holds the `lnurl` database, so the instance stays until
   Lightning moves.
2. **Labs database** (secret `openagents-labs-production-pg-url`;
   `lab_space_indexes`). Which instance it lives on is unconfirmed. Export it
   the same way.
3. **NFS `/srv/accounts/stack`**: the gateway registry, `inference-keys.json`
   (limits and spend), `quota-ledger.jsonl` (balances), receipts,
   `service.key` (the house key), `own-runs.key`, `worker/usage`, and the
   legacy `accounts.json`, `sessions.json` and `keys.json`. Copy it to a
   private bucket. Treat the key files as secrets.
4. **NFS `/srv/accounts/web`**: `byo/` (sealed own-Claude credentials) and
   `environments/`. The sealed credentials belong to users. Either delete them
   at the end with the owner's yes, or keep them sealed. Never decrypt them for
   export.
5. **Buckets**: `openagentsgemini-web-chats-prod` (590 MiB of chats and labs
   files), `openagentsgemini-web-analytics-prod` (19 MiB),
   `openagentsgemini-web-vault-prod` (empty). Keep them; they cost cents.
6. **Money owed**: check `money.published_earnings` and the quota ledger for
   any non-zero balance or unpaid earnings before the gateway stops.
7. **Secrets**: the names are listed above. Most are only used by `coder`
   (`openagents-web-*`, `openagents-gateway-production-*`). Shared ones
   (`openagents-vertex-sa-key`, `openagents-openrouter-api-key`,
   `openagents-production-pg-dsn`, `openagents-resend-api-key`,
   `coder-pool-git-token`) need a check for other users before anyone
   disables them.

## Dependents to update

- **Current iOS app (`apps/openagents-ios`)**: calls nothing on openagents.com.
  It needs only the universal-links file if links stay.
- **Older iOS builds ("The Verse")**: call `/v1/device`, `/v1/threads`,
  `/v1/home`, `/v1/zones` and `sync.openagents.com`. They break at cutover.
  Expire those TestFlight builds.
- **Coder desktop (`coder-desktop`)**: only links to `/download` and `/docs`.
  Keep both pages.
- **Coder CLI already installed** (from deleted `crates/openagents-cli`):
  device login and all `/v1/*` calls go to openagents.com and
  api.openagents.com. Those already fail on api. Installers stay at
  `/cli/install.*`; a new CLI must not depend on these routes.
- **sync.openagents.com**: already down; it depended on `/v1/account`,
  `/v1/computers/*` and `/v1/threads/synced`. Nothing to cut over, only to
  clean up.
- **Nostr relay (`openagents-nostr-relay`)**: has no push-gateway setting, so
  it does not call `coder`. The Coaxial project page uses the relay directly.
  No change.
- **Pylon**: `/v1/earnings/*` on api.openagents.com (down) and heartbeats
  (`/pylons/live`, `/api/nodes/ping`).
- **Owner tools still polling**: grid-foreman (`/v1/foreman/*`), the
  agent-assignments, labs and history pollers on the owner's Mac,
  `coder-terminal/0.5.0` (`/computers/seen`), `oa-solver-dispatch-1`
  (`/v1/work-hosts/*/claim`), and `openagents-decision-throughput`
  (`/api/v1/systemone`). Switch these off or repoint them first.
- **Webhooks and redirect URLs registered in dashboards**: Stripe
  (`/v1/billing/webhook`, `/v1/billing/prepaid/webhook`), MoneyDevKit
  (`/v1/billing/mdk-webhook`; its pinger hits `/api/mdk`), WorkOS redirect
  (`/auth/workos/*`), GitHub OAuth app (`/auth/github/callback`), GitHub App
  grant (`/v1/account/github/app/*`), Google connect (`/auth/google/connect`).
  Remove or repoint each in its dashboard.
- **In this repo**: `coax/web/smoke.sh --coder` (checks `/llms.txt`,
  `/index.md`, `/v1/projects`, `/docs`) and `coax/web/release.sh:77`. Update
  them as each route moves.

## Related services and domains

| Resource | State | Relation |
|---|---|---|
| Domain mapping `openagents.com` → `coder` | live | DNS is the Cloud Run mapping (216.239.32–38.21) |
| Domain mapping `new.openagents.com` → `coder` | live, 6,207 requests in 7 days (403 at `/`) | drop with `coder` |
| Domain mapping `l402.openagents.com` → `l402-aperture` | stale, service gone | drop |
| Domain mapping `staging.openagents.com` → `openagents-web-staging` | DNS now points to the Coaxial balancer | Coax staging (`coaxial-serve` image), not legacy |
| Domain mapping `relay.openagents.com` → `openagents-nostr-relay` | live | keep |
| Coaxial balancer `136.69.24.60` (URL map `coaxial-openagents-web-staging`) | live, cert map covers openagents.com and staging | host `openagents.com`: `/`, `/index.html`, `/favicon.svg`, `/build/*`, `/coaxial/*` go to the bucket, everything else to backend `coaxial-coder` (serverless NEG → `coder`) |
| NEG and backend `openagents-coder-neg` / `openagents-coder-api-backend` | no URL map uses it | was api.openagents.com |
| URL map `openagents-url-map` | no HTTPS proxy in front | old redirects for auth., coder., fleet., sarah., components. |
| URL map `coder-sync`, backends `coder-sync-vm`, `coder-sync-run`, NEG `coder-sync-run-neg` | no forwarding rule; Cloud Run `coder-sync` gone | sync leftovers |
| NEGs whose Cloud Run service is gone | `openagents-neg` (openagents-monolith), `openagents-forge-git-neg`, `sarah-neg`, `simply-sarah-neg`, `pro-gateway-neg`, `effect-native-gallery-neg`, `oa-convex-backend-prod-neg`, `one-*-neg` | leftovers; not part of `coder` |
| Cloud Run `verse-assets` | live, no min instances | Verse asset host, separate |
| Cloud Run `aethyr-*` | live | separate project |

## Plan

Each step can be undone until the deletion steps at the end.

1. **Freeze.** Don't deploy to `coder` again. Nothing can build it anyway.
   Undo: not needed.
2. **Export data** (read-only for the service): run a SQL export of
   `openagents` and the labs database, copy both NFS trees, and copy the
   bucket listings into a new private bucket such as
   `openagentsgemini-coder-archive`. Record row counts per table in this doc.
   Undo: not needed.
3. **Switch off our own pollers**: grid-foreman, the Mac pollers,
   coder-terminal, `oa-solver-dispatch-1`, decision-throughput. This removes
   about 99% of the requests. Undo: start them again.
4. **Put openagents.com behind the Coaxial balancer**: point the
   `openagents.com` A records at `136.69.24.60`. The URL map already sends
   everything except `/` and the Coax assets to `coder`, so nothing else
   changes. Run `coax/web/smoke.sh https://openagents.com --coder`.
   Undo: point DNS back at the Cloud Run mapping; keep the mapping until
   step 9.
5. **Move the static routes one rule at a time**: publish each item marked
   **keep** to the bucket, then add its path to the URL map's bucket path
   rule. Order: `/llms.txt`, `/llms-full.txt`, `/index.md`, `/robots.txt`,
   `/sitemap.xml`, `/.well-known/security.txt`,
   `/.well-known/apple-app-site-association`, `/.well-known/nostr.json`,
   `/cli/install.sh`, `/cli/install.ps1`, `/download*`, `/docs*`, `/blog*`,
   `/pricing*`. Run the smoke after each. Undo: remove the path rule.
6. **Send everything else to a 404 page in the bucket** for a week: change the
   URL map's default for `openagents.com` from `coaxial-coder` to the bucket.
   Watch the logs for requests from people that we didn't expect. Undo: set
   the default back to `coaxial-coder`. `coder` is still running.
7. **Repoint dashboards**: Stripe, MoneyDevKit, WorkOS, GitHub OAuth and the
   GitHub App. Disable each callback or point it at a page that says the
   service is closed. Undo: restore each URL.
8. **Scale `coder` to zero**: set min instances 0 and CPU throttling on. It
   then costs almost nothing while idle and is still there to roll back to.
   Undo: set min 1 again.
9. **Delete, only with the owner's explicit yes to each name below:**
   - Cloud Run service `coder`, including its 439 revisions and 18 tags;
   - domain mappings `new.openagents.com`, `openagents.com` (after step 4) and
     `l402.openagents.com`;
   - backend service `coaxial-coder` and NEG `coaxial-coder`;
   - backend service `openagents-coder-api-backend` and NEG
     `openagents-coder-neg`;
   - DNS records `api.openagents.com`, `new.openagents.com`,
     `sync.openagents.com` and `l402.openagents.com`;
   - sync leftovers: URL map `coder-sync`, backends `coder-sync-vm` and
     `coder-sync-run`, NEG `coder-sync-run-neg`, instance group `coder-sync-ig`
     and its VM `coder-sync-1`;
   - URL map `openagents-url-map`;
   - NFS share `oa-accounts-nfs-production` (`10.42.27.2`), only after step 2
     is verified;
   - Secret Manager secrets used only by `coder` (the `openagents-web-*` and
     `openagents-gateway-production-*` lists above), disabled first for 30
     days, then destroyed;
   - Artifact Registry images `openagents/openagents-web` and
     `openagents/openagents-stack` older than the serving digests. Keep the
     serving digests until the service itself is deleted.
   - **Keep**: `openagents-production-pg` (it holds `lnurl`), the three
     buckets, `openagents-nostr-relay`, `openagents-web-staging` and
     `verse-assets`.

## Cost saved

- **`coder` itself:** about $150 a month. The service is always on with CPU
  always allocated: 3 vCPU and 1.5 GiB for 730 hours. At us-central1
  instance-based rates ($0.000018 per vCPU-second, $0.000002 per GiB-second)
  that is about $142 for CPU and $8 for memory, plus egress.
- **NFS share:** about $160 to $200 a month more, if `oa-accounts-nfs-production`
  is a Basic HDD Filestore (1 TiB minimum). The tier is unconfirmed because the
  automation account cannot read Filestore.
- **Cloud SQL `openagents-production-pg`:** about $50 a month, saved only once
  the `lnurl` database moves too.
- **The rest** (the sync VM, if it still runs; old images; leftover backends)
  is small but worth removing.

Expected total: **about $150 a month for sure, about $350 a month with the
NFS share, and about $400 a month if the database also goes.** All of it
counts against the $1,500-a-month cap.

## Owner decisions needed

1. Should the account data (users, keys, sessions, earnings, ledger) be
   archived only, or imported into the new project/Nostr stack?
2. What happens to users' sealed own-Claude credentials in `/state/byo`:
   delete them, or keep them sealed in the archive?
3. Will we bring back api.openagents.com and sync.openagents.com (both down
   now), or retire them for good?
4. Should the public docs MCP (`/mcp/docs`, 11 callers in 7 days) and the
   `.well-known` agent and API catalogs be rebuilt, or dropped until the new
   API exists?
5. Should the Pylon heartbeat and earnings endpoints move to the new stack, or
   stop?
6. Should the decision-throughput caller (`/api/v1/systemone`, about 6,400 a
   day) get its own service, or be switched off?
7. Yes or no, by name, for each deletion in step 9.

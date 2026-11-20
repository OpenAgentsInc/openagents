# openagents.com live surface audit

Date: 2026-11-20. Scope: everything that answers at `https://openagents.com`
today, the code and infrastructure behind it, and the clients that call it.
Purpose: the input for zero-basing the site down to one Coax-native page.

Method:

- Routing comes from the live load balancer's URL map
  (`gcloud compute url-maps describe coaxial-openagents-web-staging`), not
  from the repository. The repository's copy of those rules is
  `coax/web/api/lb.sh`; the dev server's copy is `coax/web/site-routes.coax`.
- Pages come from listing the production bucket
  `gs://openagentsgemini-openagents-web`.
- API routes come from the rule table in `coax/web/api/auth.c` (`RULES[]`)
  and the dispatch order in `coax/web/api/api.c`.
- Every status below is from a plain unauthenticated `curl` GET on
  2026-11-20 (no cookies, no login, no writes).
- Infrastructure is from read-only `gcloud ... list/describe` with the
  automation service account.

## Summary

| Fact | Value |
| --- | --- |
| Front door | One global external HTTPS load balancer, IP `136.69.24.60`, URL map `coaxial-openagents-web-staging` (the name says staging; it serves production too) |
| Hosts on it | `openagents.com`, `staging.openagents.com`, `everglade.world`, `www.everglade.world` |
| Static site | Bucket `openagentsgemini-openagents-web` (Cloud CDN on), 4.0 GB, 807 `index.html` pages, 8,996 objects under `/build/` |
| Site source | `apps/openagents-web/` (384 entries), built by `coax/web/deploy.sh`, shipped only by `coax/web/release.sh` |
| Last site release | Commit `b80819d924` (2026-11-13). `/release.lock` holds a lock taken 2026-11-19 05:11 UTC |
| API | Cloud Run `oa-api` (C, `coax/web/api/`, ~300 route rules), Postgres `openagents-production-pg` |
| Game rooms | `/api/room/*` goes to VM `oa-room-1` through instance group `oa-room-vm-ig` |
| Relay | `relay.openagents.com` is VM `oa-relay-1`, not on the load balancer |
| Old Worker / Laravel / Nexus | Not live. `apps/openagents.com` no longer exists in the tree; `nexus.openagents.com` has no DNS |

## How a request is routed

Order of evaluation on the `openagents.com` host (lowest priority number
first). Anything no rule names falls to the default: the site bucket, which
answers with the Coax `404.html` for a missing object.

| Priority | Paths | Goes to |
| --- | --- | --- |
| 2–8 | `/settings/{agents,security,data,delete,devices,lightning,wallet}` and below | Bucket |
| 10, 20, 230–240 | `/api/room/s1/*` … `/api/room/s6/*`, `/api/room/*` | Backend services `oa-room-vm-s1`…`s6`, all on `oa-room-vm-ig` (one VM) |
| 30–60 | `/realms`, `/realms/*`, `/realm`, `/realm/*` | 301 to `/worlds…` |
| 70–100 | `/projects/{phone-fixture,agent-check-off,agent-check,cakbam}` | `oa-api` (private projects) |
| 110 | `/projects/*` | Bucket; a 404 is replaced by `/projects/private/index.html` (still 404) |
| 120 | `/signin`, `/signin/*`, `/logout`, `/signup` | 302 to `/login` |
| 130 | `/settings`, `/settings/*` (the rest) | 302 to `/login` |
| 140 | `/docs`, `/docs/`, `/docs/api`, `/docs/api/*` | 302 to `/docs/connect-a-pylon/` |
| 150 | `/verse/galaxy`, `/verse/galaxy/*` | 301 to `/worlds` |
| 160–170 | `/worlds/everglade-5e`, `/worlds/everglade-5e/*` | 301 to `/worlds/everglade…` |
| 180–190 | `/worlds`, `/worlds/*` | Bucket, rewritten to `/realms…` |
| 200–210 | `/project`, `/project/*` | 301 to `/projects…` |
| 220 | `/`, `/index.html`, `/favicon.svg`, `/projects`, `/build/*`, `/coaxial/*`, `/projects/*`, `/verse`, `/verse/*`, `/demo/*`, `/admin/analytics*`, `/.well-known/apple-app-site-association`, `/chat*`, `/login*`, `/chat-gpu*` | Bucket |
| 250 | `/api/*` | Cloud Run `oa-api` |
| 260 | `/memorial`, `/memorial/*` | `oa-api` |
| 270 | `/.well-known/lnurlp/*`, `/lnurlp/*` | `oa-api` |
| 280 | `/.well-known/nostr.json` | `oa-api` |
| default | everything else | Bucket |

The `everglade.world` matcher serves Everglade pages out of the same bucket
(`/` → `/realms/everglade2/domain/index.html`, `/play`, `/worlds/*` rewrites,
any other path → an Everglade "missing" page) and sends `/api/room/*` and
`/api/*` to the same room VM and `oa-api`.

### Drift between the repository and the live map

`lb.sh` describes rules for the Metaverse rename (#13405, #13486) that are
not in the live map:

| Path | lb.sh says | Production today | Staging today |
| --- | --- | --- | --- |
| `/verse`, `/verse/*` | 301 to `/metaverse` | 200, the old "The Verse" page (bucket object from 2026-11-13) | 404 |
| `/projects/metaverse` | Bucket `/projects/verse` | 404 | 404 |
| `/projects/verse` | 301 to `/projects/metaverse` | 200, the project page | — |
| `/metaverse/galaxy` | 301 to `/worlds` | 404 | 404 |
| `/forge` | 301 to `/worlds/new` | 404 | 404 |
| `/worlds/splice` | 301 to `/worlds/splice-bots` | 200, the old "Splice" page | 404 |

`/llms.txt` links `/projects/metaverse/index.md`, which is 404. The sitemap
still lists `/realms/*` (301s) and `/projects/verse`.

## Pages

"Login" means the page's data needs a session. Every page shell below is a
public static file; the data comes from `/api/*`, which checks the session.
"Linked" means reachable from the homepage, its nav, `/sitemap.xml` or
`/llms.txt`. "Orphan" means none of those link it.

### Home and marketing

| Path | Status | What it is | Source | Login | Linked |
| --- | --- | --- | --- | --- | --- |
| `/` | 200 | Home: six featured projects, signed task claims and live Pylons on the Grid (WebGPU, Nostr reads) | `newland.coax`, `newland-*.coax`, `landing-*.coax` | No | Home |
| `/index.md` | 200 | Markdown twin of the home | build | No | Home `<link>` |
| `/new-land/` | 200 | Alias of the home | `newland.coax` | No | Sitemap |
| `/newlander/` | 200 | Another home variant | build | No | Orphan |
| `/lander-bd8e3739/` | 200 | Another home variant (hashed name) | build | No | Orphan |
| `/download` | 200 | iPhone TestFlight link | `download/` | No | Sitemap |
| `/invite` | 200 | Invite landing (log in, make a world) | build | No | Sitemap |
| `/status` | 200 | Release pipeline status on the Grid (production commit, queue, recent commits) | `status-data.coax` | No | Sitemap |
| `/og/` | 200 | Link-preview gallery plus `/og/*.jpg` cards | `coax/web/og*.ts` | No | Orphan (images used by every page) |

### Metaverse, worlds and games

| Path | Status | What it is | Source | Login | Linked |
| --- | --- | --- | --- | --- | --- |
| `/metaverse` | 200 | Metaverse landing; PLAYTEST NOW opens Haunted House | `verse.coax` | No (guest) | Home |
| `/verse`, `/verse/` | 200 | Old "The Verse" landing, stale | bucket leftover | No | Orphan |
| `/verse/galaxy` | 301 | → `/worlds` | LB | — | — |
| `/worlds` | 200 | World list | `realm*` pages, bucket `realms/` | No | `/metaverse` |
| `/worlds/new` | 200 | Build your own world (editor) | `metaverse-engine/editor/` | Account to save | `/worlds` |
| `/worlds/<slug>` | 200 | One world: The Grid, Everglade, Everglade II, Haunted House, Splice Bots, Characters, Garden, Character Studio, Hecate's Cemetery, Belt 2200, Dead Gears, Hold the Stair, Ice Cream Man, Orbit League, archery range, labs and more | `apps/openagents-web/*.coax`, `metaverse/`, `everglade/` | Guest play; account for saves | Partly (`/worlds`, sitemap) |
| `/worlds/hecates-cemetery/memorial` | 200 | Pet memorial page | villa | No | Sitemap |
| `/worlds/splice` | 200 | Old Splice page, stale | bucket leftover | No | Orphan |
| `/worlds/everglade2/llms.txt`, `/worlds/splice-bots/llms.txt` | 200 | Agent references per world build | build | No | `/llms.txt` |
| `/worlds/<made-realm>` | 200 | Player-made realms written by the API into the bucket. Includes about 25 `aethyrforge-journey-test-*` test realms and one named `<img src=x onerror=alert(1)>` (rendered escaped) | `oa-api` `realms.c` | No | Orphan |
| `/create-world` | 200 | WorldBuilder chat (`/api/worldbuilder/*`) | `worldbuilder/` | No (capped per address) | Orphan |
| `/druid` | 200 | Druid world page | build | No | Orphan |
| `/memorial/<slug>` | 404 for unknown | A pet memorial, HTML rendered by the API | `villa.c` | No | From the cemetery |
| `/realms*`, `/realm*` | 301 | → `/worlds…` | LB | — | Sitemap still uses them |
| `/demo/arcade-chat/`, `/demo/arcade-feed/`, `/demo/arcade-messages/`, `/demo/microduck/` | 200 | Pinned demo builds, with per-build copies under `builds/` | `coax/web/pin.sh` | No | Orphan |
| `/demos/grid-crowd`, `/demos/speedrun-hud` | 200 | Engine demos | `demos/` | No | Sitemap |

`everglade.world` serves Everglade's own landing (`/`) and play page
(`/play`) from the same bucket and API. `www.everglade.world` 301s to the
apex.

### Projects

| Path | Status | What it is | Source | Login | Linked |
| --- | --- | --- | --- | --- | --- |
| `/projects` | 200 | Public projects on the relay with tasks | `project*.coax` | No | Home |
| `/projects/coax`, `/projects/coax/demo` | 200 | Coax project and its demo | `project*.coax` | No | Home, sitemap |
| `/projects/{coder,verse,psionic,openagents-desktop,planetary-villa,belt-2200,orbit-league,hold-the-stair}` | 200 | Project pages | `project*.coax`, Nostr | No | Sitemap |
| `/projects/{dead-gears,hello,oa44541,scene}` | 200 | Project pages (`hello`, `oa44541` look like tests) | same | No | Orphan |
| `/projects/metaverse` | 404 | Should be the Metaverse project | missing LB rule | — | `/llms.txt` (broken) |
| `/projects/private/` | 200 | The locked private-project page | `private.coax` | — | — |
| `/projects/{cakbam,phone-fixture,agent-check,agent-check-off}` | 404 to strangers | Private projects, members only, HTML from the API | `private.c` | Yes (member) | Private |
| `/project*` | 301 | → `/projects…` | LB | — | — |

### Chat and components

| Path | Status | What it is | Source | Login | Linked |
| --- | --- | --- | --- | --- | --- |
| `/chat` | 200 | Rebuilt chat shell; "nothing is sent yet" | `chat-web.coax`, `components/` | — | `/llms.txt` |
| `/chat-gpu` | 200 | Earlier chat page (direct agent chat) | `chat.coax` | Admins | `/llms.txt` |
| `/components` | 200 | DOM component catalog | `components.coax` | No | Sitemap |
| `/components-gpu` | 200 | GPU component catalog (desktop app still uses it) | `components-gpu/` | No | Sitemap |
| `/feed`, `/messages` | 200 | Arcade-port Nostr feed and messages pages | `demos/`, arcade port | Nostr key | Orphan |
| `/nostr-chat` | 200 | "Nostr from scratch" chat | `nostr-chat.coax` | No | Sitemap |

### Docs and references

| Path | Status | What it is | Source | Linked |
| --- | --- | --- | --- | --- |
| `/docs/connect-a-pylon/` | 200 | Pylon quickstart, plus `confidential-round.json` | `connect-pylon*.coax`, `coax/web/connect-pylon-page.ts` | Home |
| `/docs/limits/` | 200 | Every enforced limit (from `api --limits`) | build | Sitemap |
| `/docs`, `/docs/api*` | 302 | → `/docs/connect-a-pylon/` | LB | — |
| `/nip`, `/nip/NN` (22 pages) | 200 | NIP implementation guides | `nip/`, `coax/web/nip-pages.py` | Home |
| `/coax/changelog` | 200 | Coax compiler changelog (63 KB) | `changelog*.coax` | `/llms.txt` |
| `/pylon` | 200 | Pylon board; `/pylon/join.sh` is the join script | `pylon/` | `/llms.txt` |
| `/llms.txt`, `/llms-full.txt` | 200 | Agent index (4.5 KB) and everything as one file (789 KB) | build | `robots.txt` |
| `/robots.txt`, `/sitemap.xml` | 200 | Crawl rules; 62 URLs | build | — |
| `/scoped/coax-changelog/<time>/…` | 200 | Old scoped changelog builds | release leftovers | Orphan |
| `/releases/<time>/` (596) | 200 | Every past release's snapshot, publicly served by the default rule | `release.sh` | Orphan |

Most pages have an `index.md` twin at the same address.

### Account, login and settings

| Path | Status | What it is | Source | Login |
| --- | --- | --- | --- | --- |
| `/login`, `/login/email/` | 200 | Email code login, Nostr, passkey, phone QR | `private.coax` (page 2), `login/` | No |
| `/login/device/` | 200 | Approve a computer (`openagents login`) | `login/` | Yes |
| `/signin*`, `/logout`, `/signup` | 302 | → `/login` | LB | — |
| `/settings/{agents,security,data,delete,devices,lightning,wallet}` | 200 | Agents and grants, ways in and recovery, export/merge/delete, device keys, Lightning Address, wallet limit and backup | `agents.coax`, `devices.coax`, `settings/` | Yes |
| `/settings`, `/settings/{account,about,appearance,models,limits}` | 302 | → `/login`; the bucket still holds these pages but the LB never serves them | LB | — |

### Admin

| Path | Status | What it is | Login |
| --- | --- | --- | --- |
| `/admin/analytics` | 200 | Visits and sessions | Admin (NIP-98, `admins.conf`) |
| `/admin/access/`, `/admin/reports/`, `/admin/rooms/` | 200 | Realm invites, reports, room fleet | Admin |

The shells are public; `/api/admin/*` returns 401 without an admin session.

### Well-known files and static assets

| Path | Status | Served by | Notes |
| --- | --- | --- | --- |
| `/.well-known/apple-app-site-association` | 200 | Bucket | iOS universal links for `/login`, `/signin`, `/projects/*#invite=`; web credentials for `com.openagents.app` and `com.openagents.desktop` |
| `/.well-known/nostr.json` | 200 | `oa-api` | NIP-05 (`coax@`) |
| `/.well-known/lnurlp/<name>`, `/lnurlp/*` | 404 JSON for unknown | `oa-api` | Lightning Addresses (LUD-16) |
| `/.well-known/security.txt` | 200 | Bucket | Security contact |
| `/.well-known/assetlinks.json` | 404 | — | No Android app links |
| `/favicon.svg`, `/mark.svg`, `/character.wgsl` | 200 | Bucket | Icons; a shader at the root |
| `/build/*` | 200 per object | Bucket | 8,996 wasm, JS, WGSL and pack objects (content-hashed) |
| `/cli/install.sh`, `/cli/install.ps1` | 200 | Bucket | Installs the `coder` CLI from `gs://openagentsgemini-cli-releases` |
| `/404.html` | 200 | Bucket | Also the body of every 404 |
| `/release.lock` | 200 | Bucket | The release lock object, public. It names the machine, PID and user that hold it |

### OpenAPI and capability manifests

None are live. `/openapi.json`, `/api/openapi.json`, `/api/capabilities` and
`/.well-known/openagents.json` all return 404. The route contract lives in
the repository as `proto/verse/api-routes-v1alpha1.json` and
`proto/openagents/api/v1alpha1/`.

### Webhooks

There are no third-party webhook receivers. The only inbound machine callers
are Cloud Scheduler (`POST /api/jobs/<job>` with an OIDC token) and wallets
(`/.well-known/lnurlp/*`, `/lnurlp/*`).

### Trace pages

There is no `/trace/*` route on the live site. It falls to the bucket and
returns 404.

## API (`/api/*` on `oa-api`)

Source: `coax/web/api/` (C on Cloud Run). The access rule for each route is
in `auth.c` `RULES[]`; `api --rules` prints them. Groups, with the probe
result for one public GET in each:

| Group | Paths | Who | Probe |
| --- | --- | --- | --- |
| Health | `/api/playtest/health`, `/api/playtest/service-health`, `/api/playtest/crashes` | Public | 200 JSON. Health shows database name, migration, build SHA and revision |
| Auth and sessions | `/api/auth/*`: account, viewer, me, email start/verify/link, Nostr challenge, phone QR, CLI code/redeem, refresh, logout, sessions, freeze | Public or account | `/api/auth/account` 200 `{"logged_in":false}`; `/api/auth/me` 401 |
| Identity and Nostr custody | `/api/auth/identity/*`, `/api/auth/nostr/*`, `/api/auth/settings/*` | Account, some L2-fresh | 401 |
| Device keys, step-up, recovery, passkeys | `/api/auth/devices*`, `/api/auth/ways`, `/api/auth/step-up`, `/api/auth/recovery*`, `/api/auth/passkey*`, `/api/auth/life`, `/api/auth/export`, `/api/auth/merge/*`, `/api/auth/delete` | Account, L2 | — |
| CLI login | `/api/auth/device/{start,look,approve}`, `/api/auth/device/<id>` | NIP-98 / human | — |
| Agents and grants | `/api/agents*`, `/api/agent-keys/*`, `/api/agent/{me,owner,spend}` | Human, L2, agent key | `/api/agent-keys/x` 404 JSON |
| Wallet | `/api/wallet*` | Human, L2-fresh | 401 |
| Lightning Address | `/api/lightning/address*`, `/.well-known/lnurlp/*`, `/lnurlp/*`, `/.well-known/nostr.json` | Account / public | 401 / 404 JSON |
| Account chat (durable turns) | `/api/chat/{thread,complete,submit,stop,turn,context,input,events,conversations,conversation,library,history,project,route,delegated,world}/v*` | Human / account | 401 |
| Chat runner | `/api/chat/runner/*`, `/api/chat/runner/v2alpha1` | Runner key | — |
| Computers and tasks | `/api/chat/environment*/v1`, `/api/chat/environments/v1`, `/api/chat/task*/v1alpha1` | Account, HOST grant | — |
| Admin chat (`/chat-gpu`) | `/api/chat/{state,new,send,model,stop,tab,record,key,complete,account,turn}` | Admin | 401 |
| Room chat and presence | `/api/chat/<room>`, `/api/presence/<room>` | Guest | 200 JSON (Haunted House chat log is public) |
| GameBuilder agents | `/api/gamebuilder/agents/v1`, `/agent/v1`, `/agent/events/v1` | Human | — |
| Projects | `/api/projects/v1*` (private projects), `/api/projects`, `/api/projects/<id>` files | Account / public | `/api/projects/v1` 401; `/api/projects` 200 empty |
| Worlds | `/api/worlds`, `/api/worlds/<id>` (+ scripts) | Public read, admin write | 200 empty |
| Playtest and Metaverse | `/api/playtest/*` (players, realms, makers, seats, upgrades, labs, NPC ask, old links), `/api/metaverse/packs/*`, `/api/verse/packs/*`, `/api/villa/*` | Guest, checked per route | `/api/playtest/realms` 405 |
| Old demo paths | `/api/change/*`, `/api/log/*`, `/api/award`, `/api/planets` | Guest | `/api/planets` 200 JSON |
| Models and mesh | `/api/v1/inference`, `/api/v1/systemone` | NIP-98 or session, metered | 405 on GET |
| WorldBuilder | `/api/worldbuilder/*` | Public, capped | — |
| Private projects and drops | `/api/private/<slug>/*`, `/projects/<slug>`, `/api/drops*` | Members | 404 / 401 |
| Analytics | `/api/analytics/events` (guest), `/api/analytics/team`, `/api/analytics/realms` | Public | `realms` 200 text |
| Jobs | `/api/jobs/<job>` | Scheduler OIDC or admin | 405 on GET |
| Admin | `/api/admin/*` (flags, email, makers, upgrades, scripts, access) | Admin, L2 within five minutes for writes | 401 |

`/api/room/*` is not `oa-api`: it goes to the room VM (`/api/room/health`
answers 200 `ok 0 players …`).

## Infrastructure

Project `openagentsgemini`, region `us-central1`.

### In the request path

| Resource | Name | Role |
| --- | --- | --- |
| Global IP and forwarding rules | `coaxial-openagents-web-staging` (443), `…-http` (80, redirect) at `136.69.24.60` | Front door for all four hosts |
| Certificate map | `coaxial-openagents-web-staging`: `openagents.com`, `staging.openagents.com`, `everglade.world`, `www.everglade.world` | TLS |
| URL map | `coaxial-openagents-web-staging` | The rules above |
| Backend bucket | `coaxial-openagents-web` → `openagentsgemini-openagents-web` (CDN) | Production pages |
| Backend bucket | `coaxial-openagents-web-staging` → `openagentsgemini-openagents-web-staging` (CDN) | Staging pages |
| Backend service | `oa-api` → serverless NEG → Cloud Run `oa-api` (ingress: internal and LB only, max 4) | Production API |
| Backend service | `oa-api-staging` → Cloud Run `oa-api-staging` | Staging API |
| Backend services | `oa-room-vm-s1`…`s6`, `oa-room-vm-staging` → instance group `oa-room-vm-ig` → VM `oa-room-1` (e2-standard-2) | Game rooms (WebSocket); staging shares the VM |
| Cloud SQL | `openagents-production-pg` (Postgres 18, db-custom-1-3840, 10 GB, zonal); databases `openagents`, `oa_analytics`, `oa_analytics_staging`, `lnurl` | API data |

### Called by `oa-api`

From its environment variable names (values not read):

| Service | Kind | Used for |
| --- | --- | --- |
| `oa-agent` (min 1) | Cloud Run | Chat runner (`OA_AGENT_WAKE_URL`); mounts bucket `openagentsgemini-oa-chat-sessions` |
| `oa-spark` (min 1) | Cloud Run | Spark wallet (`OA_SPARK_URL`) |
| `oa-script-job` | Cloud Run | Sandboxed realm scripts (`OA_SCRIPT_JOB_URL`) |
| `oa-mesh-mammoth`, `oa-mesh-elephant` (min 1 each) | Cloud Run | System One mesh routes (`OA_MESH_ROUTES`) |
| Gemini | External | Decisions (`OA_GEMINI_DECISIONS_KEY`) |
| Resend | External | Email from `no-reply@openagents.com` |
| `relay.openagents.com` | VM | Publishing (`OA_PUBLISH_RELAY`) |
| Buckets | GCS | Site writes (`OA_SITE_BUCKET`), drops (`openagentsgemini-project-drops`), room checkpoints |
| Environment broker | Secret volumes | Computer/workspace control |

### Scheduled and batch

| Job | Schedule | Target |
| --- | --- | --- |
| `oa-api-service-watch` | Every minute | `POST https://openagents.com/api/jobs/service-watch` |
| `oa-api-heartbeat` | 07:00 Chicago | `POST https://openagents.com/api/jobs/heartbeat` |
| `oa-api-staging-service-watch`, `oa-api-staging-heartbeat` | Same | Staging |
| `oa-db-dump-nightly`, `oa-db-dump-auth-6h`, `oa-db-drill-monthly` | Nightly, 6-hourly, monthly | Cloud Run jobs `oa-db-dump`, `oa-db-dump-auth`, `oa-db-drill` (to `openagentsgemini-db-backups`) |
| `oa-api-migrate-production`, `oa-api-migrate-staging` | On deploy | Migrations job |

### Other `openagents.com` hosts

| Host | Today | Notes |
| --- | --- | --- |
| `staging.openagents.com` | 200, same LB | A stale Cloud Run domain mapping to `openagents-web-staging` also exists |
| `relay.openagents.com` | VM `oa-relay-1` (`34.45.121.213`), Coax relay, NIP-11 OK | A stale domain mapping to a Cloud Run service `openagents-nostr-relay` that no longer exists. `khala-sync-pg` holds `nostr_relay_prod` |
| `voice.openagents.com` | Cloud Run `oa-voice` (domain mapping) | Used by `coax/web/voice.js` |
| `updates.openagents.com` | DNS to Google-hosted; no answer | Coax Native Updates (owner plans to drop it) |
| `sync.openagents.com` | DNS still `136.69.24.60`; TLS fails (no cert, no host rule) | Dead. `coder-sync` URL map and `coder-sync-ig` (0 VMs) remain |
| `api.openagents.com` | DNS `35.201.75.247`; no answer | Not in this project's IPs |
| `coder.openagents.com` | DNS `136.68.142.56`; no answer | Retired coder service |
| `www.openagents.com` | Cloudflare; 301 to apex | Outside GCP |
| `docs.openagents.com` | Vercel, "OpenAgents Autopilot" docs | Stale product |
| `blog.openagents.com` | Vercel | Outside this repository |
| `nexus.openagents.com` | No DNS | Retired |

### Leftovers seen while auditing (not serving openagents.com)

- URL map `openagents-url-map` and `openagents-http-redirect` with no
  forwarding rule; backend services `openagents-backend`,
  `openagents-coder-api-backend`, `openagents-forge-git-backend`,
  `coder-sync-run`, `coder-sync-vm`, `oa-room` … `oa-room-6` (Cloud Run rooms,
  replaced by the VM).
- Cloud Run `openagents-web-staging` (`coaxial-serve`, 2026-10-28), `oa-room`
  … `oa-room-6`, `verse-assets`, `aethyrforge-editor`, `aethyrforge-preview`,
  `coax-phrase-gate-staging`.
- Cloud SQL `coder-pg` and `l402-aperture-db` (stopped).
- About 25 `aethyrforge-journey-test-*` realms and one XSS-named realm in the
  production bucket.

## Clients that depend on openagents.com

A zero-base must keep these answering, or move the clients first.

| Client | Source | Calls |
| --- | --- | --- |
| OpenAgents iOS (`com.openagents.app`) | `apps/openagents-ios/`, `apps/shared/app/` | `/api/auth/*` (email, Nostr, passkey, refresh, logout, account, identity, devices, recovery), `/api/chat/*` (state, submit, events, conversations, library, environment, task, turn-task), `/api/projects/v1*`, `/api/wallet*`, `/api/playtest/app-crash`; universal links via `/.well-known/apple-app-site-association` (`/login`, `/projects/*#invite=`) |
| OpenAgents Desktop (macOS, Linux; `com.openagents.desktop`) | `apps/openagents-desktop/`, `apps/shared/app/` | Same shared core: `/api/auth/*`, `/api/chat/*`, `/api/projects/v1*`; opens `/login` in the browser; web credentials in the AASA file |
| `openagents` / `coder` CLI | `apps/openagents-cli/`, `/cli/install.sh` | `/cli/install.sh` and `/cli/install.ps1`; `/api/auth/device/*` and `/login/device` (`openagents login`), `/api/auth/ways`, `/api/auth/unlink`, `/api/agents*`, `/settings/agents`, `/api/worlds*` (scripts, compile, report), `/api/playtest/{realms,seats,access,labs}`, `/api/admin/access`, `/api/drops/adopt`, `/api/projects*`, `/api/chat/task`, `/api/v1/systemone`; links to `/projects/<slug>`, `/worlds/*` |
| Coder host (`coder host`) | `coder/tui/host.coax` | Pairs over `wss://relay.openagents.com`; the person pastes its key in Settings on openagents.com |
| Pylon | `scripts/join-pylon.sh`, `psionic/` | `/pylon/join.sh` (downloaded by the quickstart), `wss://relay.openagents.com`, `POST /api/v1/systemone`; `/docs/connect-a-pylon/` is the documented entry |
| Everglade desktop and mobile | `apps/everglade-desktop/`, `apps/everglade-mobile/` | `/api/room/*` (WebSocket), `/api/auth/*`, `/worlds/*` pages (archery range, bitcoin lab, character creator, experimental) |
| Metaverse Engine desktop | `apps/metaverse-engine-desktop/` | `/api/room/relay`, `/api/playtest/{world,forge}` |
| Coax native libraries | `coax/native/` | `/api/auth/account`, `relay.openagents.com` |
| Wallets and Nostr clients | — | `/.well-known/lnurlp/*`, `/lnurlp/*`, `/.well-known/nostr.json` |
| Everglade on `everglade.world` | same LB | Bucket paths under `/realms/everglade2/`, `/api/room/*`, `/api/*` |
| Cloud Scheduler | — | `POST /api/jobs/{service-watch,heartbeat}` |
| Agents and crawlers | — | `/llms.txt`, `/llms-full.txt`, `index.md` twins, `/robots.txt`, `/sitemap.xml` |

## Zero-base: keep, keep as API/infra, remove

### The single Coax-native page keeps

- `/` with its `index.md` twin, `/favicon.svg`, `/mark.svg`, `/404.html`
  and the `/build/*` objects that one page needs.
- `/robots.txt`, `/sitemap.xml` (one URL), `/llms.txt` (rewritten to match).
- `/.well-known/security.txt`.
- One `/og/` card for the link preview.

### Must stay for the apps (API, files and infra)

- `/api/*` on Cloud Run `oa-api` and `openagents-production-pg`, at minimum
  the groups the iOS, desktop and CLI clients call: `/api/auth/*`,
  `/api/chat/*`, `/api/projects/v1*`, `/api/wallet*`, `/api/agents*`,
  `/api/agent-keys/*`, `/api/agent/*`, `/api/v1/systemone`,
  `/api/v1/inference`, `/api/jobs/*`, `/api/playtest/app-crash`,
  `/api/playtest/health`.
- `/login`, `/login/email/`, `/login/device/` (the CLI and desktop open
  them; universal links target `/login`), or an equivalent the apps are
  changed to use first.
- `/settings/agents` (the CLI points people there) and whatever settings
  page the Coder host pairing uses.
- `/.well-known/apple-app-site-association`, `/.well-known/nostr.json`,
  `/.well-known/lnurlp/*`, `/lnurlp/*`, `/api/lightning/address*`.
- `/cli/install.sh`, `/cli/install.ps1` and bucket
  `openagentsgemini-cli-releases`.
- `/pylon/join.sh` and `/docs/connect-a-pylon/` (Pylon quickstart), unless
  they move with a redirect.
- Cloud Run `oa-agent`, `oa-spark`, `oa-script-job`, `oa-mesh-*` as far as
  the kept API routes use them; Resend; the Scheduler jobs and DB dump jobs.
- The load balancer, certificate map and the `oa-api` backend.
- `relay.openagents.com` (VM `oa-relay-1`), which Pylons, the CLI and the
  Coder host use directly.
- Everglade: `/api/room/*` → `oa-room-1`, and the `everglade.world` host
  with its bucket paths, unless Everglade moves off this LB first.
- Any redirect for old addresses that clients still send
  (`/signin` → `/login` is in the AASA file).

### Can be removed

- Home variants: `/new-land/`, `/newlander/`, `/lander-bd8e3739/`.
- Stale pages: `/verse`, `/verse/`, `/worlds/splice`, `/scoped/*`, and the
  bucket-only `/settings/{account,about,appearance,models,limits}` pages.
- `/releases/*` (596 public snapshots) from the public path; keep them
  private for rollback if needed.
- `/release.lock` from the public path (move the lock out of the served
  bucket).
- Test and junk realms: `aethyrforge-journey-test-*`, the XSS-named realm,
  `releases-commit-script-ale`; test projects `hello`, `oa44541`, `scene`.
- Demos: `/demo/*`, `/demos/*`, `/druid`, `/feed`, `/messages`,
  `/nostr-chat`, `/og/` gallery page.
- Catalogs: `/components`, `/components-gpu` (move to a dev-only build;
  the desktop app consumes the components as code, not as this page).
- `/chat`, `/chat-gpu` and the admin-only `/api/chat/{state,new,send,…}`
  routes, once nothing calls them.
- `/metaverse`, `/worlds`, `/worlds/*`, `/create-world`, `/invite`,
  `/memorial*` and their API groups (`/api/playtest/*` beyond app-crash and
  health, `/api/worlds*`, `/api/worldbuilder/*`, `/api/metaverse/packs/*`,
  `/api/verse/packs/*`, `/api/villa/*`, `/api/analytics/realms`), if the
  Metaverse leaves openagents.com. The CLI's world commands and the
  Everglade apps must move with them.
- Old demo API paths: `/api/change/*`, `/api/log/*`, `/api/award`,
  `/api/planets`, `/api/playtest/old-link`.
- `/projects`, `/projects/*`, `/coax/changelog`, `/nip`, `/nip/*`,
  `/status`, `/download`, `/docs/limits/` as separate pages (fold what is
  kept into the single page or into the repository).
- `/admin/*` pages (admins can use the CLI or a private build).
- Redirect rules for retired addresses (`/realms*`, `/realm*`, `/project*`,
  `/verse/galaxy`, `/worlds/everglade-5e*`, `/docs*`, `/settings*`) after a
  grace period, except `/signin`.
- Dead infrastructure: URL maps `openagents-url-map`, `coder-sync`;
  backend services `openagents-backend`, `openagents-coder-api-backend`,
  `openagents-forge-git-backend`, `coder-sync-run`, `coder-sync-vm`,
  `oa-room` … `oa-room-6`; Cloud Run `openagents-web-staging`, `oa-room` …
  `oa-room-6`; stale domain mappings for `relay.openagents.com` and
  `staging.openagents.com`; DNS for `sync`, `api`, `coder` and
  `updates.openagents.com`; `docs.openagents.com` on Vercel.
- The unapplied Metaverse-rename rules in `coax/web/api/lb.sh` and
  `coax/web/site-routes.coax`, if those paths are removed instead of fixed.

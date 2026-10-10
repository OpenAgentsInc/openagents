# OpenAgents website port

Status of moving the openagents.com website from the private `coder`
repository (`bins/coder-serve`) into this repository's
[`crates/openagents-web`](../../crates/openagents-web/README.md). Recorded
2026-09-29.

## Live (2026-10-01)

openagents.com serves this site since 2026-10-02 02:20 UTC (#10128): Cloud
Run service `coder` (us-central1, openagentsgemini), revision
`coder-web-79692f5e9a` since 2026-10-02 06:55 UTC (now `coder-web-3a46b3c415`, below) (the complete user guide
at `/docs`; the one before it, `coder-web-dcc80c9096`, the move to
`/download`, is the rollback), two containers. `web` is this crate's image
(`openagents/openagents-web:79692f5e9a`, built by `crates/openagents-web/cloudbuild.yaml`)
on port 8080 with `--public-host openagents.com`; it serves its own pages
and passes every other path (APIs, `/v1`, `/mcp`, `/auth`, `/computers/seen`,
`/releases`, …) to `coder-serve`, the previous production image and config
unchanged, as a sidecar on port 8081 (`OPENAGENTS_WEB_UPSTREAM`), with the
original Host so sign-in and billing links stay on openagents.com. Removed
sections stay 404. `OPENAGENTS_WEB_ASK_SALT` is the secret
`openagents-web-ask-salt`.

- Since 2026-10-09: `scripts/deploy/web.sh` (below, "promote by digest")
  replaces the steps in this item.
- Deploy a new site build: build the image with Cloud Build, then replace
  the `web` container's image in a copy of the live revision spec
  (`gcloud run services describe coder --format export`) and apply it with
  `gcloud run services replace` under a new revision name with no traffic and
  a tag, check the tag URL, then move traffic. The export writes
  `CODER_CHAT_SYNC`'s value as a bare `on`, which `replace` refuses as a
  boolean; quote it (`'on'`). Use the tag `new`: the `web` container
  already lists `new---coder-ezxz4mgdsq-uc.a.run.app` as a `--public-host`,
  so that tag URL serves the site, while any other tag's host is proxied
  whole to `coder-serve`. With the image built from GitHub instead of a
  local upload, pass the full commit to `--git-source-revision` and
  `--service-account` (the org policy requires one). On 2026-10-02 the
  automation account was refused `iam.serviceAccounts.actAs` on the
  runtime account `157437760789-compute@developer.gserviceaccount.com`,
  though that account's policy lists it as `roles/iam.serviceAccountUser`,
  so the revision was applied as `chris@openagents.com`. The same held for
  `coder-web-79692f5e9a`: Cloud Build ran as the automation account
  (`gcloud builds submit https://github.com/OpenAgentsInc/openagents
  --git-source-revision=SHA --service-account=…oa-mvp-automation…`), and
  `replace` was refused `actAs` again and applied as `chris@`.
- Roll back to the old site: `gcloud run services update-traffic coder
  --region us-central1 --project openagentsgemini --to-revisions coder-00168-smb=100`.

The port reimplements the pages. It copies the published content (the terms
and the privacy policy) and no backend code,
prompts, endpoints, or secrets. The site is now drawn in four intensities of
white on near-black instead of four intensities of amber.

Live (2026-10-02, after 23:30 UTC): revision `coder-web-3a46b3c415`
(image `openagents/openagents-web:3a46b3c415`, `/live` and `/stats`,
#10196, #10197) serves 100% of the traffic; `coder-web-79692f5e9a` is the
rollback (`--to-revisions coder-web-79692f5e9a=100`). The automation
account was refused `actAs` on the runtime account again, although that
account's policy lists it as `roles/iam.serviceAccountUser`; the revision
was applied as `chris@`.

Pay host wired (2026-10-03, #10190): revision `coder-web-3a46b3c415-pay`
(the same image) serves 100% of the traffic; `coder-web-3a46b3c415` is the
rollback (`--to-revisions coder-web-3a46b3c415=100`). The `web` container
has `OPENAGENTS_WEB_PAY_HOST=http://10.128.0.46:4400`, the `pay-host`
server on `oa-pay-1`, so `/api/flow/*` and `/api/stats` answer from the
real ledger and `/stats` says "No payments yet" instead of unreachable. The
pay host has no public address: the service has Direct VPC egress on the
`default` network and subnet with `private-ranges-only` (template
annotations `run.googleapis.com/network-interfaces:
'[{"network":"default","subnetwork":"default"}]'` and
`run.googleapis.com/vpc-access-egress: private-ranges-only`), so only
private-range traffic takes the VPC and everything else leaves as before.
Keep both annotations and the variable in the copied spec for every later
site build. See [pay host](pay-host.md#flow-and-stats-for-the-website-10195).

Live (2026-10-03, after 01:55 UTC): revision `coder-web-37def7d8fc` (image
`openagents/openagents-web:37def7d8fc`, `/efficiency`, #10210), copied from
`coder-web-3a46b3c415-pay`'s spec with the pay host variable and both VPC
annotations kept, serves 100% of the traffic; `coder-web-3a46b3c415-pay` is
the rollback (`--to-revisions coder-web-3a46b3c415-pay=100`). The image was
built from GitHub by the automation account and the revision applied as
`chris@`. `/efficiency` is computed from the study rows compiled into the
image (`coder::efficiency::PUBLISHED`); publishing a new study run is a
commit of its rows and a site build. `coder-web-5e22f2af96`, an earlier
build of the same page, never took traffic.

## Ported and complete

These pages need nothing more than this repository and the public buckets:

- `/`, the homepage: what OpenAgents is, one `[ Download OpenAgents ]` link
  to `/download`, and the **Ask OpenAgents** terminal. The Grid's
  screenshot moved to its own guide, `/docs/the-grid`, on 2026-10-01 so the
  homepage leads with the terminal. The old terminal and its `/ask` box were removed on 2026-09-29 and
  brought back on 2026-10-01 at the owner's direction
  ([#10106](https://github.com/OpenAgentsInc/openagents/issues/10106)), now
  on the white theme and tied to the same OpenAgents chat worker the apps
  use, as the `web` surface: answers and knowledge about OpenAgents only,
  pointing to `/download` for anything the apps do; never Coder or a
  computer. `help`, `download` (`install` is its alias), `docs`, and
  `clear` are its commands. A
  deployment with several instances sets `OPENAGENTS_WEB_ASK_SALT` (64 hex
  characters, a secret) so a visitor keeps one signing key; it needs no
  model key, since the chat worker holds its own.
- `/download`: the one download page (`src/pages/download.rs`): the
  Coder installers and companion binaries for macOS, Linux, and Windows.
  It was `/install` until 2026-10-01; `/install` and
  `/desktop` redirect here permanently (`308`), and the guide
  `/docs/install` to `/docs/download`.
- `/terms` and `/privacy`: the published text, last updated 2026-09-03,
  unchanged and compiled in.
- `/connect`, `/.well-known/apple-app-site-association`, and
  `/.well-known/assetlinks.json`: the pairing link's landing page and the
  app-association files, as the `coder` repository serves them.
- `/docs` and `/docs/{slug}`: the user guide
  (`crates/openagents-web/content/docs/`), thirty short pages in nine
  sections (`SECTIONS` in `src/pages/content.rs`): getting started, the
  apps, chat, Coder, computers, the four plugin guides
  ([#10088](https://github.com/OpenAgentsInc/openagents/issues/10088)),
  the Gym, Verse, wallet, and decks, reference, and the FAQ and glossary.
  Expanded on 2026-10-02 at the owner's direction; `/docs/help` redirects
  to `/docs/troubleshooting`.
- `/app`: the local task browser, loopback only.

## Ported, waiting on a production backend

These pages render on a development server and say that their data needs
the production backend. Each reads through the `Backend` trait
(`crates/openagents-web/src/backend.rs`), which has no production
implementation yet:

| Page | Backend method | Production source |
| --- | --- | --- |
| `/u/{login}` | `profile` | The account store. |
| Homepage credit line | `new_account_credit_cents` | Service configuration. |

## Removed

The Forum (`/forum`), Gym (`/gym`, `/gym/results/...`), Traces (`/traces`,
`/trace/{key}`), Earn (`/earn`), Weights (`/weights`), and QA (`/qa`)
sections were ported and then removed on 2026-09-29 at the owner's
direction: the site serves none of them and links none of them.

Also removed on 2026-09-29 at the owner's direction, because the old Coder
Terminal product (installed from the private `coder` repository's bucket
`openagentsgemini-cli-releases`) is not connected to OpenAgents: the
`/releases/{name}` release proxy, the `/install-terminal.sh` and
`/install-terminal.ps1` redirects, the homepage's install commands, and the
old Docs (`/doc*`) and Blog (`/blog*`) sections, whose every document was
about Coder Terminal. All of them answer `404`. The old site's `/docs`
pages were about Coder Terminal too; this server's `/docs` is new and
serves different guides at that path.

## Not ported

These need accounts, sessions, or payments, or are internal:

- Sign-in and accounts: GitHub OAuth, sessions, `/login`, `/logout`,
  `/auth/*`, invites, the waitlist, `/settings`, agent approval, and
  computer pairing codes.
- Billing: Stripe checkout and webhooks, credit, and Pro.
- The run console: `/run/{id}`, `/ws`, `/message`, cancel, rerun, archive,
  and `/trajectories`.
- APIs: `/v1/*`, `/api/*`, MCP, the decision door, token and grant minting,
  earn receipts, trace upload, and QA uploads.
- Avatars (`/u/{login}/avatar`), calendar booking, `/map`, `/components`,
  `/showcase`, deployments, and page-view analytics.

## Before deploy

1. Implement `Backend` against production data (profiles, homepage
   answers, and the credit line), or launch without them.
2. Package and host the binary (for example on Cloud Run) with
   `--listen 0.0.0.0:PORT --public-host openagents.com`, behind TLS.
3. Serve the association files at `openagents.com` with no redirect, and
   check that iOS and Android still verify `/connect`.
4. Keep the URLs that must not move the same as the `coder` repository
   serves them: the pairing link (`/connect`) and the legal pages.
5. Decide the old Coder Terminal's fate. This server does not serve
   `/releases/*`, `/install-terminal.sh`, `/install-terminal.ps1`, the old
   `/docs` pages, or `/blog`. Once DNS moves, the installed Coder
   Terminal's self-update (which reads `/releases/`) and its published
   install command (`curl -fsSL https://openagents.com/releases/install-terminal.sh | sh`,
   and the PowerShell form) stop working, and old docs and blog links
   answer `404`. That is an owner decision: accept the break, or restore a
   redirect to the bucket before cutover.
6. Update `/download` for each Coder release (`CODER_VERSION` and its bundle).
7. When this server serves openagents.com, update the `INVARIANTS.md` row
   for the connect link, which names `coder-serve` as the server of
   `/connect`.
8. Cut DNS over from the `coder` deployment, then retire its site routes.

Live (2026-10-03): revision `coder-web-16006f7873` (image
`openagents/openagents-web:16006f7873`, fractional sats on `/live`, #10239),
copied from `coder-web-37def7d8fc`'s spec with the pay host variable and
both VPC annotations kept, serves 100% of the traffic; `coder-web-37def7d8fc`
is the rollback (`--to-revisions coder-web-37def7d8fc=100`). Built from
GitHub by the automation account, applied as `chris@`.

Live (2026-10-03, later): revision `coder-web-2f838a3b1d` (image
`openagents/openagents-web:2f838a3b1d`, `/efficiency` leading with the
standing run `2026-10-03b` after the routed start-up cut, #10279; it
includes #10239), copied from `coder-web-16006f7873`'s spec with the pay
host variable and both VPC annotations kept, serves 100% of the traffic;
`coder-web-16006f7873` is the rollback (`--to-revisions
coder-web-16006f7873=100`). Built from GitHub by the automation account,
applied as `chris@`; the tag `new` URL served the page and `/api/stats`
from the pay host before traffic moved.

Live (2026-10-03, shakeout): revision `coder-web-bbed5d89af` (image
`openagents/openagents-web:bbed5d89af`, built from GitHub by the automation
account), copied from `coder-web-2f838a3b1d`'s spec with the pay host
variable and both VPC annotations kept, serves 100% of the traffic;
`coder-web-2f838a3b1d` is the rollback (`--to-revisions
coder-web-2f838a3b1d=100`). `/`, `/live`, `/stats`, `/efficiency` and
`/api/stats` answered 200 on the `new` tag before traffic moved.
`/.well-known/agent-card.json` still returns 404: no deployed service
serves the `discovery` crate's card (#10364).

Live (2026-10-03, calibration): revision `coder-web-530b207410` (image
`openagents/openagents-web:530b207410`, the Decisions section on
`/efficiency`, #10387; built from GitHub by the automation account),
copied from `coder-web-bbed5d89af`'s spec with the pay host variable and
both VPC annotations kept, serves 100% of the traffic;
`coder-web-bbed5d89af` is the rollback (`--to-revisions
coder-web-bbed5d89af=100`). `/`, `/live`, `/stats`, `/efficiency` and
`/api/stats` answered 200 on the `new` tag before traffic moved.

## 2026-10-03: coder-web-6522f448de

Serves `/.well-known/agent-card.json`, the skills index and `SKILL.md` from the discovery crate (#10318, 6522f448de). Applied as tag `new` from the live spec (pay-host VPC and env kept), checked `/`, `/live`, `/stats`, `/efficiency`, `/api/stats` and the agent card at 200, then moved 100% of traffic. `openagents discover --origin https://openagents.com` reads the live card. Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-530b207410=100`.

## Everglade at `/everglade` (#10525)

The image carries the Everglade web build (#10524) at `/srv/everglade`: a
Dockerfile stage adds the `wasm32-unknown-unknown` target, installs
`wasm-bindgen-cli` at the `wasm-bindgen` version `Cargo.lock` pins, runs
`scripts/build-everglade-web.sh` into a directory, adds the pinned pack
(`assets/verse/everglade/*.vtp`) under its `pack/`, and the runtime image
copies that directory to `/srv/everglade`. `web.gcloudignore` lets through `assets/`, `.cargo/`, and the
build script for that stage. The server serves the directory given by
`--everglade DIR`, which the image's `CMD` passes. The live revision spec
sets the `web` container's arguments itself, so a deploy that should serve
Everglade adds `--everglade` and `/srv/everglade` to them; without it,
`/everglade` says Everglade is unavailable and every other page is
unchanged.

The page runs one same-origin loader (`static/everglade.js`) under a policy
that allows same-origin scripts and requests and `'wasm-unsafe-eval'`; no
other page's policy changed. Build files (`everglade_web.js`,
`everglade_web_bg.wasm`) are cached for five minutes, since their names
carry no digest; the pack (`/everglade/pack/<PACK_SHA256>.vtp`) is cached
for a year as immutable. After a deploy, check `/everglade` and the pack on
the `new` tag before moving traffic.

## 2026-10-04: coder-web-98ddaac99d, Everglade live

`openagents.com/everglade` serves the Everglade web build (#10523–#10525):
image `openagents/openagents-web:98ddaac99d`, built from GitHub by the
automation account, applied from the live spec with only the image and the
`web` container's `--everglade /srv/everglade` arguments changed (pay-host
VPC annotations and variables kept, `CODER_CHAT_SYNC` quoted). On the `new`
tag, `/`, `/download`, `/docs`, `/live`, `/stats`, `/efficiency`,
`/api/stats`, `/terms`, the agent card, `/everglade`, its build files, and
the 13,373,560-byte pack answered 200, and Chrome rendered the glade over
WebGPU before traffic moved. Rollback:
`gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-6522f448de=100`.

The first build of this change (`9088c30a1d`) failed: `breez/boltz-client`,
a git dependency of `breez-sdk-spark`, returned 404, so no clean build of
the workspace could fetch it. It is now vendored under `vendor/boltz-client`.

## 2026-10-04: coder-web-10535d62b5

Everglade loads sooner and draws where WebGPU cannot (888be7cb7b,
10535d62b5): the loader downloads the module with progress over the canvas,
the image serves gzip copies of the build (the module drops from 11.3 MB to
7.7 MB), and the page retries on WebGL2 when a browser's WebGPU rejects the
physical renderer. Both paths rendered the glade on the `new` tag before
traffic moved. Rollback: `--to-revisions coder-web-98ddaac99d=100`.

## 2026-10-04: coder-web-4b966fdc06

Everglade's player is the ritual chamber's ranger with no companion
(#10534, 4b966fdc06), from the re-pinned pack `3680adf2…7297`. The `new`
tag served every page and the pack at 200 and rendered the ranger before
traffic moved. Rollback: `--to-revisions coder-web-10535d62b5=100`.

## 2026-10-04: coder-web-570fe0ba18

`/everglade` draws no zone panel over the glade (570fe0ba18), and its
station captions offer no key the page cannot use (cc345ccb7a). Checked on
the `new` tag before traffic moved. Rollback:
`--to-revisions coder-web-8fb2a97218=100`, or `coder-web-4b966fdc06` for the
last revision that served traffic before it.

## 2026-10-04: coder-web-36fbc814b1

`/everglade` fills the window (36fbc814b1): the canvas covers the whole
viewport with no site header or footer, the page doesn't zoom, and the
loading status stays over the canvas's foot. Built from GitHub by the
automation account and applied from the live spec with only the revision
name and image changed (`CODER_CHAT_SYNC` quoted). On the `new` tag,
`/everglade` served the full-screen page and its CSS, and `/`, `/docs`,
`/live`, `/stats`, `/api/stats`, the build files, and the pack answered 200
before traffic moved. Rollback: `--to-revisions coder-web-570fe0ba18=100`.

## 2026-10-04: coder-web-61053682b1

`/everglade` carries the Universal Animation Library walk and run, the
cultist-style hotbar with held climbing, roofs to land on, and the
rebuilt pack `cf6abad272…` (61053682b1). Built from GitHub by the
automation account and applied from the live spec with only the revision
name and image changed (`CODER_CHAT_SYNC` quoted). On the `new` tag, `/`,
`/everglade`, `/docs`, `/live`, `/stats`, `/api/stats`, and the new pack
answered 200, and the served wasm pins the new pack, before traffic moved.
Rollback: `--to-revisions coder-web-36fbc814b1=100`.

## 2026-10-04: coder-web-db7f3ddf47

`/everglade` gains the daylight sky (fb48b98886), the four untargeted
spells on the hotbar (5c08a2928b), the hotbar on the page with its keys and
pointer presses, and strafe and backpedal clips in the repinned pack
`4bbd3b18ae…` (db7f3ddf47). Built from GitHub by the automation account and
applied from the live spec with only the revision name and image changed
(`CODER_CHAT_SYNC` quoted). On the `new` tag the pages and the new pack
answered 200, the served wasm pins the new pack, and headless Chrome drew
the sky and the seven-slot hotbar before traffic moved. Rollback:
`--to-revisions coder-web-61053682b1=100`.

## 2026-10-05: coder-web-addbb9b72f, the druid demo at `/druid`

`openagents.com/druid` serves the Grove full screen (#10611): the same
Everglade web build and pack as `/everglade`, which starts in the Grove when
the page's path ends in `/druid`, with the Archdruid's four-row bar
(#10609) and Wild Shape (#10610). Image
`openagents/openagents-web:addbb9b72f`, built from GitHub by the automation
account and applied as `chris@` from the live spec with only the revision
name and image changed (`CODER_CHAT_SYNC` quoted). On the `new` tag, `/`,
`/druid`, `/everglade`, `/docs`, `/download`, `/live`, `/stats`,
`/efficiency`, `/api/stats`, `/terms`, the agent card, the build files, and
the 27,965,018-byte pack `136a9389…` answered 200, and headless Chrome drew
the Grove at `/druid` (Wild Shape and a cast from the keys) and Everglade's
town at `/everglade` over WebGPU before traffic moved. Rollback:
`--to-revisions coder-web-db7f3ddf47=100`.

## 2026-10-05: coder-web-f6ed2aa223, the Grid at `/grid`

`openagents.com/grid` serves the browser Grid with NIP-MV presence (#10587):
browser players appear with names, can block and mute from a name tag, and
see when the world is full. It runs on WebGL2 as well as WebGPU (#10626).
The same image carries round 3 and 4 of Everglade (the 9.8 MB `VTP3` pack,
levels of detail, wildlife) and destructible town buildings with Meteor
Swarm. Image `openagents/openagents-web:f6ed2aa223`, built from GitHub by
the automation account and applied as `chris@` from the live spec with only
the revision name and image changed (`CODER_CHAT_SYNC` quoted). On the `new`
tag, `/`, `/grid`, `/everglade`, `/druid`, `/docs`, `/download`, `/live`,
`/stats`, `/efficiency`, `/api/stats`, `/terms`, and the agent card answered
200, the page policy allowed `wss://relay.openagents.com`, and headless
Chrome driven in real time drew `/grid` online on the public relay with a
name tag, Everglade's town at `/everglade`, and the Grove at `/druid`
before traffic moved. Rollback:
`--to-revisions coder-web-addbb9b72f=100`.

## 2026-10-05: coder-web-9cbc7e6dd4, destruction on the web

Everglade on the web now destroys buildings (9cbc7e6dd4): the physics clock
no longer panics in the browser on the first strike, WebGL2 starts again
(the geometry budget fits the town), the Meteor Swarm circle lies over
roofs and walls, and every building, landmark, and prop breaks, the
workshop included. Image `openagents/openagents-web:9cbc7e6dd4`, built from
GitHub by the automation account and applied as `chris@` from the live spec
with only the revision name and image changed (`CODER_CHAT_SYNC` quoted).
On the `new` tag, `/`, `/grid`, `/everglade`, `/druid`, and `/api/stats`
answered 200, headless Chrome drew the town on WebGPU and WebGL2 (`?gl`)
and the Grove, and a Meteor Swarm cast with real key and mouse input
destroyed the workshop hall in front of spawn without stopping the page,
before traffic moved. Rollback: `--to-revisions coder-web-f6ed2aa223=100`.

The Grove at dusk with the tower goes live (2c68a678f4): dusk lighting and
spell lights, the sacred grove, the concrete tower that topples, Meteor
Swarm and Thunderbolt on keys 1 and 2, Fire Breath damage to the tower, 3D
surface aiming, and the dragon. Image `openagents/openagents-web:2c68a678f4`,
built from GitHub by the automation account and applied as `chris@` from the
live spec with only the revision name and image changed (`CODER_CHAT_SYNC`
quoted). On the `new` tag, `/`, `/grid`, `/everglade`, `/druid`, and
`/api/stats` answered 200, and headless Chrome drew the Grove on WebGPU and
WebGL2 (`?gl`) and Everglade. A Meteor Swarm cast with real key and mouse
input showed its ring and falling meteors before traffic moved. Rollback:
`--to-revisions coder-web-9cbc7e6dd4=100`.

Budgets degrade instead of stopping Everglade (1726d56f8d): a geometry
budget overrun no longer halts a zone with "Renderer geometry bytes exceed
the admitted quality budget"; frames draw less, the debris pool is bounded,
and the low and medium tiers reserve room for destruction. Also live: the
Grove's support fix for pieces stuck in the sky, the dragon's reach and
Fire Breath's near-wall hits, and Alice's revised look. Image
`openagents/openagents-web:1726d56f8d`, built from GitHub by the automation
account and applied as `chris@` from the live spec with only the revision
name and image changed (`CODER_CHAT_SYNC` quoted). On the `new` tag, `/`,
`/grid`, `/everglade`, `/druid`, and `/api/stats` answered 200; headless
Chrome cast Meteor Swarm 50 times in Everglade and alternated Meteor Swarm
and Thunderbolt about 40 times in the Grove, each on WebGPU and WebGL2
(`?gl`), and no page stopped, before traffic moved. Rollback:
`--to-revisions coder-web-2c68a678f4=100`.

## 2026-10-06: Coder RC3 installers

Revision `coder-web-337c008eb0` serves 100% of traffic. `/download` offers
Coder `1.0.0-rc.3`, and `/cli/install.sh` and `/cli/install.ps1` serve the
committed installers as plain text. The image was built from GitHub commit
`337c008eb0`; the production sidecar, runtime settings, and VPC configuration
were preserved. The download page, both exact installer bodies, and the
existing public pages passed checks on the `new` tag and production.
Rollback: `--to-revisions coder-web-1726d56f8d=100`.

## 2026-10-06: Coder-only downloads

Revision `coder-web-0a15818d70` serves 100% of traffic. `/download` and its
guide offer only Coder RC3 installers and the companion bundles for seven
platforms; the legacy Terminal and desktop Mac downloads are removed.
Cloud Build `43a22f4e-d0d9-4a6a-bb06-ed4780086393` built the web server from
commit `0a15818d70` over the previous image's unchanged game assets. The
production sidecar and runtime configuration were preserved. Both exact
installer bodies, all 22 executable links and checksums, and the existing
public pages and game assets passed checks on the `new` tag and production.
Rollback: `--to-revisions coder-web-337c008eb0=100`.

Everglade without offensive spells, with the owner's house and instanced
rendering (ba484fcf52): the Plaza gate is gone, Everglade's hotbar has five
movement and utility spells (Meteor Swarm and the sledgehammer only in
local builds with the `dev-destruction` feature), round 8's trails and
foliage, the Greco-futurism house, Alice's revised model, and instanced
static meshes that cut Everglade's resident geometry from 208.7 MiB to
116.6 MiB. Image `openagents/openagents-web:ba484fcf52`, built from GitHub by the
automation account and applied as `chris@` from the live spec with only the
revision name and image changed (`CODER_CHAT_SYNC` quoted). On the `new`
tag, `/`, `/grid`, `/everglade`, `/druid`, and `/api/stats` answered
200; headless Chrome drew Everglade on WebGPU and WebGL2 (`?gl`) with the
five-slot hotbar, key 6 cast nothing there, and the Grove still cast Meteor
Swarm and Thunderbolt, before traffic moved. Rollback:
`--to-revisions coder-web-0a15818d70=100`.

The owner's lit house with Alice at her workstation (676f61ec11): the
Greco-futurism house's candlelit great room and lanterns, Alice at a
workstation there, owner-only admission for her requests (on the web she
shows "owner only" and takes no input), and the Everglade pack that carries
them. Image `openagents/openagents-web:676f61ec11`, built from GitHub by the
automation account and applied as `chris@` from the live spec with only the
revision name and image changed (`CODER_CHAT_SYNC` quoted). On the `new`
tag, `/`, `/grid`, `/everglade`, `/druid`, and `/api/stats` answered
200, headless Chrome drew Everglade on WebGPU and WebGL2 (`?gl`), and the
Grove still cast, before traffic moved. Rollback:
`--to-revisions coder-web-ba484fcf52=100`.

## 2026-10-07: medieval kit deploy

Revision `coder-web-d3f3ad546c-20261008043217` serves 100% of production
traffic after P9 lands and its final Verse and Everglade tests pass. It was
first staged under the `new` tag with no production traffic; the preceding
revision `coder-web-docs-c6415404e8-20261007003942` is the rollback.
It uses the requested existing image `openagents/openagents-web:d3f3ad546c`,
from commit `d3f3ad546cff031f58b2b63eb9b62bf29d9a2fb0`, pinned to image digest
`sha256:fa564ef756f3f42de88f76dbbc0310064a17bf28ff1c17398f9c2b2ff3ea323f`.
Artifact Registry records its creation at `2026-10-08T03:39:18Z`; no matching
Cloud Build receipt was found, and this deployment runs no new build.

The revision was applied as `chris@openagents.com` from the live export,
with only its name, the web image, and the zero-traffic `new` tag changed.
The `coder-serve` image, secrets, pay host environment, VPC annotations,
service account, volumes, concurrency, and timeout are preserved, and
`CODER_CHAT_SYNC` is quoted as `'on'`.

All 25 HTTP checks pass on the tag and production: public pages, pairing and association
files, discovery, installers, CSS, loader, wasm, and both packs. The kit at
`/everglade/kit/dae1612d4c22438a933c27b406c1e18fe134b13eab8eb5240ddcf5506ffb0b93.vtp`
returns `200`, exactly 10,238,689 bytes, and the matching SHA-256 digest.
Offscreen Chrome renders the medieval town and hotbar on WebGPU and forced
WebGL2 on both the tag and production. The font `/fonts/PaperMono-Variable.woff2` returns `404` on both the
tag and the preceding production revision; rendering succeeds with the
fallback font.

This requested image predates P9 and serves the earlier public pack
`367da275afc505543d77841dd4f44efafbb6d784b783a17742985b13ce922fb7`
(12,303,751 bytes; size and digest verified). A later build must include
P9's cleaned public pack and any subsequent kit or tier changes.
Evidence stays outside git in
`/Users/christopherdavid/.openagents/scratch/codex-01a119ae-08ea-73f2-9344-ec959f74a795/`.
Both `candidate/` and `production/` contain `http-checks.json`,
`browser-checks.json`, `everglade-webgpu.png`, and `everglade-webgl2.png`.
Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-docs-c6415404e8-20261007003942=100`.

## 2026-10-08: direct component catalog

Revision `coder-web-components-6daf112710-202610081303` serves 100% of
traffic. `/components` now serves the shared Rust catalog and its interactive
Wasm bundle: 40 component families and 483 variants. The public header stays
Download, Docs, Pilot; the homepage carries no Cloud or component promotion.

Cloud Build `f3f7acdc-c74b-47e8-8eed-246a84c03125` builds clean commit
`6daf1127104f7cf91e64eea2df483fc47881d8dd` through
`cloudbuild-components.yaml`, over the previous production image so the game
Wasm and packs stay unchanged. The resulting image digest is
`sha256:b73ce7216e07dbe1349bf3ce7acc1510c3b73649ba89f2c76f17aa752009449a`.
The live spec preserves the sidecar, runtime settings, secrets, and VPC;
the web arguments add `/srv/components` and `/srv/cloud` asset directories.
No Cloud account or host binding is activated.

Staging and production each pass 31 HTTP checks and 34 browser assertions,
including real catalog interactions and Everglade on WebGPU and WebGL2.
The 19 checked existing public pages and game assets match the previous
production response bodies byte for byte. Evidence remains in the deployment
session's scratch directory and
`codex-01a11a7c-eb19-7780-b9e7-cd6e305db168/components-production-proof/`.
Rollback: `--to-revisions coder-web-d3f3ad546c-20261008043217=100`.


## 2026-10-08: P3 kit and P9 public pack

Revision `coder-web-p3-d2fb95d33d-20261008140843` serves 100% of traffic.
It carries P3's reviewed private kit and P9's smaller public pack while
preserving the direct component catalog, Cloud assets, and public navigation.
Cloud Build `016b8bd2-94a3-4815-ad70-4fd46da3ab19` builds main commit
`d2fb95d33d1d5c668be3d85c53c9bedaaab174af`. The image
`openagents/openagents-web:p3-d2fb95d33d` has digest
`sha256:0db42f8c43471fa3763f786fffae00b901c0320fc4bb6cd9cc0aca926a919d55`.
The build runs every Cargo invocation through the build lease with an
external target directory, four build jobs, and a 25 GB disk floor. The
private build configuration's SHA-256 is
`3683dfa1638185d2e70581585591d0d7539aacc04597a9f880f34a620396e4a8`.

The revision starts under `new` with no traffic. Before promotion, its
readiness, unchanged live configuration, and prior 100% traffic are checked.
The live export preserves the sidecar, secrets, pay host settings, VPC,
service account, volumes, asset arguments, concurrency, and timeout.

Staging and production each pass 33 HTTP checks. The private kit URL
`/everglade/kit/c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e.vtp`
returns `200`, exactly 21,467,658 bytes, and its matching SHA-256. The public
pack `a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f`
returns 10,636,202 bytes with its matching digest. The preceding private
kit and public pack URLs also return their exact bytes and hashes.

Offscreen Chrome on `coderos-4080` renders Everglade through hardware
WebGPU and forced WebGL2 with both current packs, no proxy warnings, and no
browser exceptions or failed HTTP requests. Both views are inspected.
Linux headless WebGPU compositor screenshots are black on the preceding
and new images. The accepted WebGPU capture instead reads the application's
GPU framebuffer, adding only `COPY_SRC` texture usage for the inspection;
the black compositor images are retained as rejected evidence. This check
does not measure frame rate. Each environment also passes 30 component
catalog assertions with synthetic local fixtures; the browser profile is
removed and the quiet and GPU leases are released.

Receipts, capture hashes, the build configuration, and private images remain
outside Git in
`/Users/christopherdavid/.openagents/scratch/codex-01a119ab-cb4c-7331-b0dc-8ddce4fb09a0/p3-web/`.
`verification-manifest.json` binds the accepted evidence. The kit exceeds
the web and phone soft budgets; B4 (#10908) owns the tier work and physical
device checks remain in `NEEDS_OWNER.md` (#10901). The later `/demo` source
change is not part of this image.
Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-components-6daf112710-202610081303=100`.

## 2026-10-08: Original Coder demo

Revision `coder-web-demo-7bb5e9bccd-202610081448` serves 100% of traffic.
`/demo` mounts the original `coder-new` terminal demo through Rust Native:
five local conversations, retained drafts and cursors, native tools and
panels, independent elapsed time, and a 125 ms animation. It creates no
provider connection or real task and exports only a local synthetic ATIF
artifact. The page has no site navigation. The public header remains
Download, Docs, Pilot, with no Cloud, component, or demo promotion.

Cloud Build `795226f7-2c40-4a5d-9c09-fd908f6285ab` builds commit
`7bb5e9bccd796afee03396a79fa2b41a625cb18a`. The concurrent P3 deployment
changes the live image before staging; the base-image guard refuses the
stale candidate. Build `a5c1c97c-fd00-46df-b2b4-028693ccc284` then copies
only the verified web binary and component and Cloud bundles over the new
P3 image. The final image digest is
`sha256:61a6867164f308323e24507ace303d6575d56eae678f940de2e918ced3264867`.
The live spec preserves the sidecar, secrets, runtime settings, and VPC.
No Cloud account or host binding is activated.

The portable and native demo adapters each match all 93 independently
captured original frames. Targeted ATIF, UI, native demo, browser adapter,
and web checks pass. Staging and production each pass 69 demo browser
checks and 34 catalog and game browser checks, with no browser errors.
Both game graphics backends render the preserved P3 assets. Each environment
also passes 33 regression HTTP checks and six demo HTTP checks; all 19
checked public response bodies match the P3 production baseline byte for
byte, including the existing game modules. Both new and preceding game
pack and kit URLs retain their exact bytes and hashes.

Evidence remains in the deployment session's scratch directory and
`codex-01a11a7c-eb19-7780-b9e7-cd6e305db168/demo-production-proof/`.
Rollback: `--to-revisions coder-web-p3-d2fb95d33d-20261008140843=100`.

## 2026-10-08: Environment onboarding demo

Revision `coder-web-onboarding-88cb5f7599-20261008172714` serves 100% of
traffic since 17:32 UTC. [The demo](https://openagents.com/demo) now shows
the complete synthetic onboarding conversation and six chats with independent
drafts and history (#10985, #10988).

Cloud Build `0b8ac09e-b592-498c-b69f-14b24961972b` builds main commit
`88cb5f75996a05de9a8fe03a8c50a31e8b379a19` through
`cloudbuild-components.yaml`. Image `openagents/openagents-web:demo-onboarding-88cb5f7599`
has digest `sha256:e3b2add25e9d435a0a36108341726e7c193a65f678c54056aa84427c85fc03ed`.
The Docker overlay retains the preceding production image's game files and
replaces the native web binary and component and Cloud browser bundles.
The live spec preserves the sidecar, secrets, VPC, and runtime settings.
The separate `onboarding` tag and public host keep the existing `new` tag
on its previously staged revision. Readiness and unchanged production traffic
are checked before promotion.

Staging and production each return `200` for the demo, its CSS, loader,
generated JavaScript, Wasm module, homepage, and existing game loader. All
seven response bodies match between staging and production. Browser inspection
shows Wasm mounting on both, sidebar switching on staging, and **Latest**
reaching the first task on the saved environment on both. The production
revision has no error-level Cloud Run log entries at the post-deploy read.
Evidence, configuration exports, and screenshots remain outside Git in
`codex-01a11c18-367b-7793-b27f-8381cea14e2f/demo-production/` under operator scratch.

The native Rust server supplies the initial HTML, sidebar, transcript preview,
CSS, and assets. Rust/Wasm owns the local conversation state, input handling,
navigation, transcript layout, and changed-row HTML updates. The browser paints
ordinary HTML and CSS; the demo uses no canvas or SVG renderer. A 400-byte
JavaScript loader initializes the generated `wasm-bindgen` bridge. The shared
catalog and demo module is 5,689,644 bytes. The onboarding calls and outputs
are fixtures; this demo performs no model or environment execution.

Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-demo-7bb5e9bccd-202610081448=100`.


## Baked town layer delivery, October 8, 2026

Revision `coder-web-b2-88cb5f7599-20261008174258` serves 100% of traffic.
Cloud Build `b8bc150d-1be4-4e75-af79-e92713a5f3e4` produces image
`openagents-web:b2-layers-88cb5f7599`, digest
`sha256:310714646cd0f0f6c3ebd611f1231ff55635639c4b627393b8377ccc27818cb5`.
It adds only the pinned 51,682,623-byte VLAY file to the preceding
onboarding image `sha256:e3b2add25e9d435a0a36108341726e7c193a65f678c54056aa84427c85fc03ed`.
The server and browser bundles retain source `88cb5f75996a05de9a8fe03a8c50a31e8b379a19`.
This preserves the newer deployment while main advances, without another
compile or bake. The spec retains the sidecar, VPC, runtime settings, and
onboarding tag.

The `new` tag receives no traffic until its complete layer download passes
SHA-256 and size checks. Staging and production each return `200` for the
layer file, both pinned packs, homepage, Everglade, demo, CSS, game JavaScript
and Wasm, components, cloud, and stats; invalid layer names return `404`.
Ten preceding production responses remain byte-identical. Native loading
from an empty cache verifies and decodes the four sun layers and 4,326,184
vertices. The layer digest is
`14ae7f75e9ce4f81483f6f44369753545cb2cab892177607438b3077ebbbae23`.
Route fix `a80bde9114` streams bounded chunks to avoid Cloud Run's buffered
response limit; both focused route tests pass. The earlier buffered staging
revision and superseded server image never receive production traffic.
Evidence remains in operator scratch under `b2-verification/layers-overlay/`.

Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-onboarding-88cb5f7599-20261008172714=100`.

## October 8, 2026: verified baked-town browser loading

Revision `coder-web-b4-dbd84fdb3d-20261008195645` serves 100% of traffic.
Cloud Build `889f9249-7d6a-427e-8889-9178dba04e35` produces image digest
`sha256:b82817f4a64bb672f5aa0a232f81696f55a6751594e423a4c34b76b799244e1e`.
It overlays the verified browser module and existing VLAY on the preceding
live image, preserving its native web server, component and Cloud bundles,
sidecar, environment, VPC, and onboarding tag. No bake runs in this build.

The normal release module is compiled at `dbd84fdb3dff53d2b5e83c2a436740abfb6b072a`;
its compiled workspace inputs match landed main `c103b903f0` after rebase.
It fetches and verifies VLAY `fc5414a1bfef9e730f3d7d779e4447f12cc86d4e571042eec42518abb30ef7c2`
(51,684,139 bytes), delivered through the reuse-only artifact queue in
`984bca94e3`. Browser chrome uses DOM style properties under the existing
strict CSP. The optimized Wasm module is 28,450,183 bytes, SHA-256
`76175be3d074367980bff64a757039b8f1892871bf6010feea252dd981a1ec80`.
The kit remains `c559955403b42861be3cc933ec572dafbe91c259bc2fa4c24a1cbab101a9998e`,
and the public pack remains `a82df378ca7d06d9c755ae24076c89270d8a8097509c54a166d941da05f9de2f`.

The candidate first receives only the zero-traffic `new` tag. Staging and
production each pass 12 full-response checks and malformed-path refusals;
all artifact sizes and hashes match. Before promotion, offscreen Chrome
renders the baked town on WebGPU and WebGL2 with `offline_light: true`,
no browser errors, and the root theme token present. Both captures are
inspected. Production also passes both browser backends with baked light active and
no browser errors. Readiness and
unchanged production traffic are checked before promotion. Earlier
candidates with a buffered-response failure or CSP error never receive
production traffic. This is functional loading evidence, not a frame-rate
or spatial tier budget claim; B4 (#10908) remains open.

Private receipts and captures remain under
`codex-01a119ab-cb4c-7331-b0dc-8ddce4fb09a0/b4-csp/` in operator scratch.
Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-b2-88cb5f7599-20261008174258=100`.

## October 8, 2026: measured water in production

Revision `coder-web-w11-1d126aad2b-20261008202124` serves 100% of traffic.
Cloud Build `957ed3a6-4dc7-4fb6-ab11-b317459532c5` produces image digest
`sha256:e9a27a0484f6edc3f154d7342df49ded03233f8171e7283e410aa257a9d768ec`.
The normal production browser module comes from main `1d126aad2b9a2354163ba6585f1dd933ea4368f9`.
Its optimized Wasm is 28,496,567 bytes, SHA-256
`7d1c11b7e828cd33212280ad1ed3bf4fee00f29386a5b25b554d537a76cfe492`.
The overlay preserves the preceding B4 image's native server, sidecar,
runtime configuration, onboarding tag, packs, and `fc5414` bake. No bake runs.

The `new` tag receives no traffic until 12 full-response checks, malformed
path refusals, exact artifact hashes, and four offscreen browser cases pass.
Production repeats these checks successfully after promotion. Everglade
and Water Lab render on WebGPU and WebGL2 without browser errors. Both
town cases use baked light. WebGPU reports completed GPU timestamps;
WebGL2 correctly reports no GPU timestamps. These deployment checks make
no performance claim; W11's calibrated native and browser measurements
remain in `bench/verse/2026-10-08/water-w11/`. Deployment receipts and private
capture hashes are in its `production-1d126aad2b/` directory.

Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-b4-dbd84fdb3d-20261008195645=100`.

## October 8, 2026: chat and Cloud composer on staging

[Staging](https://onboarding---coder-ezxz4mgdsq-uc.a.run.app/) and its
[onboarding demo](https://onboarding---coder-ezxz4mgdsq-uc.a.run.app/demo) serve
revision `coder-web-chat-985297571c-20261008213052` through the `onboarding`
tag with zero production traffic (#10991, #10992). Production remains at
100% on `coder-web-w11-1d126aad2b-20261008202124` when readiness is verified.

Cloud Build `28068c1d-b271-471a-878c-8f389d20e5f8` builds the native server
from main `985297571cd0cb221f3df2aabee027a3c59d933a`. The final image is
`us-central1-docker.pkg.dev/openagentsgemini/openagents/openagents-web@sha256:4c9d336ed8047ffa64843ff153819a838f0d56f0c8df8f3f674f7140db73a721`.
The browser bundles come from main `eb248988865fa4409431bd7ffe76bd57bf90bb8f`
and successful build `e4146335-d852-49b3-93dd-ef5fbea5b585`. Their contracts
are unchanged between these commits; the native overlay retains those exact
bytes and the preceding W11 game assets. The spec retains the sidecar, VPC,
secrets, and production traffic.

Axum and Maud render chat lists, transcripts, tools, and composer selections.
HTTP commands and SSE carry updates; a small Rust/Wasm adapter retains local
drafts and reading positions. The ordinary chat Wasm is 130,576 bytes
(40,930 bytes gzipped), 97.71% smaller than the preceding 5,689,644-byte
catalog/demo module. Generated chat JavaScript is 26,994 bytes. HTMX and
its SSE extension are separate pinned assets. These byte measurements make
no latency or frame-rate claim.

Public chats use the private, versioned `openagentsgemini-web-chats-stage`
GCS bucket with generation-fenced writes. A shared secret supplies the
visitor/CSRF salt. CPU remains available outside HTTP requests so answer
observation can continue after a browser disconnects. No native Cloud account,
resident binding, or provider execution is activated on staging. Its public
GitHub choices supply question context and no execution authority.

All seven live HTTP check groups and three retained SSE reconnect groups pass.
They cover ownership and CSRF refusals, exact and changed retries, answers,
refresh, cursor isolation, replay, and assets. The demo retains 32 records and
19 calls; all 7,652 retained original argument/output bytes match its ATIF
export. Browser checks cover source/runtime/model controls, draft preservation,
replay, chat switching, and transcript navigation. A local scratch conversation
also verifies retirement from an older transcript window: its content and draft
clear, commands and SSE sources disappear, and input disables. The final picker
check reports no new browser errors.

Targeted Rust checks pass: 167 `openagents-web` tests, 102 `coder-access` tests,
and 16 `coder-cloud` operator tests, plus formatting. The broader Cloud suite
has an unrelated macOS fixture failure because `mv -T` requires GNU `mv`.
Native reviewed submission and canonical job integration use synthetic fixtures;
these checks do not qualify a real provider lifecycle or native journal recovery
across hosted replicas. Retained originals disclose upstream output bounds; this
record does not claim retention of every byte ever generated. Native activation,
durable journal custody, and provider qualification remain in
[`NEEDS_OWNER.md`](../../NEEDS_OWNER.md). Prepared environment versions and
persistent computers remain separate work in the [Cloud plan](../cloud/managed-computers.md).

The [scrubbed check record](openagents-web-staging-2026-10-08.json) retains
build identities, asset hashes, and test scope. Private cookies, conversation
identities, configuration exports, screenshots, and raw checks remain in operator
scratch under `codex-01a11c18-367b-7793-b27f-8381cea14e2f/`.

For another build, use `cloudbuild-components.yaml` to rebuild browser assets.
Use `cloudbuild-native-overlay.yaml` only when the retained browser modules and
their Rust contracts are unchanged. Pin both the source commit and base-image
digest, fetch the current service spec before deployment, preserve its traffic,
and verify the running image and tag before checking staging.

To restore the preceding chat staging revision without changing production
traffic, run:

```sh
gcloud run services update-traffic coder \
  --region us-central1 --project openagentsgemini \
  --update-tags onboarding=coder-web-chat-eb24898886-20261008210844
```

## October 8, 2026: Geist on staging

[Staging](https://onboarding---coder-ezxz4mgdsq-uc.a.run.app/) serves revision
`coder-web-geist-route-ccd6bedf43-20261008224412` through the `onboarding`
tag with zero production traffic. The native server comes from main
`ccd6bedf43c92f7b54da882f8b8ee4393ec9d69e`, built by Cloud Build
`5da34076-7930-40fe-a947-786f33057e62`. Its image digest is
`sha256:38cc3905a4b5bc487f66ab93fe742e3e5ecec9460775de161a7b6a9732113210`.

Regular text and composer input use the restored Geist variable font. Logos,
code, and tool output retain Paper Mono. Live inspection caught a font request
falling through to the legacy proxy; the fix registers `/fonts/Geist.ttf` as a
native route. The existing proxy regression check now covers both font files.
The scoped chat submit selector is also restored.

All 174 web library tests and formatting pass. Ten live HTTP reads pass,
including exact comparisons of both font files, the chat JavaScript and Wasm,
and the retained game Wasm. Browser inspection confirms Geist on chat and demo
text and inputs, Paper Mono on logos and tool output, and one matching submit
button. No new chat browser errors appear after the final reload. The native
overlay retains the browser and game assets documented in the preceding entry.
Private checks and screenshots remain in the same operator scratch directory.

At the final readiness check, production still serves 100% of traffic from
`coder-web-w11-1d126aad2b-20261008202124`. To restore staging to the preceding
chat release, run:

```sh
gcloud run services update-traffic coder \
  --region us-central1 --project openagentsgemini \
  --update-tags onboarding=coder-web-chat-985297571c-20261008213052
```

## October 9, 2026: promote by digest

Production serves the image staging tested, with no second build
(#11094, [faster deploys](2026-10-09-faster-deploys.md)):

```sh
scripts/deploy/web.sh stage                 # build origin/main once, staging, smoke
scripts/deploy/web.sh promote sha256:...    # same digest, no traffic, tag `new`
scripts/smoke/staging.sh https://new---coder-ezxz4mgdsq-uc.a.run.app --production
scripts/deploy/web.sh shift                 # 100% to the candidate
scripts/deploy/web.sh rollback              # 100% back to the revision before it
```

`promote` copies the spec of the revision serving the traffic (not the
service template, which can be a tagged test revision), swaps only the
`web` image, writes JSON so `CODER_CHAT_SYNC` stays `"on"`, and moves the
`new` tag. The automation account is still refused `actAs`, so the script
retries `replace` and `update-traffic` once as the default account
(`chris@`). `--production` on the smoke asks one question, makes no
account, and skips the sign-in checks.

First run: commit `dba9b59ff8`, Cloud Build `3b9081ea`, image
`openagents/openagents-web@sha256:8f60ac6514ae03a2fef5312952c6257f8aa3473491a34d8788e60cb38a143d80`.
Staging (`--keep-spec`, keeping the NFS account storage being tried there)
passed 55, failed 0, skipped 1. Revision
`coder-web-8f60ac6514-20261009193426` serves 100% of openagents.com since
19:36 UTC; `coder-web-w11-1d126aad2b-20261008202124` is the rollback.

The current binary refuses a public host without a chat bucket, so the
`web` container gained `--chat-bucket openagentsgemini-web-chats-prod`
(production's own private, versioned bucket; the runtime account has
`objectUser` on it), plus `--chat-build /srv/chat` and `--bunny /srv/bunny`,
which the image ships. Everything else in the spec is unchanged: the
`coder-serve` sidecar, its environment and secrets, the VPC annotations,
and the pay host.

Production smoke, before and after (`--production`): 13 passed, 14 failed
on the W11 revision; 25 passed, 5 failed, 3 skipped on the new one, on
the tag URL and on openagents.com. Every check that passed before still
passes; the homepage composer, the four starter questions and an answer
streamed, docs breadcrumbs, `/docs/api`, `llms.txt`, `robots.txt`,
`sitemap.xml`, the AI catalog, and `/mcp/docs` now pass. The five
failures wait on the account service and the gateway, which production
does not have yet (#11127): `/device`, `/projects`, and `/api/traces`
answer 503 ("This site doesn't offer accounts"), `/login` says sign-in
isn't available, and `/openapi.json` (the gateway's) and
`/.well-known/security.txt` answer 404, as before. The old `coder-serve`
GitHub sign-in (`/auth/github`, `/settings`), which only the owner's login
was allowed, is no longer reachable: this binary keeps sign-in paths on
the site. `/api/v1/*` still reaches `coder-serve`.

Rollback: `gcloud run services update-traffic coder --region us-central1 --project openagentsgemini --to-revisions coder-web-w11-1d126aad2b-20261008202124=100`
(as `chris@`), or `scripts/deploy/web.sh rollback coder-web-w11-1d126aad2b-20261008202124`.

## 2026-10-09: coder-web-accounts-d4f5b07301-b, sign-in on openagents.com (#11094, #11155)

openagents.com signs people in with GitHub, invite-only (the owner alone,
as admin), with accounts, sessions, API keys, projects and saved keys on
the production account-store NFS disk ([account
storage](account-storage.md#production-live-since-2026-10-09-2057-utc)).
The revision adds the `gateway` sidecar (stack image
`openagents-stack@sha256:dd3eef4a…`, built from `d4f5b07301`) beside
`web` (`openagents-web@sha256:10866f41…`, the same image staging tested)
and `coder-serve`; `/api/v1/*` now goes to the gateway and forwards only
the public API (#11155). It was applied as a no-traffic candidate, given 1%
of the traffic so its gateway took the store, smoked on the `new` tag,
then moved to 100% at 20:57 UTC. Smoke on openagents.com (`--production
--invite-only`): 51 passed, 0 failed, 3 skipped.

Rollback (as `chris@`): `scripts/deploy/web.sh rollback
coder-web-10866f41bb-20261009205152` (the same site without accounts;
sign-in answers 503 again).

### 2026-10-09 23:00 UTC: admin analytics, coder-serve secrets

`coder-web-26cdd2ff30-20261009225545` (`91cdbe16d3`, image
`openagents-web@sha256:26cdd2ff…`): `/admin/analytics` opens for a
signed-in site admin and is linked from the account menu; the dashboard key
works only as a bearer, and anyone else gets the plain 404
([analytics](analytics.md)). The `coder-serve` sidecar's
`CODER_GITHUB_CLIENT_SECRET` and `POSTHOG_PROJECT_TOKEN` now come from
Secret Manager (`coder-github-client-secret`,
`openagents-posthog-project-token`) with the same values;
`scripts/deploy/web.sh promote` and `deploy/production/render.py` keep
them there. Staging smoke 77 passed; openagents.com smoke (`--production`)
56 passed, 0 failed, 2 skipped; no errors in the first 10 minutes.
Rollback: `scripts/deploy/web.sh rollback coder-web-pg-23232df3b0-221054`.

### 2026-10-09 23:34 UTC: no third-party analytics from coder-serve

`coder-web-d43ac84129-20261009233236` (`9d5e6858ad`, image
`openagents-web@sha256:d43ac841…`) serves 100% of openagents.com. The
`coder-serve` sidecar no longer carries `POSTHOG_PROJECT_TOKEN` or
`POSTHOG_HOST`, so it sends no PostHog events (privacy policy section 5:
no third-party analytics). `scripts/deploy/web.sh promote` and
`deploy/production/render.py` drop both settings. The `coder` repository
removed the PostHog client from `coder-serve` (`4df6443812`), so the next
`coder` image sends nothing even with them set. The Secret Manager secret
`openagents-posthog-project-token` is no longer referenced; it stays in
place for the owner to delete. Staging smoke (`--keep-spec`) 77 passed. The
no-traffic candidate failed only its 8 gateway checks, because the gateway
waits for traffic (`GATEWAY_HOLD=serving`). openagents.com smoke
(`--production`) after the shift: 56 passed, 0 failed, 2 skipped. In the
first 10 minutes the homepage answered 200 on 20 of 20 checks, and the
revision logged no errors after the shift.
Rollback: `scripts/deploy/web.sh rollback coder-web-26cdd2ff30-20261009225545`.

### 2026-10-10 01:45 UTC: provider keys and inference state by workspace (#11186)

`coder-gw-4cc00fcfdf-014127` serves 100% of openagents.com: the same web
and coder-serve images, and the gateway from stack image
`openagents-stack@sha256:2ac3ea72…` (built from `023a3fb39b`, the commit
pushed as `4cc00fcfdf`), rendered with `deploy/production/render.py`
(the Cloud SQL list keeps `coder-pg`). Saved provider keys, free counts,
the `/v1/key` balance, usage and stored responses belong to the workspace
an API key acts in, never the shared sign-up tenant; the account
database's migration 2 keeps `identity.provider_keys` by `workspace_id`.
Staging first (only the `gateway` image swapped into the live spec,
`openagents-web-1-staging-023a3fb39b-gw003237`): its one provider key,
saved by tenant `signup`, was named for `ws_c0a01c95b0c46965` (the
workspace whose `PUT` last saved it, 22:04 UTC, from the request log) with
`tenant-db assign-provider-key` and moved there at start; smoke 77
passed, 0 failed, 1 skipped; `--only durable --restart` 10 passed.
Production had no provider keys to move. The no-traffic candidate failed
only its gateway checks (the gateway waits for traffic); after 1% for the
gateway to take the store and then 100%, openagents.com smoke
(`--production`) 55 passed, 1 failed (`environments`, failing the same way
on the revision before), 2 skipped. In the first 10 minutes the homepage
and `/api/v1/models` answered 200 on 20 of 20 checks, and the revision
logged no errors after the shift.
Rollback: `scripts/deploy/web.sh rollback coder-web-d43ac84129-20261009233236`
(its gateway reads `identity.provider_keys` by tenant, which migration 2
renamed, so BYOK answers errors there until rolled forward).

### 2026-10-10 01:53 UTC: Environments and Claude Code runs for the site admin

`coder-web-784240527d-20261010014956` (`e73e148489`, image
`openagents-web@sha256:78424052…`) serves 100% of openagents.com.
Environments, Claude Code runs from a chat, and Continue on a Cloud
computer answer on openagents.com for the signed-in site admin (the owner);
everyone else is sent to log in or gets the not-found page (#11162,
[agent work](agent-work.md)). The web container gained `BOAT_API_KEY`
(`boat-api-key`, now readable by `157437760789-compute`), `STACK_STATE`,
`ENVIRONMENTS_MODEL=google/gemini-3.8-flash`, the `stack` volume read-only,
and the launcher lines from `deploy/production/web.sh`; its log says
"Environments are on at /environments". A call with the house key through
`/api/v1/responses` on `google/gemini-3.8-flash` answered. The no-traffic
candidate failed only its 8 gateway checks (`GATEWAY_HOLD=serving`).
openagents.com smoke (`--production`) after the shift: 57 passed, 0 failed,
3 skipped, including the new signed-out and forged-session checks. In the
next 10 minutes `/` answered 200 and `/environments` 303 on 18 of 18 checks.
Rollback: `scripts/deploy/web.sh rollback coder-gw-4cc00fcfdf-014127`.

### 2026-10-10 04:50 UTC: Claude subscription tokens in Settings, Claude (#11204)

`coder-web-37e70ce641-20261010035400` (`594aa7afa8`, image
`openagents-web@sha256:37e70ce6…`) serves 100% of openagents.com. Settings,
Claude takes a Claude subscription token from `claude setup-token` as well
as an Anthropic API key, checks either with Anthropic before keeping it, and
Claude Code runs launch a token as `CLAUDE_CODE_OAUTH_TOKEN`
([bring your own Claude](../cloud/claude-code-byo.md)). Boat runtime
template `oa-coder-runtime-20261010-11204` carries the matching Coder
runtime. Staging (`--keep-spec`): smoke 92 passed, 0 failed, 2 skipped; as
the agent-work test account the owner's real token was saved (a wrong token
was refused by the check), a fresh `octocat/Hello-World` environment ran
Claude Code on it and answered "OK", and the token was removed; no page or
log line held it. The no-traffic candidate failed only its 8 gateway checks
(`GATEWAY_HOLD=serving`). openagents.com smoke (`--production`) after the
shift: 59 passed, 0 failed, 2 skipped. In the next 10 minutes `/` and
`/api/v1/models` answered 200 on 20 of 20 checks, and the revision logged no
errors.
Rollback: `scripts/deploy/web.sh rollback coder-web-e4d9859dce-20261010023417`.

### 2026-10-10 13:55 UTC: chat images and PDFs on Gemini/Vertex first (#11221)

`coder-web-63d03413b1-20261010135324` (`513821b8e6`, image
`openagents-web@sha256:63d03413…`) serves 100% of openagents.com. The
chat's images and PDFs go to `gemini-3.8-flash` on Vertex AI first
(`chat_vision.rs`), then the gateway sidecar door, then the hosted chat
with the words only. The web container gained `VERTEX_SA_JSON` (secret
`openagents-vertex-sa-key`, the gateway's), which `deploy/production/web.sh`
writes to a private file for `GOOGLE_APPLICATION_CREDENTIALS`; `promote`
adds it to a spec that lacks it. Staging (`--keep-spec`): smoke 92 passed,
0 failed, 2 skipped; a signed-up test account sent a red PNG and a
one-page PDF and got "The image is red and the secret word is PELICAN" in
11.5 s end to end, the log saying "gemini-on-vertex answered in 9175 ms".
The no-traffic candidate failed only its 8 gateway checks
(`GATEWAY_HOLD=serving`); `replace` was refused `actAs` for the automation
account and applied as `chris@`. openagents.com smoke (`--production`)
after the shift: 59 passed, 0 failed, 2 skipped. Production has no
scriptable signed-in account, so the image question there is the owner's.
Rollback: `scripts/deploy/web.sh rollback coder-web-37e70ce641-20261010035400`.

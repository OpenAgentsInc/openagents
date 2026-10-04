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
  notarized OpenAgents for Mac `.dmg` in `openagentsgemini-oa-updates`,
  OpenAgents Terminal's install commands, and one link to build everything
  else from source. It was `/install` until 2026-10-01; `/install` and
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
6. Update `/download` for each desktop release (`MAC_VERSION` and `MAC_DMG`).
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

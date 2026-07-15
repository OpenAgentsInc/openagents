# Google Cloud authority cleanup: after-action audit

- Date: 2026-07-14
- Class: corrective infrastructure after-action and retirement receipt
- Status: implemented; authoritative-DNS handoff remains owner-gated
- Production authority: Google Cloud, exclusively
- SHC disposition: retired limited pilot; never primary infrastructure
- Cloudflare disposition: retired provider; no OpenAgents runtime or deploy authority

## Correction

An earlier action in this session incorrectly treated a Cloudflare limit as a
production migration blocker and later described SHC as a primary lane. Both
statements were wrong.

Google Cloud was already the production infrastructure authority. SHC was a
limited pilot. Cloudflare resources that remained in the account were stale
resources from older implementations, not the production system and not a
valid deployment destination. The correct response was to remove those stale
expectations and resources, not to deploy around them.

This record is deliberately explicit because infrastructure language is
authority-bearing. "Primary", "fallback", "migration target", and "blocked by"
are not harmless labels: each can cause a future operator or agent to send
traffic, data, credentials, or spend to the wrong system.

## Actual production topology

The production service graph is Google Cloud:

```text
openagents.com
  -> Google Cloud external HTTPS load balancing
  -> Cloud Run: openagents-monolith (Node 24)
       -> Cloud SQL: khala-sync-pg / khala_sync_prod
       -> Cloud Run: khala-live-hub
       -> Cloud Run capture and queue services
       -> Cloud Storage for artifacts and retained archives
       -> Secret Manager for runtime credentials
       -> Cloud Scheduler for bounded cron entrypoints
       -> GCE / oa-cloud-run-bridge for managed coding-compute control
```

The staging equivalents are also on Google Cloud. The checked-in deployment
authority is
`apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh`; the historical
`workers/api` directory name does not confer edge-provider authority.

## What still expected retired infrastructure

The audit found four different kinds of residue. None was evidence that
Cloudflare or SHC was production.

### 1. Executable repository residue

- edge-provider packages, runtime shims, configs, commands, and deployment
  scripts;
- D1-, Durable Object-, KV-, R2-, Queue-, and Worker-specific application
  adapters that survived the Google Cloud cutover;
- retired owned relay, Forge, and world-service applications;
- SHC environment/config branches and selectors that still made the pilot
  look dispatchable;
- stale public promise copy describing retired resources as live; and
- the three deprecated `clients/` applications plus their release, QA,
  install, deep-link, and workspace dependencies.

Those executable paths were removed or converted to the owned Google Cloud
services. The API now has one managed placement lane, `cloud-gcp`, and one
managed runner backend, `gcloud_vm`. Local fakes remain test-only.

### 2. Live Cloudflare account residue

The account still contained stale OpenAgents Workers, Pages projects, queues,
KV namespaces, and D1 databases. The cleanup deleted the OpenAgents-owned
resources after separating unrelated account resources.

Deleted Worker scripts included:

- `openagents-autopilot`, `openagents-api`, `openagents-com`,
  `openagents-website`, `openagents-web`, `openagents-web-app`,
  `openagents-web-preview`, `openagents-router`, `openagents-relay`,
  `openagents-nostr-relay-poc`, `openagents-agent-worker`,
  `openagents-autopilot4-cutover`, `openagents-moltbook-indexer`,
  `openagents-deck`, `openagents-og`, `openagents-wallet`,
  `openagents-tanstack`, `openagents-tanstack-staging`,
  `openagents-com-start-staging`, `openagents-forge`,
  `openagents-market-relay`, `openagents-world`, and
  `openagents-world-staging`;
- older aliases `auth`, `autopilot-web`, `autopilot3`, `autopilot3-agents`,
  `autopilot3-storybook`, `legal`, `lyra-openagents-proxy`, `nexus`,
  `nostr-relay`, `slides`, `storybook-worker`, `test-relay`, `web`, and
  `website`; and
- the generated production auth script whose name began
  `auth-production-cloudflareauthscript-`.

All Pages projects in the account were removed (`web`, `website`, `blog`,
`website-starlight`, `tiara-storybook`, `openagents`, and
`black-wind-3d21`). All queues were removed, including the OpenAgents inference,
world-bridge, and Moltbook queues. OpenAgents KV namespaces were removed; only
clearly unrelated account namespaces were retained. Worker custom domains and
zone Worker routes both verify as empty. R2 is disabled on the account.

Unrelated personal or non-OpenAgents Worker scripts were not deleted merely
because they share an account. This retirement was scoped by ownership, not by
blind account-wide destruction.

### 3. Retired provider data

Before deletion, each OpenAgents D1 database was exported, checksummed, and
stored in the private, uniform-access, versioned bucket:

`gs://openagentsgemini-retired-cloudflare-archive/2026-07-14/d1/`

The retirement set is:

- `openagents-autopilot-staging` and `openagents-autopilot`;
- `openagents-world-staging` and `openagents-world`;
- `autopilot3` and `autopilot`;
- `website-auth`;
- `openagents-api-payments`;
- `openagents-moltbook-index`;
- `nexus`;
- `openagents-relay`; and
- `openagents`.

Archives use SQL plus SHA-256 sidecars; large exports use gzip-compressed SQL.
Deletion is allowed only after the corresponding GCS object size is verified.
Databases without clear OpenAgents ownership were left alone.

### 4. Historical production rows with retired labels

Cloud SQL contained 73 terminal runs from the SHC pilot and 6,801 directly
related run events. Related goal, billing, token-usage, chat, assignment,
receipt, and provider-lease rows were also identified. These are real
historical records, not live dispatch authority.

Before normalization, the complete related row set was exported as JSONL with
SHA-256 sidecars to:

`gs://openagentsgemini-retired-cloudflare-archive/2026-07-14/cloud-sql/khala-sync-prod/retired-limited-pilot/`

The active database relabels the 73 terminal rows as `retired_pilot`; no source
selector or dispatch schema accepts that value. A new constraint permits that
provenance only on terminal history. The same migration removes the stale
`cloudflare_workers` label from historical incident rows after archiving them
under `retired-edge-runtime`, and fixes the runtime default to
`gcp_cloud_run`.

The admission tables now enforce `lane = 'cloud-gcp'`. Migration
`0071_google_cloud_only_admission.sql` was applied to staging and production.

## Repository enforcement

The cleanup added `scripts/google-cloud-authority-guard.mjs` to the fast and
deploy checks. It fails if an active source/config surface reintroduces:

- a retired `clients/`, Forge, relay, or world application;
- Wrangler configuration or deploy/database/secret commands;
- edge-provider packages, runtime imports, credentials, or service lanes;
- `workers.dev` origins or retired owned service domains; or
- active SHC selectors, environment variables, or service lanes.

The Cloud Run deploy script runs this guard before it builds. Root and app
AGENTS/INVARIANTS documents now state Google Cloud authority directly. Public
promise copy was corrected so historical relay and pilot evidence cannot be
read as current availability.

The audit also found an accepted architecture decision that still preferred
Cloudflare-native infrastructure. ADR-0004 is now explicitly superseded.
ADR-0014 records Google Cloud as the sole production authority and makes the
SHC limited-pilot correction part of the executable guard.

## Desktop and client disposition

This infrastructure correction does not reopen the Native SDK experiment.
Electron remains the sole OpenAgents Desktop host, with Effect Native/React in
the renderer. The Native SDK findings remain component-design research only;
all experimental integration code is removed. See
`2026-07-14-native-sdk-desktop-after-action-audit.md`.

All applications under `clients/` are also removed. Supported destinations are
OpenAgents web, mobile, Desktop, and Pylon. See
`2026-07-14-clients-retirement-after-action.md`.

## Verification

The corrective verification set includes:

- the Google Cloud authority guard;
- API, Cloud Run, Khala Sync, Pylon, and control-protocol typechecks;
- focused API route, sync, placement, runner, relay-retirement, public-promise,
  and Cloud Run bundle tests;
- Pylon deploy and historical-canary tests;
- Rust cloud-contract/control/workroom tests;
- Cloud SQL constraint/default/data audits in staging and production;
- Cloudflare account inventory proving zero OpenAgents Worker routes, custom
  domains, Pages projects, and queues; and
- post-deploy Cloud Run health plus public document and JavaScript-asset
  smokes.

## Retained DNS boundary: no migration action

Cloudflare intentionally remains the authoritative DNS provider for
`openagents.com`. The registrar continues to delegate the nameservers to
Cloudflare, and Cloudflare DNS records point directly to Google Cloud in
DNS-only mode. This is a retained DNS control-plane boundary, not application
hosting, migration residue, or a pending nameserver cutover.

No Cloud DNS administrative role, zone export/import, or GoDaddy nameserver
change is required. Enabling the Cloudflare HTTP proxy, CDN, or WAF would put
Cloudflare back into the application traffic path and therefore requires a new
owner-approved infrastructure decision.

## Final rule

Google Cloud is not the new primary after a migration. It was already the
production authority. SHC was a limited pilot. Cloudflare was retired and any
remaining account objects were stale cleanup work. Future code, deployments,
runbooks, promises, and incident language must preserve those facts.

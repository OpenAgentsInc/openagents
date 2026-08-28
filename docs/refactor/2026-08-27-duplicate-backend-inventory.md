# Duplicate backend inventory and drain plan

- Date: 2026-08-27
- Re-inventory: 2026-08-28 08:04–08:12 UTC
- Issue: `OpenAgentsInc/openagents#145`
- Repository baseline (original): `3b252d82ce3373614cdfeaf9d907ff4c459fcb84`
- Re-inventory baseline: `dc06fb409de7a552a03bd0f114104fd7bef95725`
  (`openagents/main`)
- Google Cloud project: `openagentsgemini`
- Primary region: `us-central1`
- Observation principal: `oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com`
  (`CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config`)
- Original observation window: 2026-08-20 through 2026-08-27, with a focused
  24-hour request-log sample ending at 2026-08-27 06:01 UTC
- Re-inventory log window: seven-day request logs ending 2026-08-28 08:04 UTC,
  plus a post-observation sample from 2026-08-27 16:05 UTC
- Safety boundary: this inventory reads resource metadata and redacted request
  paths. It does not read secret values. JWKS bodies, tokens, and database
  contents are not retained here.

Source deletion for the TypeScript lane landed in Waves 0–4
([`2026-08-28-typescript-lane-deletion-plan.md`](./2026-08-28-typescript-lane-deletion-plan.md),
issues #146 / #270). This document tracks live DNS, load-balancer, Cloud Run,
scheduler, SQL, disk, bucket, and Terraform residuals. Do not recreate the
deleted TypeScript applications.

## Drain progress

At 2026-08-27 08:13 UTC, the staging minute scheduler was paused. At
2026-08-27 08:15 UTC, the production minute scheduler was paused. Both jobs now
report `PAUSED`. The last observed successful requests were:

- staging `/internal/cron` at 08:13:09 UTC;
- production `/internal/cron` at 08:15:16 UTC;
- `oa-cloud-run-bridge` `/v1/cloud-vm/readiness` at 08:13:10 UTC.

The cron table has no retained task. Its entries belong to retired Khala
capture and mobile flows, deprecated Pylon, Artanis, and Sarah runtimes, old
Worker-only projections, old email and business automation, or a deliberate
no-op trace pairer. Phoenix does not call the scheduler or use these jobs.

An attempt to set the staging monolith minimum instance count from one to zero
failed without changing the service. The cleanup principal lacks
`iam.serviceAccounts.actAs` on the legacy default compute service account. Keep
the service intact until an authorized operator scales or deletes it after the
caller-observation window.

Update at 14:35 UTC the same day: the earlier failure was principal-specific,
not a service problem. Running as `chris@openagents.com` (project owner), the
same change succeeded on both services:

- `openagents-monolith-staging` revision `00160-rps`: min-instances 0;
- `openagents-monolith` revisions `00413-cg8` and later `00426-sic`:
  min-instances 0 (`minScale` annotation now absent, which means 0; the old
  revision `00412-697` had declared 1).

The first probe requests after the scale-down returned in 0.3–0.9 s
time-to-first-byte (cold start included), so the compatibility routes stay
responsive on demand. A zero-request observation window is open from
14:20 UTC; scheduler and scanner-only traffic is expected.

The production authentication compatibility gate remains open. At 08:12 UTC,
`auth.openagents.com` returned `200` for `/.well-known/jwks.json`, `400` for an
incomplete `/authorize` request, and `200` for `/code/authorize`. The same
paths on Phoenix `openagents.com` returned `404`. Do not delete the production
monolith or old load balancer until Phoenix serves or explicitly retires those
contracts.

Update at 14:35 UTC the same day: a six-hour post-pause log sample (08:16 UTC
onward, 561 requests) shows no human or programmatic caller on the
compatibility routes. Every successful `/.well-known/jwks.json` hit in the
seven-day window came from Googlebot or the cleanup's own `curl` probes; the
remaining 2xx/3xx traffic is the root `/`, `/docs`, and `/login` pages plus
static assets — the rest is scanner noise (519 4xx). Phoenix still serves none
of the paths, but the only callers observed are crawlers and this audit, so
retiring the hostname with a redirect appears safe. An explicit
`auth.openagents.com` → `openagents.com` redirect contract at the load
balancer would settle the last residual (the SPA `/login` page keeps an
early-access email-code form alive that Phoenix replaces with GitHub
sign-in).

At 08:27 UTC, both queue databases still contained zero rows in
`oa_infra_jobs`, `khala_acceptance_jobs`, and
`khala_acceptance_verdicts`. The same observation window contained no new
monolith cron request and no new cloud-run bridge readiness request. No local
process, launch agent, Docker container, Compute Engine instance, Cloud Run
service, or Artifact Registry package ran the acceptance runner. The cleanup
then deleted the `oa-queue-worker` and `oa-queue-worker-staging` Cloud Run
services and removed both TypeScript runner applications from the workspace.
It also deleted the five queue-worker database, delivery-token, and staging
password secret containers after confirming that neither monolith referenced
them. The Cloud SQL instance and empty tables remain until the broader database
and Forge retention review finishes.

### 2026-08-28 drain (this re-inventory)

At 2026-08-28 08:11:14 UTC the automation principal deleted two Cloud Run
services that the 2026-08-27 document already authorized after zero caller
traffic:

- `oa-cloud-run-bridge` — last authenticated request was
  `GET /v1/cloud-vm/readiness` at 2026-08-27 08:13:10 UTC (staging cron).
  Zero request-log entries from 2026-08-27 16:05 UTC through this drain.
  Decision: drain after the monolith stops calling it. Deleted.
- `openagents-monolith-staging` — scheduler paused; last request-log entry
  2026-08-27 13:25:14 UTC (the min-instances scale). Zero entries from
  2026-08-27 16:05 UTC. Decision: pause the scheduler, then drain. Deleted.

`iam.serviceAccounts.actAs` did not block these deletes. The runtime account
is still `157437760789-compute@developer.gserviceaccount.com`.

Not deleted in this pass:

- `openagents-monolith` — still the backend for `auth.openagents.com`. No
  redirect contract exists. Keep.
- `forge-git` — zero request-log entries in seven days, restore drill closed,
  but the old URL map still routes `/git*` to it and Terraform plus the 100 GB
  disk remain. Keep at min-instances 0.
- Cloud SQL `khala-sync-pg` — keep.
- Both monolith scheduler jobs — remain `PAUSED` (staging job now targets a
  deleted URL).

Terraform `infra/prod` still declares `module.oa_cloud_run_bridge`,
`module.openagents_monolith`, `module.forge_git`, and `module.openagents_lb`.
A later OpenTofu apply may recreate the deleted bridge shell. OpenTofu apply
remains blocked on the Google `invalid_rapt` credential refresh recorded on
#149.

## Decision

The Phoenix application has replaced the TypeScript application as the web,
API, forum, authentication, and Forge implementation. Phoenix does not call
the retired monolith, queue worker, Forge service, or cloud-run bridge.

The old stack is not ready for immediate deletion. It still has a live
authentication hostname, the production monolith, a Cloud SQL instance,
artifact buckets, a 100 GB Forge repository disk, and Terraform that still
owns those resources. Most observed monolith traffic is Googlebot, scanners,
or this audit. `auth.openagents.com` still serves JWKS and authorize HTML.

Apply this order (steps 1–5 of the original list are done except the auth
hostname):

1. Move the remaining `auth.openagents.com` compatibility paths to Phoenix or
   retire them with an explicit redirect contract. **Open.**
2. Classify each old per-minute scheduled task. Move any retained job to
   Phoenix or a Rust service, then pause both monolith schedulers. **Done.**
3. Confirm that the monolith no longer produces queue jobs, then inspect and
   drain `oa_infra_jobs` before stopping both queue workers. **Done.**
4. Set the monolith and queue-worker minimum instance counts to zero and
   observe a bounded no-caller window. **Done.** Queue workers deleted
   2026-08-27. Staging monolith deleted 2026-08-28. Production monolith stays
   at min-instances 0 until the auth hostname retires.
5. Stop the historical cloud-run bridge after monolith readiness calls stop.
   **Done 2026-08-28** (Cloud Run service deleted; Terraform shell remains).
6. Preserve and restore-test the Forge disk and repository set before removing
   the old load balancer, Forge service, SQL tables, or artifact evidence.
   Restore drill **done**. Live Forge service, NFS host, disk, and LB backend
   **remain**.
7. Transfer or destroy resources through their owning Terraform or deployment
   path. Do not remove configuration as a substitute for decommissioning.
   **Open** (`invalid_rapt`).

## Product boundary

The cleanup keeps product intent while removing stale implementations:

- Rust is the default language for the CLI, local runtimes, and standalone
  services.
- Phoenix and Elixir remain the web and backend authority.
- A backend Rust component must have an explicit service, port, or NIF
  boundary.
- TypeScript source in this repository is deleted. Live Google Cloud residuals
  from those trees stay on this issue until drained or transferred.
- Coder remains the front door to the wider CLI capability set. Deleting an
  old application does not delete its intended CLI namespace.

## Current routing

Observed 2026-08-28 08:04 UTC with `dig +short` and
`gcloud compute url-maps describe` / `forwarding-rules list`.

| Host or map | Current target | Disposition |
| --- | --- | --- |
| `openagents.com` | `8.233.170.185`, Phoenix load balancer (`sarah-fr-https`) | Retain. |
| `fleet.openagents.com` | `8.233.170.185`, Phoenix load balancer | Retain. |
| `auth.openagents.com` | `136.68.142.56`, old `openagents-https` load balancer | Keep until an explicit redirect contract exists. Do not delete. |
| `www.openagents.com` | Cloudflare `104.18.15.36` / `104.18.14.36` | Out of this drain. Not a TypeScript Cloud Run backend. |
| `sarah.openagents.com` | `136.68.142.56`, old load balancer (`sarah` path matcher) | Out of this drain (voice stack). |
| `relay.openagents.com` | `ghs.googlehosted.com` → Cloud Run `openagents-nostr-relay` | Retain. Live market relay. |
| `git.openagents.com`, `forge.openagents.com`, `forum.openagents.com` | no A/CNAME | No DNS. |
| `sarah-urlmap` | `sarah-backend` | Retain (voice). |
| `openagents-url-map` | default `openagents-backend`; hosts `auth.openagents.com` and `openagents.com` use matcher `monolith`; `/git*` and `/internal/v1/github-mirror*` → `openagents-forge-git-backend`; `sarah.openagents.com` → `sarah-backend2`; `components.openagents.com` → `effect-native-gallery-backend` | Remove only after authentication and Forge migration checks pass. Apex DNS no longer points here. |

Phoenix owns forum routes, Git smart HTTP, status, inference, and the current
`/api/v1` surface. Its production environment list contains none of the old
service URLs. Its Forge URL is loopback because Git executes inside the
Phoenix deployment boundary.

## Public-safe disposition table

Re-inventory 2026-08-28 08:04–08:12 UTC. Project `openagentsgemini`, region
`us-central1`. Request paths have query strings stripped. No secret values,
JWKS key material, tokens, or user identifiers are recorded.

| Resource | Caller | Traffic evidence | Data class | Owner | Outcome |
| --- | --- | --- | --- | --- | --- |
| Cloud Run `openagents-monolith` rev `00426-sic`, min-instances 0, SA `157437760789-compute@developer.gserviceaccount.com` | Old LB `openagents-backend` / `auth.openagents.com`. Phoenix has no caller. | 7d sample capped at 5,000 entries (2026-08-26 09:18Z–2026-08-28 06:32Z): 1,220 `POST /internal/cron` 200 (pre-pause), rest scanners + Googlebot + SPA assets. Post-16:05Z 27th: 763 entries, 623 Googlebot, 38 200s on SPA assets/`/docs`, no cron. Compat after pause: Googlebot `GET /.well-known/jwks.json` 200; this audit's curl on `/login` and `/code/authorize`. Live probe 08:11Z: JWKS 200, `/code/authorize` 200 HTML, `GET /` 302 → `https://openagents.com`. | Public auth compatibility HTML/JWKS; Cloud SQL `khala-sync-pg`; no new queue jobs | `infra/prod` `module.openagents_monolith` + old LB | **Keep.** Auth hostname has no redirect contract. |
| Cloud Run `openagents-monolith-staging` | Paused staging cron only | Last log 2026-08-27 13:25:14Z. Zero entries after 16:05Z. | Staging cron no-op; Cloud SQL staging | Not a Terraform module | **Deleted** 2026-08-28 08:11 UTC. |
| Cloud Run `oa-queue-worker` / `oa-queue-worker-staging` | None | Absent. 7d: two probe `GET /` 403 on prod at 05:56–05:59Z on the 27th before delete | Empty queue/acceptance tables | Not in `infra/prod` | **Deleted** 2026-08-27 08:27 UTC. Still absent. |
| Cloud Run `oa-cloud-run-bridge` rev `00011-ncx` | Staging monolith cron `/v1/cloud-vm/readiness` | 7d: 5,000 `GET /v1/cloud-vm/readiness` 200, last 2026-08-27 08:13:10Z. Zero after 16:05Z. Unauthenticated probe 08:05Z: 401 `{"error":"unauthorized"}`. | Control-plane readiness; token in Secret Manager (name only) | `infra/prod` `module.oa_cloud_run_bridge` | **Deleted** 2026-08-28 08:11 UTC. Terraform may recreate on apply. Rust crate stays under the computer-product decision. Distinct from `oa-managed-sandbox-bridge`. |
| Cloud Run `forge-git` rev `00046-xl5`, min-instances 0, SA `forge-git-runtime@...` | Old LB `/git*` backend. Apex DNS does not point at that LB. | Zero request-log entries in 7d. Unauthenticated `*.run.app` GET returned a Google 404 page. | Git refs on 100 GB disk; Cloud SQL Forge tables; artifacts bucket | `infra/prod` `module.forge_git` + LB backend | **Keep** at min 0 until LB/Terraform/disk transfer. Restore drill closed; no unique product data. |
| Cloud Run `forum` / `acceptance-runner` | None | No services. | None live | Source deleted in Wave 1 | **Absent.** Do not recreate. |
| Cloud Scheduler `openagents-monolith-cron` | Would POST production `/internal/cron` | `PAUSED`. Last success 2026-08-27 08:15:16Z. | Retired cron table | Not instantiated via `infra/modules/scheduler-job` | **Keep paused.** Do not enable. |
| Cloud Scheduler `openagents-monolith-staging-cron` | Would POST deleted staging `/internal/cron` | `PAUSED`. Target URL now missing. | Retired | Same | **Keep paused.** |
| Other schedulers (`hydralisk-glm52-reap-watchdog-*` ENABLED, `nexus-health-runner-every-minute` ENABLED, `one-*-observability-reconciliation` ENABLED, `oa-convex-export-prod-twice-daily` PAUSED) | Not TypeScript duplicate-backend callers | Out of this issue | Various | Voice / ONE / Convex | **Leave.** Not authorized here. |
| Global LB `openagents-url-map` / forwarding `openagents-https` `136.68.142.56` | DNS: `auth.openagents.com`, `sarah.openagents.com` | Auth hostname live (see probes). Apex `openagents.com` DNS is Phoenix, not this IP. | TLS + host routing | `module.openagents_lb` | **Keep** until auth redirect and Forge backend removal. |
| Serverless NEG `openagents-neg` → backend `openagents-backend` | Auth host + unused apex host rule | Same as production monolith | HTTPS to Cloud Run | Terraform LB module | **Keep** with production monolith. |
| Serverless NEG `openagents-forge-git-neg` → `openagents-forge-git-backend` | `/git*` on old URL map | Zero Cloud Run request logs | HTTPS to Cloud Run | Terraform LB module | **Keep** with `forge-git`. |
| DNS `auth.openagents.com` A `136.68.142.56` | Browsers, Googlebot, this audit | JWKS 200, authorize HTML 200, `/` 302 to Phoenix | Public hostname | Cloudflare DNS, Google LB | **Keep** until explicit redirect contract. |
| DNS `openagents.com` / `fleet.openagents.com` A `8.233.170.185` | Public product | Phoenix HTML 200; `/.well-known/jwks.json`, `/authorize`, `/login` 404 | Public product | Phoenix / Cloudflare DNS-only | **Retain.** |
| GCE `forge-git-nfs` + disk `forge-git-repositories` 100 GB `us-central1-a` | `forge-git` Cloud Run (idle) | Instance RUNNING. Newest snapshot `forge-git-repositor-us-central1-a-20260827081102-n7piqnoq` READY | Git object store | `module.forge_git` | **Keep.** Restore/fsck/ref comparison closed 2026-08-27. |
| Cloud SQL `khala-sync-pg` POSTGRES_17 RUNNABLE | Production monolith, `forge-git`, nostr relay, audio staging | Instance live. Queue/acceptance tables were empty on 2026-08-27. | Production/staging DB, Forge tables | `module.khala_sync_pg` | **Keep.** Do not delete. |
| GCS `openagentsgemini-oa-artifacts` and `-staging` | Historical Forge/app evidence | Names listed; contents not read | Artifacts | `module.oa_artifacts_bucket` / staging | **Keep** until prefix inventory and transfer. |
| GCS `openagentsgemini-terraform-state` | OpenTofu backend | Live state bucket | Infra state | `module.terraform_state_bucket` | **Keep.** Never destroy an active backend from its own state. |
| Secret containers (names only): `openagents-monolith-*`, `openagents-forge-git-policy-authority-token`, `oa-cloud-run-bridge-control-token`, `khala-live-hub-*`, `khala-sync-*` | Retired or mixed callers | Values not read | Credentials | Secret Manager; some Terraform containers | **Keep names.** Delete only after no retained caller; never print values. Queue-worker secrets already removed 2026-08-27. |
| Cloud Run `openagents-com-start-stage1` | Not in the 2026-08-27 table | Still deployed (last deploy 2026-07-26). Not a named drain target in this pass. | Historical start-app shell | Not this issue's authorized delete list | **Recorded leftover.** Do not recreate TypeScript; do not delete without its own traffic proof. |
| Cloud Run `openagents-nostr-relay` | `relay.openagents.com` | Retained 2026-08-27 (websocket Upgrade handshakes) | Market relay | Product | **Retain.** |
| Source trees `apps/openagents.com`, `apps/forum`, `apps/forge-git-service`, `apps/oa-queue-worker`, `apps/acceptance-runner` | None | Deleted on `openagents/main` (Waves 1–4 / #146 / #270) | Source | Git history | **Deleted.** Do not recreate. |

## Deployed service inventory

| Service | Ready revision | Minimum instances | Traffic vs 2026-08-27 | Decision |
| --- | --- | ---: | --- | --- |
| `openagents-monolith` | `openagents-monolith-00426-sic` | 0 | Still Googlebot/scanner/compat. Cron stopped. Auth hostname live. | Keep until auth redirect. |
| `openagents-monolith-staging` | — | — | Zero after scale-down | Deleted 2026-08-28 08:11 UTC. |
| `oa-queue-worker` | — | — | Absent | Deleted 2026-08-27. |
| `oa-queue-worker-staging` | — | — | Absent | Deleted 2026-08-27. |
| `forge-git` | `forge-git-00046-xl5` | 0 | Still zero request logs in 7d | Keep until LB/disk/Terraform. |
| `oa-cloud-run-bridge` | — | — | Zero after 2026-08-27 08:13:10Z | Deleted 2026-08-28 08:11 UTC. |

All remaining listed revisions receive 100% of their service traffic. The
production monolith image is unchanged (`00426-sic`, last ready
2026-08-27 13:27 UTC). `forge-git` last ready 2026-07-26.

## Scheduled work

These Cloud Scheduler jobs remain, both `PAUSED`:

| Job | Schedule | State | Target |
| --- | --- | --- | --- |
| `openagents-monolith-cron` | Every minute | PAUSED | Production monolith `/internal/cron` |
| `openagents-monolith-staging-cron` | Every minute | PAUSED | Deleted staging URL `/internal/cron` |

No retired TypeScript scheduler is `ENABLED`. Do not resume these jobs.

## Durable data and shared resources

| Resource | Risk | Required proof before removal |
| --- | --- | --- |
| Cloud SQL `khala-sync-pg` | Contains the old production and staging databases, queue rows, acceptance receipts, and Forge tables. | Preserve a backup and restore proof. Keep until the forum-import observation period and all queue and Forge checks close. |
| `forge-git-repositories` | 100 GB disk attached to `forge-git-nfs`; historical source calls it the repository-ref authority. | Restore the latest snapshot in isolation, enumerate every repository, run `git fsck --full`, and compare refs with Phoenix Forge. **Done 2026-08-27.** Disk still attached. |
| `openagentsgemini-oa-artifacts` | Contains Forge and old application evidence. | Inventory prefixes and transfer retained evidence before deletion. |
| `openagentsgemini-oa-artifacts-staging` | Contains staging artifacts. | Inventory prefixes and retain only required acceptance or migration evidence. |
| `openagentsgemini-terraform-state` | Owns the active infrastructure state backend. | Retain until every state prefix moves and the destination produces a no-op plan. Never destroy an active backend from its own state. |
| Monolith and Forge secrets | Some secret containers are shared by old and retained systems. | Inventory names and consumers. Delete only containers with no retained caller; never expose values in the receipt. |

The Forge disk has ready snapshots through 2026-08-27, newest
`forge-git-repositor-us-central1-a-20260827081102-n7piqnoq`. A snapshot is not
a restore proof; the restore drill below is.

## Source disposition

| Source root | Disposition | Gate |
| --- | --- | --- |
| `apps/forum` | Deleted (Wave 1 / #146). | Phoenix owns the forum. |
| `apps/acceptance-runner` | Deleted. | No deployed runner. |
| `apps/openagents.com` | Deleted (Wave 1 / #146). | Live auth hostname and SQL remain on this issue. |
| `apps/oa-queue-worker` | Deleted. | Cloud Run services deleted. |
| `apps/forge-git-service` | Deleted (Wave 1 / #146). | Live Cloud Run, NFS, disk, and LB backend remain. |
| `crates/oa-cloud-run-bridge` | Source remains (computer-product / #148). Cloud Run service deleted 2026-08-28. | Terraform module may recreate the service. Do not recreate TypeScript callers. |

## Verification commands

The inventory used bounded, public-safe forms of these commands:

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
gcloud run services list --project openagentsgemini --region us-central1
gcloud scheduler jobs list --project openagentsgemini --location us-central1
gcloud compute instances list --project openagentsgemini
gcloud compute disks list --project openagentsgemini
gcloud compute snapshots list --project openagentsgemini \
  --filter='sourceDisk~forge-git-repositories'
gcloud compute url-maps describe openagents-url-map \
  --project openagentsgemini --global
gcloud compute forwarding-rules list --project openagentsgemini
gcloud logging read '<service-scoped request-log filter>' \
  --project openagentsgemini --freshness=7d --limit=5000
dig +short auth.openagents.com A
dig +short openagents.com A
```

The log review stripped query strings before grouping paths. It did not retain
authorization headers, request bodies, tokens, user identifiers, or JWKS key
coordinates.

### 2026-08-28 receipts (redacted)

```text
observed_at: 2026-08-28T08:04:41Z through 2026-08-28T08:12:16Z
project: openagentsgemini
region: us-central1
principal: oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com

gcloud run services delete oa-cloud-run-bridge \
  --project openagentsgemini --region us-central1 --quiet
# 2026-08-28T08:11:14Z Deleted service [oa-cloud-run-bridge].

gcloud run services delete openagents-monolith-staging \
  --project openagentsgemini --region us-central1 --quiet
# Deleted service [openagents-monolith-staging].

gcloud run services describe oa-cloud-run-bridge ...
# Cannot find service [oa-cloud-run-bridge]
gcloud run services describe openagents-monolith-staging ...
# Cannot find service [openagents-monolith-staging]
gcloud run services describe oa-queue-worker ...
# Cannot find service [oa-queue-worker]

remaining duplicate-backend Cloud Run:
  openagents-monolith  revision openagents-monolith-00426-sic  minScale absent (0)
  forge-git            revision forge-git-00046-xl5            minScale 0

schedulers:
  openagents-monolith-cron          PAUSED  * * * * *
  openagents-monolith-staging-cron  PAUSED  * * * * *

auth.openagents.com probes (User-Agent oa-145-inventory-probe, 60s):
  GET /.well-known/jwks.json  -> 200 application/json  ttfb 0.26s  (JWKS body redacted)
  GET /code/authorize         -> 200 text/html         ttfb 0.14s
  GET /                       -> 302 Location: https://openagents.com

openagents.com (Phoenix, 8.233.170.185):
  GET /.well-known/jwks.json  -> 404
  GET /authorize              -> 404
  GET /login                  -> 404
  GET /                       -> 200 text/html (Phoenix)
```

## Scheduled-task classification

Every task in the monolith per-minute `scheduled()` table (the
`Promise.allSettled` array plus the two pre-array runs in the deleted
`apps/openagents.com/workers/api/src/index.ts`), classified against the
retained product set — Phoenix backend and the Rust CLI:

| Task | Class | Evidence and disposition |
| --- | --- | --- |
| `HydraliskGlmPoolHeartbeat.run` | Retired | Hydralisk GLM pool is the retired own-capacity inference fleet; Phoenix serves `glm-5.3-flash` through the provider route and keeps no D1 heartbeat store. |
| `runKhalaSyncCaptureStalenessProbe` | Retired | Probes the retired Khala capture pipeline; the capture producer is itself dead (its local listener fails against the drained Cloud SQL, and Phoenix serves no hub-append route). |
| `sendPendingReviewReadyArtifactNotifications` | Retired | Sends Resend email for retired artifact-review commerce. `CRM_RESEND_SEND_ENABLED=0` already disables sends. |
| `reconcileTokensServedProjection` | Retired | Detect-only reconcile of the public tokens-served projection whose producers are retired. |
| `FleetBurnStallDetector.tick` | Retired | Watchdog for the retired own-capacity Codex fleet (`FLEET_WATCHDOG_ENABLED=true` only writes alert rows nobody reads). |
| `ServingRateMonitor.tick` | Retired | Monitors serving rates of the retired pool. |
| `ArtanisScheduledRunner.runTick` | Retired | Artanis runtime ticks; Phoenix has no Artanis surface (`KHALA_SYNC_ARTANIS_READS=postgres` names only a legacy read mode). |
| `HostedRuntimeTurnDispatch.tick` | Retired | Drained mobile Khala Code turns; the mobile application is deleted. |
| `ManagedCloudRuntimeTurnDispatch.tick` | Retired | Same consumers as hosted dispatch; no current caller. |
| `SarahAutonomousTick.tick` | Retired | Armed only by `SARAH_AUTONOMOUS_TICK_ENABLED`, which is `false` in production: a clean no-op since before this audit. |
| `PortableSessionCommandDispatch.tick` | Retired | Portable-session commands for the retired mobile and Pylon snapshot flows. |
| `PylonCapacityFunnel.recordSnapshots` | Retired | Capacity-funnel snapshots of the retired own-capacity pool; the retained Pylon CLI does not read them. |
| `PublicActivityTimeline.refreshSnapshot` | Retired | Rebuild-on-cron snapshot of the old site's activity page; Phoenix has no activity-timeline surface. |
| `RelayHealth.probeTick` | Retired | Probed the market relay through a service binding that only existed inside the old Worker. The relay itself is retained (see external consumers); this probe is not. |
| `SelfServeWindowProducer.topUp` | Retired | Tops up claimable training windows for a Tassadar training run that is not armed. |
| `EmailCampaignDispatcher.dispatchDue` | Retired | Old CRM email automation; `CRM_RESEND_SEND_ENABLED=0`. |
| `BusinessFulfillmentLoop.dailyMotion` | Retired | Old business fulfillment loop over the retired commerce tables. |
| `AgentDefinitionScheduler.tick` | Retired | Cron-triggered agent-definition runs served by the old Worker API; Phoenix owns the current agent surface. |
| `AutopilotContinuationPolicy.sweep` | Retired | Continuation sweep hard-coded to skip: `billingAllowsContinuation` returns `continuation.skipped.paid_capacity_retired` unconditionally. |
| `ArtanisResponder.scan` / `.compose`, `ArtanisAdmin.tick`, `ArtanisFleet.tick`, `ArtanisAdmin.closeoutVerifier` | Retired | Artanis responder/ops fleet; no Phoenix or CLI consumer. |
| `TassadarTracePairing.tick` | Retired | Deliberate no-op pairer (`TASSADAR_TRACE_PAIRING` unset; resolver returns `[]` by design). |

No task has a Phoenix or Rust replacement requirement: Phoenix implements its
own projections and the CLI ships its own release checks. The production
scheduler stays paused. The Worker source that owned the table is deleted.

## External-consumer verdicts

| External system | Verdict | Evidence |
| --- | --- | --- |
| `auth.openagents.com` issuer (OpenAuth + jwks) | Safe to retire behind a redirect; **not retired yet** | 2026-08-27 zero-caller window. 2026-08-28 live probes still 200 on JWKS and `/code/authorize`; Phoenix 404s the same paths. Root `/` already 302s to `openagents.com`. Keep the hostname until an explicit redirect contract. |
| `openagents-nostr-relay` (relay.openagents.com) | **Retain — live and load-bearing** | 502 HTTP 426 (Upgrade Required) websocket handshakes in the ~40 hours before 2026-08-27; serves a valid NIP-11 document. Still deployed 2026-08-28. |
| `sarah` (sarah.openagents.com) | Retain for now (voice-stack scope) | DNS still the old LB. Disposition is not this drain. |
| `oa-sarah-nostr-signer` | Zero requests in 7 days (2026-08-27) | Candidate for the voice-stack disposition. |
| `khala-live-hub` (Cloud Run) | Deleted with #149 | Cloud SQL remains here. |
| Local `khala-heartbeat` / `khala-canary` launch agents | Retired (owner notice) | They probe the retired Khala inference surface through the same retired gateway. |
| Local `khala-sync-capture` launch agent | Effectively dead | Its listener cannot connect (Cloud SQL capture drained). |

## Completed zero-caller observation

The bounded no-caller window required by the drain order ran from 14:20 UTC to
16:05 UTC on 2026-08-27 with both monolith schedulers paused and both monolith
services at min-instances 0. Observed traffic: three `GET /` redirects on
`auth.openagents.com` (two browser one-offs, one scanner-class) and zero
requests to the monolith's own routes, zero queue or bridge calls, and zero
compatibility-path successes. This closes the observation gate.

Follow-up through 2026-08-28 06:32 UTC: production monolith traffic is
Googlebot, scanners, SPA static assets, `/docs`, and this audit. No cron, no
bridge, no queue worker.

## Forge restore and ref comparison receipt

A restore drill ran on 2026-08-27 against the newest snapshot
`forge-git-repositor-us-central1-a-20260827081102-n7piqnoq` (2026-08-27
01:11 UTC, 100 GB):

- Created a scratch `e2-small` instance and a 100 GB disk from the
  snapshot in `us-central1-a`, mounted it read-only-equivalent (no service
  touched it), and inspected the tree.
- Layout: one tenant directory `tenant.openagents` containing nine bare
  repositories — `omega.git` plus eight `repo.openagents.forge02-live-*.git`
  acceptance seeds.
- `git fsck --full` passed on all nine (exit 0; the only diagnostics were
  dangling objects on `omega.git` and two unborn-branch notices on empty
  seed repositories).
- Ref comparison: `omega.git` carries `refs/heads/main` =
  `refs/heads/forge/omega-journey` = `585af0d8c0` and tag
  `forge-omega-import-2026-07-26` -> `ae32ebd9`. The import-point commit is
  an ancestor of the live Omega repository's history, and the forge tip is
  the import point plus one docs-only commit
  (`forge-receipts/2026-07-26-owned-forge-journey.md`, 5 insertions). The
  eight `forge02-live` seeds hold only the July FORGE-02 acceptance
  commits. Nothing on the disk is unique, load-bearing product data; the
  one unique docs commit exists in the restored snapshot and is recorded
  here.
- The scratch instance and disk were deleted after the drill. The
  snapshot itself is retained until Terraform transfer closes issue #145.

This closes the restore, integrity, and ref-comparison gate. The live NFS
host and 100 GB disk were still RUNNING/READY on 2026-08-28.

## Remaining evidence

Resolved by this document: scheduled-task classification, the bounded
zero-caller observation, the Forge disk restore, integrity, and ref
comparison, queue-worker deletion, historical cloud-run bridge Cloud Run
deletion, and staging monolith deletion.

Issue #145 stays open. Retired production services still reachable or
pending transfer:

1. **`auth.openagents.com` DNS + old load balancer** — keep until an
   explicit `auth.openagents.com` → `openagents.com` redirect contract.
   Phoenix still 404s JWKS, `/authorize`, and `/login`.
2. **Cloud Run `openagents-monolith`** — keep while the auth hostname
   points at `openagents-backend`.
3. **Cloud Run `forge-git`**, GCE `forge-git-nfs`, disk
   `forge-git-repositories`, LB `/git*` backend — keep until Terraform
   transfer. Restore drill is done.
4. **Cloud SQL `khala-sync-pg`** — keep.
5. **Paused schedulers** `openagents-monolith-cron` and
   `openagents-monolith-staging-cron` — delete with Terraform/scheduler
   cleanup; do not enable.
6. **Terraform `infra/prod`** — still declares the deleted
   `oa-cloud-run-bridge` shell, the production monolith, Forge, LB, SQL,
   and artifact buckets. OpenTofu apply blocked on Google `invalid_rapt`.
7. **GCS artifact buckets and mixed secret containers** — prefix inventory
   and caller unbind; no values in this receipt.
8. **Leftover Cloud Run `openagents-com-start-stage1`** — not drained here;
   needs its own zero-traffic proof before delete.

Close #145 only when no retired duplicate-backend service remains reachable
or scheduled and the auth hostname is redirected or cut over.

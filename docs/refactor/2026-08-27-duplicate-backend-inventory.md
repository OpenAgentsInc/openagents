# Duplicate backend inventory and drain plan

- Date: 2026-08-27
- Issue: `OpenAgentsInc/openagents#145`
- Repository baseline: `3b252d82ce3373614cdfeaf9d907ff4c459fcb84`
- Google Cloud project: `openagentsgemini`
- Primary region: `us-central1`
- Observation window: 2026-08-20 through 2026-08-27, with a focused
  24-hour request-log sample ending at 2026-08-27 06:01 UTC
- Safety boundary: this inventory reads resource metadata and redacted request
  paths. It does not read secret values. The drain uses reversible scheduler
  pauses before any service or data deletion.

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

The production authentication compatibility gate remains open. At 08:12 UTC,
`auth.openagents.com` returned `200` for `/.well-known/jwks.json`, `400` for an
incomplete `/authorize` request, and `200` for `/code/authorize`. The same
paths on Phoenix `openagents.com` returned `404`. Do not delete the production
monolith or old load balancer until Phoenix serves or explicitly retires those
contracts.

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

## Decision

The Phoenix application has replaced the TypeScript application as the web,
API, forum, authentication, and Forge implementation. Phoenix does not call
the retired monolith, queue worker, Forge service, or cloud-run bridge.

The old stack is not ready for immediate deletion. It still has active
schedulers, deployed services, an authentication hostname, a Cloud SQL
instance, artifact buckets, and a 100 GB Forge repository disk. Most observed
monolith traffic comes from its own scheduler or internet scans, but recent
successful requests to old documentation and authorization routes mean that
the drain must preserve compatibility deliberately.

Apply this order:

1. Move the remaining `auth.openagents.com` compatibility paths to Phoenix or
   retire them with an explicit redirect contract.
2. Classify each old per-minute scheduled task. Move any retained job to
   Phoenix or a Rust service, then pause both monolith schedulers.
3. Confirm that the monolith no longer produces queue jobs, then inspect and
   drain `oa_infra_jobs` before stopping both queue workers.
4. Set the monolith and queue-worker minimum instance counts to zero and
   observe a bounded no-caller window.
5. Stop the historical cloud-run bridge after monolith readiness calls stop.
6. Preserve and restore-test the Forge disk and repository set before removing
   the old load balancer, Forge service, SQL tables, or artifact evidence.
7. Transfer or destroy resources through their owning Terraform or deployment
   path. Do not remove configuration as a substitute for decommissioning.

## Product boundary

The cleanup keeps product intent while removing stale implementations:

- Rust is the default language for the CLI, local runtimes, and standalone
  services.
- Phoenix and Elixir remain the web and backend authority.
- A backend Rust component must have an explicit service, port, or NIF
  boundary.
- TypeScript is transitional. A retained TypeScript path must name a current
  consumer, owner, replacement boundary, and retirement plan.
- Coder remains the front door to the wider CLI capability set. Deleting an
  old application does not delete its intended CLI namespace.

## Current routing

| Host or map | Current target | Disposition |
| --- | --- | --- |
| `openagents.com` | `8.233.170.185`, Phoenix load balancer | Retain. |
| `fleet.openagents.com` | `8.233.170.185`, Phoenix load balancer | Retain. |
| `auth.openagents.com` | `136.68.142.56`, old load balancer | Migrate or retire before deleting the monolith. |
| `sarah-urlmap` | `sarah-backend` | Retain. |
| `openagents-url-map` | `openagents-backend`; `/git*` routes to `openagents-forge-git-backend` | Remove only after authentication and Forge migration checks pass. |

Phoenix owns forum routes, Git smart HTTP, status, inference, and the current
`/api/v1` surface. Its production environment list contains none of the old
service URLs. Its Forge URL is loopback because Git executes inside the
Phoenix deployment boundary.

## Deployed service inventory

| Service | Ready revision | Minimum instances | 24-hour request-log summary | Decision |
| --- | --- | ---: | --- | --- |
| `openagents-monolith` | `openagents-monolith-00412-697` | 1 | 3,628 entries; 1,281 were `/internal/cron`. Remaining traffic was mainly scans, static files, and compatibility pages. | Drain after authentication and scheduled-task migration. |
| `openagents-monolith-staging` | `openagents-monolith-staging-00159-6kw` | 1 | 1,442 entries; 1,439 were `/internal/cron`, and three were health checks. | Pause its scheduler first, then drain. |
| `oa-queue-worker` | Deleted at 08:27 UTC | 0 | The production queue and acceptance tables contained zero rows before and after the scheduler pause. | Deleted. |
| `oa-queue-worker-staging` | Deleted at 08:27 UTC | 0 | The staging queue and acceptance tables contained zero rows before and after the scheduler pause. | Deleted. |
| `forge-git` | `forge-git-00046-xl5` | 0 | No request entries in the 24-hour sample. | Preserve until repository-disk restore and ref comparison pass. |
| `oa-cloud-run-bridge` | `oa-cloud-run-bridge-00011-ncx` | 0 | 1,439 successful `/v1/cloud-vm/readiness` calls, aligned with staging cron frequency. | Drain after the monolith stops calling it. |

All listed revisions receive 100% of their service traffic. The deployed
monolith, queue-worker, and Forge images were last published between 2026-07-06
and 2026-08-05. Image age supports retirement but does not prove zero use.

The production monolith also served a small number of successful
`/.well-known/jwks.json`, `/docs`, `/changelog`, `/login`, `/authorize`, and
`/code/authorize` requests during the seven-day sample. Preserve or redirect
those compatibility paths before the old hostname or service disappears.

## Scheduled work

These Cloud Scheduler jobs remain enabled:

| Job | Schedule | Target |
| --- | --- | --- |
| `openagents-monolith-cron` | Every minute | Production monolith `/internal/cron` |
| `openagents-monolith-staging-cron` | Every minute | Staging monolith `/internal/cron` |

The monolith cron fans out into more than 20 historical tasks. The list
includes Khala capture checks, inference projections, fleet monitors, Artanis,
hosted and managed runtime dispatch, Sarah autonomous ticks, portable-session
commands, Pylon snapshots, public activity projections, relay health, email
campaigns, business fulfillment, agent scheduling, and continuation logic.
Do not pause the production scheduler until each task is marked remove or has
a Phoenix or Rust replacement. The staging scheduler can be paused as the
first rehearsal after the same classification identifies no staging consumer.

## Durable data and shared resources

| Resource | Risk | Required proof before removal |
| --- | --- | --- |
| Cloud SQL `khala-sync-pg` | Contains the old production and staging databases, queue rows, acceptance receipts, and Forge tables. | Preserve a backup and restore proof. Keep until the forum-import observation period and all queue and Forge checks close. |
| `forge-git-repositories` | 100 GB disk attached to `forge-git-nfs`; historical source calls it the repository-ref authority. | Restore the latest snapshot in isolation, enumerate every repository, run `git fsck --full`, and compare refs with Phoenix Forge. |
| `openagentsgemini-oa-artifacts` | Contains Forge and old application evidence. | Inventory prefixes and transfer retained evidence before deletion. |
| `openagentsgemini-oa-artifacts-staging` | Contains staging artifacts. | Inventory prefixes and retain only required acceptance or migration evidence. |
| `openagentsgemini-terraform-state` | Owns the active infrastructure state backend. | Retain until every state prefix moves and the destination produces a no-op plan. Never destroy an active backend from its own state. |
| Monolith and Forge secrets | Some secret containers are shared by old and retained systems. | Inventory names and consumers. Delete only containers with no retained caller; never expose values in the receipt. |

The Forge disk has ready snapshots through 2026-08-26, including
`forge-git-repositor-us-central1-a-20260826081102-np29pte8`. A snapshot is not
a restore proof.

## Source disposition

| Source root | Disposition | Gate |
| --- | --- | --- |
| `apps/forum` | Delete. | Phoenix owns the forum; the TypeScript package contains only a mount contract and no persistence. |
| `apps/acceptance-runner` | Deleted. | No deployed runner or packaged image existed, and both databases contained zero acceptance jobs and verdicts. |
| `apps/openagents.com` | Delete after the controlled drain. | Authentication compatibility, cron task disposition, SQL, buckets, and load-balancer ownership must move first. |
| `apps/oa-queue-worker` | Deleted. | Both Cloud Run services were deleted after two zero-row SQL observations and a zero-caller observation. |
| `apps/forge-git-service` | Delete after Forge migration proof. | Preserve and verify the repository disk, SQL state, outbox, purgatory, and artifact evidence. |
| `crates/oa-cloud-run-bridge` | Delete after caller separation. | The historical monolith still calls it, and the same image also supports managed-sandbox services handled by the managed-computer issue. |

## Verification commands

The inventory used bounded, public-safe forms of these commands:

```sh
gcloud run services list --project openagentsgemini --region us-central1
gcloud scheduler jobs list --project openagentsgemini --location us-central1
gcloud compute instances list --project openagentsgemini
gcloud compute disks list --project openagentsgemini
gcloud compute snapshots list --project openagentsgemini \
  --filter='sourceDisk~forge-git-repositories'
gcloud compute url-maps describe openagents-url-map \
  --project openagentsgemini --global
gcloud logging read '<service-scoped request-log filter>' \
  --project openagentsgemini --freshness=24h --limit=10000
```

The log review stripped query strings before grouping paths. It did not retain
authorization headers, request bodies, tokens, or user identifiers.

## Remaining evidence

Issue #145 remains open until these items land:

- authentication hostname and compatibility-route disposition;
- scheduled-task classification and migration;
- a bounded zero-caller observation after schedulers and producers stop;
- Forge disk restore, repository integrity, and ref comparison;
- Terraform transfer, destroy, or retained-resource receipts.

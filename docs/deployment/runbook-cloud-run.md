# Runbook: production relay on Cloud Run

`wss://relay.openagents.com` is the relay binary from this repository running
on Cloud Run. This runbook is how it's built, deployed, checked, and rolled
back. The [Debian VPS runbook](runbook-debian-vps.md) covers self-hosting the
same binary on one box.

## What's running

| | |
| --- | --- |
| Project and region | `openagentsgemini`, `us-central1` |
| Service | `openagents-nostr-relay`, one instance (`min=max=1`), concurrency 250, request timeout 3600 s, session affinity, startup CPU boost |
| Domain | `relay.openagents.com` is a Cloud Run domain mapping to the service |
| Image | `us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/nostr-relay:<tag>`, built from the root `Dockerfile` |
| Tag convention | the 10-character commit hash the image was built from (`git rev-parse --short=10 HEAD`) |
| Database | Cloud SQL `openagentsgemini:us-central1:khala-sync-pg`, database `nostr_relay_v2`, user `khala_app`, over the `/cloudsql/...` socket |
| Secrets | `PGPASSWORD` from `openagents-monolith-pgpassword`, `NOSTR_RELAY_SECRET_KEY` from `openagents-nostr-relay-private-key` |
| Runtime service account | `157437760789-compute@developer.gserviceaccount.com` |

The rest of the environment (`NOSTR_RELAY_URL=wss://relay.openagents.com`,
`NOSTR_RELAY_TRUST_PROXY=true`, the rate limits, name, and description) lives
on the service. A deploy that changes only the image keeps all of it. Capture
it before any change:

```sh
gcloud run services describe openagents-nostr-relay \
  --region us-central1 --project openagentsgemini --format=export > relay-before.yaml
```

History:

| Date | Revision | Image tag | Notes |
| --- | --- | --- | --- |
| 2026-09-19 | `openagents-nostr-relay-00023-kax` | `9b5bb212f1` | First image from this repository; migrations 1-8 |
| 2026-09-26 | `openagents-nostr-relay-00025-jes` | `c1bac69fdd` | Applied migrations 9 (`nip29_groups`) and 10 (`private_protocol_search`); adds NIP-67 and NIP-77 to NIP-11 |
| 2026-09-26 | `openagents-nostr-relay-00027-toh` | `965fa00671` | No migrations. NIP-11 `max_limit`/`default_limit` now advertise the real per-`REQ` cap (127 with defaults). Deploy and traffic shift ran as `chris@`; verified by a full `kb sync` (104 entries) on `relay.openagents.com` |


## Accounts

Use the automation service account for builds, reads, and logs:
`CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`
(see the workspace `AGENTS.md`). On 2026-09-26 it was still refused
`iam.serviceaccounts.actAs` on the runtime service account for
`gcloud run deploy` and `update-traffic`, even with a resource-level
`roles/iam.serviceAccountUser` binding, so those two steps ran as the owner's
`chris@openagents.com` login.

## 1. Check migrations before you build

The binary applies pending migrations at startup, in one transaction, before
it listens. So the new revision migrates the shared database as soon as it
starts, even with no traffic. Before a deploy:

- List the migrations added since the running image:
  `git diff --stat <running-tag> HEAD -- migrations/`.
- Each new migration must keep the running binary working: add columns with
  defaults, add tables, rebuild derived columns. No dropped data, no renamed
  columns the old binary reads.
- Check the size of what it rewrites. A migration holds its locks for the
  whole transaction.
- Check new configuration in `crates/nostr-relay/src/gateway/config.rs`. A new
  required variable must be set on the service first.

An older binary refuses to start against a ledger with a version it doesn't
know (`database has unknown version`). The running instance keeps serving, but
a revision built before the migration can't cold-start again. See
[Roll back](#5-roll-back).

## 2. Build the image

From a clean checkout of `main` (a worktree is fine):

```sh
TAG=$(git rev-parse --short=10 HEAD)
gcloud builds submit --project openagentsgemini \
  --config deploy/nostr-relay.cloudbuild.yaml \
  --substitutions _TAG=$TAG .
```

[`deploy/nostr-relay.cloudbuild.yaml`](../../deploy/nostr-relay.cloudbuild.yaml)
builds with the `oa-cloud-run-source-builder` service account and Cloud
Logging only; the org policy refuses a build without an explicit account.
The root `.gcloudignore` uploads only what the `Dockerfile` copies, about
40 MiB instead of the whole tree. The build takes about two minutes.

## 3. Deploy with no traffic, then smoke it

```sh
gcloud run deploy openagents-nostr-relay \
  --image us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/nostr-relay:$TAG \
  --no-traffic --tag next --region us-central1 --project openagentsgemini
```

The tagged URL is `https://next---openagents-nostr-relay-ezxz4mgdsq-uc.a.run.app`.
Check it:

```sh
N=next---openagents-nostr-relay-ezxz4mgdsq-uc.a.run.app
curl -s https://$N/health                                   # {"status":"ok"}
curl -s -H 'Accept: application/nostr+json' https://$N      # NIP-11, same pubkey as prod
nak req -k 3190 -l 5 wss://$N                               # a REQ returns stored events
SK=$(nak key generate)                                      # a throwaway key
nak event -k 1 -c "relay deploy smoke" --sec $SK wss://$N   # the relay accepts a write
nak req -k 1 -a $(nak key public $SK) wss://relay.openagents.com  # the old revision reads it
```

Confirm the migration ledger through the Cloud SQL proxy:
`SELECT version, name, applied_at FROM schema_migrations ORDER BY version`.

## 4. Shift traffic

```sh
gcloud run services update-traffic openagents-nostr-relay \
  --to-revisions <new-revision>=100 --region us-central1 --project openagentsgemini
```

Then run the same checks against `relay.openagents.com`, and check the new
revision's error logs:

```sh
gcloud logging read 'resource.type="cloud_run_revision" AND
  resource.labels.revision_name="<new-revision>" AND severity>=ERROR' \
  --project openagentsgemini --limit 20
```

## 5. Roll back

To a revision that knows every applied migration, shift traffic back:

```sh
gcloud run services update-traffic openagents-nostr-relay \
  --to-revisions <previous-revision>=100 --region us-central1 --project openagentsgemini
```

Revisions from before the last migration can't start against the current
ledger. As of 2026-09-26 that means `openagents-nostr-relay-00023-kax`
(`9b5bb212f1`, migrations 1-8) won't cold-start, so the safe rollback is
forward: fix `main`, build, deploy. If an older binary must run anyway,
migrations 9 and 10 left a schema it can use (new columns with defaults, a
new table, a rebuilt search column), so deleting ledger rows 9 and 10 lets it
start. That edits the production ledger: do it only with the owner's
sign-off, and a later deploy then re-runs those migrations and fails on the
existing columns, so it needs the same care going forward.

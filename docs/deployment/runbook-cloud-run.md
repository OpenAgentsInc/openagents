# Runbook: production relay on Cloud Run

This runbook records the repository relay's Cloud Run deployment and the
operator procedure for building, checking, deploying, and rolling it back. The
last recorded revision is dated below; this document is not a live service-health
or current-traffic check. The [Debian VPS runbook](runbook-debian-vps.md) covers self-hosting the
same binary on one box.

## Recorded deployment

| | |
| --- | --- |
| Project and region | `openagentsgemini`, `us-central1` |
| Service | `openagents-nostr-relay`, one instance (`min=max=1`), concurrency 250, request timeout 3600 s, session affinity, startup CPU boost |
| Domain | `relay.openagents.com` is a Cloud Run domain mapping to the service |
| Image | `us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/nostr-relay:<tag>`, built from the root `Dockerfile` |
| Tag convention | the 10-character commit hash the image was built from (`git rev-parse --short=10 HEAD`) |
| Database | Cloud SQL `openagentsgemini:us-central1:khala-sync-pg`, database `nostr_relay_v2`, user `khala_app`, over the `/cloudsql/...` socket |
| Secrets | `PGPASSWORD` from `openagents-monolith-pgpassword`, `NOSTR_RELAY_SECRET_KEY` from `openagents-nostr-relay-private-key` |
| Runtime service account | `oa-nostr-relay@openagentsgemini.iam.gserviceaccount.com` since revision `00036-toy` (2026-10-02); earlier revisions ran as `157437760789-compute@developer.gserviceaccount.com`. See [Accounts](#accounts) |
| Media | Bucket `gs://openagentsgemini-relay-media` (public read; `oa-nostr-relay` is `roles/storage.objectAdmin` on it) mounted as volume `media` (Cloud Storage FUSE) at `/var/lib/nostr-relay/media`, with `NOSTR_RELAY_MEDIA_ROOT=/var/lib/nostr-relay/media`, `NOSTR_RELAY_MEDIA_CLOUD_BASE_URL=https://storage.googleapis.com/openagentsgemini-relay-media`, `NOSTR_RELAY_MEDIA_MAX_BLOB_BYTES=16777216` (the CLI's `MAX_BLOB`; Cloud Run's request limit is 32 MiB). Uploads are `PUT /upload` with a NIP-98 event signed by the uploader's key (`docs/protocol/media.md`); reads redirect to the bucket |

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
| 2026-09-27 | `openagents-nostr-relay-00029-nar` | `24fc83269a` | Applied migration 11 (`push_executor`, new tables only; NIP-PL delivery stays off without `NOSTR_RELAY_PUSH_SECRET`). NIP-CAP heads with `requires: ["oa-x402-v1"]` are now accepted. Build ran as the automation account; deploy and traffic shift ran as `chris@` after a `--no-launch-browser` login. Verified by `openagents x402 advertise --binding mcp:1` publishing to `next` and `openagents cap describe` reading it from `relay.openagents.com` |
| 2026-09-29 | `openagents-nostr-relay-00031-mel` | `358975bdbd` | No migrations. Pipelined admission statements, in-memory fan-out after commit, `TCP_NODELAY` (`f20742ebf7`). Build ran as the automation account; its deploy was again refused `actAs` although it holds `roles/iam.serviceAccountUser` on the runtime account, so deploy and traffic shift ran as `chris@`. Verified by the step 3 checks and `chat-load-bench` against `relay.openagents.com`: median request `OK` 78 to 71 ms, request to reply 221 to 178 ms, chat open done 424 to 331 ms |
| 2026-09-29 | `openagents-nostr-relay-00034-pit` | `17cefc1703` | No migrations. A lost Postgres notification listener no longer stops the relay: it reconnects with backoff and catches up by sequence; a cancelled history read no longer fails the next statement on its worker, which stopped the relay four times on 2026-09-29 (#9947). Rollback revision: `00031-mel`. Build ran as the automation account; the deploy, the traffic shift, and removing the crash-looping `candidate` tag and revision 00023-kax ran as `chris@` (the account was refused `actAs` again, including for `update-traffic --remove-tags`). Verified by the step 3 checks on `next` and production, `live_basic_coder_streams_a_reply` (first words 0.66 s, answer 5.2 s), an XP referee pass reading the relay after the shift, and 11 minutes of logs with no warning, error, or restart (70 WebSocket sessions, no 5xx) |
| 2026-10-02 | `openagents-nostr-relay-00036-toy` | `17cefc1703` | No migrations; same image. Turns Blossom media on (the volume and variables above) and runs as the new `oa-nostr-relay` account, so `plugin publish` and `plugin test publish` upload without cloud credentials (#10181). Its first start failed (`boss::NOT_AUTHORIZED` on `khala-sync-pg`: the account had no project `roles/cloudsql.client`); after the owner granted `cloudsql.client`, `logging.logWriter`, and `monitoring.metricWriter`, the revision became ready. The whole deploy ran as the automation account (no `actAs` refusal on `oa-nostr-relay`). Verified on `next` by the step 3 checks and `PUT /upload` answering `401` (was `405`), then on production after the shift by a real `openagents plugin publish` with no `--blossom` (throwaway plugin `blob-upload-test-10181`, release `a9571ee442`: four blobs uploaded, `plugin install` fetched and verified each through a `307` to the bucket; then revoked and its listing deleted, after which install finds no such plugin). Rollback revision: `00034-pit` (no media) |

## Media checks

`curl -s -o /dev/null -w '%{http_code}' -X PUT -H 'content-length: 0' https://relay.openagents.com/upload`
answers `401` (no authorization), not `405`; a `plugin publish` with no
`--blossom` uploads its files, and `HEAD /<sha256>` on the relay answers
`307` to `https://storage.googleapis.com/openagentsgemini-relay-media/...`.
A revision that should not take uploads leaves out `NOSTR_RELAY_MEDIA_ROOT`.

## Accounts

Use the automation service account for builds, reads, and logs:
`CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`
(a recorded operator-local configuration path, not a repository prerequisite).
On 2026-09-26 it was still refused
`iam.serviceaccounts.actAs` on the runtime service account for
`gcloud run deploy` and `update-traffic`, even with a resource-level
`roles/iam.serviceAccountUser` binding, so those two steps ran as the owner's
`chris@openagents.com` login. The same refusal recurred on 2026-09-29. The account
cannot read the project IAM policy, so it cannot grant itself more; plan on
`chris@` for these two steps until an owner fixes the binding.

Since 2026-10-02 the service runs as a dedicated account,
`oa-nostr-relay@openagentsgemini.iam.gserviceaccount.com`, on which the
automation account holds a working `roles/iam.serviceAccountUser`, so the
automation account now deploys and shifts traffic itself. `oa-nostr-relay`
holds project `roles/cloudsql.client`, `roles/logging.logWriter`, and
`roles/monitoring.metricWriter`, reads `openagents-monolith-pgpassword` and
`openagents-nostr-relay-private-key`, and is `roles/storage.objectAdmin` on
the media bucket.

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
- The pose lane (`NOSTR_RELAY_RATE_POSE_PER_SEC_PUBKEY`, default 12, and
  `NOSTR_RELAY_RATE_POSE_PER_SEC_IP`, default 400) counts NIP-MV frames and
  gestures by the second, apart from the per-minute event budget. Verse
  clients publish 5 frames a second while moving; a relay without the lane
  throttles them to jerky motion. Set the IP lane with the largest crowd
  behind one address in mind (20 players at 5 Hz is 100 a second).

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
NIP-42 `AUTH` is bound to the service's `NOSTR_RELAY_URL`
(`wss://relay.openagents.com`), so an authenticated exchange such as
`chat-load-bench` cannot pass on the tagged URL; run it against
`relay.openagents.com` after step 4. Check it:

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
ledger. As of 2026-09-26 that meant `openagents-nostr-relay-00023-kax`
(`9b5bb212f1`, migrations 1-8) wouldn't cold-start, so the safe rollback is
forward: fix `main`, build, deploy. Don't leave such a revision tagged: a
tagged revision keeps its minimum instance, and 00023-kax crash-looped
under its `candidate` tag (`schema migration drift: database has unknown
version 9`) from 2026-09-26 until 2026-09-29, when the tag was removed
(`update-traffic --remove-tags candidate`) and the revision deleted, as
`chris@`; the serving revision, 00031-mel, was not touched (#9947). Do not delete migration-ledger rows to force an older binary to start: that
separates the recorded schema from the installed schema and causes subsequent
migration replay to fail. If a code rollback is necessary, build the corrected
code with the current migration set and verify compatibility, or use a separately
reviewed backup-restore procedure with its data-loss and downtime consequences.
An application traffic rollback does not roll back a database migration.

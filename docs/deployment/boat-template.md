# The daily Boat template `oa-coder-main-<date>`

Coder runs on Boat (issue
[#10219](https://github.com/OpenAgentsInc/openagents/issues/10219), plan B5 in
[the Boat SDK plan](../cloud/2026-10-02-boat-sdk-plan.md)) start from a named
snapshot that already holds the repository, the toolchains and a compiled
`main`. This page is how that snapshot is built, scheduled, checked and
repaired.

## What builds it

| Part | Where |
| --- | --- |
| Binary | `crates/boat-template` (`boat-template build`, `probe NAME`, `list`, `prune`), on `crates/boat` |
| Host setup | `scripts/cloud/coder-host-setup.sh --warm`, fetched from `origin/main` on every build. The GCE image `oa-coder-host` (#10224) runs the same script |
| Image | `us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/boat-template:<commit>`, built by `deploy/boat-template/cloudbuild.yaml` from a GitHub commit (no local upload) |
| Job | Cloud Run job `oa-boat-template` (us-central1, 1 vCPU, 512 MiB, task timeout 6 h, no retries), runtime account `oa-mvp-automation`, args `build` |
| Key | Secret Manager `boat-api-key` (project `openagentsgemini`), mounted as `BOAT_API_KEY`. Created from `~/work/.secrets/boat.env`; never printed |
| Schedule | Cloud Scheduler `oa-boat-template-daily`, `0 8 * * *` UTC, POSTs `jobs/oa-boat-template:run` with the automation account's OAuth token |

`boat-template build`:

1. Checks `GET /limits` for a start left today, then creates a `large`
   sandbox (`noEnv: true`, TTL 4 h as a backstop).
2. Uploads and runs a driver that fetches `coder-host-setup.sh` from
   `origin/main` and runs it with `--warm` as the sandbox's `user`. The
   driver also writes `~/.boxignore` with `.cache/sccache/` (the disk sccache
   duplicates the warm target) and links `~/openagents/target` to the warm
   slot. It never excludes `target/` or the slot.
3. Stops the sandbox, saves the named snapshot `oa-coder-main-YYYYMMDD` (UTC),
   and waits for it to be `ready`.
4. Keeps the newest 3 `oa-coder-main-*` and deletes the rest. Boat allows 10
   named snapshots per account; when the account is full it drops only its
   own oldest templates, never another name.
5. Deletes the build sandbox. Boat reports that deletion as `blocked`
   because the named snapshot still reads the sandbox's snapshot chain; the
   sandbox itself is gone, which is the expected end.

On a failure it stops the build sandbox and keeps it for inspection (a
stopped sandbox costs nothing); its log is `~/.oa-coder-host-setup.log`.

## Starting a run from the template

```text
POST /sandboxes {"from": "oa-coder-main-YYYYMMDD", "type": "large", "noEnv": true, "ttlSeconds": ...}
```

Then, **before anything else, synchronously** (not `detached`):

```sh
sudo -n find "$HOME" -xdev -user root -exec chown -h "$(id -u):$(id -g)" {} +
```

A sandbox created from a template comes back with about 30 directories owned
by root (`~/.cargo`, `~/.openagents` and directories in the warm slot,
`~/.ascii/processes`; never files). Until they are repaired, Boat's own
detached commands fail with `EACCES ... ~/.ascii/processes/...log` (HTTP 400
`sandbox_direct_failed`) and cargo cannot write the slot. The repair took
19.7 s on a fresh fork. `boat-template probe` does it first.

Build in the warm slot that the manifest names, as Coder does:

```sh
slot=$(jq -r .warm_target.slot ~/.openagents/coder-host.json)
CARGO_TARGET_DIR="$slot" cargo build -p openagents-cli
```

## Operating it

Use the automation account:
`export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`.

```sh
# Build now (same as the schedule)
gcloud run jobs execute oa-boat-template --region us-central1 --project openagentsgemini
# Measure a template
gcloud run jobs execute oa-boat-template --region us-central1 --project openagentsgemini \
  --args=probe,oa-coder-main-YYYYMMDD
# List templates
gcloud run jobs execute oa-boat-template --region us-central1 --project openagentsgemini --args=list
# Logs of one execution
gcloud logging read 'resource.type="cloud_run_job" AND labels."run.googleapis.com/execution_name"="EXECUTION"' \
  --project openagentsgemini --order asc --format='value(textPayload)'
# Run the schedule once by hand
gcloud scheduler jobs run oa-boat-template-daily --location us-central1 --project openagentsgemini
```

Redeploy after changing `crates/boat-template` (pushed to `main` first):

```sh
C=$(git rev-parse HEAD)
gcloud builds submit --no-source --config deploy/boat-template/cloudbuild.yaml \
  --substitutions _COMMIT=$C --project openagentsgemini --region us-central1 \
  --service-account projects/openagentsgemini/serviceAccounts/oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com \
  --gcs-log-dir gs://openagentsgemini-oa-mvp-cloud-build-logs/boat-template
gcloud run jobs update oa-boat-template --region us-central1 --project openagentsgemini \
  --image us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/boat-template:$C
```

A change to the host setup needs no redeploy: the job fetches
`scripts/cloud/coder-host-setup.sh` from `origin/main` each day.

## Named-snapshot slots

On 2026-10-02 the account held 10 named snapshots, all `gym-*` from
2026-09-05 (the Gym-on-Boat lane, which moved to owned compute on 09-06). To
leave room for three templates, the three oldest were deleted:
`gym-regex-log-69671fba-90101b2e8153`,
`gym-openssl-selfsigned-cert-69671fba-4c948a4e630a` and
`gym-git-leak-recovery-69671fba-62160e522a42`. Seven `gym-*` remain. If a
build fails with "too few are ours to drop", remove a stale name with
`DELETE /named-snapshots/{name}`.

## Cost

- Build sandbox: `large` at $0.072/h for about 20 minutes, about $0.025 a day.
- Cloud Run job: 1 vCPU and 512 MiB for about 20 minutes, a few cents a day.
- A fork: $0.072/h while it runs. The 2026-10-02 probe fork ran 7,255 s
  (mostly the slow first build below), $0.073.
- Boat's docs list no separate charge for snapshot storage.

## Measurements

See [the plan, §7](../cloud/2026-10-02-boat-sdk-plan.md#7-b5-the-template-measured).

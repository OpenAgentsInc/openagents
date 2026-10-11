# The daily Boat template `oa-coder-main-<date>`

Coder runs on Boat (issue
[#10219](https://github.com/OpenAgentsInc/openagents/issues/10219), plan B5 in
[the Boat SDK plan](../cloud/2026-10-02-boat-sdk-plan.md)) start from a named
snapshot that already holds the repository, the toolchains and a compiled
`main`. This page is how that snapshot is built, scheduled, checked and
repaired.

> **Since 2026-10-10 (#11256) the template builds on our own backend**,
> [`oa-boat`](../cloud/oa-boat.md), not hosted boat.dev: the job's
> `BOAT_API_KEY` is Secret Manager `oa-boat-api-key` and `BOAT_API_BASE` is
> `https://oa-boat-157437760789.us-central1.run.app/api/v1`. The build
> sandbox is a GCE `n2d-standard-8` from the newest `oa-coder-host` image,
> and `oa-coder-main-<date>` is a GCE image of its boot disk (label
> `openagents-managed=oa-boat-template`). The Boat-specific notes below
> (lazy restore, `.boxignore`, named-snapshot slots) describe the hosted
> history; on GCE a disk made from an image needs no restore wait.

## What builds it

| Part | Where |
| --- | --- |
| Binary | `crates/boat-template` (`boat-template build`, `probe NAME`, `list`, `prune`), on `crates/boat` |
| Host setup | `scripts/cloud/coder-host-setup.sh --warm`, fetched from `origin/main` on every build. The GCE image `oa-coder-host` (#10224) runs the same script |
| Image | `us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/boat-template:<commit>`, built by `deploy/boat-template/cloudbuild.yaml` from a GitHub commit (no local upload) |
| Job | Cloud Run job `oa-boat-template` (us-central1, 1 vCPU, 512 MiB, task timeout 6 h, no retries), runtime account `oa-mvp-automation`, args `build` |
| Key | Secret Manager `oa-boat-api-key` (project `openagentsgemini`), mounted as `BOAT_API_KEY`, with `BOAT_API_BASE` set to our service; never printed. (Was `boat-api-key`, hosted Boat, until #11256) |
| Schedule | Cloud Scheduler `oa-boat-template-daily`, `0 8 * * *` UTC, POSTs `jobs/oa-boat-template:run` with the automation account's OAuth token |

`boat-template build`:

1. Checks `GET /limits` for a start left today, then creates a `large`
   sandbox (`noEnv: true`, TTL 4 h as a backstop).
2. Uploads and runs a driver that fetches `coder-host-setup.sh` from
   `origin/main` and runs it with `--warm --keep-binaries "openagents
   microcoder"` as the sandbox's `user`. The driver also writes
   `~/.boxignore` with `.cache/sccache/` (the disk sccache duplicates the
   warm target), links `~/openagents/target` to the warm slot, and cuts the
   mtimes Cargo compares to whole seconds (below). It never excludes
   `target/` or the slot.
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

Then run [`scripts/cloud/boat-fork-ready.sh`](../../scripts/cloud/boat-fork-ready.sh)
before anything else (#10251, #10274). `boat-template probe` and
`chat work --on boat` both do:

```sh
bash boat-fork-ready.sh --bookkeeping-only   # plain, synchronous command: repairs ~/.ascii
bash boat-fork-ready.sh                      # waits for the restore, then repairs $HOME
```

What it handles:

- **Ownership.** A fork comes back with directories owned by root
  (`~/.cargo`, `~/.openagents`, directories in the warm slot,
  `~/.ascii/processes`). Until `~/.ascii` is the user's, Boat's detached
  commands fail with `EACCES ... ~/.ascii/processes/...log`; until the rest
  is, cargo cannot write the slot. A repair walked through the lazy mount
  took 26 to 100 s and missed directories the restore created later (the
  `Permission denied` of #10274); on plain disk it takes about 1 s.
- **The lazy restore.** Boat serves `/home/user` through a FUSE mount
  (`ascii-lazyfs`) while a background extract fills the real disk, then
  "retires" the mount (moves it to `/var/lib/ascii-lazy/retired/...`) and
  `/home/user` is plain disk. Building before that is slow and unreliable:
  on the 108 GB template the restore stalled for over 20 minutes on two 2 GB
  debug executables a build was waiting for, and on the 33 GB template a
  build on the mount failed (`Permission denied` removing a build script,
  `can't find crate` for rlibs the slot holds). A process whose working
  directory is inside the mount when it retires keeps the retired path,
  which is how #10219 saw the workspace under
  `/var/lib/ascii-lazy/retired/home/openagents`. The script waits until the
  mount is gone (`grep '^ascii-lazyfs /home/user fuse' /proc/mounts`); start
  builds from a new process after it.

Then build in the warm slot that the manifest names, as Coder does:

```sh
slot=$(jq -r .warm_target.slot ~/.openagents/coder-host.json)
cd ~/openagents && CARGO_TARGET_DIR="$slot" cargo build -p openagents-cli
```

Build one package per invocation, as the template warmed them: a combined
`-p a -p b` build unifies features and misses the slot.

## What the template holds, and why

- The warm slot is pruned like the GCE image's (#10224): the workspace's
  own test executables, binaries and incremental caches are deleted, except
  `openagents` and `microcoder`, which `chat work --on boat` runs in place
  (`--keep-binaries`). Dependency rlibs, rmeta and build-script outputs
  stay. The unpruned slot was 93 GB and its 108 GB template never finished
  restoring; the pruned one is 24 GB in a 33 GB template.
- **Mtimes cut to whole seconds.** Boat's restore keeps some mtimes to the
  nanosecond and cuts others to the second. Cargo marks a unit stale when a
  dependency's output is newer than its own, so a dependency and a
  dependent built within the same second rebuilt in every fork (53 crates,
  `StaleDependency` with `max_mtime ... nanos: 0`). The build driver sets
  every mtime under the slot, `~/.cargo/registry/src`,
  `~/.cargo/git/checkouts` and the clone (not `.git`) to whole seconds
  before the save; a fork's first build in the clone then compiles only
  `openagents-cli` itself.
- sccache's disk cache stays out of the snapshot (`.boxignore`). sccache in
  GCS instead of a warm slot was measured and not adopted (see the plan).

## Operating it

Use the automation account:
`export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`.

```sh
# Build now (same as the schedule)
gcloud run jobs execute oa-boat-template --region us-central1 --project openagentsgemini
# Measure a template: fork, fork-ready, build in the clone and in a worktree of origin/main
gcloud run jobs execute oa-boat-template --region us-central1 --project openagentsgemini \
  --args=probe,oa-coder-main-YYYYMMDD
# ... or build at once on the lazy mount, for comparison
gcloud run jobs execute oa-boat-template --region us-central1 --project openagentsgemini \
  --args=probe,oa-coder-main-YYYYMMDD,--mode,now
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
  (mostly the stalled restore, #10251), $0.073; the 2026-10-03 probe of
  `oa-coder-main-20261003b` cost $0.012, most of it waiting for the restore.
- Boat's docs list no separate charge for snapshot storage.

## Measurements

See [the plan, §7](../cloud/2026-10-02-boat-sdk-plan.md#7-b5-the-template-measured).

## Interactive Coder runtime

`boat-template build --runtime-binary PATH --runtime-revision COMMIT` installs
a portable `coder-cloud-runtime` in a separate `oa-coder-runtime-YYYYMMDD`
namespace. It retains a revision manifest and checks the executable, Git, Python,
and process locks before snapshotting. It excludes build caches and the repository
from this template. The existing `oa-coder-main-*` issue runner keeps its warm
build cache. Interactive builds never prune issue-runner templates.

Build the artifact from a clean main commit with
`scripts/cloud/build-coder-runtime.sh`. Boat Coder delegation chooses the newest
ready interactive template, or uses an explicit `--template NAME`. GCE pool
preparation builds this same runtime alongside the issue-runner CLI and records
its manifest under `~/.oa-pool/bin/runtime.json`.

The opt-in `coder-new` test `cloud_live` exercises CLI dispatch, continuation,
patch application, and cancellation. Run it once for each
`OA_CODER_CLOUD_LIVE_KIND=boat-integrated|boat-coder|gce`, with
`OA_CODER_CLOUD_LIVE=I_ACCEPT_CLOUD_COST`, `OPENAGENTS_SCRATCH`, and an admitted
`OPENAI_API_KEY`. It retains private evidence in scratch; stop or delete any
GCE pool you created for the check when it finishes.

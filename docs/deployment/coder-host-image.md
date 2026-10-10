# Coder host image (`oa-coder-host`)

A GCE image that a cloud Coder host boots from, rebuilt every day so a new
host starts with the repository, the toolchains and a compiled `origin/main`
instead of building from nothing. It is phase 3 of the
[cloud parallel execution audit](../cloud/2026-10-02-cloud-parallel-execution-audit.md)
(audit issue 2, [#10224](https://github.com/OpenAgentsInc/openagents/issues/10224)),
done ahead of the pool because it decides how fast every later phase starts.
Boat's daily template ([#10219](https://github.com/OpenAgentsInc/openagents/issues/10219),
[Boat SDK plan](../cloud/2026-10-02-boat-sdk-plan.md) B5) runs the same setup
script.

## What is on the image

| Thing | Where |
| --- | --- |
| Debian 12, Shielded VM (Secure Boot, vTPM) | image family `oa-coder-host`, names `oa-coder-host-YYYYMMDD` (UTC) |
| User `coder` | `/home/coder` |
| Repository, detached at the day's `origin/main` | `/home/coder/openagents` |
| Rust 1.97.1 (from `rust-toolchain.toml`) with clippy and rustfmt, through rustup | `/home/coder/.cargo`, `/home/coder/.rustup` |
| A warm Cargo target for `openagents-cli`, `microcoder` and `coder`: each package's build and its test targets, one package per invocation | `/home/coder/.openagents/targets/openagents-cc2c5b3cc6c4-slot-0`, the first slot the Coder task runner leases for this clone (`crates/coder/src/task/targets.rs`) |
| The release `openagents` binary of that commit | `/usr/local/bin/openagents` |
| sccache 0.18.0 behind a constant wrapper | `/usr/local/bin/oa-rustc-wrapper`, set in `/home/coder/.cargo/config.toml`; cache `gs://openagentsgemini-autopilot-rust-sccache/oa-coder-host/` |
| Node 24 LTS | `/opt/node`, linked into `/usr/local/bin` |
| Codex (`codex`), Claude Code (`claude`) | npm globals in `/usr/local/bin` |
| Grok Build (`grok`) | `/home/coder/.grok/bin` |
| git, gh, ripgrep, jq, build-essential, clang, cmake, protobuf; bubblewrap (Coder's run boundary on Linux) from the bake after 2026-10-02 | Debian packages (gh from GitHub's apt repository) |
| What was built, and every tool version | `/home/coder/.openagents/coder-host.json` and `/var/lib/oa-coder-host/manifest.json` |
| Boot unit `oa-coder-host-ready.service` | fetches `origin/main` into the clone, checks sccache can reach its bucket, prints `OA_CODER_HOST_READY` on the serial console and writes `/run/oa-coder-host/ready` |

**No credential is on the image.** No engine is logged in (the setup script
refuses to finish if a Codex, Claude or Grok login file exists), there is no
git or GitHub token, and no key file: sccache reaches the bucket through the
VM's own service account. Engine logins happen per host, later (audit issue 9,
Boat B7).

## Scripts

| Script | What it does |
| --- | --- |
| [`scripts/cloud/coder-host-setup.sh`](../../scripts/cloud/coder-host-setup.sh) | The shared, idempotent setup. As root it sets up user `coder`; as any user with passwordless sudo (a Boat sandbox) it sets up that user. `--warm` builds the warm target and the release binary; `--sccache-bucket` points sccache at GCS, otherwise it uses a 20 GB local disk cache. |
| [`scripts/cloud/coder-host-bake-guest.sh`](../../scripts/cloud/coder-host-bake-guest.sh) | The builder VM's startup script: clone at the pinned commit, run the setup with `--warm`, install the ready unit, seal the disk (no SSH host keys, empty machine id, apt and npm caches dropped), print `OA_CODER_HOST_BAKE_OK {manifest}`. |
| [`scripts/cloud/build-coder-host-image.sh`](../../scripts/cloud/build-coder-host-image.sh) | The bake: spot builder, follow its serial console, image, boot smoke, prune to the newest 3. Dry run without `--apply`. `--local-setup` bakes with this checkout's setup script instead of the one on `origin/main`, for testing a change before it lands. |
| [`scripts/cloud/coder-host-image-schedule.sh`](../../scripts/cloud/coder-host-image-schedule.sh) | Creates or updates the daily schedule; `--run-now` triggers one bake. |
| [`scripts/cloud/measure-coder-host-image.sh`](../../scripts/cloud/measure-coder-host-image.sh) | Boots a spot `c3-standard-8` from an image and times boot to ready and the builds below; deletes the VM. |

## The daily bake

```text
Cloud Scheduler  oa-coder-host-image-daily   07:00 UTC
   └─ POST cloudbuild.googleapis.com …/locations/us-central1/builds   (inline build, no repo trigger)
        └─ cloud-sdk step: sparse clone of scripts/cloud at origin/main
             └─ build-coder-host-image.sh --apply
                  1. origin/main → commit; skip if today's image is READY
                  2. spot c3-standard-22 builder, 200 GB pd-balanced, no external address
                     (Cloud NAT egress), service account oa-coder-host; zones a, b, c, f
                     in turn when one is out of capacity, then on demand
                  3. serial console until OA_CODER_HOST_BAKE_OK; a preempted spot
                     builder is retried once on demand
                  4. stop builder → image oa-coder-host-YYYYMMDD in family oa-coder-host
                     (storage location us-central1) → delete builder
                  5. spot e2-standard-4 smoke VM from the image must print
                     OA_CODER_HOST_READY within 10 minutes, else the image is deleted
                  6. delete all but the newest 3 images in the family
```

Every VM the bake creates is deleted on exit, success or failure. A failed
bake leaves yesterday's images in place, so `--image-family oa-coder-host`
keeps resolving to the newest good image.

Identities:

- **`oa-coder-host@openagentsgemini.iam.gserviceaccount.com`** is attached to
  the builder, the smoke VM and (later) pool hosts. It has one grant:
  `roles/storage.objectAdmin` on the sccache bucket. No project roles.
- **The build and the scheduler run as `oa-mvp-automation@`**, which already
  holds `compute.admin`, `iam.serviceAccountUser` and
  `cloudbuild.builds.editor`. That account cannot change project IAM, so a
  narrower dedicated bake account needs the owner; it is not required.
  Build logs go to `gs://openagentsgemini-oa-mvp-cloud-build-logs/log-<id>.txt`.

## Operate

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config

# the newest image and its manifest
gcloud compute images describe-from-family oa-coder-host --project openagentsgemini \
  --format='value(name,labels.openagents-source-revision,labels.openagents-boot-smoke,archiveSizeBytes)'
gcloud compute images list --project openagentsgemini --filter=family=oa-coder-host

# bake now (same as the schedule), or locally
scripts/cloud/coder-host-image-schedule.sh --run-now
scripts/cloud/build-coder-host-image.sh --apply             # from a laptop; builds nothing locally

# a bake's log
gcloud builds list --region us-central1 --project openagentsgemini --filter=tags=oa-coder-host-image --limit 5
gcloud storage cat gs://openagentsgemini-oa-mvp-cloud-build-logs/log-<BUILD_ID>.txt

# pause or resume the schedule
gcloud scheduler jobs pause  oa-coder-host-image-daily --location us-central1 --project openagentsgemini
gcloud scheduler jobs resume oa-coder-host-image-daily --location us-central1 --project openagentsgemini

# leak check: nothing named oa-coder-host-* should be running outside a bake
gcloud compute instances list --project openagentsgemini --filter='name~^oa-coder-host-'
```

To rebake a day whose image already exists, delete that image first (the bake
is idempotent per day) or pass `--image-name oa-coder-host-YYYYMMDDb`.

## Start a host from it

```sh
gcloud compute instances create HOST --project openagentsgemini --zone us-central1-a \
  --machine-type c3-standard-8 --provisioning-model SPOT --instance-termination-action DELETE \
  --image-family oa-coder-host --image-project openagentsgemini \
  --boot-disk-type pd-balanced --boot-disk-size 300GB \
  --no-address --service-account oa-coder-host@openagentsgemini.iam.gserviceaccount.com \
  --scopes cloud-platform --shielded-secure-boot --shielded-vtpm --shielded-integrity-monitoring
```

The root partition grows to the disk size on first boot. Use 300 GB for a
pool host: the image disk is 200 GB, the image holds 35 GB, the warm target 23 GiB, and every further
slot or cold target adds 20 to 80 GB once tests are built. Builds in the clone or a worktree reuse the warm slot
with `CARGO_TARGET_DIR=/home/coder/.openagents/targets/openagents-cc2c5b3cc6c4-slot-0`;
Coder task runs lease it by themselves.

## Measurements

Measured 2026-10-02 on image `oa-coder-host-20261002` (commit `bf30328c27`),
each on a fresh spot `c3-standard-8` (the pool host shape) with a 200 GB
pd-balanced disk created from the image, by
`scripts/cloud/measure-coder-host-image.sh --rev HEAD`. Every build is
`cargo build --locked -p openagents-cli` (debug); "cold" is an empty target
directory on the same VM.

| What | Result |
| --- | --- |
| Bake on a spot `c3-standard-22`: clone 58 s, packages, Node, Rust, sccache, engines under 1 min; warm build of the three packages with their tests about 12 min; prune; release `openagents` about 5 min | 1,015 s on the builder (17 min) |
| Image creation from the stopped builder's disk | 228 s |
| Whole bake, create builder to smoke passed | 1,343 s (22 min); the scheduled Cloud Build run of the unpruned image took 1,289 s |
| Image size | 9.5 GB archive (`archiveSizeBytes`), 35 GB used of a 100 GB disk; warm target 23.1 GiB (dependency rlibs 15.6, rmeta 1.7, build scripts 0.8) |
| Boot to ready (create call to `OA_CODER_HOST_READY` on the serial console, fetch of `origin/main` included) | 53 s on `c3-standard-8`; 65 and 107 s for the `e2-standard-4` smoke; the guest's own uptime at ready was 8 to 45 s. One run took 152 s, including a stock-out retry in a second zone |
| `git worktree add` of `origin/main` | 6 s |
| **Build in a fresh worktree on the warm slot** (what a Coder run does) | **94 s** |
| One edit to `crates/openagents-cli/src/main.rs`, rebuild | 15 s |
| Cold, sccache warm in the bucket (754 of 759 compiles hit) | 165 s |
| Cold, sccache off | 250 s |
| Build in the baked clone itself, no change | 78 s (it relinks and recompiles the workspace crates, whose outputs the prune removed) |

What the numbers say:

- **The warm image cuts a Coder run's first build from 250 s to 94 s**
  (2.7 times). The rest of those 94 s is the workspace's own crates: a
  worktree lives at a new path, so Cargo rebuilds every workspace member
  whatever the target holds. Only dependency artifacts carry over.
- **That is why the bake prunes.** The first per-package image kept the
  workspace's test executables, binaries and incremental caches: an 83 GB
  target, a 26 GB image, 92 of 100 GB used. The worktree build on it took
  98 s, no faster than on the pruned 23 GiB target.
- **sccache earns its place for cold targets** (a second slot, a new
  machine type, a day's new dependencies): 165 s against 250 s. It cannot
  cache the workspace crates while Cargo compiles them incrementally (the
  default for dev builds), so on the warm slot it adds nothing.
- **Build per package.** A combined `-p a -p b -p c` warm build unified
  features across the three packages, so a later `-p openagents-cli` missed
  the target and sccache alike (0 of 597 hits); one package per invocation
  fixed both.
- **Capacity.** Spot `c3-standard-8` was out of stock in us-central1-a twice
  during the day. The bake and the measure script now try the region's zones
  in order, and the bake falls back to on-demand.

Known gaps, as of the first image:

- A day whose `origin/main` cannot build (a stale `Cargo.lock` under
  `--locked` was seen at `ceac875a81`) fails the bake, and the family keeps
  the previous image.

## Cost

About $8 a month, list prices, us-central1:

| Item | Monthly |
| --- | --- |
| Builder: spot `c3-standard-22`, about 20 minutes a day | about $4 |
| Builder disk: 200 GB pd-balanced for about 25 minutes a day | under $0.10 |
| Smoke VM: spot `e2-standard-4` for 1 to 2 minutes a day | under $0.10 |
| Images: 3 kept × 9.5 GB at $0.05 per GB-month | about $1.40 |
| Cloud Build: one `E2_MEDIUM` step of about 25 minutes a day, mostly waiting | $0 inside the 2,500 free build-minutes, at most about $2.30 |
| Cloud Scheduler: one job | $0.10 |
| sccache objects under `oa-coder-host/` (2.5 GB after the first day; the bucket deletes objects after 14 days) | about $0.10 |

A host started from the image costs what its machine and disk cost; the
image adds nothing per boot.

## See also

- [Boat template runbook](boat-template.md): the daily template `oa-coder-main-<date>` is built by the same `scripts/cloud/coder-host-setup.sh --warm`.

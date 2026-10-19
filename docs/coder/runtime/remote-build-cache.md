# The shared remote compile cache

Builders can share one compile cache in Cloud Storage,
`gs://openagentsgemini-rust-build-cache`, so a crate one machine compiled is
downloaded instead of rebuilt on the next one (issue #12176). This page
covers the design, the bucket, credentials, how each build process uses it,
and what it measured.

**Summary.** The remote works and is correct across machines and checkouts,
but it only pays off for some builds. Measured on `oa-dev-env-1`, a warm
remote cut a cold release build of `coder` from 392 s to 235 s. It made a
cold debug build of `coder` slower, 142 s without it against 208 to 227 s
with it. So builds through `scripts/dev/cargo-build-env.sh` stay local unless
they set `OA_BUILD_CACHE=remote`, and the remote is turned on for cold
release builders, not for the debug and test builders.

## Design

kache is already the `rustc` wrapper on the Mac and on the land-queue
integrator, and kache 0.28.1 has a native Cloud Storage remote
(`[cache.remote] type = "gcs"`). So the remote is kache's own, not sccache
layered on top:

- **Correct across worktrees and machines.** kache keys each compile by the
  content of its inputs: the compiler's identity and target, flags,
  features, every source file the dep-info names, compile-time environment
  values, and the contents of every `--extern` dependency and linked native
  archive. It maps the workspace, Cargo home, rustup home, target directory,
  and temporary directory to fixed placeholders before it hashes, and passes
  `--remap-path-prefix` so outputs carry the same paths. The stale-artifact
  bugs of #12071 and #12113 were Cargo calling a unit fresh without running
  the wrapper at all; the source-keyed build directories of
  `cargo-build-env.sh` still prevent that.
- **Build directories map to one root.** The helper's build directory,
  `$CARGO_TARGET_DIR/worktrees/<hash of the checkout>`, would otherwise enter
  every key that names a build-script output (`OUT_DIR`, `-L`), and so every
  crate that depends on one, such as everything above `secp256k1-sys`. The
  helper sets `KACHE_BASE_DIR` to that directory, which kache maps to a fixed
  placeholder. Traced in two checkouts, the keys of `secp256k1-sys`,
  `nostr`, and `serde` differed before this and matched after. This also
  helps local kache across worktrees on one machine.
- **What can't be shared.** About 70 workspace crates read
  `env!("CARGO_MANIFEST_DIR")` in library code, which bakes the checkout's
  absolute path into the output. kache keys that path on purpose, so those
  crates and everything that depends on them compile again in every
  checkout. For `coder`, that's `nostr`, `gym`, `knowledge`, `coder`, and
  about 15 others: few crates, but the slowest ones, on the critical path.
- **Why not sccache.** The GCE pool hosts and `oa-dev-env-1` used sccache with
  `gs://openagentsgemini-autopilot-rust-sccache`. sccache keys include
  absolute paths, so it misses whenever the checkout or target directory
  changes, which is why the Mac moved to kache.
- **Degrades to local.** A remote error (no credentials, a refused read, no
  network) is logged and the crate compiles locally; tested with a bad key
  and a missing key, the build still succeeded. Uploads happen in kache's
  daemon after the compile.
- **One kache version everywhere.** Keys and the remote layout belong to a
  kache release. Every builder runs 0.28.1 (the Mac's version), pinned with
  its SHA-256 in `scripts/dev/remote-build-cache.sh`. Upgrade all builders
  together.
- **Who shares hits.** A key includes the host compiler and target, so Linux
  x86-64 builders share with each other, and a Mac shares only with another
  Apple-silicon Mac.

### Why debug builds got slower

With a remote configured, kache 0.28.1 does two things on the build's path
for each crate it doesn't have locally:

- It runs a dep-info pre-pass (`rustc --emit=dep-info`) to compute the key
  before compiling, so it can ask the remote. Without a remote it compiles
  first and keys afterward. Measured: 3 ms of key time per crate without a
  remote, 720 to 760 ms with one.
- It asks the remote synchronously, with a fixed 3-second budget per crate
  that includes the download. Debug artifacts are large (`coder`'s library
  is 670 MB), so big hits miss the budget and compile anyway, and three
  timeouts in a row turn the remote off for 45 seconds.

Release artifacts are a fraction of the size and take several times longer
to compile, so the trade goes the other way. kache's prefetch, which pulls a
saved build manifest's entries in the background, worked for the release
build (656 of 680 crates were already local when the compiler asked) but is
capped at 2 GiB a plan by default, below a debug build's size.

## The bucket

| Setting | Value |
| --- | --- |
| Bucket | `gs://openagentsgemini-rust-build-cache`, project `openagentsgemini`, `us-central1` |
| Access | uniform bucket-level access, public access prevention enforced |
| Layout | kache's, under the prefix `artifacts/` |
| Expiry | 30 days after an object is written; prefetch catalogs after 14 days, as kache's [bounding guide](https://kunobi.ninja/docs/kache/remote-cache/bounding) requires |
| Writers (`roles/storage.objectUser` on the bucket) | `oa-build-cache@` (key-file machines), `oa-mvp-automation@` (`oa-dev-env-1`), `oa-coder-host@` (pool hosts, `oa-agent-host-1`, the image bake), the Cloud Build accounts (`157437760789@cloudbuild`, `oa-cloud-run-source-builder@`, `oa-cloud-image-builder@`, `one-prod-release-builder@`), and the default compute account |

kache never deletes remote objects; the lifecycle rules bound the bucket.

## Credentials

kache's daemon reads Application Default Credentials: the key file named by
`GOOGLE_APPLICATION_CREDENTIALS`, then gcloud's application default
credentials, then the GCE metadata server.

- **GCE VMs and Cloud Build**: the VM's or build's own service account. No
  key file. A Docker build in Cloud Build reaches the metadata server only
  with `docker build --network=cloudbuild`.
- **Machines without a GCE identity** (CoderOS, Archil, a Mac): the
  `oa-build-cache@` service-account key at
  `~/work/.secrets/oa-build-cache-sa.json` (mode 0600). It is on the owner's
  Mac and on CoderOS. `enable --key` puts its path in kache's daemon service
  only (a systemd user drop-in on Linux, the launchd plist on macOS), so
  Cargo builds and tests never see `GOOGLE_APPLICATION_CREDENTIALS`: Vertex
  and other Google clients in the workspace read that variable.

The key is never printed, logged, or committed.

## Setting up a machine

```sh
scripts/dev/remote-build-cache.sh install-kache          # Linux: pinned kache into ~/.local/bin
scripts/dev/remote-build-cache.sh enable                 # a GCE VM, or gcloud ADC
scripts/dev/remote-build-cache.sh enable --key ~/work/.secrets/oa-build-cache-sa.json \
  --local-max-size 8GiB --set-wrapper                    # a machine without a GCE identity
scripts/dev/remote-build-cache.sh status
scripts/dev/remote-build-cache.sh disable                # back to a local cache
scripts/dev/check-build-cache.sh                         # is this machine using the remote?
```

`enable` adds a marked `[cache.remote]` block to kache's config file
(`~/.config/kache/config.toml`) and leaves the rest alone; the daemon notices
and restarts. `check-build-cache.sh` checks the wrapper, the kache version,
the remote, the key and the daemon service, reaches the bucket through
`kache doctor`, and reports what the last build got from the remote. It
exits 1 when a required check fails; `--wrapper PATH` checks a service's
`RUSTC_WRAPPER` instead of the shell's Cargo configuration.
`scripts/dev/remote-build-cache.test.sh` tests the setup script and the
helper.

`cargo-build-env.sh` sets `KACHE_LOCAL_ONLY=1` unless `OA_BUILD_CACHE=remote`,
and also when the key file the config names is missing. A build outside the
helper uses whatever kache's config file says.

To use the remote from a Docker build (Cloud Build), in the builder stage:
install kache 0.28.1 from its release tarball with the SHA-256 the setup
script pins, write the `[cache.remote]` block to `/root/.config/kache/config.toml`,
set `RUSTC_WRAPPER=/usr/local/bin/kache`, and end the `RUN` that builds with
`kache save-manifest && kache daemon stop`, so the uploads finish before the
layer is cut. A Docker build always uses the same `WORKDIR`, so its
workspace crates hit too.

## Every build process

| Build process | Uses the remote? | Why, and the numbers |
| --- | --- | --- |
| Mac dev and agent builds (`cargo-build-env.sh`) | No: local kache | Debug builds; the remote slowed them. They gain `KACHE_BASE_DIR`, so build-script crates now hit across worktrees locally. No second Apple-silicon builder exists to share with. |
| `oa-dev-env-1`, the land-queue integrator | No: local kache | `oa-land-worker.service.d/build-cache.conf` sets `RUSTC_WRAPPER=/home/coder/.local/bin/kache` (was sccache with GCS); its config has no remote, because its checks are debug test builds. Restarted between landings with the drain file. |
| Other builds on `oa-dev-env-1` (user `coder`) | No | `/usr/local/bin/oa-rustc-wrapper` now runs kache when the config names a bucket, else sccache as before. |
| GCE pool hosts and `oa-agent-host-1` | Opt-in at bake | `build-coder-host-image.sh` with `OA_KACHE_BUCKET=openagentsgemini-rust-build-cache` installs kache, enables the remote, and puts it first in `oa-rustc-wrapper`. Off by default: hosts run debug builds. `oa-coder-host@` can write the bucket. |
| CoderOS | Not yet | The key is in `~/work/.secrets`. The disk was full (0 bytes free) on 2026-10-19, so nothing more was installed. When there's room: `install-kache`, then `enable --key ... --local-max-size 8GiB --set-wrapper`. |
| Archil issue runs (#12136, #12158) and fan-out runners | No | Archil's network allowlist doesn't include `storage.googleapis.com` or `oauth2.googleapis.com`. When it does: kache in `crates/coder-cloud/archil/Dockerfile`, the key with the run's other credentials, and `enable --key`. |
| Desktop `openagents host-upgrade` and `apps/openagents/scripts/run-macos-dev.sh` | No: local kache | Plain `cargo build` on the Mac, through `~/.cargo/config.toml`. |
| `scripts/install-coder.sh` release builds | Machine's setting | Plain `cargo build --release`: uses the remote where kache's config has one. On the Mac it's local. |
| iOS core (`apps/openagents/scripts/ios/build-core.sh`) | No: local kache | Runs Cargo with a clean environment but the same Cargo configuration. |
| Android core (`apps/openagents/scripts/android/build-core.sh`) | No: local kache | Sources the helper. |
| coder-sync deploy (`scripts/coder-sync/deploy.py`) and the web and stack Cloud Builds (#12177) | Ready | Release builds in Docker on Cloud Build: the case the remote helps. The Cloud Build accounts can write the bucket; the Dockerfile change belongs to #12177 (recipe above). |

## Measurements

All on `oa-dev-env-1` (22 vCPU, 86 GB, Debian 12), commit `015a2d369f`, 12
Cargo jobs, rustc 1.97.1 (root) and 1.98.1 (app). Each run used a fresh
checkout at a new path, a fresh target directory, and an empty local kache
store, so only the bucket carried over: the same as a second machine. The
"warm" runs came after an earlier run at another path filled the bucket.

| Build | Setup | Wall time | Crates from the cache |
| --- | --- | --- | --- |
| `cargo build -p coder` | no kache | 136 s | none |
| | kache, no remote | 142 s, 144 s | none |
| | kache, warm remote | 227 s, 213 s, 208 s | 599, 547, 247 of 646 (the last after the 45-second cutoff) |
| | `kache sync --pull`, then build | 173 s + 148 s | 625 local, 13 compiled |
| | one-line change in `crates/coder`, rebuild | 109 s, 116 s | all but the changed crates |
| `cargo build --release -p coder` | kache, no remote | 392 s | none |
| | kache, warm remote | **235 s** | 656 prefetched, 6 remote, 15 compiled of 680 |
| `cargo build -p openagents` (`apps/openagents`) | kache, no remote | 893 s, 911 s | none |
| | kache, warm remote | 896 s | 704 of 1,156 |
| | `kache sync --pull`, then build | 162 s + 325 s | 1,062 local, 100 compiled |
| | one-line change in `crates/brand`, rebuild | 160 s | all but the changed crates |

The app's Linux build compiles but fails its final link on Debian 12: the
prebuilt ONNX Runtime that `ort-sys` downloads needs glibc 2.38
(`__isoc23_strtol`). The times are to that link. App times also varied by
more than 300 s between runs, so read them as rough.

A fresh-machine check from a second home path, and runs on CoderOS, are
still to do (CoderOS's disk was full).

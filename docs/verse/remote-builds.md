# Remote builds

Issue: [#13077](https://github.com/OpenAgentsInc/openagents/issues/13077).
Code: `coax/remote/remote.sh` (the caller), `coax/remote/host-job.sh` (one job
on a host), `coax/remote/host-startup.sh` (a host's startup script).

Heavy Coax work runs on a build host in Google Cloud instead of the Mac. The
Mac's two build slots (`~/.openagents/build-slots/heavy.sh`) stay as the
fallback.

```sh
coax/remote/remote.sh run -- coax/web/wasm.sh apps/openagents-web/fivelab.coax \
  apps/openagents-web/fivelab-face.c apps/openagents-web/build/fivelab.wasm \
  apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/remote/remote.sh run -- coax/bin/coax test apps/openagents-web/fivelab.coax
coax/remote/remote.sh status        # hosts, free slots, spend
```

Or keep calling `heavy.sh` and set `OA_HEAVY_REMOTE=1`: heavy.sh then sends
the command to `remote.sh run` first.

## Why

On 2026-11-09 the Mac reached a load of 110. About eight agents shared two
build slots and waited 30 minutes or more for one. Most of that time was one
step: Coax to C for an app with an `#authority` unit runs every check the
unit keeps. fivelab has 1,050 checks, and its Coax to C took 265 to 583
seconds on the Mac.

## What a job does

1. **Snapshot.** `remote.sh` copies the checkout's index to a scratch file,
   adds every file to it (`git add -A`, so untracked files count and ignored
   ones don't), and writes a commit on `HEAD` from it. The checkout's own
   index, files and branches don't change. The snapshot's tree hash is the
   job's source fingerprint.
2. **Send.** A Git bundle of the snapshot, minus what `origin/main` already
   has, goes to the host over SSH with the job's spec: the command, its
   directory in the checkout, and the `COAX_*` and `COAXIAL_*` settings
   (except those that name paths on the Mac). An argument that names the
   checkout's absolute path names the host's tree instead. Untracked files
   over 200 MB in all stay home: the job runs on the Mac.
3. **Run.** The host takes one of three job slots (a `flock`), checks the
   snapshot out in that slot's worktree of its clone, and installs the
   compiler for the tree's compiler sources with `coax/build.sh`: the
   published Linux build, or a bootstrap with the release toolchain (static
   GCC for musl from nixpkgs, as `publish-host.sh` builds) when none is
   published. Then it runs the command. Output streams back as it runs.
4. **Bring back.** Every file the command wrote or changed in the tree (newer
   than a marker made just before it ran, except `.git`, `coax/bin` and
   `node_modules`) comes back into the checkout as a tar stream: modules,
   timings reports, test logs. The exit status is the command's.

A command that passed is kept on the host under a key of the source
fingerprint, the command, its directory and its settings. The same job again
replays the output and files without running (results go after 3 days).

The host's caches live on its disk and serve every job: `~/.cache/coax-wasm`
(each unit's C, object and module, wasm.sh), `~/.cache/coax-check` (check
verdicts and per-check reuse records), and `~/.cache/coax-release`
(compilers). Slot worktrees stay between jobs, so a job checks out only the
files that differ from the slot's last job, and build outputs (ignored files)
stay for the next job.

## Hosts

| Setting | Value |
| --- | --- |
| Name | `oa-build-1` to `oa-build-4` (label `oa-remote=build`) |
| Machine | `c3d-highcpu-30` (30 vCPU, 59 GiB), spot, Debian 12 |
| Disk | 200 GB pd-balanced (the caches stay when the host stops) |
| Network | default VPC, an ephemeral external address, SSH by key only (`~/.openagents/remote/ssh_ed25519`, project keys blocked) |
| Account | none: the VM has no service account and no scopes |
| Stop | itself, after 15 idle minutes (no slot held); spot preemption stops it too; 12 hours of running is the backstop |

`remote.sh` uses a running host with a free slot. With none, it starts a
stopped host, or creates the next one up to `OA_REMOTE_MAX_HOSTS` (4). A new
host's first boot installs Nix, Bun and a blob-less clone (about 4 minutes);
a stopped host starts in about a minute. Jobs on a full host wait for a slot
there.

`remote.sh down` stops every host; `remote.sh delete` deletes them and their
disks.

## When the Mac runs the job

`remote.sh run` runs the command through `heavy.sh` (or directly, with no
heavy.sh) when:

- `OA_REMOTE=0` or `--local` is given;
- the snapshot can't be made or is too big;
- the spend estimate over 24 hours has reached `OA_REMOTE_DAILY_USD`;
- no host is running, none can start (spot capacity), and none can be made;
- the host can't run the job (SSH fails, the checkout or the compiler build
  fails: exit 97).

`--no-fallback` exits 97 in those cases instead.

## Cost

List prices, us-central1, from the Cloud Billing catalog on 2026-11-09:

| Item | Price |
| --- | --- |
| `c3d-highcpu-30` spot: 30 × $0.00711 a vCPU-hour + 59 × $0.00096 a GiB-hour | $0.270 an hour |
| External address while running | $0.005 an hour |
| 200 GB pd-balanced disk, running or stopped | $20 a month a host |
| Same machine on demand (not used) | $1.12 an hour |

`remote.sh` keeps a ledger, `~/.openagents/remote/ledger.tsv`: each job's
wall time on the Mac at the host's whole price, each start plus its 15 idle
minutes, and each creation as 15 minutes of setup. Concurrent jobs are each
counted at the whole price, so the estimate errs high. `remote.sh cost`
sums it. The default cap is $15 over 24 hours (`OA_REMOTE_DAILY_USD`); at
the cap no host starts and jobs run on the Mac.

Expected: with eight agents at 10 heavy jobs a day each, about 10 host-hours a
day, about $80 a month for one host's compute plus $20 for its disk. The cap
bounds the worst case at $450 a month for compute.

## Measured

2026-11-09, `oa-build-1` (`c3d-highcpu-30` spot) against the Mac's own
reports (`apps/openagents-web/build/.c/fivelab.timings.json` in agents'
worktrees the same morning, at loads of 13 to 76). Remote times are the
caller's wall time, snapshot and transfer included; cost is that time at
$0.275 an hour.

| Job | Mac (local slots) | Remote | Remote cost |
| --- | --- | --- | --- |
| fivelab wasm.sh, nothing cached (1,050 checks) | 265 to 583 s, plus the wait for a slot | 321 s with the new compiler (406 s with main's) | $0.025 |
| fivelab wasm.sh after a one-line edit to fivelab.coax | 265 to 583 s (no check reused) | 223 s: 1,033 checks reused, 17 run | $0.017 |
| fivelab wasm.sh, same sources, another slot's checkout | 1 to 12 s (wasm.sh's cache) | 29 s (10 s of it the link and wasm-opt) | $0.002 |
| fivelab wasm.sh, the same job again | 1 to 12 s | 11 s (the result replayed) | $0.001 |
| `coax test apps/openagents-web/fivelab.coax` (1,313 checks) | not measured (it would load the Mac) | 995 s | $0.076 |
| `tests/run.sh --only check_reuse` | not measured | 187 s | $0.014 |
| Installing a published compiler in a slot | | 12 s | |
| Bootstrapping an unpublished compiler (`COAX_MUTATION=0`) | | 15 to 21 minutes | $0.07 to $0.10 |
| A host's first boot (Nix, Bun, the blob-less clone) | | about 4 minutes | $0.07 (billed as 15 minutes) |

Every remote job costs the Mac about one second of CPU: a `git add` into a
scratch index, a bundle, and an SSH stream. The rest of the Mac's slots
stay free for the jobs that need it.

What still takes minutes after an edit: the checks whose results carry a
capacity note (a check that used more than three quarters of its step or
storage budget). Those are never reused, so their note points at the current
source, and they run one after another in the compiler itself. In fivelab, 17
such checks take about 180 of the 223 seconds. Giving those checks the larger
`#steps` their notes ask for, or splitting them, removes the notes and lets
them be reused.

## Incremental Coax to C

wasm.sh already keeps each unit's C, header, object and linked module under
a hash of its inputs, in one cache shared by every checkout
(`~/.cache/coax-wasm`, #12925). The "2/3 cached" seen on the Mac is the unit
whose `#load` closure an agent edited: the app's own unit. That one unit
takes minutes because its compile runs all its `#authority` checks.

The compiler reuses checks too (#12647): the whole pass when nothing changed,
and each check whose inputs didn't change otherwise. On the Mac that reuse
found nothing: fivelab's reports said `"check_reuse": {"reused": 0, "run":
1049}` and `"authority_checks": {"cached": 0, "checked": 1}`. Two causes:

1. **Keys named absolute paths.** The whole-pass key, the per-check record's
   file name and every declaration's identity used each unit's canonical
   path, `/Users/.../oa-TOPIC/apps/openagents-web/fivelab.coax`. Each agent
   works in its own worktree and release.sh builds in a temporary one, so a
   record from one checkout was never found by another. Now a unit is named
   by its path inside its Git checkout (below the directory holding `.git`):
   `apps/openagents-web/fivelab.coax`. `coax/tests/check-reuse.coax` checks
   that a second checkout of the same sources reuses the first's pass, and
   that after the same edit in both, one runs only the edited check and the
   other reuses its pass.
2. **The cache was too small.** It kept 4,096 records. Agents filled it in
   about 90 minutes (the oldest record on the Mac was from 07:58 at 09:20), so
   few records lived until the next build. The default is now 65,536 records
   and 2 GiB (`COAX_CHECK_CACHE_FILES`, `COAX_CHECK_CACHE_BYTES`).

The C those compiles emit is unchanged, so wasm.sh's own cache keys stay
valid. The change reaches agents with the next published compiler.

`coax/tests/check-reuse.coax`'s corpus step fails on the host with main's
compiler as with this one (its 120-second limit); the other steps pass.

## The Linux toolchain

The size budgets in `apps/openagents-web/wasm-budgets.txt` were measured
with the Mac's Homebrew clang 23. With nixpkgs' clang 21, fivelab built on
the host was 2,732,416 bytes, over its 2,725,000 budget, and the build
failed. `coax/web/toolchain.sh` now links clang 23 on Linux (23.1.3): fivelab
is 2,719,737 bytes, as the Mac's 2,720,222. A toolchain linked for another
clang major is linked again on first use.

## What stays on the Mac

- **Shots** (`coax/web/tool.sh shot`, goldens, frame times). The hosts have no
  GPU. Chrome's software WebGPU (SwiftShader) is not what the goldens and
  frame budgets were measured on, so a shot from the host would not compare.
  A GPU host (`g2-standard-8` spot, about $0.30 an hour) could take them; that
  is not built.
- **release.sh**. It deploys with the Mac's credentials and compares against
  production. Its builds can use the same caches once the hosts warm them, but
  the release itself stays where it is.
- **room/deploy.sh's drift check** needs the room's credentials; its build
  step can run remotely as any other command.

## Limits

- Files the command writes outside the checkout don't come back.
- If the caller is killed, the host's job may run to its end in its slot.
- Files changed on the Mac while a job runs are overwritten by the files the
  job brings back, if the job wrote the same paths.

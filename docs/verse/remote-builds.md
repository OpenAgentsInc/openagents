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

`remote.sh run --commit REV --out DIR -- CMD` runs CMD from the root of commit
REV instead of a snapshot: the green-main pipeline's checks
([green-main-pipeline.md](green-main-pipeline.md)) use it to check exact
commits. The files CMD writes go to DIR. Such a job never falls back to the
Mac.

When the picked host's slots filled after it was picked, the host sends the job
back, and `remote.sh` asks the hosts again every 10 seconds until one has a
free slot (for up to `OA_REMOTE_WAIT` seconds, 1800 by default; then it waits
on one host).

A command that passed is kept on the host under a key of the source
fingerprint, the command, its directory and its settings. The same job again
replays the output and files without running (results go after 3 days).

The host's caches live on its disk and serve every job: `~/.cache/coax-wasm`
(each unit's C, object and module, wasm.sh), `~/.cache/coax-check` (check
verdicts and per-check reuse records), and `~/.cache/coax-release`
(compilers). Slot worktrees stay between jobs, so a job checks out only the
files that differ from the slot's last job, and build outputs (ignored files)
stay for the next job.

## The shared cache

Issue: [#13119](https://github.com/OpenAgentsInc/openagents/issues/13119).
Code: `coax/remote/shared-cache.sh`, `coax/remote/pages.sh`, and the shared
tier in `coax/web/wasm.sh`.

Each unit compiles once for every host. A Cloud Storage bucket,
`gs://openagentsgemini-coax-build-cache` (us-central1), sits behind each
host's disk caches, under the same keys:

- **wasm.sh's unit cache.** `wasm/KEY.c`, `.h`, `.notes`, `.o` and `.wasm`,
  keyed as `~/.cache/coax-wasm` is: a unit's `#load` closure and the compiler's
  hash for its C, what clang reads for an object, the objects and tools for a
  module. A lookup reads the host's disk first, then the bucket, and keeps what
  it fetched on the disk. A compile writes its result to the disk and then to
  the bucket, the C before its header, so a host that finds the header finds
  the C.
- **The check cache.** The compiler's records stay keyed as they are. After
  wasm.sh compiles a unit, the records that compile read or wrote go to the
  bucket as one archive for the unit (`checks/COMPILER/UNIT.tar.gz`), and the
  next compile of that unit on any host unpacks it into its check cache first.
  A compile after an edit so reuses every check the edit didn't touch, wherever
  those checks ran before. A remote job does the same for its whole command
  (`checks-job/COMMAND.tar.gz`), so `coax test` on a second host reuses the
  first host's checks.
- **The compiler.** A host that needs a compiler for a tree's compiler sources
  first looks for `compiler/ARCH/FINGERPRINT.tar.gz` (`coax/build.sh`'s cache
  entry, keyed by `coax/provenance.sh fingerprint`). `coax/build.sh` then
  verifies it against the tree's fingerprint and its provenance record before
  installing it, and bootstraps when it doesn't verify. A host that built or
  already had one shares it after the job's compiler step. A restarted host
  that spent 48 minutes building the compiler now fetches it (12 MB) in about
  10 s; the job's line says `compiler from the bucket in N s`. It also makes
  the other keys match: they name the compiler's SHA-256, and a compiler each
  host bootstrapped differs in its bytes (one fingerprint, `1fa13a88e401`,
  had four different binaries on oa-build-1 to oa-build-4).

`remote.sh` mints a token for each job that can read and write this bucket and
nothing else (the automation account's token, exchanged at Google's STS for
one with a credential access boundary on the bucket; it lasts an hour), and
the host's job passes it to the command. The hosts have no service account,
so this needs no change to them.

The Mac's own builds leave the tier off. No key can match between the Mac and
a host: the keys name the compiler's bytes and each tool's install path. The
Mac shares the hosts' work through built pages instead (below).

Every remote job's report line says what the caches did:

```
remote: oa-build-2: 31 s in all (...; cache 4/4 hit (100%: disk 0, shared 4), 0 compiled (0 Coax to C, 0 s), 0 duplicates); about $0.002
```

`disk` is the host's own cache, `shared` the bucket, `compiled` the steps that
ran, and `duplicates` the compiles whose result another host wrote first. Each
compile also leaves an empty object under `log/DAY/KEY/`;
`coax/remote/shared-cache.sh report [DAY]` counts the keys compiled more than
once that day across every host.

### One compile per key

A cache alone doesn't stop two builds that start together: both miss, and both
compile. `build.sh` starts about 60 `wasm.sh` runs side by side, and many of
them share a unit (Everglade's controller, the thumbs, the Grid). On
2026-11-10, 27 of 783 keys compiled more than once (37 extra compiles), all
but one of them on one host in the same second.

So before a compile, `wasm.sh` takes the right to make that key (`once` in
`coax/web/wasm.sh`):

- **On one machine,** a lock folder in the disk cache (`.once.KEY`, holding its
  holder's process ID). A run that finds it waits for the holder's result and
  reads it from the disk. A lock whose holder is gone is taken over.
- **Across hosts,** a claim object in the bucket (`claim/wasm/KEY`, written
  only when absent). A host that finds a claim polls the bucket for the result
  every 3 seconds and says `another host is compiling KEY; waiting for its
  result`. The holder rewrites its claim every 30 seconds and deletes it when
  the compile ends. A claim that is 150 seconds old belongs to a holder that is
  gone (a failed build, a preempted host), and the next host takes it.

The Mac's builds use the lock folder too, so a local `build.sh` also compiles
each shared unit once.

The offline room's build (`coax/web/room/worker.coax`, `everglade-room.wasm`)
keeps its units' C in the same disk cache under its own keys. It now reads and
writes the bucket and takes the same claims. Its `room.coax` unit ran for more
than 24 minutes on a host that had not compiled it (oa-build-2, 2026-11-10),
which every host used to repeat after each change to the room's sources.

Measured on 2026-11-10 (`shared-cache.sh report`, and the report line of a
full `apps/openagents-web/build.sh` job):

| | Before | After |
| --- | --- | --- |
| Keys compiled more than once in a day, all hosts | 27 of 783 (37 extra compiles); 6 of 121 the day before | |
| Duplicates in one full `build.sh` job | 2 of 15 compiles | 0 of 41, 0 of 14 and 0 of 44 compiles (three jobs, three hosts) |
| Two builds of one new key started together, two disk caches | both compile | one compiles, the other waits 3 s and reads it; same bytes |
| The same, one disk cache and no bucket | both compile | one compiles 9 steps, the other reads all 9 |

The hosts ran 414,746 job-seconds on 2026-11-09 (591 jobs, $31.68 in the
ledger) and 215,836 on 2026-11-10 (268 jobs, $16.49). The ledger counts a
job's whole wall time, so it shows how busy the hosts are, not what the cache
saved; the report line of each job does.

### One page build per commit

`coax/remote/pages.sh COMMIT` puts COMMIT's built pages into this checkout:
`apps/openagents-web/build/` (every module, the render packages) and the
build's other untracked files (`nip-guide.wgsl`, `pylon/join.sh`). It never
changes a tracked file. When the bucket has them
(`pages/COMMIT/pages.tar.gz` and its `MANIFEST`), it fetches them and checks
the MANIFEST's tree against the commit's and the archive's SHA-256. When it
doesn't, it builds them once on a host (`apps/openagents-web/build.sh` in
release mode, from the commit's own files), stores them, and fetches them.
While one caller builds a commit, every other caller waits for it instead of
building it again.

To drive pages headless, fetch main's pages and rebuild only what you changed:

```sh
coax/remote/pages.sh origin/main
coax/remote/remote.sh run -- coax/web/wasm.sh apps/openagents-web/fivelab.coax \
  apps/openagents-web/fivelab-face.c apps/openagents-web/build/fivelab.wasm \
  apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

Don't run a full `apps/openagents-web/build.sh` for this.

`after.sh` (the green-main pipeline) runs `pages.sh build` on each new
main-green in the background, so the release finds its commit's pages built.

Until 2026-11-10 no commit's pages reached the store: `build.sh` exited 1 on
every host (#13117), so each `pages.sh` build failed after its 15 to 35
minutes and the release built everything on the Mac. The causes were in the
CLI's build, which the web build runs for the homepage's warm start: a second
`#import "Basic"` (E0332), no `xxd` and no OpenSSL headers on the hosts, and a
missing `#load` in the Linux-only stream host. `build.sh` now ends with
`build.sh: FAILED (status N) at line L: COMMAND` when a step fails, and stops
the modules still building.

### The release reuses them

`release.sh` fetches the pages of the commit it ships (`pages.sh fetch`) and
sets `COAXIAL_PREBUILT=1`. Each wasm.sh module the hosts built has a record
beside its report, `build/.c/MODULE.built`: the SHA-256 of what it was built
from (its options, its units, every file in each unit's `#load` closure, and
the face's C), its bytes' SHA-256 and each unit's compile notes. wasm.sh takes
a module whose record names the same inputs and bytes, then checks its compile
headroom and its size budget against those bytes as it would after a build. A
module with no record, or whose inputs differ here (the project pages' tables
come from Nostr at build time), builds on the Mac as before. A release of a
commit without stored pages builds everything, as before.

The release then ships the hosts' bytes (Linux, clang 23.1.3), not the Mac's
(Homebrew clang 23); they differ by a few hundred bytes a module, so the first
such release renames each module's content-named URL once.

`release.sh --build-only` builds and stages a release in a scratch folder and
stops: no lock, no upload, no checks against a site. It is a dry run of the
build step.

### Cost

List prices, us-central1 Standard storage: $0.020 a GB-month, $0.005 per
1,000 writes, $0.0004 per 1,000 reads, and no charge for traffic between the
hosts and the bucket in the same region. Objects go 14 days after they are
written (the bucket's lifecycle rule; no soft delete). A host's cache holds
about 1 GB, so the bucket holds a few GB, and a busy day of builds makes tens
of thousands of reads and writes: under $2 a month in all.

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
stopped host, or creates the next one up to `OA_REMOTE_MAX_HOSTS` (8). A new
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
sums it. The default cap is $50 over 24 hours (raised from $15 by the owner on 2026-11-09) (`OA_REMOTE_DAILY_USD`); at
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

### A comment reruns no check (#13123)

Each check's record is keyed on the declarations it reaches, by their tokens
(a comment or a blank line is no token), plus the compiler's hash. Two
things still ran checks again after a comment:

- **Notes near a limit.** A check whose steps or traversal came within the
  near-limit fraction of its budget prints a note naming what used them, at
  their current lines. Its record didn't keep the note, so every compile ran
  it again: on the status page, 11 of 90 checks after any edit. Now the record
  keeps the note, with each consumer as the declaration it is in and which of
  its tokens it is, and a reused check prints the note a cold compile prints.
  A check whose value slots, arrays or strings come near their limits still
  runs again, since that note names who held the storage at the time.
- **Library verdicts.** A loaded unit's own verdict was keyed on the bytes of
  its `#load` closure. It is now keyed on each unit's tokens.

`coax/tests/check-reuse.coax` checks that a comment appended to a loaded file
runs no check and prints a cold compile's note, that lines added above the
note's consumer name its new line, and that an edit to one engine procedure
runs only the check that reaches it. `coax/tests/library-checks.coax` checks
that a comment in a library reuses its verdict.

Measured on oa-build-3 (30 cores, load 11 to 19), fivelab's own unit:

| Edit | Checks run | Coax to C |
| --- | --- | --- |
| None (a cold compile) | 1,259 of 1,259 | 132 to 185 s |
| A comment appended to `verse/experimental-dungeon.coax` | 0 of 1,259 | 27 to 34 s (before: 104 s) |
| `DG_HEIGHT :: 5.0;` to `5.0 + 0.0;` in that file | 961 of 1,259 | 130 s |

The one-line edit runs 961 checks because they all reach `DG_HEIGHT`. With
no check to run, a compile of fivelab still takes about 27 s: 11 s to key
its 1,259 checks (`authority.reuse`), 9 s to lex and load its sources, and
the rest to check and emit them. The 15-second target for a one-line edit is
not met yet.

`coax/tests/check-reuse.coax`'s corpus step fails on the host with main's
compiler as with this one (its 120-second limit); the other steps pass.

### A recompile that runs no check (#13131)

With no check to run, fivelab's compile spent its time in two places that
did the same work twice:

- **Keys.** A check's key was the SHA-256 of one line of text for each
  declaration the check reaches. fivelab's checks reach 2.8 million
  declarations between them, so each compile hashed 321 MB of lines. Now each
  declaration's line is hashed once a compile, and a check's key is the
  SHA-256 of its first lines, how many declarations it reaches and the sum of
  their line hashes. A key depends on the same declarations as before. A
  compile that records checks makes the same keys: on oa-build-3,
  `authority.record` took 13.1 s for 1,652 checks before and 1.3 s for 1,306
  after.
- **Lexing.** The loader lexes each unit to find its `#load`, `#module` and
  `#export` directives, and the compiler then lexed the composed text again.
  Now the composed text's tokens are the units' own tokens, moved to their
  place in it. `coax/tests/sources.py` checks a token's line, column and
  value after blanked directives and after a unit with no final newline.

Measured on fivelab's own unit: a comment appended to
`verse/experimental-dungeon.coax`, 0 of 1,306 checks run, three edits each.
The first two rows are one job on oa-build-4 (30 cores, load 2 to 9), the
new compiler built there from these sources. The third is the bootstrapped
compiler on oa-build-3 while other jobs ran (load 12 to 20).

| Compiler | Coax to C | `authority.reuse` | `lex` | `load` |
| --- | --- | --- | --- | --- |
| Before | 26.3 to 29.6 s | 12.2 to 14.6 s | 3.9 s | 4.8 s |
| After | 11.4 to 11.8 s | 1.0 s | none | 4.9 s |
| After, on a busy host | 14.2 to 14.9 s | 1.3 s | none | 5.8 to 6.9 s |

What is left on a quiet host: 4.9 s to load (4.0 s of it the units' lexing),
1.8 s to check, 1.3 s for the libraries' verdicts, 0.8 s to parse, 0.6 s to
emit and 1.8 s for the `#authority` pass.

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
  is not built. On the Mac a shot uses the checkout's resident browser
  ([fast-shots.md](fast-shots.md)).
- **release.sh**. It deploys with the Mac's credentials and compares against
  production. Its build takes the hosts' modules for its commit (see "The
  release reuses them"), but the release itself stays where it is.
- **room/deploy.sh's drift check** needs the room's credentials; its build
  step can run remotely as any other command.

## Limits

- Files the command writes outside the checkout don't come back.
- If the caller is killed, the host's job may run to its end in its slot.
- Files changed on the Mac while a job runs are overwritten by the files the
  job brings back, if the job wrote the same paths.

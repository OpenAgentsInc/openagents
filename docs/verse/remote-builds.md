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
verdicts and per-check reuse records), `~/.cache/coax-release`
(compilers), and `~/.cache/coax-sanitized` (the compiler's units' sanitizer
objects). Slot worktrees stay between jobs, so a job checks out only the
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
- **The compiler's sanitizer objects** (#13145). Two of the compiler's units
  (`parser` and `strings` in `coax/tests/run.sh`) link the whole compiler
  under ASan and UBSan. The compiler is one translation unit, so that compile
  is about three minutes of one core on an idle host, and it used to run from
  nothing in every run of the unit. `coax/tests/sanitized_build.py` now keeps
  each object under a key of the C compiler's identity, the flags and the
  preprocessed source: on the host's disk (`~/.cache/coax-sanitized`, 3 days),
  then in the bucket (`sanitized/KEY.o.gz`). A unit compiles again only when
  what it reads changed, once for every host. The job's report line counts
  these lookups with the others. A compile that does run is limited by the
  CPU seconds it uses, not by the clock, so a busy host changes how long it
  takes and not whether it passes.

  Measured on 2026-11-10. In the pipeline, `parser` took 337 s when it passed
  and failed twice at 210 and 228 s, on the compile's 180-second limit; with
  the objects kept it takes 77 to 89 s, and compiler 2/8 took 279, 302 and
  296 s in three runs in a row on three hosts, against 433 s. An object comes
  from the bucket in 1 to 2 s (the compiler's is 120 MB, sent packed) and from the disk in
  0.2 s, and links to the same compiler bytes on every host. Held to two
  cores shared with four busy loops, the old build stopped at 180 s; the new
  one compiled the unit in 375 s and went on. With nothing kept, on eight
  cores shared with 16 busy loops (a new compiler's first run on a host far
  busier than any the pipeline has seen), `parser` passed in 1,493 s: 304 s
  for the compile, and 1,169 s for the sanitized compiler's first check of
  its own sources, 660 CPU seconds that no cache can skip the first time.

`remote.sh` mints a token for each job that can read and write this bucket and
nothing else (the automation account's token, exchanged at Google's STS for
one with a credential access boundary on the bucket; it lasts an hour), and
the host's job passes it to the command. A job can run longer than that, or
wait half an hour for a slot first, so `remote.sh` writes a new token to the
host a minute into the job and every 15 minutes after (`~/tokens/JOB`, which
`shared-cache.sh` reads first). Before this, a job past its token's hour
stopped writing to the bucket without a word: the first pages build of
`68fcf1a65e` ran 59 minutes and could not store its pages. The hosts have no service account,
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

The offline room's module (`everglade-room.wasm`, built by
`coax/web/room/worker.coax`, not `wasm.sh`) keeps the same kind of record,
`build/.c/everglade-room.built`, and is taken the same way. Its inputs are the
mode, each unit's `#load` closure by its files' hashes, and the C linked with
them; not the compiler's bytes, which differ between a host and the Mac.

Measured on 2026-11-10, `release.sh --ref 8df6f203ef --build-only` on the Mac,
the first commit with stored pages: the fetch took 10 s, and 49 of the 58
`wasm.sh` modules were taken prebuilt. Nine were built on the Mac: eight whose
inputs differ there (`project`, `projects`, `status`, `connect-pylon`, `admin`,
`audiolab`, `og`, `private`: they load tables the build writes from Nostr and
the release's own snapshot; 5 to 77 s each, side by side) and `realm`, which
has no record. The room module had no record yet and compiled `room.coax` and
`everglade.coax` on the Mac, most of the run's 27 minutes; with its record it
is taken in under a second.

With that, `release.sh --ref 68fcf1a65e --build-only` built in 746 s (860 s
with the compiler, the fetch and staging), down from about 27 minutes: the
same 49 modules prebuilt, the same nine built on the Mac in 4 to 42 s each.
The run used 224 s of CPU in those 746 s, so what is left is not compiling:
it is the build's serial steps and waits (the render packages one after
another, the CLI's build, the project snapshots from Nostr, the Mac's build
slots). The 2-minute target is not met; #13154 has the numbers.

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
stopped host, or creates the next one up to `OA_REMOTE_MAX_HOSTS` (16). A new
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

### The resident compiler (#13138)

Every `coax c`, `coax test` and `coax check` started from nothing: it read,
lexed and syntax-checked every unit of the `#load` closure, hashed each one
for the cache keys, and hashed the compiler binary. `coax serve` keeps that
work between commands.

**What it is.** One resident process per compiler binary and user. The
commands `c`, `check`, `test`, `lib`, `header`, `build`, `deps`, `layout` and
`checks-c` connect to it over a socket in `~/.cache/coax-serve/` (or
`$COAX_SERVE_DIR`), pass their arguments, directory, environment and standard
streams, and the resident forks a child that runs the command as a new
process would. The child inherits the resident's memo, and when the command
ends it sends back what it computed. The resident itself never runs a
compiler phase.

**What it keeps.** For each unit, by the unit's bytes (compared whole, not by
a hash): its tokens, that it passed the unit syntax check, the SHA-256 of its
bytes and the SHA-256 of its tokens. And the compiler binary's SHA-256. A
unit you edited is lexed again; the others are not. The parse, the type
check, the effect contracts and the C are still made for the whole program
on every compile.

**One resident serves every checkout.** Its directory is named by the
binary's content, so worktrees with the same compiler share one resident and
its memo, and a host's three slots do too. A resident leaves after 15 idle
minutes (`COAX_SERVE_IDLE`, seconds), when its binary is replaced or
deleted, or on `coax serve stop`. It keeps at most 4 GiB of unit lexes
(`COAX_SERVE_MEMORY_MB`), least recently used out first; fivelab's 294 units
take 160 MiB.

**Failure is safe.** With no resident, the command starts one and runs cold
(the resident never delays a command). A resident that refuses (the client is
another binary), doesn't answer in 5 seconds, or whose request dies, makes
the command run cold. A command whose client is killed is stopped too.
`COAX_SERVE=0` turns it off. `coax serve status` prints what a resident holds.

**The output is a cold compile's.** `coax/tests/serve.coax` (in
`tests/run.sh` as `serve`) compiles a three-unit `#authority` program and the
compiler's corpora cold and in a resident and compares stdout, stderr and the
exit status: after an edited line, an appended comment, a syntax error and
its repair, after the resident is killed, and after it is stopped. Given a
checkout and `UNIT:EDITED` pairs it does the same for real units, before and
after a comment and a new procedure in `EDITED`, comparing the C and the
header:

```sh
coax/remote/remote.sh run -- bash -c 'cd coax && web/tool.sh tests/serve.coax "$PWD/bin/coax" "$PWD/.." \
  apps/openagents-web/fivelab.coax:verse/experimental-dungeon.coax \
  apps/openagents-web/everglade2.coax:apps/openagents-web/everglade2-move.coax \
  apps/openagents-web/charactercreator.coax:verse/character-glade.coax'
```

The same change makes a cold compile cheaper: a large store the parser and
checker never touch is no longer cleared (the Mac's allocator cleared a
recycled block by writing all of it), the checker's node store is sized once,
each syntax node finds its unit by bisection, and the parser reads the
loader's tokens without copying them.

Measured 2026-11-10 with `coax c UNIT --library --header`, wall time, each
edit made two or three times. "First compile" is the first after the edit
(each check is keyed and reused); "again" is the same sources once more (the
whole pass is cached). The edited files are
`verse/experimental-dungeon.coax` (fivelab), `apps/openagents-web/everglade2-move.coax`
(everglade2) and `verse/character-glade.coax` (Character Create).

On a build host (oa-build-2 and oa-build-8, 30 cores, load 3 to 6). Before is
main at `9d847d9398`, which has #13131:

| Unit | State | Before | Now, `COAX_SERVE=0` | Now, in a resident |
| --- | --- | --- | --- | --- |
| fivelab | unedited, or again after an edit | 10.5 s | 9.6 s | 3.5 s |
| fivelab | a comment, first compile | 11.5 s | 10.6 s | 4.5 s |
| everglade2 | unedited, or again | 10.7 s | 9.6 to 9.8 s | 3.5 to 3.6 s |
| Character Create | unedited, or again | 7.4 to 7.5 s | 6.9 to 7.1 s | 1.6 to 1.7 s |

fivelab's first compile after a comment, by phase, on the host:

| Phase | `COAX_SERVE=0` | Resident |
| --- | --- | --- |
| load (read, lex and syntax-check each unit) | 4.9 s | 0.3 s |
| libraries (each library's verdict key) | 1.2 s | 0.1 s |
| parse | 0.6 s | 0.6 s |
| check | 1.7 s | 1.7 s |
| effect contracts | 0.2 s | 0.2 s |
| authority (key 1,314 checks; run none) | 1.35 s | 1.0 s |
| emit | 0.6 s | 0.6 s |
| Total | 10.6 s | 4.5 s |

On the Mac (18 cores, other agents' builds at load 12 to 15, down to 5 for
the last fivelab row). Before is main at `92578eb750`, before #13131:

| Unit | State | Before | Now, `COAX_SERVE=0` | Now, in a resident |
| --- | --- | --- | --- | --- |
| fivelab | unedited, or again | 5.1 s | 2.4 to 4.5 s | 1.3 to 2.3 s |
| fivelab | a comment, first compile | 9.1 to 9.9 s | | 1.6 to 2.8 s |
| fivelab | a new procedure and its check, first compile | | | 3.0 s |
| fivelab | a syntax error in the zone file | 1.75 s | 1.6 s | 0.17 s |
| fivelab | a type error in the zone file | 4.5 s | 3.2 s | 1.05 s |
| everglade2 | unedited, or again | 5.5 to 6.4 s | 3.1 to 4.0 s | 1.4 to 2.6 s |
| everglade2 | a comment, first compile | 17.7 to 22.8 s | | 13.6 to 14.9 s |
| Character Create | unedited, or again | 3.2 s | 2.3 to 3.5 s | 0.9 to 1.0 s |
| Character Create | a comment, first compile | 4.6 s | | 0.8 to 1.1 s |

everglade2's first compile after an edit runs 8 checks again whatever the
edit (they come near a storage limit, so their notes are never reused,
#13123): 11.6 to 13.1 of its 13.6 to 14.9 seconds.

The issue's target, a one-line edit of a fivelab zone file to C in under 3
seconds, was met on the Mac and not on a build host (4.5 s). What the host's
4.5 s held was work no cache keys by file: the type check of the whole
program (1.7 s), keying its checks (1.0 s), emitting its C (0.6 s) and
parsing it (0.6 s). The next section is what changed.

### One-line edits on a build host (#13142)

Three changes to the compiler, none to its output (`coax/tests/serve.coax`
compares a resident's compiles with cold ones, and the C of fivelab is the
C the compiler before them emits):

- **The compiler's own Coax at `-O3`.** The lexer, parser, checker and
  SHA-256 are Coax, compiled to C; the bootstrap compiled that C at `-O1`.
  On fivelab the parser takes 0.26 s at `-O1` and 0.10 s at `-O3`, and the
  checker 1.45 s and 0.54 to 0.60 s (1.07 s at `-O2`). The compiler's own C
  takes about five minutes longer to compile in a bootstrap.
- **Threads for work that is one item's alone.** Up to 16, and never more
  than the cores or `COAX_JOBS`: the parser's node copies (its objects and
  lists are laid out in node order first, so their places don't depend on
  the threads), the checker's input copy, each `#authority` check's reuse
  key, each procedure's effect-contract trace and each item's skeleton
  bytes. Every thread writes only its own slice. `COAX_BURST_THREADS=1`
  runs them inline. A check reuse record also names each procedure a check
  ran once, where it copied and looked up every name for every check.
- **The changed bodies, checked first.** A resident remembers each compile
  that passed the type check as a skeleton: every top-level item's tokens
  outside its procedure body, and each body's tokens, compared whole (at
  most two for an entry unit and six entry units; fivelab's is 30 MB). When
  the next compile differs only inside the bodies of procedures or checks, a forked
  child parses the program with every other body emptied (a tenth of
  fivelab's tokens) and checks that. If it finds errors in the changed
  bodies they are the whole check's errors, and the compile reports them and
  stops. If it finds none, the compile checks the whole program as before.
  The compile asks for the answer when its tokens have parsed, when its tree
  is made, and once more before the checker runs, waiting up to a tenth of a
  second that last time (`COAX_SKELETON_WAIT_MS`).
  The rules that make the child's errors the whole check's are in
  INVARIANTS.md ("A changed body's errors are reported alone only when they
  are the whole check's") and the book, chapter 46. `COAX_SKELETON=0` turns
  it off; `coax serve status` counts the compiles it answered.

Measured 2026-11-10 on a build host (oa-build-1, 30 cores, load 5 to 13),
fivelab to C with `coax c UNIT --library --header`, in a resident, wall
time. Each edit was made four times. The compiler before this change
(main's published build) and the one with it, built with the release
toolchain, compiled each edit by turns, each in its own resident, so both
saw the same machine load. Each emitted the same C:

| Edit of `verse/experimental-dungeon.coax`, first compile | Before | Now |
| --- | --- | --- |
| a comment | 4.6 to 5.3 s | 2.5 to 2.6 s (one of four 3.5 s, while the load spiked and the other took 5.3 s) |
| one line of a procedure's body | 4.6 s | 2.5 s |
| a type error in a procedure's body | 2.3 s | 0.63 s |
| a type error in a new procedure (more than a body changed: the whole check) | 2.3 s | 1.0 s |

The first compile after a comment, by phase (`--timings-table`; before on
oa-build-2 at load 3 to 6, now on oa-build-1 at load 5 to 13):

| Phase | Before | Now |
| --- | --- | --- |
| load | 0.32 s | 0.29 s |
| imports, libraries, evaluate | 0.16 s | 0.13 s |
| parse | 0.60 s | 0.24 s |
| check | 1.77 s | 0.70 s |
| effect contracts | 0.23 s | 0.21 s |
| authority (key 1,319 checks; run none) | 1.04 s | 0.21 s |
| emit | 0.63 s | 0.59 s |
| Total | 4.77 s | 2.40 s |

`coax/tests/serve.coax` with the bootstrapped compiler, on oa-build-10: a
type error put in a procedure of a loaded file, against a cold compile's
stderr.

| Unit | Cold | In a resident |
| --- | --- | --- |
| fivelab | 3.0 s | 0.7 s, from the changed body's check |
| everglade2 | 3.1 s | 0.7 s, from the changed body's check |
| Character Create | 2.2 s | 0.7 s, from the whole check (its edited file checks alone as a library, so its own checks come back into the compile) |

What is left on a compile that passes is the type check of the whole
program (0.7 s), emitting its C (0.6 s) and loading and parsing it (0.5 s).
The checker has to take a previous typed graph and the changed declarations
before the first falls, and the emitted C needs unit-relative trap lines
before a declaration's C can be kept; see the book, chapter 46. A profile of the check (callgrind, the
`-O2` build) puts a third of the checker's instructions in the borrow and
retain analysis (2.2 of 6.3 billion on fivelab).

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

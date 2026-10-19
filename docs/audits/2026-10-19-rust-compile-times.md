# Rust compile times: where the time goes and what to change

Recorded 2026-10-19 at `037cf47ad8`; the changes landed as `b69abe5197`,
`7194a6242e` and `8b172f3570`. This audit measures the three loops
that wait on the Rust compiler: an agent's edit-to-test loop, the land
queue's checks, and deploy and host builds. It ranks the options to shorten
them, records the changes made today with before and after numbers, and
files the larger items as issues.

## Summary

- **The edit loop is where the hours go.** `main` took 2,379 commits in the
  week to 2026-10-19. A one-line change to `openagents-web` cost 92.6 s to
  rebuild and 119.2 s to build its tests on a 22-vCPU Linux VM; `coder`'s test
  build took 284.8 s. At three test builds per commit, the four hottest root
  packages cost agents about 70 hours of waiting a week.
- **Three changes landed today** (#12178), and the same one-line change now
  costs: `openagents-web` **17.3 s** to rebuild (−81%) and **22.0 s** to build
  its tests (−82%); `coder` tests **92.3 s** (−68%); `openagents-cli` 10.7 s
  (−66%); the desktop app's `engine` tests **121.9 s** instead of 467.2 s
  (−74%). The same four packages now cost about 21 hours a week, a saving of
  about 49 hours.
  1. Incremental compilation for the large leaf crates, with every other
     crate still cached by kache. AGENTS.md's `CARGO_INCREMENTAL=0` rule is
     replaced (owner, 2026-10-19).
  2. `debug = "line-tables-only"` in dev and test. Full debug information
     made each Linux test binary about 1.2 GB and one target 246 GB.
  3. On the Mac, kache stops storing executables. It ran `dsymutil` on each
     one, 40 to 51 s for the largest; a `coder-host` test build went from
     36.8 s to 20.4 s.
- **On the Mac with kache**, `openagents-web` rebuilds in 19.1 s instead of
  106.0 s and builds its tests in 29.4 s instead of 169.7 s; `openagents-cli`
  rebuilds in 10.2 s instead of 137.9 s.
- **Linkers don't matter here.** Rust already links with `rust-lld` on
  Linux; mold changed results by 0 to 3%, so it isn't added.
- **What's left is structural**, and filed: crates too large for one
  compiler thread (#12184), too many test binaries and an SDK that compiles
  the website for its tests (#12199), the Verse stack inside the website
  (#12183), per-command feature sets (#12185), land checks that find compile
  errors after 14 to 38 minutes (#12198), and the opt-in nightly parallel
  front end (#12186, owner decision). Other chats landed the shared remote
  cache (#12176) and deploy image caching (#12177) the same day; a web image
  build fell from 858 to 1,288 s to 499 to 573 s.
- **Don't rewrite.** Changing language or build system would cost far more
  than these fixes and lose shared type safety; see
  [The extreme options](#the-extreme-options).

## How we measured

| Machine | What ran there | Notes |
| --- | --- | --- |
| `oa-ca-bench-1`, `oa-ca-bench-2` | Root workspace, Linux | Two new c3-standard-22 VMs (22 vCPUs, 86 GB, 500 GB pd-balanced), Debian 13, Rust 1.97.1, nothing else running. Deleted after the runs. |
| `oa-dev-env-1` | Root workspace, Linux, first pass | The land-queue integrator's machine (same type). Its numbers overlap another agent's builds for #12176, so this audit quotes them only where the clean VMs agree. |
| The owner's Mac | Root and `apps/openagents` workspaces, macOS | 18 cores, 128 GB. Other agents were building at the same time: load 7 to 23 for the "before" runs and 40 to 120 for the "after" runs, so Mac numbers carry far more noise than Linux ones, and the Mac A/B comparisons alternate the two settings on one target. |

`scripts/dev/build-bench.sh` (added today) ran every measurement. For each
package it builds once to warm the target, builds again with nothing
changed (no-op), then adds one private function to the package's root
source file and builds again (edit). It does this for `cargo build -p P`
and for `cargo test -p P --no-run`. It loads `scripts/dev/cargo-build-env.sh`
first, so the source-keyed build directory applies, as it does for every
agent. "Before" rows use the rules of the morning of 2026-10-19
(`CARGO_INCREMENTAL=0`, full debug information; the script's
`noincr+full` variant); "after" rows use the repository's settings after
#12178. Linux runs used 11 jobs (what one land-queue
slot gets) and no compile cache (`--no-wrapper`), so they time the compiler
itself. Mac runs used 7 jobs (the Mac's `~/.cargo/config.toml`) and kache,
as agents do. Each variant had its own empty target directory.

The packages are the ones agents changed most in the week to 2026-10-19
(2,379 commits on `main` in seven days):

| Package | Commits touching it, 7 days | Rust lines | Crates in its build (macOS host) | Of which ours |
| --- | ---: | ---: | ---: | ---: |
| `openagents-web` | 299 | 184,049 | 821 | 149 |
| `apps/openagents` `mobile` | 185 | 39,348 | 753 | 68 |
| `openagents-cli` | 111 | 115,479 | 962 | 138 |
| `coder` | 102 | 280,920 | 556 | 77 |
| `apps/openagents` `engine` | 102 | 120,144 | 750 | |
| `openagents-sdk` | 96 | 22,133 | 118 | 11 |
| `apps/openagents` `ui` | 74 | 235,010 | 1,076 | |
| `apps/openagents` `client` | 39 | 16,568 | 429 | |
| `coder-sync` | 29 | | 422 | 45 |
| `nostr` | 27 | 91,368 | 36 | 2 |
| `coder-host` | 15 | 33,123 | 359 | 40 |


## Where the time goes

### The edit loop: one change, then `cargo build -p` and `cargo test -p`

Before today's changes. Linux: clean VM, 11 jobs, no cache. Mac: 7 jobs,
kache, other agents building at the same time. Times in seconds; "edit" is
the time after a one-line change.

| Package | Linux build | Linux test | Mac build | Mac test |
| --- | ---: | ---: | ---: | ---: |
| `openagents-web` | 92.6 | 119.2 | 106.0 | 169.7 |
| `coder` | 61.2 | 284.8 | 85.5 | 203.2 |
| `openagents-cli` | 31.8 | 33.7 | 137.9 | 89.6 |
| `openagents-sdk` | 5.8 | 156.5 | 3.0 | 134.4 |
| `coder-host` | 10.7 | 21.7 | 6.7 | 48.0 |
| `nostr` | 11.5 | 16.1 | 6.7 | 12.2 |
| `coder-sync` | 2.9 | 5.7 | 1.9 | 7.2 |
| `apps/openagents` `engine` | | | 22.4 | 415.8 |
| `apps/openagents` `ui` | | | 27.2 | 384.5 |
| `apps/openagents` `mobile` | | | 14.8 | 37.4 |
| `apps/openagents` `client` | | | 4.9 | 13.6 |

A no-op build took 0.5 to 6 s everywhere, with one exception: on the Mac,
`cargo build -p openagents-mobile` rebuilt for 346.8 s with nothing changed
(see [Open questions](#open-questions)).

Five causes account for nearly all of it.

1. **No incremental compilation.** AGENTS.md required
   `CARGO_INCREMENTAL=0` for every agent build, so a one-line change
   recompiled the whole crate: all 184,049 lines of `openagents-web`, all
   280,920 lines of `coder`. With incremental compilation on, the same edit
   rebuilt `openagents-web` in 26.6 s instead of 92.6 s, `coder` in 23.7 s
   instead of 61.2 s, and `openagents-cli` in 13.4 s instead of 31.8 s.
2. **Full debug information, linked into every test binary.** The dev and
   test profiles had no `debug` setting, so every crate carried full DWARF
   and every test executable linked all of it. The Linux target for the
   seven packages grew to **246 GB**; 142 executables over 100 MB held
   168 GB, about 1.2 GB each. A test build of `coder` links 35 integration
   test binaries and 8 examples, so it was link-bound: a load average of 43
   on 22 vCPUs, mostly `rust-lld`. Line tables only cut that test build from
   284.8 s to 99.5 s. macOS splits debug information out of the link by
   default, which is likely why the Mac `coder` test build was faster than
   Linux.
3. **One thread per crate.** rustc's front end is single-threaded on stable.
   `-Ztime-passes` on the `openagents-web` library: 79.8 s in all, of which
   the monomorphization collector took 31.6 s and metadata generation
   33.6 s (they overlap), type checking 13.3 s, borrow checking 10.1 s,
   coherence 7.9 s, and LLVM 16.8 s. The nightly parallel front end
   (`-Zthreads=8`) cut that compile to 58.0 s, and `coder`'s from 51.2 s to
   26.8 s.
4. **Many test binaries per package.** `openagents-cli` has 49 integration
   test files, `apps/openagents` `engine` 49, `coder` 35, `openagents-sdk`
   21, and `coder-host` 16. Each is its own crate that compiles against the
   edited library and links the whole dependency graph. That is why an
   `openagents-sdk` test build took 156.5 s when its library rebuilt in
   5.8 s, and why `engine` and `ui` test builds took over six minutes on the
   Mac. Merging 31 of `coder`'s 35 test files into one binary cut its test
   build from 284.8 s to 136.0 s with full debug information, and from 99.5 s
   to 70.7 s with line tables. `openagents-sdk` gained little (145.1 to
   140.8 s): it dev-depends on `openagents-web`, so each SDK test build
   recompiles the website.
5. **Feature unification.** Cargo resolves features per command. After one
   cold build of all seven packages (342 s), `cargo build -p openagents-web`
   alone took another 264.5 s, `-p openagents-cli` 154.9 s, and `-p coder`
   118.5 s, rebuilding dependencies with a different feature set. The Mac
   numbers with kache were similar (274.7, 262.9, 166.1 s).

### Cold builds: a new worktree or a new machine

| Build | Wall time | CPU time | Notes |
| --- | ---: | ---: | --- |
| Seven root packages, Linux, no cache | 342.4 s | 3,092 s | 1,559 units. Our 204 crates are 1,670 CPU-s (54%). |
| Same, Mac, fresh target, kache warm | 489.0 s | | What a new worktree costs today. |
| Four app packages, Mac, fresh target, kache warm | 822.9 s | 5,379 s | 1,801 units, 68% of CPU in code generation |

- **The Verse stack is in the website.** `openagents-web` depends on
  `verse` for a few CAD physics types and spawn points, which pulls in
  `verse-world` (184 s, 171 s of it code generation), `verse-zone-everglade`
  (107 s), `verse-content` (97 s), `verse-pbr` (78 s), `verse`, `verse-engine`,
  `physics`, `wgpu-core`, `wgpu-hal` and `naga`. They are at `opt-level = 3`
  in dev, about 750 CPU-s, 24% of the cold build. Verse changed in hundreds
  of commits in two weeks, and each one also recompiles the website.
- **The app optimizes our domain crates.** `apps/openagents` sets
  `[profile.dev.package."*"] opt-level = 2` for GPUI's sake, and `"*"` also
  covers the root crates the app links by path: `coder-host` (138 s),
  `openagents-chat` (124 s), `coder-access` (95 s), `nostr` (81 s), `pylon`
  (62 s), `coder-delegate` (62 s).
- **Closures are large.** `openagents-cli` builds 962 crates (138 ours),
  `openagents-web` 821 (149 ours), `coder` 556 (77 ours). `cargo tree -d`
  shows 151 crate names in more than one version across all platforms,
  including five `itertools` and five `hashbrown`.

### kache on the Mac

`kache report --since 7d` on 2026-10-19:

| Measure | Value |
| --- | --- |
| Hit rate by count | 45.8% (10,901 of 23,788) |
| Hit rate weighted by compile time | **7.1%** |
| Compile work avoided | about 1.2 hours |
| Compile work on misses | about 15.6 hours |
| Store | 100.2 GiB logical against a 30 GiB cap (334%), 32.7 GiB of blobs |

The crates that cost the most are ours, and they change, so they miss. The
largest misses were `verse_world`, `openagents_chat`, `openagents_web` (560
MB per entry), `loro_internal`, `breez_sdk_spark`, `coder_host`, and the
app's `openagents_ui` (633 MB per entry, from full debug information). A
cache helps a new worktree; it can't help the crate you just edited. The
shared remote cache (#12176) extends the same benefit across machines.

### The land queue

`openagents land status --all` and `land show` on 2026-10-19 (the listing
keeps about 90 entries, from 2026-10-10 on):

| Code entries | Count | Median | 75th percentile | 90th percentile | Longest | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| All | 47 | 13 min | 27 min | 46 min | 81 min | 14.7 h |
| Landed | 19 | 4 min | 21 min | 36 min | 62 min | 4.1 h |
| Bounced | 28 | 20 min | 34 min | 66 min | 81 min | 10.5 h |

- **Bounced entries used 71% of the check time.** Of the 28 bounces, 11
  failed to compile, often after 14 to 38 minutes of building and testing
  (`20261011T005633Z-...-06be`: 38 min), 5 had formatting differences (13
  entries had one or both), and 3 could not fetch GPUI. `cargo check
  --tests` after a one-line edit took 41.7 s for `openagents-web`, 35.9 s
  for `coder` and 76.2 s for `openagents-sdk`: a fraction of their test
  builds, and enough to catch every one of those compile errors.
- **Rebases repeat whole test runs.** The 62-minute entry
  (`20261018T213336Z-...-bbbc`) ran the `openagents-sdk` and
  `openagents-web` tests three times, because `main` moved by 41 and then 26
  commits that touched their dependencies while the checks ran.
- Each slot builds into its own target with 11 jobs and sccache, and the
  entries' different `-p` sets hit the feature-unification cost above. One
  slot ran out of disk on 2026-10-19 (`No space left on device` in
  `coder`'s incremental folder).

### Deploys and host upgrades

| Build | Runs, 7 days | Median | 90th percentile | Total |
| --- | ---: | ---: | ---: | ---: |
| Web image (`crates/openagents-web/cloudbuild.yaml`) | 81 | 12.3 min | 17.2 min | 16.7 h |
| Staging stack image (`deploy/staging/cloudbuild.yaml`) | 81 | 6.3 min | 8.8 min | 9.4 h |

The steps of web build `c68b0d8d` (2026-10-19, 14 min 18 s, E2_HIGHCPU_32):

| Stage | Time | Notes |
| --- | ---: | --- |
| `cargo build --release -p openagents-web -p cad-lab` | 610 s | From nothing: no cache, no saved `target/`. 300 s on 2026-10-09. |
| Everglade WebAssembly chain | about 800 s in sequence | `build-everglade-web.sh` 306 s, robot 108 s, components 92 s, LayerTree 79 s, then the smaller games |
| CAD WebAssembly (`cadweb` stage) | 373 s | |
| `wasm-bindgen-cli` and `wasm32` target, twice | 38 s each | Compiled from source each build |

The stages run in parallel on 32 vCPUs and compete for them. The native
release build doubled in ten days, and nothing was reused between builds.
Two other chats fixed that the same day:

- **#12177** (`0a67be2c2d`): each image builds its dependencies first against
  a manifest-only skeleton, keeps that layer in a registry cache, and runs
  every Cargo command under kache with the shared remote. Cloud Build time
  for no change and for a one-line change: web 858 to 1,288 s became 573 and
  499 s; the stack image 379 to 551 s became 142 to 195 and 251 s;
  coder-sync 364 to 475 s became 80 and 79 s. A build with both caches empty
  is slower (web 2,212 s).
- **#12176** (`7b04d3f328`): the shared kache remote in Cloud Storage. A warm
  remote cut a cold release build of `coder` from 392 s to 235 s on
  `oa-dev-env-1`, but made debug builds slower, partly because debug
  artifacts were large (`coder`'s library was 670 MB; it's 346 MB with line
  tables) ([remote-build-cache.md](../coder/runtime/remote-build-cache.md)).
  With today's smaller debug artifacts, its debug measurement is worth
  repeating.

Host upgrades of the desktop app on the Mac (`host-upgrade/upgrades.jsonl`,
nine installs from 2026-10-17 to 2026-10-19) took 1.2 to 32.4 minutes, with
the build 52 s to 29 min 51 s. The 9 min 16 s build on 2026-10-19 compiled
only 12 crates: the chain `openagents-harness`, `openagents-mcp`,
`openagents-engine`, `openagents-ui`, `openagents` runs one after another
at `opt-level = 2` in `dev-opt`.

### How much time this is

With 2,379 commits on `main` in a week, the edit loop dominates. Assuming
three test builds per commit in the package it touches (a conservative guess
for an agent's edit, test, fix cycle), the week's commits to the four
heaviest packages cost about:

| Package | Commits, 7 days | Test build before | Hours | Test build after | Hours |
| --- | ---: | ---: | ---: | ---: | ---: |
| `openagents-web` | 299 | 119.2 s | 29.7 h | 22.0 s | 5.5 h |
| `coder` | 102 | 284.8 s | 24.2 h | 92.3 s | 7.8 h |
| `openagents-sdk` | 96 | 156.5 s | 12.5 h | 79.5 s | 6.4 h |
| `openagents-cli` | 111 | 33.7 s | 3.1 h | 12.2 s | 1.1 h |
| **Total** | | | **69.5 h** | | **20.8 h** |

Linux clean-VM times; the Mac was slower before (it also ran `dsymutil` on
every test binary). The app's `engine` alone, 102 commits at 467.2 s per
Linux test build, was another 39.7 hours; at 121.9 s after, 10.4 hours.

The land queue spent 14.7 hours on checks in the listed entries, and Cloud
Build spent 26.1 hours on the two images in a week before #12177.


## Options, ranked

Ordered by measured or expected effect on the edit loop, then on the land
queue and deploys. None of these options weakens type safety.

| # | Option | Measured or estimated effect | Effort | Risk | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | **Incremental compilation for the large leaf crates** (`[profile.dev] incremental = false` plus a per-package list; the build helper stops forcing `CARGO_INCREMENTAL=0`; kache `preserve_incremental = true`) | `openagents-web` edit: build 92.6 to 26.6 s, test 119.2 to 52.9 s. `openagents-cli` 31.8 to 13.4 s. | Small | Low. Incremental state stays in the per-worktree build directory; the #12071 regression test now covers it. Listed crates aren't cached by kache. | **Done today** |
| 2 | **Line tables only** for dev and test (`debug = "line-tables-only"`) | `coder` test build 284.8 to 99.5 s; cold build 342 to 278 s. The seven packages' Linux target: 246 GB before, 104 GB after, incremental state included | Trivial | Low. Panics and backtraces keep file and line; a debugger session needs `--config 'profile.dev.debug="full"'`. | **Done today** |
| 3 | **Linker**: mold on Linux, lld on macOS | Linux: mold against Rust's default `rust-lld`, 0 to 3% (`openagents-web` build 80.3 to 77.9 s, test 101.4 to 100.9 s). Mac: one `coder-host` test link took 1.3 to 1.8 s with Apple's ld and 2.4 to 3.6 s with lld | Trivial | Low to medium: every builder needs the linker installed | **Measured; not adopted** |
| 4 | **Fewer test binaries**: one integration-test crate per package (`tests/all/main.rs` with modules) | `coder` test build 99.5 to 70.7 s with line tables (284.8 to 136.0 s with full debug information); `openagents-sdk` needs its dev-dependency on the website removed first | Medium | Medium: tests that rely on process isolation (environment, current directory, globals) must be fixed first | #12199 |
| 5 | **Shared remote compile cache** (GCS) for every builder | Cold release `coder` 392 to 235 s; debug builds were slower with it | Medium | Medium | #12176, landed; on for release builders |
| 6 | **Deploy image caching** (dependency layers in a registry cache, the shared cache in Cloud Build) | Web image 858 to 1,288 s to 499 to 573 s; coder-sync 364 to 475 s to 79 to 80 s | Medium | Medium | #12177, landed |
| 7 | **Land queue fail-fast stage**: `cargo fmt --check` and `cargo check --tests` before building tests | 13 of 28 bounces had a compile error or a formatting difference, found after up to 38 minutes; `cargo check --tests` takes 36 to 76 s | Small | Low | #12198 |
| 8 | **Drop Verse from `openagents-web`** (move the CAD physics types to a small crate) | About 750 CPU-s of every cold web build, and Verse edits stop recompiling the site | Small | Low | #12183 |
| 9 | **Split the god crates** (`openagents-web`, `coder`, the app's `ui`) | Front end parallelism on stable; an edit recompiles one piece. Bounded by the nightly result: up to 28 to 48% of a crate's compile | Large | Medium | #12184 |
| 10 | **One feature set** (`cargo-hakari` workspace-hack) | Removes the 118 to 265 s rebuilds when the `-p` set changes | Small to medium | Low to medium: unified features may enable code a crate didn't use | #12185, landed after these measurements (`2796c12a25`); not re-measured here |
| 11 | **Nightly parallel front end** (`-Zthreads=8`), opt-in for dev | `openagents-web` lib 81.0 to 58.0 s, `coder` 51.2 to 26.8 s; with Cranelift 50.1 and 19.9 s | Small | Medium: a nightly compiler for dev builds | #12186, owner decision |
| 12 | **kache without stored executables on macOS** (`cache_executables = false`) | `coder-host` test build 36.8 to 20.4 s, `coder` 152 to 155 s to 105 to 116 s | Trivial | Low: a new worktree relinks executables instead of restoring them | **Done today** |
| 13 | **App dependency optimization**: `opt-level = 1` (or 0) for the root domain crates the app links by path, keeping 2 for GPUI | Up to about 600 CPU-s of the app's cold build (`coder-host`, `openagents-chat`, `coder-access`, `nostr`, `pylon`, `coder-delegate`) | Trivial | Medium: run-time speed of the dev app (#12152) | In this audit; measure before changing |
| 14 | **Cranelift backend for dev**, whole graph | 13 to 17% per crate alone (above); more on code-generation-heavy crates | Small | Medium: nightly only; slower run time; some crates (inline assembly, SIMD) need LLVM | Part of #12186 |
| 15 | **cargo-nextest** | No build-time change; runs test binaries in parallel processes | Small | Low | Not filed: test run time was not measured here |
| 16 | **Prebuilt GPUI as a dylib, or hot reload (`subsecond`)** | A GPUI view edit could reload in seconds | Large | High: dylib ABI ties to one compiler; hot reload is experimental in GPUI | Not recommended now |
| 17 | **Bazel, Buck2, or Nix with remote execution** | Remote execution and caching for every target | Very large | High: a second build system for 270 crates, two workspaces, GPUI's git dependencies and build scripts | Not recommended; #12176 gets most of the caching benefit |

## What changed today

All three changes are on `main`:

| Commit | What |
| --- | --- |
| `b69abe5197` | `scripts/dev/build-bench.sh`, the benchmark used here |
| `7194a6242e` | `debug = "line-tables-only"` and a list of incremental leaf crates in both workspaces' `[profile.dev]`; `cargo-build-env.sh` and both `coder-lease` cargo shims unset `CARGO_INCREMENTAL` instead of forcing 0; AGENTS.md, `leases.md` and `kache.md` updated; the #12071 two-worktree test now compiles the differing crate incrementally |
| `8b172f3570` | kache on macOS: `cache_executables = false`, documented beside `preserve_incremental = true` |

Two settings live in the Mac's `~/.config/kache/config.toml`, not in the
repository: `preserve_incremental = true` (without it kache strips Cargo's
`-C incremental` flag and caches the crate instead) and
`cache_executables = false`. Both are set on the owner's Mac; other kache
machines need them too. sccache, on the GCE pool hosts, already passes
incremental compiles through.

**Why only leaf crates.** An incremental crate isn't cached, and its output
isn't byte-identical between worktrees (measured: the same two-crate
workspace in two folders gave different `.rlib` and `.rmeta` files for the
incremental crate and identical files for the cached one). So a crate on the
list makes its dependents miss kache in every new worktree. The list holds
only crates whose workspace dependents are all on it:

- Root: `openagents-web`, `openagents-cli`, `coder-new`, `gateway`,
  `openagents-desktop`, `oa-computers`, `openagents-ui`, `microcoder`,
  `coder-sync`.
- `apps/openagents`: `openagents`, `openagents-ui`, `openagents-engine`,
  `openagents-mcp`, `openagents-harness` (the chain a host upgrade waits on),
  `openagents-mobile`, `openagents-client`.

`coder`, `openagents-sdk` and `nostr` stay off: they have 26, 46 and 145
workspace dependents. Incremental compilation of every crate would have cut
a `coder` edit to 23.7 s, but cost every new worktree the cached builds of
the crates above it.

Nothing changes for release builds, deploys, the mobile profiles, or any
script that sets `CARGO_INCREMENTAL=0` itself.

### Before and after

Linux, clean VM, 11 jobs, no cache; seconds after a one-line edit:

| Package | Command | Before | Line tables | Incremental (all crates) | After (both, as committed) | Change |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Seven packages, cold | `cargo build` | 342.4 | 278.2 | 368.2 | **281.2** | −18% |
| `openagents-web` | `cargo build` | 92.6 | 80.3 | 26.6 | **17.3** | −81% |
| `openagents-web` | `cargo test --no-run` | 119.2 | 101.4 | 52.9 | **22.0** | −82% |
| `coder` | `cargo build` | 61.2 | 50.1 | 23.7 | **48.3** | −21% |
| `coder` | `cargo test --no-run` | 284.8 | 99.5 | 288.4 | **92.3** | −68% |
| `openagents-cli` | `cargo build` | 31.8 | 24.0 | 13.4 | **10.7** | −66% |
| `openagents-cli` | `cargo test --no-run` | 33.7 | 25.5 | 16.6 | **12.2** | −64% |
| `openagents-sdk` | `cargo build` | 5.8 | 5.6 | 2.3 | **5.4** | −7% |
| `openagents-sdk` | `cargo test --no-run` | 156.5 | 145.1 | 45.8 | **79.5** | −49% |
| `coder-host` | `cargo build` | 10.7 | 8.6 | 2.5 | **7.9** | −26% |
| `coder-host` | `cargo test --no-run` | 21.7 | 14.9 | 12.5 | **14.0** | −35% |
| `nostr` | `cargo build` | 11.5 | 10.2 | 3.5 | **9.8** | −15% |
| `nostr` | `cargo test --no-run` | 16.1 | 14.8 | 4.8 | **14.3** | −11% |
| `coder-sync` | `cargo build` | 2.9 | 2.4 | 1.2 | **1.1** | −62% |
| `coder-sync` | `cargo test --no-run` | 5.7 | 4.2 | 3.2 | **2.2** | −61% |

The no-op build stayed at 0.7 to 3.3 s. Incremental compilation alone did
nothing for `coder`'s test build (288.4 s): its 35 test binaries were
link-bound until line tables removed most of what the linker copied.
Incremental, line tables and mold together gave `coder` 67.1 s and
`openagents-web` 16.5 and 21.1 s; mold contributed 0 to 3% of that.

The Mac, kache, 7 jobs. Load averaged 7 to 23 before and 40 to 120 after, so
treat the crates that weren't made incremental as unchanged within noise:

| Package | Command | Before | After | Change |
| --- | --- | ---: | ---: | ---: |
| `openagents-web` | `cargo build` | 106.0 | **19.1** | −82% |
| `openagents-web` | `cargo test --no-run` | 169.7 | **29.4** | −83% |
| `openagents-cli` | `cargo build` | 137.9 | **10.2** | −93% |
| `openagents-cli` | `cargo test --no-run` | 89.6 | **14.3** | −84% |
| `coder-sync` | `cargo test --no-run` | 7.2 | **2.8** | −61% |
| `coder` | `cargo test --no-run` | 203.2 | 174.9 | within noise |
| `openagents-sdk` | `cargo test --no-run` | 134.4 | 107.3 | within noise |

The Mac's `openagents-cli` was slow before because kache ran `dsymutil` on
the 870 MB `openagents` binary each time it stored it (51.5 s when timed on
its own). Incremental crates pass through kache, so they skip that step, and
`cache_executables = false` now skips it for the rest. Alternating the
setting on one target, three rounds and two rounds:

| Test build after an edit | kache stores executables | It doesn't |
| --- | ---: | ---: |
| `coder-host` (16 test binaries) | 35.8, 37.5, 37.2 s | **21.3, 19.9, 20.1 s** |
| `coder` (35 test binaries and 8 examples) | 151.9, 154.7 s | **104.9, 115.5 s** (at a higher load) |

`apps/openagents` on Linux (Rust 1.98.1, clean VM, 11 jobs, no cache).
`ui` and `openagents` need GPUI's window-system libraries and were measured
on the Mac only:

| Package | Command | Before | After | Change |
| --- | --- | ---: | ---: | ---: |
| Four packages, cold | `cargo build` | 882.2 | **708.0** | −20% |
| `openagents-engine` | `cargo build` | 39.2 | **6.5** | −83% |
| `openagents-engine` | `cargo test --no-run` | 467.2 | **121.9** | −74% |
| `openagents-mobile` | `cargo build` | 34.2 | | |
| `openagents-mobile` | `cargo test --no-run` | 51.0 | | |
| `openagents-client` | `cargo build` | 10.3 | | |
| `openagents-client` | `cargo test --no-run` | 14.1 | | |

The "after" run for `mobile` and `client` was stopped before it reached them
to free capacity; their crates are on the incremental list, like `engine`.
The before target was 189 GB.

### openagents-web without the Verse stack (#12183)

Issue: [#12183](https://github.com/OpenAgentsInc/openagents/issues/12183).

`openagents-web` used `verse` for the CAD physics it runs for placed models
(`crates/openagents-web/src/cad_web/world_physics.rs`) and `verse` plus
`verse-engine` to build the `/lander3` scene. That pulled `verse-world`,
`verse-zone-everglade`, `verse-content`, `verse-pbr`, `verse` and
`verse-engine`, all at `opt-level = 3`, into every web build, and every
Verse commit recompiled the site.

#### What moved

- `crates/cad-world` (glam, serde, serde_json, sha2, bytemuck, physics) now
  holds the demolition `Site`, the CAD rigs and their sync messages
  (`cad_physics`), the placements (`cad_place`, without the zone-file
  poller), the Everglade ground (`everglade`, `everglade_water`), Reverse
  Gravity and Wind Wall, `Footprint`, `box_hit`, `LitVertex`, and the
  meteor zones' arrival points. The files moved unchanged apart from their
  paths; `verse`, `verse-world`, `verse-pbr` and `verse-zone-everglade`
  re-export each under its old path. The sledgehammer and Meteor Swarm
  `Target` trait stays in Everglade, since it reads the zone's sight rules.
- `/lander3`'s scene builder moved to `verse::grid_scene`, which bakes
  `assets/verse/grid/lander3-scene.json`. The site embeds that file. A
  `verse` test fails when the file is stale;
  `UPDATE_FIXTURES=1 cargo test -p verse --lib grid_scene` rewrites it.

#### Measurements

`cargo tree -p openagents-web` (normal and build edges, distinct package
versions) at `e00039ebda` and with this change:

| | Before | After |
| --- | ---: | ---: |
| Package versions | 912 | 860 |
| `verse`, `verse-engine`, `verse-world`, `verse-zone-*`, `verse-content`, `verse-pbr`, `verse-core`, `verse-gfx` | in the tree | none |
| `wgpu`, `wgpu-core`, `wgpu-hal`, `naga` | in the tree | none |

The only crates named `verse-*` still in the tree are `verse-lagrange` and
`verse-lagrange2`, which `ship-lab` uses for the spacecraft model. They depend
on glam, serde and physics only.

The cold build was timed by the earlier Codex run on this issue, on an
equivalent split that was never merged (`a09123d6ea`, before the
cargo-hakari change #12185): an Apple M5 Max, Rust 1.97.1, eight jobs,
incremental compilation and the compiler cache off,
`scripts/dev/build-bench.sh --cold --only build --variant noincr --no-wrapper --jobs 8 openagents-web`,
single samples on a shared machine.

| | Before | After |
| --- | ---: | ---: |
| Cold `cargo build -p openagents-web`, seconds | 592.2 | 381.6 |
| Compiled units | 1,073 | 999 |

This change wasn't timed again from cold.

#### wgpu and naga through the workspace-hack

Since #12185, `wgpu`, `wgpu-core`, `wgpu-hal` and `naga` also reached
`openagents-web` through `openagents-workspace-hack` (by way of `acp-client`
and others), not only through Verse. `.config/hakari.toml` now leaves
`wgpu`, `wgpu-core`, `wgpu-hal`, `wgpu-types` and `naga` out of the hack's
traversal, so only the crates that render build them.

The Docker native stage wasn't timed here either; it's in the master
delegation's after-close checks.

## The extreme options

The owner asked whether to rewrite, change language, or change patterns.
Measured against what the cheap changes bought:

| Option | What it would buy | What it would cost | Recommendation |
| --- | --- | --- | --- |
| **Rewrite services in Go** (for example the website or the gateway) | Go compiles `openagents-web`-sized services in seconds | A second language for product code, losing the shared Rust domain crates (Nostr, tenancy, the SDK) the site links today; the website alone is 184,049 lines | No. The cost is in how the crates are shaped and built, not in Rust: the website now rebuilds in 17.3 s, and a split (#12184) brings it lower. |
| **Zig for some subsystems** | Fast builds, simple cross-compilation | No ecosystem for our async, HTTP, Nostr and CRDT needs; a third language | No. |
| **TypeScript** | | AGENTS.md forbids it for product code | Owner decision only; not recommended. |
| **A scripting layer for fast-changing UI or policy** (Rhai, Lua, or WebAssembly components through the `wasmtime` we already ship) | Copy, layout tweaks and policy tables change without a Rust build | Loses compile-time checks at the boundary; a second runtime to secure | Partly. Move content out of the binary first (docs, legal text, copy, the answer bank; the [2026-10-09 deploy audit](../deployment/2026-10-09-faster-deploys.md) planned this), which needs no new runtime. Keep logic in Rust. |
| **Hot reload** (`subsecond`-style binary patching) | GPUI view changes without a restart | Experimental; patches only some changes | Watch GPUI upstream; don't build it ourselves now. |
| **Split the monorepo** | Smaller workspaces build faster | Two workspaces already exist; more would fragment the shared domain crates and their lock files | No. Split crates instead (#12184). |
| **Bazel or Buck2** | Hermetic remote caching and execution | Months of migration and a second build system to maintain | No, unless the remote cache (#12176) and the crate splits fall short. |

Recommendation: keep Rust and its type safety. The changes measured here
attack the actual causes: whole-crate recompiles, debug information,
oversized crates, too many test binaries, and caches that start cold.

## Next steps

| Issue | What |
| --- | --- |
| #12176 | Shared remote compile cache (landed by another chat; repeat its debug measurement with line tables) |
| #12177 | Deploy image caching (landed by another chat) |
| #12183 | `openagents-web` stops building the Verse stack |
| #12184 | Split `openagents-web`, `coder` and the app's `ui` (open; `f40eed6e9a` only changed the benchmark script) |
| #12185 | One feature set with `cargo-hakari` (landed, `2796c12a25`; rerun the "prep" column to measure it) |
| #12186 | Owner decision: nightly parallel front end for dev builds |
| #12198 | Land queue: formatting and `cargo check --tests` before building tests |
| #12199 | Merge integration tests per package; drop `openagents-sdk`'s dev-dependency on `openagents-web` |
| This audit | Measure option 13 (app dependency optimization) against the dev app's run-time speed before changing it |

## Open questions

- **No-op builds that rebuild on the Mac.** Before the changes, a no-op
  `cargo build -p openagents-mobile` took 346.8 s; after them, one no-op
  `cargo test -p openagents-web --no-run` took 95.3 s. Neither happened on
  Linux, and neither recurred: a fresh probe with the new settings built
  `openagents-mobile` in 1,231 s (load 60 to 120), then twice more in 7 s and
  4 s with Cargo's fingerprint log showing nothing to rebuild. kache's
  adaptive incremental mode had picked `openagents_mobile` and
  `openagents_engine` on its own before today ("adaptive seed" in `kache
  report`), which is the first suspect. If it recurs, rerun with
  `CARGO_LOG=cargo::core::compiler::fingerprint=info` and
  `KACHE_LOG=debug`.
- **kache's store is over its cap.** 100.2 GiB against 30 GiB in the morning,
  70.3 GiB in the afternoon, with `gc_evict_shared = true` set.
- **Test run time** was not measured; only builds. `cargo-nextest` and test
  sharding in the land queue need their own numbers.
- **`coder` and `openagents-sdk` stay non-incremental** because of their
  dependents. After the splits in #12184, revisit the list; a kache that
  keyed incremental outputs reproducibly would remove the restriction.
- **Other kache machines** (CoderOS, any new Mac) need `preserve_incremental
  = true`, and Macs need `cache_executables = false`, in their own
  `~/.config/kache/config.toml`.

## Rerun these measurements

`scripts/dev/build-bench.sh` reruns any row. Each variant needs its own
empty target directory, and a target can reach hundreds of gigabytes with
full debug information, so check free space first and delete the target
afterward.

```sh
# The edit loop with the repository's current settings, on Linux, without a cache:
scripts/dev/build-bench.sh --no-wrapper --jobs 11 --cold \
  --target-dir "$(openagents scratch)/bench" --out bench.tsv \
  openagents-web coder openagents-cli openagents-sdk coder-host nostr coder-sync

# The rules before 2026-10-19, for comparison: no incremental crates, full debug information.
scripts/dev/build-bench.sh --no-wrapper --jobs 11 --cold --variant noincr+full ...

# The app workspace on the Mac, with kache:
scripts/dev/build-bench.sh --jobs 7 --cold --manifest-path apps/openagents/Cargo.toml \
  openagents-engine openagents-mobile openagents-client openagents-ui
```

Each run prints a table and appends tab-separated rows to `--out`: time,
host, package, variant, command, jobs, then the warm-up, no-op and edit
times in seconds. `--timings` adds Cargo's HTML timing report. The per-crate
nightly numbers came from `cargo +nightly-2026-08-03 rustc -p P --lib --
-Zthreads=8` (or `-Zcodegen-backend=cranelift`, or `-Ztime-passes`) after a
one-line edit.

# Release and implementation status

[`release.coax`](release.coax) is the single authored catalogue for the current
compiler version and feature status. It renders the CLI metadata, JSON inventory,
accepted-feature reference and book status. [`release-model.coax`](release-model.coax)
checks its identities, fields, status states and historical versions. The model
is an authority unit with checks and a zero-survivor mutation gate.

## Three version domains

- `IMPLEMENTATION_VERSION` identifies the compiler release. The current format
  is three numeric components, each 0–999, without leading zeroes or prerelease
  suffixes. Extending that format needs a model and schema review.
- `STATUS_SCHEMA_VERSION` identifies the JSON inventory shape, currently 1.
  A compiler release does not increment it automatically. It is unrelated to
  protobuf, reflection or signed protocol schema versions.
- A feature's `introduced` records its historical implementation version.
  It does not change when the compiler version changes. `null` in JSON, or an
  em dash in the tables, means unrecorded or unimplemented; it does not mean
  the feature arrived in the current release.

The stable feature `id` identifies a reviewed claim. `works` is an implemented
bounded feature, `partial` states its remaining limits, `reserved` is recognized
and refused, and `designed` is planned. Updating a status requires reviewing the
actual implementation and its evidence. Generation proves consistency of these
claims across surfaces; it does not prove the claims themselves, general compiler
correctness or authority to execute a decoded request.

## Updating a release

Every version bump, including a patch bump, requires a new first release entry
in [`CHANGELOG.md`](CHANGELOG.md). Every release heading, including historical
entries, requires a plain-text summary of **2–4 words** between its version and
America/Chicago date (`YYYY-MM-DD`), for example:

```markdown
## 0.43.0 — Generic Literal Inference — 2026-10-31
```

Describe the main delivered capability in the heading and keep the detailed
changes and their scope in substantive bullets below it. Use ASCII letters and
digits, with internal hyphens or apostrophes if needed, and single spaces
between words so the canvas reader can display every summary. Blank,
placeholder or marked-up titles refuse. Both
`--check` and `--write` validate every heading before changing any output.
Follow the existing headings and use `Commit: pending` while
preparing the bump. Missing, stale, duplicate, out-of-order, malformed or
placeholder-only entries fail release generation before any snapshots change.
The tool preserves authored notes and historical entries; it does not invent
release summaries.

From the repository root:

```sh
# Edit coax/release.coax and add the matching CHANGELOG.md entry first.
scripts/coax-release.sh --write
# Commit the version bump, notes and generated snapshots locally.
# The introducing commit now exists; record its exact hash:
scripts/coax-release.sh --write
coax/build.sh
scripts/coax-release.sh --check --binary coax/bin/coax
python3 -m unittest discover -s coax/tests -p release.py
# Commit the finalized changelog; push both commits together after rebasing.
# Finish the release by publishing its website and canonical changelog:
coax/web/release.sh --ref "$(git rev-parse HEAD)"
```

`--write` permits `Commit: pending` for preparation. When Git history contains
the version's introducing commit, it replaces only the current entry's commit
line with that full hash and link. `--check` always rejects pending entries;
in a Git checkout it also rejects an uncommitted version or a hash that differs
from the observed introducing commit. A rebase can change that hash: rerun
`--write`, amend the changelog-finalization commit and check before pushing.
Source archives without Git still require a concrete, well-formed commit line.
Finalize the hash before building from the bootstrap's private source snapshot.

The public history lives at <https://openagents.com/coax/changelog>, linked from
the Coax project page. Every website build runs the Coax changelog generator
before compiling its reader, deriving the GPU content, accessible HTML and
byte-identical Markdown twin from `coax/CHANGELOG.md`. The normal staging and
production smoke checks require that source parity and the page's assets, so
publishing a site release cannot silently leave an older changelog online.
Publishing the pushed commit with `coax/web/release.sh --ref COMMIT` is the final
required Coax release step; no separate changelog edit or upload is needed.
A version bump or GitBook publication alone does not deploy the website.

Run the model's checks/mutation when its policy changes. The normal bootstrap
already runs them; an unchanged model does not need an additional manual run.
Build the compiler once when its embedded metadata changes. Ordinary prose
edits outside managed regions need only the relevant documentation checks.

The Coax generator emits these checked-in snapshots:

| Output | Consumer |
| --- | --- |
| `src/release.h` | `coax version`, `coax status` and `coax status --json` |
| `release-status.json` | Tools consuming inventory schema 1 |
| `STATUS.md` | Current human-readable status |
| `book/appendix-a-keywords.md` | Published accepted-feature reference |
| `generated/current.md` | Bounded current-version regions in README/spec/roadmap/book |
| `generated/version-example.md` | The book's CLI version example |

The shell adapter compiles/runs the Coax generator, transports its fixed output
bundle and compares or writes files. Metadata validation and rendering live in
Coax. Normal generation, bootstrap and app generation need only the Coax/native
toolchain, shell and standard file utilities. Python is optional test tooling;
the separate GitBook API publisher also uses Python.

The authority-checked `changelog-model.coax` validates the changelog and plans
the current commit-line replacement; `changelog.coax` renders that line. The C
bridge transports file bytes and command-line observations. The shell reads
the first-parent version transition from Git, when available. Changelog prose
is checked on every build, including cache reuse, without becoming an input
that forces an otherwise unchanged compiler to rebuild.

Every managed region is validated before writes begin. On macOS the catalogue
is compiled for the Mac host, even when Xcode passes an iPhone/Simulator SDK;
`COAX_HOST_TARGET` supplies an explicit host triple when needed. The tool must
execute its generator locally, so an inherited cross-target SDK is unsuitable.

`--write` updates the listed snapshots, explicitly marked regions and the
current changelog commit line when its introducing hash is available. It
does not replace version numbers in historical prose or generate the error-code
catalogue, grammar or every explanatory paragraph. Keep those accurate through
review. Add a managed region explicitly if another document needs current status.

## Build caches

`coax/build.sh` uses two caches outside any worktree. Both are shared by every
worktree and agent on the machine.

- **Checked builds** live in `~/.cache/coax-release/<fingerprint>/`
  (`$COAX_RELEASE_CACHE`). The fingerprint hashes every compiler input
  (`provenance.sh fingerprint`). When the cache holds this tree's fingerprint,
  `build.sh` installs that build with `provenance.sh install`, which verifies
  the record against this tree first, and then validates the release documents
  with the installed compiler. A warm build compiles nothing and takes a few
  seconds. One builder runs per fingerprint: the others wait on
  `<fingerprint>.building` and install its result. Installs copy into a
  temporary directory and rename, so a reader never sees half a compiler.
- **Gate results** live in `~/.cache/coax-gates/<key>` (`$COAX_GATE_CACHE`).
  The bootstrap keeps each passing stage0 `test` and `mutate` gate under a key
  that hashes stage0's inputs (`src/`, `bootstrap/`, the embedded proto
  runtime, compilers, flags, SDK and build environment), the gate's command,
  and the bytes of every source the gate reads (`stage0 deps`). A later
  bootstrap reuses a gate only when its key matches exactly. A change to one
  compiler file reruns the gates that read it and reuses the others. Gates run
  by the final compiler and the seed comparisons always run.
- **Killing checks** live in `~/.cache/coax-gates/killers/` (#12646), one file
  per stage0 `mutate` gate command, written by its last complete run
  (`coax mutate --killers`). When a mutate gate does run, each mutant first
  runs the check that killed it last time. That check failing on this graph
  is a kill; when it passes, the mutant runs its whole schedule as before.
  The gate's verdict, its survivors and its cache key are the ones a run
  without the file gives, so the file is trusted with nothing: a stale or
  altered file only costs time. A gate's verdict is not reused across a
  change, and couldn't soundly be, even per part of the checker: a mutant in
  `checker/types.coax` is killed by checks in the fixture and matrix parts
  that run through every other part, an edit to any part can stop a check
  from reaching it, and any checker edit refreshes `bootstrap/checker.c`,
  which is part of stage0 itself. What the file keeps is the search: on the
  full checker, 522 check runs per mutant without it (a 1/64 shard,
  2026-11-02), about one with it. Measured on `checker/numeric.coax` (`coax
  mutate lib/compiler/checker.coax --unit lib/compiler/checker/numeric.coax`,
  719 mutants, load average about 50 on 28 cores): 86,365 check runs and
  511 s without the file, 719 check runs and 202 s with it, most of that the
  baseline run of all 1,217 checks.
- **The checker gate in parts** (#12646, #12638, 2026-11-03, load average 35
  to 170 on 28 cores, other agents' work). Cold, without the file, one 1/256
  shard (`--shard 1/256`, 49 mutants, 23,133 check runs) takes 486 s with 28
  jobs, and stage0 and the installed compiler take the same time (562 and 564
  s side by side, 14 jobs each). The sum of those shards, the cold gate, is
  about 256 × 486 s, some 35 hours at that load; a 1/64 shard does not fit in
  15 minutes. `--unit` runs of each part are complete runs, so each writes the
  part's killers; `expressions`, `borrow`, `retain` and most policy modules
  need `--unit` with `--shard` and a merge to stay under 15 minutes. Warm,
  after a trivial change to a part, with that part's killers: `--unit
  checker/types.coax`, 1,910 mutants, takes 1,910 check runs and 212 s with 14
  jobs (the installed compiler), most of it the baseline. A survivor still
  runs every check, about 180 s of one core each, so each survivor left costs
  a cold schedule on every run.

The build log prints each gate's time, or `reused` with its command.
`COAX_BUILD_FRESH=1` ignores both caches. A cached gate result is a record
that the same gate passed on this machine; it is as trusted as the machine's
cache directories, like the checked builds beside it.

## Published builds

A machine with neither cache bootstraps: about 90 minutes on CoderOS's 28
cores (2026-11-02: 84 of them in one gate, `stage0 mutate
lib/compiler/checker.coax`, 5,064 s) and several hours on a 4-core container,
every time the compiler's sources change. Published builds remove that wait
for everyone but the release host; a published build installs in about 17 s. Published builds remove that wait. Each platform's release host
bootstraps once per compiler change and publishes the result; every other
machine downloads it.

- **The name.** A build is published at
  `https://storage.googleapis.com/openagentsgemini-cli-releases/coax/builds/<source>/<platform>.tar.gz`,
  with `.sha256` beside it. `<source>` is `provenance.sh source-fingerprint`:
  the same compiler inputs as the fingerprint, without the local compilers,
  SDK or environment, so every machine of a platform names the same sources
  the same way. Platforms: `linux-x86_64`, `linux-arm64`, `darwin-arm64`,
  `darwin-x86_64`.
- **What is in it.** The compiler, the provenance helper, the C artifacts
  native tests link, and a manifest: the source fingerprint, platform,
  commit, compiler version, toolchain, link flags and the SHA-256 of every
  file. No `provenance.record`: a record binds a build to one machine.
- **Who publishes.** `coax/published.sh publish` packs `coax/bin` only when it
  is a checked build of a committed tree with no surviving mutant, and
  uploads it only when the name is not taken (published builds are
  immutable). `coax/publish-host.sh --every 600` runs on each release host as
  a service on the owner's Mac (launchd, `com.openagents.coax-publish`) and
  publishes `origin/main`'s compiler for `darwin-arm64` whenever its sources
  change, with the system's clang and `MACOSX_DEPLOYMENT_TARGET=13.0`. When it
  sees a change it also starts the Linux release VM `coax-publish-linux`
  (`COAX_PUBLISH_LINUX_VM=us-central1-a/coax-publish-linux`, n2d-standard-16,
  off between builds). The VM's startup script, `coax/publish-vm-startup.sh`,
  publishes `linux-x86_64` with GCC for musl from nixpkgs and
  `COAX_LDFLAGS=-static`, so one build runs on any Linux of its architecture,
  then powers off. It runs as the automation service account, so no key is
  copied to it. Its log is `/var/log/coax-publish.log` on the VM. No GitHub
  Actions.
- **Who installs.** `coax/build.sh`, when its cache has no build for this
  tree, tries the published one before bootstrapping: it checks the download
  against its checksum, the manifest's source fingerprint and platform against
  this tree and machine, every file against its hash, that a Linux build links
  no shared library, and that the compiler runs; then writes this machine's own
  provenance record, validates the release documents with it, installs it and
  keeps it in the cache. A missing or failing build falls back to the
  bootstrap and says why. `COAX_BUILD_FRESH=1` or `COAX_NO_PUBLISHED=1`
  bootstraps.
- **Trust.** A published build is as trusted as the release hosts and the
  bucket's write access (the automation service account), the way the local
  caches are as trusted as their machine. The checksum guards the transfer,
  not the publisher. A tree whose compiler sources differ from every published
  build always bootstraps.

Setting up the release hosts again: `coax/published-builds-handoff.md`.

Check: `coax/tests/published-build.sh` (a build's manifest, names and hashes
are checked, and a wrong source, platform, hash or extra file is refused).

## Which surviving mutants stop a build

A mutation gate in the bootstrap fails on a surviving mutant only when the
change since a base commit caused it: the survivor is on a changed line, or
the change removed or edited a check line (`#assert`, `#test`, `#forall`,
`#authority`) in the survivor's file or in the file the gate mutates. Other
survivors are listed in `coax/bin/mutation-survivors` and printed, but they
don't stop the build. Only a regression that a change causes may stop it.
[`mutation-gate.sh`](mutation-gate.sh) lists the changed lines and judges the
survivors. A gate that fails for any other reason (a failing check, a
procedure no check reaches) still stops the build.

| Build | Base | Survivors that stop it |
| --- | --- | --- |
| `coax/build.sh` | `COAX_MUTATION_BASE`, or else where the checkout left `origin/main` | Those the checkout's changes caused, including untracked `.coax` files |
| `coax/web/release.sh` | None: it builds with `COAX_MUTATION=0` | None. No mutation gate runs, and a cached or published build's survivors are reported in `releases/<time>/MUTATION` |
| `COAX_MUTATION=0 coax/build.sh` | None | None. No mutation gate runs |
| `COAX_MUTATION_FULL=1 coax/build.sh` (the full check) | None | Every survivor |
| No Git history and no base | None | Every survivor |

A release never waits on mutation (owner, 2026-11-02). It builds the compiler
with `COAX_MUTATION=0`, which skips every mutation gate, and keeps that build
in its own cache (`~/.cache/coax-release-fast`), so it never stands in for a
gated build. No survivor stops a release. It lists any it finds in `releases/<time>/MUTATION`. A cached build
records the survivors it tolerated, and a build or release reuses it only when
none of them gates for that build's changes.

A survivor on an unchanged line that a code change elsewhere caused
indirectly is reported, not gating; the full check catches it.

The full check runs outside releases. The Coax taskmaster owns it: it runs
`COAX_MUTATION_FULL=1 coax/build.sh` after a release that reports survivors,
and gives each survivor to a worker as a queue task. The release prints a
`mutation` line whenever `releases/<time>/MUTATION` is written.

## Bootstrap, application and publication gates

The snapshots are committed so a clean checkout can start from its reviewed
frontend C seeds. When no compiler exists, `coax-release.sh` builds a temporary
stage0 generator frontend using those seeds and the checked-in metadata header.
It does not publish that temporary compiler. `COAX_RELEASE_COMPILER` can select
an already trusted compiler, including for recovery of stale generated files.

The bootstrap validates the model and compares generated files with its stage0
before the expensive compiler gates, and `coax/build.sh` checks them again with
the installed compiler. Before publishing the final compiler, it compares
that binary's version, Markdown and JSON with fresh generator output. A matching
version string alone cannot hide stale feature data. The native frontend,
generator, shell adapter and toolchain remain trust inputs.

### Seeds must match on every host compiler

The seeds are emitted by stage0, which is C++ built by the host compiler:
Clang on a Mac, GCC in the API image (`web/api/Dockerfile`). Until #12611 the
C emitter (`src/emit_c.cpp`) built some C from two `ex()` calls in one
expression, such as `"(" + ex(lhs) + " || " + ex(rhs) + ")"`. Each call
numbers temporaries, and C++ leaves the order of those operands unspecified:
Clang emits the left operand first and GCC the right one. A seed refreshed on
a Mac (`checker.c` since 373400c446, the first checker `||` whose operands
both need temporaries) then failed every Linux bootstrap with
"Checker seed is stale", and the API image couldn't build from `main`.

The emitter now emits each operand into its own variable, in source order,
and the bootstrap refuses an `emit_c.cpp` line with two `ex()` calls in one
expression. After refreshing a seed or changing the emitter, run
`coax/seed-portability.sh --linux`: it compares the seeds with what stage0
emits on this host and under Debian GCC (Docker when it runs, else Cloud
Build), in about a minute and without gates.

The iOS generator performs the same source/snapshot/binary drift check before
emitting application C. The book publisher does so before reading credentials
or creating a remote draft; its existing clean, pushed-main and page checks
still apply. A drift failure is fixed locally with the canonical source and
`--write`, followed by rebuilding only if embedded binary metadata changed.
Neither app nor book publication silently repairs source files.

The [focused evidence](../docs/verification/coax-release-status-2026-10-28.md)
records the actual tests and their limits. [R0](ROADMAP.md#3-r0--establish-the-adoption-gate-now)
remains open for device/frame-tail measurements, complete development-loop
attribution and independent review enforcement.

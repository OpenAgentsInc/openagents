# Setup findings before scored execution

These changes precede every scored executor outcome. The source-ranking
policy remains the version bound by [treatment-freeze.json](treatment-freeze.json).
The overall protocol is still a draft; these infrastructure findings do not
constitute a successful coding comparison.

## Native admission differs from the CLI budget

A native request declares up to 128,000 output tokens. Reserving for that
request exceeds a $2 broker ceiling before any inference, even though a
normal answer can be short. Two early probes admit no upstream request.
The authenticated capability check then requests a $0.20 CLI budget and
records $0.288885 before the CLI stops at its budget limit.

The prospective configuration therefore distinguishes the requested $2
native budget from the $8 broker admission target. A complete six-arm block
reserves $48 plus preparation before launch against the $120 accounting
ceiling. Byte-based reservations are a heuristic, not a guaranteed invoice
cap. No billed amount is clipped to a requested budget. See the
[capability record](../../../../bench/delegation-study/infrastructure-preflight.json).

## Full source setup is larger than a source brief

The historical Git exports contain approximately 1.53–1.55 GB of tracked
files. The first 512 MiB export limit refuses them before Cargo. The bounded
exporter now admits up to 2 GiB. Registration hashes that artifact by streaming
it; candidate payloads retain their separate limits. The isolated compiler
also needs the system's `/etc/alternatives` links mounted read-only.

A successful small-package setup takes 27.46 seconds, including a 3.45-second
export, 14.51-second isolated Git snapshot, and 5.95-second Cargo build. It
produces a 132.9 MB seed. A separate copied-seed check takes 24.78 seconds,
including 4.95 seconds of compilation. These are infrastructure observations.
They explain why a subsecond source pack is not a subsecond runnable task.

## A verified baseline cache differs from a shared target

The calibration's shared target contains artifacts from several revisions.
Revisiting an older tree can incorrectly reuse a newer artifact if the old
source timestamps precede the build. The invalid development attempt and
its correction remain in the [calibration record](calibration.md).

The trusted seed builder refreshes all exported source timestamps and
requires Cargo to recompile workspace units. It exports only named baseline
artifacts, bound to the source archive, bytes, modes, and toolchain. It never
copies the shared target wholesale. Unreported artifacts, private checkers,
build-script output directories, and incremental state stay out of the seed.

A candidate check starts from a verified copy of that exact baseline seed.
It can reuse unchanged baseline units. Reconstruction verifies preimages and
postimages and refreshes changed or new inputs, including non-Rust include
files, plus the injected checker. Refreshing every unchanged source at this
stage would force unnecessary compilation. The independent checker uses its
own target and never trusts artifacts produced by the executor.

## Full test executables exceed the available scratch space

The first full `coder` seed compiles successfully in about 261 seconds, then
exits with an empty result receipt during artifact copying. Disk pressure
is the supported diagnosis; the terminating exception was not retained.
The partial seed alone reaches
29.0 GB; the shared target is about 62.2 GB. Inspection identifies 50 final
executables totaling approximately 42.59 GB, plus approximately 5.02 GB of
libraries. The failed setup is retained. Cleanup removes the invalid copied
seed and verified reconstructible setup directories after saving their
evidence. The shared long-lived target remains intact.

The common pre-scoring build configuration now sets
`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_PROFILE_TEST_DEBUG=0` for seed creation,
native execution, and independent checking. This removes debug symbols while
retaining the existing optimization and assertion settings. The seed policy
also omits final test and binary executables; every arm regenerates needed
executables under the same configuration. Toolchain environment and cache
policy must be bound in registration, and full-check feasibility must be
measured before the timeout is sealed.

All five final seeds now compile and export successfully under this common
configuration. The [retained seed evidence](../../../../bench/delegation-study/seed-preflight.json)
records every reported workspace unit as rebuilt from its pinned source:

| Task | Seed size | Files | Rebuilt workspace units | Build | Full setup |
| --- | ---: | ---: | ---: | ---: | ---: |
| Development | 76.55 MB | 202 | 2 | 1.92 s | 27.12 s |
| Reserved A | 63.41 MB | 188 | 7 | 5.24 s | 30.59 s |
| Reserved B | 2.552 GB | 3,880 | 93 | 131.71 s | 163.79 s |
| Reserved C | 2.600 GB | 3,894 | 95 | 65.10 s | 96.41 s |
| Reserved D | 2.599 GB | 3,894 | 95 | 62.94 s | 90.90 s |

Sizes use decimal units. The five seeds total 7.89 GB and 408.82 seconds of
setup, excluding earlier attempts and later cleanup. Full setup includes
archive creation, extraction, Git initialization, compilation, and seed
copying and validation. Copying is not separately timed by the builder.
These measurements share an evolving dependency cache; the table is not
an independent cold-build comparison. Seed creation does not establish
ordinary-test success or acceptance latency.

Per-run target and workspace copies need disk admission and cleanup after
candidate artifacts, receipts, and private logs are retained. The original
coordinator puts all cleanup outside the primary endpoint; the later amendment
below moves the release needed before acceptance inside it. The source
archives and canonical candidate artifacts allow reconstruction. Retaining
two 1.5 GB workspaces per session would exceed 140 GB over 48 sessions even
before compiled targets. The coordinator removes only its closed, verified
scratch directories and preserves shared targets and retained evidence.

These changes may improve setup feasibility. They are common conditions for
all arms and supply no evidence that System One, model selection, or source
packing improved a coding result. The [infrastructure review](infrastructure-review.md)
records separate candidate-identity, capture-bound, and malformed-log fixes.

## Full checks expose requirements that compilation misses

The first full acceptance attempt on reserved A passes formatting and 76
ordinary tests, then fails to launch doctests because the namespace cannot
find `rustdoc`. Its independent checker fails on the broken source as
expected. This is a tool-visibility failure, not a failing doctest assertion.
The attempt and its phase logs remain retained.

The [toolchain amendment](../../../../bench/delegation-study/rustdoc-amendment.json)
binds the canonical Rust 1.97.1 `rustdoc` executable, hash, and version in
all five seed manifests and the common environment. It preserves the prior
manifests, source archives, library bytes, and original build receipts.
No library rebuild is needed for this metadata and executable-visibility
change. The amendment takes an additional 10.86 seconds. New seed creation
validates the explicit executable before exporting or compiling source.

The same attempt encounters a disappearing Git `gc.pid` file during scratch
cleanup. Snapshot creation now disables automatic Git garbage collection
and maintenance before indexing; a focused regression confirms that the
snapshot commit stays identical. Scratch cleanup tolerates only a child
that has already disappeared. Permission, root-path, and other errors remain
visible. These corrections precede scoring and apply to every arm.

## Release native scratch before final verification

The [D reference observation](eligibility-cd.md#disk-and-cleanup) measures
8.45 GB of allocated acceptance workspace and target, with 5.64 GB free at
that point. Keeping a similar native tree concurrently is not qualified by
that observation. The seed's exported size alone understates the storage a
full check needs.

The [storage amendment](storage-amendment.json) changes the common trial
coordinator before scoring. It validates and durably retains candidate bytes,
changes, native results, provider accounting, and logs, confirms native process
closure, then removes only the reconstructible native workspace and target.
Acceptance starts after a successful release. A failed release stops it and
later trial admission, retaining the failure without automatic cleanup retry.
Unknown closure or missing evidence preserves the scratch files.

Native release time counts inside the endpoint; final acceptance cleanup
remains separate. All 132 [local checks](local-validation-followup.json) pass,
including release ordering, retained artifacts, failure handling, and shared
target preservation. This establishes those tested behaviors, not an actual
current native-to-acceptance run or a storage quota. Native home caches,
arbitrary output, and other processes can still consume disk. Required live
preflight and final registration remain incomplete.

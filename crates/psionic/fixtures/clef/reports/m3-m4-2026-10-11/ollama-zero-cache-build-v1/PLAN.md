# Paired scratch Ollama builds

Prepared, not executed. Root must hand off each workload phase after source and
peer review. No model, server, GPU, installed-app change, native-library rebuild,
or quiet lease is part of this workflow. The original
[cache-reset proposal](source-proposal/README.md) and [patch](source-proposal/candidate.patch) are unchanged.

## Available inputs

[local-inventory.json](local-inventory.json) records read-only inspection. Go
1.26.3 darwin/arm64 is installed; the pinned module requires at least Go 1.26.0.
Four of the 78 declared module ZIPs are cached, totaling 275,781 bytes. No complete
Ollama source checkout was found in this task's scratch or the three conventional
public checkout locations inspected. This is a scoped search, not a claim that
no other checkout exists on the machine.

The selected installed `mlx_metal_v4` payload has five files, totaling
229,369,648 bytes. Both scratch binaries use that same directory read-only,
through scratch-local links. The staging phase hashes every file and the later
phases recheck them. The selected test verifies the actual loaded library path
and MLX version. A change or fallback fails the phase.

The installed Ollama executable is universal x86_64/arm64. The build-info record
returned by Go describes the x86_64 slice, reports Go 1.26.0, pins the intended
commit, and sets `vcs.modified=true`. This does not establish the arm64 slice's
compiler or reproduce undocumented release edits. The two new Go 1.26.3 binaries
are explicitly same-source controls, not byte-identical rebuilds of the installed
release. Later model qualification must include stock fresh-runner answers.

## Fixed phases

1. `stage.py` obtains only public Ollama commit
   `b061384d90ff455462bc32745dd0de479de717a3`, verifies the fetched commit and
   retained source hashes, and copies regular tracked files into two scratch
   trees. It adds the identical test file to each, then applies the original
   patch only to the reset tree. Complete source manifests must differ in exactly
   `mlxrunner/prefix_cache.go`. Symlinks or missing submodules stop staging.
   It copies only the declared public local module-cache entries, fetches missing
   entries through `proxy.golang.org`, checks each against the pinned `go.sum`,
   and runs `go mod verify`. Go toolchain downloads are disabled. No CMake or Go
   compile runs in this phase.
2. `build.py` works offline with the pinned installed Go toolchain, two jobs,
   fixed release flags, and the same native payload. It builds both server
   executables and both selected-test binaries sequentially. It never starts
   either executable. `-buildvcs=false` avoids incidental differences from the
   scratch trees; source manifests and exact patches are recorded separately.
   Missing offline dependencies fail instead of triggering more downloads.
3. `run_tests.py` runs exactly eight named top-level tests in each binary. It
   rejects missing, duplicated, skipped, or failed tests. Every command's stdout,
   stderr, arguments, and exit are saved before interpreting them, including a
   failing test command. The final result binds both binaries and full test JSON.

Stage and build each use a 4 GiB process-group cap, 16 GiB available memory at
startup, and a 12 GiB floor. Their deadlines are 300 and 900 seconds respectively.
Tests use 2 GiB, startup 14 GiB, the same floor, and 60 seconds total. Go and native
thread controls are two; tests run one at a time. All phases reserve 4 GiB of disk
and sample logical scratch usage every second, including logs and Go temporary
files. This is sampled enforcement, not a filesystem quota; any observed
overshoot fails and stays recorded. Symlink targets are not counted as new disk
storage. A separate 25 GiB free-disk floor is checked before launch, on every disk
sample, and after cleanup. Memory, disk, sampling, deadline, source, or cleanup failure stops the attempt.
No automatic retry, cap increase, deletion, or cleanup of unrelated work occurs.

Build requires this exact descriptor's successful stage guard and confirmed
owned-group cleanup; tests require both stage and build receipts. Each receipt
binds its result artifact hash and source-freeze hash. Artifacts repeat that
source identity. Preconditions are checked before and after each dependent
phase, so revising this workflow cannot reuse stale successful staging/build
records. A failed guard cannot qualify its otherwise complete child artifact.

## Scalar lifecycle coverage

The added Go tests use upstream rewindable/recurrent token fakes, a test-only
hidden-row fake, and nil stateless slots. Each fake's `State()` returns no arrays.
The unchanged `cacheSession.close()` therefore reaches the empty-input return in
`mlx.AsyncEval`; these tests do not construct or evaluate model tensors.
`TestScratchLibraryIdentity` calls only library metadata functions. This scope
does not prove real tensor deallocation or numerical parity.

Tests cover first and repeated scores, divergent suffixes, a 1,057-token request
fed in 17-token chunks, cancellation after 113 of 211 tokens, repeated close,
unchanged generation reuse, actual library identity, and upstream
`TestPrefixCacheClose`. The reset build must begin every score with all tokens
remaining, zero offset, empty model/hidden state, and all old snapshots closed
exactly once. Chunk feeding must preserve within-request state. The stock build
must demonstrate full exact-repeat reuse and reuse after cancellation, making
it a positive control. Generation must retain its one-token decode seed in both.

## Commands after handoff

Run each command from this directory only after root explicitly hands off that
phase. The guard refuses pre-existing attempt directories and quiet/GPU leases.
The build lease is required only for compilation; the other phases do not build.

```sh
openagents lease memory --amount 4 --receipt stage-memory-lease-1.json -- \
  openagents lease disk --amount 4 --receipt stage-disk-lease-1.json -- \
  python3 guard.py stage

openagents lease build --keep-target-dir --receipt build-lease-1.json -- \
  openagents lease memory --amount 4 --receipt build-memory-lease-1.json -- \
  openagents lease disk --amount 4 --receipt build-disk-lease-1.json -- \
  python3 guard.py build

openagents lease memory --amount 2 --receipt test-memory-lease-1.json -- \
  openagents lease disk --amount 4 --receipt test-disk-lease-1.json -- \
  python3 guard.py test
```

Before any model work, root reviews all source, build, test, resource, and cleanup
receipts. Later comparison still needs identical request bytes, artifact and
token identities, complete unrounded probabilities, 100% top agreement, the
existing strict numerical gate, and a present integer top-level cached-token
count of zero in every patched reply. No raw-logit or warmed-latency acceptance
follows from this workflow alone. Stock reload remains a first-score correctness
control; warmed timing requires a separately frozen plan.

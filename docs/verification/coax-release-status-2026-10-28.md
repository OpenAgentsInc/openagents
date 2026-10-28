# R0: single-source release/status and publication drift

Date: 2026-10-28. Compiler: coax0 0.14.0, status inventory schema 1.
Host: arm64 macOS; Apple clang 21.0.0 (`clang-2100.1.1.101`).

## Delivered boundary

`coax/release.coax` owns the implementation version and 92 reviewed feature
records. Its Coax renderer supplies the CLI's version/Markdown/JSON, current
status, accepted-feature book reference and bounded current-version regions.
The schema identity and historical introduction versions are independent of the
release version. Unknown introductions stay unknown rather than becoming the
latest release.

`release-model.coax` is an authority unit admitting the metadata. It rejects
invalid/duplicate identities, invalid fields, unknown states/sections, future
introductions and unsupported inventory schemas. The native shell adapter
transports the fixed output bundle and compares/writes files. All managed
regions are checked before any write. Historical/unmanaged prose is preserved.

Bootstrap gates source/snapshot drift before the expensive compiler checks and
compares the final binary's entire status and version before publication.
The iOS generator runs the same check before application C emission. The book
publisher checks before reading credentials or creating a remote draft.
Matching a version string alone is insufficient to pass stale status data.

## Focused evidence

| Check | Observed result |
| --- | --- |
| `coax test coax/release-model.coax` | 6 checks passed |
| `coax mutate coax/release-model.coax` | 134 mutants; 134 killed; 0 survived; no exemptions |
| `coax test coax/release.coax` | 7 checks passed, including the actual catalogue |
| `python3 -m unittest discover -s coax/tests -p release.py` | 10 tests passed |
| Ordinary Python-free bootstrap | Passed once, including existing authority/mutation and reproducible seed/final regeneration gates; Python invocation tripwire was not triggered |
| No-existing-compiler fallback | Temporary stage0, generation and binary/snapshot comparison passed without Python |
| Fallback under inherited iPhone Simulator SDK | Passed with `SDKROOT` set to the Simulator SDK and `IPHONEOS_DEPLOYMENT_TARGET=16.0`; generator compiled/executed for the Mac host |
| `bash -n` on changed shell adapters | Passed |
| Book navigation, local links and checked excerpts | Passed |

The ten regressions exercise CLI/snapshot identity, corruption and repair of
each snapshot, two consecutive temporary version bumps with preserved schema
and introduction versions, invalid state/section rejection, missing/duplicate
regions, preflight failure without partial writes, stale binary metadata with
a matching version, actual accepted/refused language probes, inherited Xcode
SDK handling, and publication rejection before credential/network access.

Review found and reproduced a new adapter defect: under an inherited Simulator
SDK its generator was compiled as a Simulator executable, then failed at Mac
execution with `DYLD_ROOT_PATH not set`. The adapter now selects the Mac SDK
and host target explicitly. Review also caught unknown sections disappearing
from Markdown and a version-bump regression tied to a literal current version;
both have negative regressions.

The normal bootstrap completed before these narrow refinements. Afterward only
the metadata model/rendered explanatory text and shell/test adapter changed.
The model checks/mutation and focused tests were rerun; the native driver was
relinked against the same validated generated compiler module to embed the final
metadata. Source/snapshot/final-binary comparisons passed again. Unchanged
frontend/importer mutation suites were not repeated for this metadata-only
refinement.

The subsequent rebase preserved the upstream Metareal-to-Verse rename and new
`value_barrier` intrinsic. The latter was added to the catalogue with no guessed
historical introduction. The refreshed upstream checker seed and current native
driver were linked into a new local compiler; compiler-module self-regeneration
and all four frontend C/header seed comparisons passed. Compilation runs the
physical authority checks. The affected `constant_time.py` suite passed three
tests, including native and wasm32 output inspection. The ten release
regressions and minimal uncorrupted differential replay also passed on that
integrated compiler. Unchanged broad mutation/performance/app suites were not
restarted because of the rebase.

Final source/artifact identities (SHA-256):

| Input/artifact | SHA-256 |
| --- | --- |
| `coax/release.coax` | `dc76743d59b042c1877ab76109a446761b129b279aea45d1a72f31cb5680c3df` |
| `coax/release-model.coax` | `c9521dba6c5377ea5c699f78d802b90e59ebf7ce8c0686db5dceb6ef554d8434` |
| `coax/release-status.json` | `2b62ae631cd7671a8959b7aea1f9647b0fa2ed448c8993981a3ab06f82ab3378` |
| Final local compiler | `acd6c3957cef3aa78e39d7231c49bd9f52df5e791956ff58046d80ebb3f92837` |
| Validated generated compiler module | `8d7dfba68179131936658527eb07d459f54068b5ba2773b7c6919b126d4a0c58` |

## Parallel roadmap delivery

The optional [bounded differential tool](../../coax/verification-differential.md)
has ten focused tests. Its retained 24-program sample passed 96 evaluator input
cases and 192 checked native input executions across `-O0`/`-O2`. A separate
integer/state oracle covers arithmetic, branches, aggregates, caller-owned view
writes and reverse cleanup. A deliberately corrupted emitted multiply helper
is detected in both native profiles and reduced from twelve operations/four
inputs to its base arithmetic/one `(0, 0)` input in four attempts.

That sample retains the earlier trusted 0.13.0 binary's identity rather than
relabelling old evidence as a new compiler run. After integration, its minimal
fixture was replayed without corruption on the final 0.14.0 binary: evaluator
and both native optimization profiles passed. The complete suite/sample was
not repeated for a metadata-only driver change. This verification tool's
optional Python use does not enter normal compiler/app builds.

## Limits and remaining R0 work

Consistent status is not independent semantic correctness evidence. The native
compiler, seeds, generator, adapter and toolchain remain trust inputs. Reviewed
feature claims can still be wrong; representative real frontend probes do not
exhaustively establish every claim. The error-code catalogue, grammar and
unmanaged explanatory/historical prose remain separately reviewed sources.

No iOS application binary, device benchmark, GPU path, TestFlight release or
full compiler regression suite was run for this metadata/tooling slice. The
changed app integration is exercised at its release-adapter boundary, including
the actual inherited SDK case. No runtime product logic changed.

R0 remains open for physical-device/frame-tail budgets, full asset/install/
launch/GPU-preparation attribution and independently enforced reference/gate
review. Neither this catalogue nor the differential sample closes those gates.

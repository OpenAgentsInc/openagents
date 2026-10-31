# Semantic profiles, version 1

Coax 0.25.0 separates runtime arithmetic, native optimization, `BUILD_DEBUG`
source selection and native source debugging. The checked policy is
[`lib/compiler/profiles.coax`](lib/compiler/profiles.coax). This version preserves
the previous defaults; it does not migrate existing applications to new profiles.

| Choice | No flags | Legacy `--release` | Explicit override |
| --- | --- | --- | --- |
| Runtime integers | checked | wrapping | `--arithmetic checked\|wrapping` |
| Native optimization | `-O0` | `-O2` | `--opt 0\|1\|2\|3` |
| `BUILD_DEBUG` | true | false | `--build-profile development\|release` |
| Native source debugging | on | off | `--debug-info` / `--no-debug-info` |

Explicit choices win regardless of where `--release` appears. Changing one axis
does not change the others. Native debugging defaults apply to `run`, `build` and
`lib`; plain C emission still needs an explicit debugging request. `--source-map`
implies debugging and refuses an explicit `--no-debug-info`. Repeated or invalid
axis choices refuse compilation.

`--opt` is accepted only by `run`, `build` and `lib`, which invoke the C compiler.
Arithmetic and build-profile choices apply to source commands, including checks,
headers and GPU source emission. Debugging options apply to native commands and
`c`. Proto generation has no runtime profile. GPU source flags select existing
lowering; they supply no new device qualification.

Runtime wrapping selects wrapping addition, subtraction, multiplication and
negation. Bounds, casts, shifts and division retain their checks. Compile-time
evaluation, `#run`, `#test` and `#forall` remain checked under every runtime
profile. Authority checks remain mandatory. One loaded graph has one profile.
A separately compiled C/foreign callee keeps its own arithmetic; the caller's
flags do not rewrite it. ABI compatibility does not establish semantic
interchangeability, and there is no general mixed-object linker enforcement.

## Optional artifact report

`--profile-report FILE` writes a standalone JSON report. Its schema version is
1; the timing, dependency and debugger-map schemas are unchanged. Ordinary
emission has no new banner or metadata without this option.

| Field | Meaning |
| --- | --- |
| `schema_version`, `compiler`, `command`, `backend` | Report version and producing command/compiler |
| `runtime_arithmetic`, `interpreter_arithmetic` | Selected runtime mode and the always-checked evaluator |
| `build_profile`, `debug_info`, `selected_optimization_level` | Independent effective selections |
| `profile_identity` | Version-1 selection identity described below |
| `native_optimization_flag` | Flag passed to a started native compiler, or null |
| `sources`, `descriptors` | Paths and SHA-256 hashes of consumed input bytes |
| `emitted_sha256` | Exact emitted text hash, or null when nothing was emitted |
| `artifact_sha256` | Native output hash, emitted text hash, or null when no artifact exists |
| `success`, `exit_status` | Observed command outcome |

Source hashes use retained physical-unit bytes; descriptor hashes use the bytes
consumed by the importer. The existing Coax SHA-256 implementation hashes inputs
and outputs. A native run's executable is hashed before execution and temporary
cleanup. A failed frontend or compiler start cannot claim applied optimization.
`check`, `test` and `mutate` have no emitted artifact.

Report destinations must be distinct from input files and generated outputs,
including aliases and native intermediate files. No report is written when the
complete input graph could not be read and admitted. A report after an admitted
graph fails typechecking records that failure with null artifact hashes. These
observations do not lock files against concurrent modification or prove program
correctness. Build-only stage0 refuses profile reports.

The selection identity is an injective encoding of the resolved axes:

`arithmetic + 2*optimization + 8*development + 16*debug_info + 32*native_command`

Here checked arithmetic is 0, wrapping is 1, and booleans are 0 or 1. Its meaning
is scoped to this policy version. It is not a declaration identity, artifact
digest or ABI identity. Mutation fingerprints bind this selection even when
different profiles produce identical source trees and mutation sites.

Future cache keys must include the effective profile, compiler and toolchain
identities, target and full source/descriptor content. A bare selection integer
or file path is insufficient. This release supplies observations for that work;
it does not supply a semantic build cache.

## Conformance scope and migration

The focused native vectors compare checked and wrapping behavior at `O0`/`O2`,
including overflow, checked edges, evaluation order, cleanup and float bits.
Actual driver arguments and artifact hashes are checked across `O0` through
`O3`; authority and compile-time failures remain failures.

Decimal vectors preserve the existing binary64-then-binary32 path. For example,
`1.0000000596046448f32` currently becomes binary32 bits `0x3f800000` through a
binary64 midpoint. Direct decimal-to-binary32 rounding remains a separate
design/migration question. This release does not silently change that behavior.
Arbitrary native check executables and wider backend conformance remain separate
roadmap gates. See [`tests/semantic_profiles.py`](tests/semantic_profiles.py).

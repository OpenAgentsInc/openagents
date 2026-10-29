# Fixed-base source revalidation

The existing fixed-base arithmetic, timing and mutation observations remain
pinned to their original inputs. Their source hashes still match the current
fixed-base files; they were not rerun for unrelated repository changes. This
source revalidation addresses the reopened aggregate-auditor prerequisite and
does not prove generated-code timing, guards or production secret safety.

Both real wrapper entry points are analyzed in the native-C checked-library
profile with Coax 0.21.0, SHA-256
`73d2aeb0e1e57c355d87242946f89e1462f8b765f57087f4307d471a0277e1d4`.
Each reaches 31 procedures in 30,820 source visits and completes with one
secret-derived assertion at `secp_field.coax:239`. Both reports have
`accepted=false` and `analyzed-with-guard-obligations`: the authored guard
remains unproved. These observations are not a Wasm, release-object, JIT,
timing or production-secret approval.

The source driver now accepts an explicit bounded compiler `--run-steps` input;
omitting it preserves the compiler default. The real generated tables were
evaluated with a 250,000,000-step cap for each compiler command. That input does not change the
separate aggregate source audit's visit, depth, region or guard limits.
The driver records the compiler hash, native-C checked-library profile, chosen
step budget and source-map and dependency-output hashes. It checks compiler
source-map hashes against the exact source bytes supplied to the parser, checks
ordered dependencies, refuses nonempty descriptors, and rechecks the source and
compiler identities after analysis.

The explicitly attested compiled-source bridge is reserved by convention for
this trusted driver after those checks; the C symbol is not an enforcement
boundary. The ordinary bridge and direct graph entry refuse
`#run` expressions. The compiled-source entry models the result as opaque public
data, without inventing a value, type, extent or region; local binding types
remain compiler-resolved, and view ownership is not fabricated. The pinned
compiler rejects an attempted runtime-secret capture into `#run`. This
interface is not an external arbitrary-IR attestation API.

The policy passed 174 checks and 2,574 of 2,574 mutants, with 20,759 selected
and 124,186 skipped checks under the dependency scheduler and no fallback or
exemptions. Thirteen aggregate and seven scalar integration tests passed.
Controls cover no attestation, source-map absence and hash mismatch, changed or
malformed dependency inventories, descriptor presence, out-of-range budgets,
runtime-secret capture and an unknown `#run` loop bound. The default and
explicit compiler-input paths are tested separately. An independent review
checked the source-map byte binding, dependency closure, profile and trusted
bridge boundary; it found no concrete issue within this restricted scope.

[Recorded reports and test observations](2026-10-29-secp-fixed-base-source-reaudit.json)
include exact source, compiler and audit-tool identities, source-map and
dependency digests, and the guard location and origin. The original
[arithmetic and timing evidence](2026-10-29-secp-fixed-base.md) remains separate.

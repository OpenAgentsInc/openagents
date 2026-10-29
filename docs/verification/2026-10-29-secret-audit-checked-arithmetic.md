# Secret-source audit checked-arithmetic correction

**Security status:** Historical secret-source audit acceptances from all four
profiles are suspended. This correction is a narrow fail-closed repair, not a
complete implicit-guard audit or approval of any cryptographic source graph.
Previously recorded reports and hashes remain historical evidence. Revalidate
an exact graph only after all relevant analyzer soundness issues are resolved.

The scalar-only and all three aggregate profiles accepted a secret `u64`
argument used in checked addition. The compiler emits an overflow-checking
helper in checked C, making termination depend on the secret. The exact
[addition source](data/secret-checked-arithmetic-2026-10-29/add.coax) has
SHA-256 `84ec02862f8708fe9757065e30660676dcabb256593c630e97c278968d9cf462`.
The falsely accepting [scalar](data/secret-checked-arithmetic-2026-10-29/add-scalar.json),
[legacy](data/secret-checked-arithmetic-2026-10-29/add-legacy.json),
[public-view](data/secret-checked-arithmetic-2026-10-29/add-public.json), and
[large](data/secret-checked-arithmetic-2026-10-29/add-large.json) reports have
SHA-256 `55952fd08d29893a98a9938653c6afb330c0845f9310829af4d786f4b203217d`,
`4b2708546244039ee5f8275b3317bcd14fa65b9bb4738c18355f77400e8a0589`,
`ca101b944baa2facb0157c1089ae9809dba8a85551a3d0b702f6619b889f28d6`, and
`5c7149ac0219e2a4908a84dd47e5c175f1fef6dbcc074f67b8e9509ab780486b`.
The generated checked C was `5774525ad173e612f088135e390a5e5b2574b1779f570d89e956c112f0e30bff`;
it invokes `coax_add_u64`, which traps at the maximum argument.

A separate [unary source](data/secret-checked-arithmetic-2026-10-29/neg.coax)
(`269de5b524670fe7646e6e4b8b6b9797000041b9b0cbc5594df3cac27170bbf1`)
was falsely accepted by the [large profile](data/secret-checked-arithmetic-2026-10-29/neg-large.json)
(report SHA `e4079a5d0c05448ab3ea1a4d66250ff017fd9fa04e2b7f8751efc689c9f298b8`).
Its checked C (`538fb7f459e47010b2973e5c2b4a2bd69c9e9fd1b66e1087dad2c68a99ca1f53`)
calls a signed negation helper that traps on the signed minimum. Compound
writes are a separate analyzer path and require the effective origin of both
the old target and the new operand, including aliases.

The four policies now conservatively refuse checked addition, subtraction,
multiplication, division and remainder with a secret-dependent operand,
secret-dependent unary negation, and secret-dependent compound arithmetic.
Explicit wrapping operations and the existing secret shift-count and checked
cast refusals remain separate. Public arithmetic remains analyzable; its
runtime checks can still trap on public input. No general range proof or
secret-dependent trap exemption is introduced.

The direct Coax tests passed 41/41 scalar and 194/194, 198/198 and 201/201
for the respective aggregate profiles. Scalar full-reference mutation passed
749/749 with 8,891 checks and zero skips; its retained [log](data/secret-checked-arithmetic-2026-10-29/scalar-reference.log)
has SHA `251d159074f546a40566e5035ffbebcdefcb44f08b44c557ef97f44154faa71c`.
The classified real-source public-addition control passed each aggregate
profile; its [report](data/secret-checked-arithmetic-2026-10-29/public-classification.json)
has SHA `87685673ee56698c2d9e457943ed43c84183ddfe6f57b87b1abdfd8011bf7f0d`.
The scalar fixture's signed case refuses at type admission, so the hand-graph
unary test and aggregate runtime controls are distinct evidence.

The real-source refusal and runtime [report](data/secret-checked-arithmetic-2026-10-29/runtime-v3.json)
has SHA `63ee855f3c2a746ac3faf5cf602b929c4b855b4357b38a805ed4d3aec9a65582`.
Each checked and release native/WASI build ran ten cases, requiring expected
trap diagnostics where applicable; native traps additionally require SIGABRT.
The report pins compiler, audit inputs, generated C and target binaries. The
[prior v2 harness failure](data/secret-checked-arithmetic-2026-10-29/runtime-v2-diagnostic.txt)
(SHA `f67a35acd53faddbde85e8eed1b2abb0de6237d63a3c900df019d77def079616`)
expected a more specific division diagnostic than the emitted one. The revised
fixture requires the actual division-by-zero message and reran the full matrix.

Two independent read-only reviewers found no concrete bypass in the scoped
arithmetic changes; they did not rerun tests or approve other implicit guards.
The reviewed policy SHA-256 values, scalar then legacy, public-view and large,
are `3e7385219a694808e7d55caca47a0abe631de07911062c4a1c29605d79c9bba8`,
`3c49933150583c3828ab23eaed21dbe282c8273367f3aec787bf7889acce5df6`,
`391dfdbbc101f1f0ecff76b0e0c4cc850763b596bc2e335b8838df5dfe54895b`, and
`8f219d6692df6c34a14ee156c6a964f1215a8e5072b6b5bc6ac7ef0394df2be3`.
The second reviewer also inspected the fixture source, C host and runtime
driver hashes recorded in the report. An investigated foreign-call default
initializer did not compile under the pinned compiler (E0408), so that
specific suspected counterexample was withdrawn; no complete default-initializer
closure follows.

These observations used the pinned `coax0 0.22.0` binary SHA
`f335e7ec7b3f6d5966244a09aa8002b11b5ba607a1f673353c51effd9dab84a1`.
Current main subsequently changed `coax/src/eval.cpp` for frozen aggregate
constants; these tests do not claim revalidation with a newly rebuilt compiler.
The subsequent full-reference aggregate runs killed all 2,803 legacy,
2,864 public-view and 2,955 large-profile mutants, with 160,669, 157,838 and
165,833 full checks respectively and zero skipped, borrowed or unstable checks.
Their retained [legacy](data/secret-checked-arithmetic-2026-10-29/legacy-reference.log),
[public-view](data/secret-checked-arithmetic-2026-10-29/public-reference.log),
and [large](data/secret-checked-arithmetic-2026-10-29/large-reference.log) logs
have SHA-256 `7cc25e93282092ccd201a7a16d0cda47080e75d5d2a3344eac1cac903621b84a`,
`dcd430fb65a8803d6681f1ae8431d44683fb6834df158f74552a13c73e0ccc96`, and
`13958e6c435a8426818b6026e0398119cc4e9cae4e66b1e01b22d8fc87b85ace`.
The shared tests were `17fb7be86bed9c44bb15cff5a7eae76c9bc98eb8657c2846ece2dc82a2512cd0`,
`58c054b4bfb4e60626a30830acf566e1f772c29fc269113c9078fe771faa51ac`, and
`c352871a69fa5615754b863adbd570b0d3dd907ebcd1c811a03544bae8beea0a`
for scalar, aggregate and aggregate cases, respectively.

The inherited compiler, source-map, view ownership and filesystem premises
still apply. This repair does not establish target constant time, production
secret handling, complete compiler equivalence, or absence of other implicit
runtime guards. The separate [cast correction](2026-10-29-secret-audit-checked-cast.md)
is retained; neither fix independently reinstates earlier conditional decisions.

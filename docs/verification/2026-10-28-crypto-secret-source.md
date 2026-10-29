# Aggregate crypto source analysis

The `aggregate-source-v1` profile is separate from the deliberately smaller
`unsigned-scalar-source-v1` profile. It inspects real typed source and direct
callees, including the selected field, scalar and masked group paths. It is
source/dataflow evidence. It grants no signing, ECDH, encryption, key custody or
production secret multiplication permission.

```sh
coax/bin/coax test tools/coax-nostr/secret-check/aggregate.coax
coax/bin/coax mutate tools/coax-nostr/secret-check/aggregate.coax
python3 tools/coax-nostr/secret-check/test_aggregate.py
python3 tools/coax-nostr/secret-check/aggregate.py \
  coax/lib/crypto/secp_field.coax --entry sf_inverse \
  --report /tmp/inverse-source.json
```

The last command exits unsuccessfully while authored guard obligations remain.
Inspect `supported_algorithm_dataflow` and `source_status`; do not turn a
completed analysis into an admission by ignoring `accepted`.

| Status | `accepted` | Meaning |
| --- | --- | --- |
| `refused` | false | Invalid or unsupported source, flow, type, alias or budget |
| `analyzed-with-guard-obligations` | false | Algorithm/dataflow analysis completed; actual authored assertions need discharge |
| `admitted-source` | true | Supported source/dataflow profile with no unresolved authored assertions; target guard/timing/security gates remain open |

`checked_runtime_guards_proved`, `object_timing_approved` and
`production_crypto_approved` remain false in every status. In particular,
ordinary checked arithmetic, indexing and narrowing may generate target guards
that this source analysis does not prove redundant or independent of secrets.

## Source and type identity

The trusted compiler performs actual type validation and emits its physical
source-map closure, resolved variable types and source SHA-256 values. The
unchanged Coax lexer, load scanner and parser then process those same snapshots.
Coax hashing and identity checks reject missing/changed sources before and after
analysis. Tool compilation also uses actual compiler maps and verifies its
compiled source snapshots. Native code only owns buffers, projects parser nodes
and transports evidence. Python is optional tool I/O, not a compiler, app or
bootstrap dependency. No user-authored AST, safe-callee list or flow proof is an
admission input.

A report binds the actual compiler binary, audit binary, source closure and tool
inputs. Canonical, inspectable compiler-resolved type inventories have their own
Coax SHA-256 and use physical unit indices, so retained path normalization does
not change their identity. It says nothing about files changed after the report
returns. Source
coordinates and resolved type rows join by physical unit, procedure name and
declaration line, variable name, role and line. Unrelated procedures in the
physical closure are hashed and normally type-checked; only reached procedures
are claimed flow-analyzed.

The entry has 1–8 parameters. Every entry's data is classified secret, with a
separate origin bit. There are no user-declared output declassifications in this
slice. Public fixed extents and admitted slice counts come from actual source
types and public bounds, not from treating a secret field as public. Literal and
global compile-time constants are public in this boundary model; it does not
discover private data embedded in source constants.

## Supported flow and explicit refusals

The type profile includes `u8`, `u32`, `u64`, `bool`, public-only `int`, fixed
arrays with literal extents of 1–1,024 elements and concrete structs with supported fields. Type
nesting is bounded by eight; immutable literal-field range traversal uses the
corresponding seven-level budget before the field leaf. Secret signed integers, unknown types, enums,
dynamic arrays, pointers and nested view containers refuse. Direct callee
signatures, local types and range-induction types come from real compiler
metadata. The actual compiler validates operator/cast/intrinsic argument
legality before this policy runs; this is not an external AST admission API.

Known-region views may be created from admitted fixed storage and passed to
typed direct helpers. Aggregate value copies get separate analysis regions;
views preserve their source region. Partial aggregate writes union origins, and
existing aliases observe whole-array replacements. Unknown aliases, view
returns and view descriptor rebinding refuse. A resolved view type currently
needs a matching supported type node in the physical closure: a bare inferred
slice without any corresponding view type declaration can refuse conservatively.

Field and array reads retain aggregate origins. Secret indices and unknown or
out-of-extent public index ranges refuse. Public intervals are bounded; unknown
numeric values do not become guessed constants. Constructor metadata is
invalidated by any write through its region, including a view.

The earlier implementation did not preserve that invalidation when copying a
dirty aggregate into a fresh region. A public field write followed by a
by-value helper call could therefore recover the old constructor field and skip
a reachable unsafe callee. The correction clears copied constructor metadata
when the source region is dirty. The earlier aggregate results below are
historical evidence, not a general soundness claim; previously accepted paths
must be revalidated against the corrected policy.
Identity casts could separately erase that dirty-region provenance; the corrected
profile conservatively refuses aggregate casts while retaining scalar casts.
Public byte arithmetic could also use an out-of-range mathematical interval to
skip a branch that release-mode wrapping reaches. The corrected profile loses
such an interval rather than using it as a control-flow fact.

Half-open range loops require known public bounds within 0–4,096 and must be
nonempty for every possible bound in their intervals. Induction
bindings cannot be written. `while`, collection loops, body returns, break,
continue and unsupported control flow refuse. Public statement conditions must
be known; public conditional expressions are conservatively analyzed on both
arms. Secret conditions, loop bounds and short-circuit controls refuse.

Loop analysis does not pretend one iteration proves a fixed point. Before
analyzing its body, it over-approximates backedges by tainting written outer
scalars and all outer aggregate regions with the union of incoming origins.
This also covers direct-callee writes through known views. It may reject safe
programs or retain an origin after a public overwrite; it never uses such an
overwrite as an unaudited aggregate declassification.
The earlier one-body analysis did not preserve zero-trip state; a possibly empty
range could regain a scalar constant and skip a reachable branch. Such loops
now refuse, rather than guessing the state after the loop.

Transitive named callees are inspected with actual argument origins and public
bounds. Foreign, indirect, unknown, recursive and shadowed calls refuse.
`_vartime` and `_public` procedures are excluded, including public-only decoding
helpers. Intrinsics retain origins; a secret shift/rotation count or secret
division/remainder refuses. Unsupported syntax stays refused.

## Guard obligations remain real

An authored `#assert` on secret-derived data is a real conditional guard, even
when a mathematical comment explains why it should always hold. It is recorded
with physical source identity, location, guard kind and origin mask. An unproved
public assertion is also an obligation. A known false public assertion refuses;
a known true public assertion does not need an obligation.

For example, `sf_normalize` asserts that a secret-derived carry is zero under
its admitted magnitude precondition. This profile does not supply a general
range-proof engine and does not trust that comment as a proof. `sf_multiply` and
`sf_inverse` therefore complete algorithm/dataflow analysis with unresolved
authored guards and `accepted=false`. The guard obligation must be discharged
by later range/target evidence and independent review, or the code must be
changed with an independently verified equivalent contract.

This is distinct from inserting an output declassification. No authored
secret `if`, loop bound, index or public-only callee receives an exception.

## Bounded evidence, remaining work

The common physical source bounds remain 16 units, 65,536 bytes per unit,
262,144 total bytes, 16,384 nodes and 4,096 resolved variable rows. The aggregate
profile separately bounds locals to 64, active calls to 16, distinct reached
procedures to 64, expression/block nesting to 16, visits to 262,144, regions to
4,095 and authored obligations to 256. These limits do not alter the scalar
profile's 65,536-visit budget or any event/protobuf/JSON/socket admission limit.

The integration cases exercise actual `sf_multiply`, `sf_inverse`, `sc_mul` and
`sg_choose` closures; public-only entries remain excluded. Real-source negatives
cover field controls, secret indices/bounds, aggregate copies, whole-array/view
aliases, loop backedges, induction writes and unknown effects. Checked and
optimized native aggregate masking runs against an independent bit oracle.
A deliberately lost aggregate write in emitted policy C must create a false
acceptance that the source oracle detects. Sanitizers cover the real frontend,
bridge, guard transport and Coax policy.

Generated object and Wasm/JIT control flow, instruction timing, platform effects,
complete alias/lifetime safety, secure erasure, full arithmetic range discharge
and independent cryptographic review remain separate gates. Completing this
profile does not close those gates or approve production secret use.

The retained [observations](../../tools/coax-nostr/secret-check/evidence/aggregate-2026-10-28.json)
contain 156 passing Coax checks and all 2,373 policy mutants killed, with no
waivers. Six real-source tests pass, with 42 retained cases, native ASan/UBSan,
4,000 checked/optimized aggregate masking comparisons, compiler-change refusal
and an independently caught lost-alias-write defect. The shared driver also
passes the existing six scalar-profile regressions. These checks cover the
changed policy and tool boundaries; no full compiler/bootstrap or app rebuild
was needed.

At this snapshot `sc_mul` completes with eight reached procedures and no authored
obligation; `sg_choose` completes with three. `sf_multiply` completes with six
procedures and one obligation, and `sf_inverse` with thirteen and two obligations.
The latter two remain `accepted=false`. Exact source, type inventory, compiler
and tool identities in the observations take precedence over future file
contents. The default/development compiler path is analyzed; conditional build
selection is not an advertised capability of this profile.

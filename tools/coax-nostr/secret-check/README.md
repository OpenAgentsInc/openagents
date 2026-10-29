# Scalar secret-path source audit

**Security status:** historical scalar and aggregate audit acceptances are
untrusted pending revalidation after the checked-cast and checked-arithmetic
corrections. See the [arithmetic advisory](../../../docs/verification/2026-10-29-secret-audit-checked-arithmetic.md)
and [cast advisory](../../../docs/verification/2026-10-29-secret-audit-checked-cast.md).

This is an executable, conservative source/dataflow policy, not a new language
annotation and not cryptographic admission. It uses the real Coax lexer, parser,
source-load scanner and typed compiler source map. It does not accept a supplied
AST, a flow manifest or a list of purportedly safe callees as proof.

```sh
coax/bin/coax test coax/lib/crypto/secret-flow.coax
coax/bin/coax mutate coax/lib/crypto/secret-flow.coax
python3 tools/coax-nostr/secret-check/test_secret.py
python3 tools/coax-nostr/secret-check/check.py coax/lib/crypto/bytes.coax \
  --entry cb_ct_select64 --report /tmp/select-source-audit.json
```

The optional Python command is tooling I/O: compiler invocation, files, C ABI
buffers and JSON reporting. It is not part of the compiler bootstrap, application
build or runtime. The C++ bridge owns storage and projects the real parser's
indexed nodes; compile-time assertions bind its tags/operators to generated
frontend headers. All source scanning, parsing, flow decisions, source identity
matching and admission limits are Coax. No source grammar is reimplemented with
regular expressions. The test-only lost-origin control modifies generated C to
prove that the independent real-source rejection oracle detects false acceptance.

## Snapshot and entry contract

The tool first runs the trusted compiler's `c --library --source-map`, without
executing the target. The typed map supplies the actual physical source closure,
SHA-256 fingerprints and resolved parameter/local types. Each snapshot is then
parsed with the unchanged Coax frontend. Coax SHA-256 and identity checks compare
its bytes to the typed map before and after analysis. Changed or missing source,
reordered/duplicate closure metadata, failed type validation and unsupported
syntax refuse. A report applies to those source/compiler/tool artifact hashes;
it is not a promise that the filesystem stays frozen after the report returns.
There is no persistent allowlist cache.

The compiler's normal validation and allocation budgets apply first. The audit
then applies smaller Coax-owned bounds: 1–16 source units, at most 65,536 bytes per
unit and 262,144 total bytes, 16,384 projected nodes, 4,096 resolved variable rows,
64 local bindings, 64 distinct reached procedures, 16 active procedure frames,
16 expression nesting levels and 65,536 visits. Descriptor-generated imports are
not admitted by this profile. These are audit limits, separate from protobuf,
event, JSON and socket limits.

The named entry must have 1–8 parameters. **Every entry parameter is secret**;
there is no caller-supplied public/secret classification that can downgrade one.
Independent origin bits propagate into the result, through direct helper calls,
locals and compound assignment. Plain assignment replaces the old origin.
Resolved parameters/locals and explicit return types must be `u32`, `u64` or
`bool`. Literal constants are public; referenced global constants are inspected.

Admitted expressions include unsigned scalar bit/arithmetic operations, selected
casts, public conditional expressions and the explicitly handled
`value_barrier`, `add_wrap`, `mul_wrap`, `rotl` and `rotr` intrinsics. Secret casts
require explicit truncation. A secret rotation/shift count refuses. Secret
short-circuit conditions, conditional expressions and statement conditions
refuse. Secret division/remainder refuses. Direct helper calls are traversed
transitively; recursion, foreign/indirect/unknown calls, shadowed callees and any
`_vartime` callee refuse, even if its current argument is public.

Reachable arrays/views, field access, aggregate types, loops, nested control flow,
reflection/metaprogram expressions, uninitialized locals, unreachable trailing
statements and any unhandled expression or statement refuse. Unrelated procedures
in the same source closure are not claimed to have been flow-audited. Public loops do not yet get an
exception. Secret indices have a specific diagnostic; public memory access is
still unsupported. Reports keep unsupported sources rejected.

## Evidence and open gates

The retained [source cases](evidence/source-2026-10-28.json) record the trusted
compiler, actual tool/target hashes and source decisions. Forty Coax checks pass;
all 741 policy mutants are killed, with no equivalent waivers. The six real-source
tests cover transitive callees, changed helpers, concurrent snapshot drift,
actual byte helpers, a deliberately broken origin-return rule, 4,000 checked and
optimized native masked-select comparisons against independent Python bit
operations, and the real frontend/bridge/policy under ASan and UBSan. The expression
nesting boundary also runs through actual parsed source, not only model fixtures.

`cb_mask`, `cb_ct_select32` and `cb_ct_select64` pass this source profile.
**The actual scalar multiplication implementation is explicitly refused.** Its
arrays and loops require a later aggregate/public-loop analysis and real crypto
closure integration. This slice clears neither the production secret
multiplication gate nor signing, ECDH, encryption or key-custody admission.

A source dependency mask is not a complete constant-time proof. There is no
proof of arithmetic ranges or secret-dependent checked runtime guards, emitted
object control flow, optimizer transformations, JIT behavior, speculation,
platform instruction timing, alias/lifetime safety or secure erasure. The report
sets `checked_runtime_guards_proved`, `object_timing_approved` and
`aggregate_memory_loops_supported` to false. Existing native/Wasm timing probes
and caller-buffer erasure contracts remain separate, and do not upgrade this
profile automatically. No `#secret` syntax is implemented or advertised.

The separate [aggregate crypto source profile](../../../docs/verification/2026-10-28-crypto-secret-source.md)
adds actual typed fields, fixed arrays, known view regions and bounded public
loops. It preserves scalar-v1 semantics and reports unresolved authored guards
as `analyzed-with-guard-obligations` with `accepted=false`. Its source analysis
is also subject to the independent guard, target timing and production gates.

`source-policy.coax` refuses actual lexer `#if` directives in every physical
source unit before either audit calls the build-selecting parser. Ordinary
runtime `if` and string contents such as `"#if"` keep their normal meaning.
Neither source profile advertises conditional build-selection coverage.
Run `test_source_policy.py` with `COAX_SOURCE_POLICY_COMPILER` set to a
conditional-capable compiler to exercise real development/release inputs; this
is focused audit integration tooling, not an app or bootstrap dependency.

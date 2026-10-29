# Coax inline artifact-envelope profile

Task `nostr-envelope`, worker `codex-nostr-codecs`, 2026-10-28.

## Boundary and responsibilities

`coax/lib/nostr/envelope.coax` builds and consumes the selected inline body of
`openagents.artifact-envelope.v1` from the pinned
[`contracts.md`](../../nips/openagents/contracts.md). The memory bridge observes
physical overlap; Coax owns schema shape, feature policy, budgets, canonical
serialization, digest binding and failure results. C/Wasm test faces only move
public fixtures and expose scalar observations.

The builder accepts a JSON object, an opaque caller-selected schema identifier,
an issued time, a strictly later retain time, scratch storage and an output
buffer. It hashes the safe-integer canonical JSON bytes, then emits the legacy
field order while embedding the supplied original inline JSON. It does not
sort or reserialize the original inline bytes inside the envelope. The consumer
requires the same expected schema, recomputes canonical size and digest, and
returns the original inline byte range and the admitted timestamps. Schema
identity matching does not validate that schema's semantics or fetch it.

Required root fields are `v`, `requires`, `artifact`, `inline`, `issued_at` and
`retain_until`. The artifact requires `digest`, `size`, `media_type` and `schema`.
Only `application/json`, inline objects and an empty `requires` array are
supported. Unknown semantic fields, nonempty feature requirements, external
`inline: null`, ArtifactRef event/source locators, wrong versions and duplicate
or escaped-equivalent fields refuse. An optional root `meta` object is inert;
its contents still pass complete bounded JSON/Unicode admission.

This is a plaintext-body component. NIP-44 encryption, event-ID/signature
verification, recipient authorization, persistence, schema-specific admission,
locator resolution and retention enforcement remain separate. The time fields
are observations and a retention request; this component does not compare a
clock or issue an authorization lease. Original signed event bytes are never
replaced by a reserialized form.

## Independent budgets and closure

The immutable protocol wire ceiling remains **1,048,576 bytes**. The selected
Coax body profile is narrower; it does not redefine the published protocol:

| Resource | Selected limit |
| --- | ---: |
| Observed outer wire count | 1 through 1,048,576 bytes |
| Original body/input fragment, canonical bytes, output capacity | 524,288 bytes each |
| Raw and decoded individual string | 65,536 bytes each |
| Whole-body container depth | 16 |
| Whole-body value/container nodes | 2,048 |
| Whole-body object members | 1,024 |
| Whole-body decoded member-name bytes | 65,536 |
| Whole-body UTF-16 comparison work | 1,048,576 iterations |

The body's wrapper consumes ten nodes, ten keys, seventy member-name bytes,
twenty-two comparison iterations and one container level. The builder reserves
each quota before emitting. Thus the inline object is limited to 2,038 nodes,
1,014 keys, 65,466 key bytes, 1,048,554 comparison iterations and fifteen
container levels. Limiting only output bytes would have allowed the builder to
emit bodies its own consumer refuses. The native and actual Wasm harnesses
exercise successful build-to-consume closure and one-over refusal separately
for all these axes, including real adversarial long-prefix comparison inputs.

A full 512 KiB canonical inline payload cannot also fit its wrapper within the
512 KiB body cap. Canonical admission is tested independently, and an exact
512 KiB emitted body is tested with original whitespace and a small canonical
inline value. Bounds are explicit refusals, never truncation.

`envelope_check_body` has no outer-wire observation; its successful
`wire_bytes` is zero. `envelope_check_observed` separately admits a positive
caller-supplied observed count. That count must be measured truthfully by the
host at the protocol boundary. Neither entry point derives encrypted wire size
from plaintext, and a successful build makes no promise that a later encrypted
and framed event will satisfy its wire ceiling.

Numbers and Unicode follow the shipped safe-integer JCS profile: plain integer
tokens within ±(2^53−1), negative zero canonicalized to zero, no fractions or
exponents, UTF-16 property ordering, Unicode-scalar/noncharacter admission and
no normalization. General floating RFC 8785 serialization remains unsupported.

## Storage and failure behavior

Callers own valid, stable allocated spans. The bridge rejects input/schema
overlap with scratch/output and scratch/output overlap. Count and physical
overlap checks do not establish pointer ownership. Output is measured completely
before commit: malformed, unsupported, digest, time, alias and capacity failures
return zero output/metadata and preserve the output. Successful output tails and
all input/schema bytes stay unchanged.

Scratch is mutable workspace, including on failure. The canonical prefix is
ordinarily zeroed after hashing, including a subsequent digest mismatch. Local
tree/decoded storage and SHA state are not securely erased, and ordinary reset
is **not a secure-erasure guarantee**. No production key, private content,
storage or logging path is introduced. The bounded JCS tree and decoded arena
use local storage; there is no dynamic allocation.

Error values are `AE_LIMIT` 1, `AE_JSON` 2, `AE_SHAPE` 3, `AE_VERSION` 4,
`AE_FEATURE` 5, `AE_SCHEMA` 6, `AE_DIGEST` 7, `AE_SIZE` 8, `AE_TIME` 9,
`AE_CAPACITY` 10, `AE_ALIAS` 11, `AE_WIRE` 12 and `AE_HASH` 13. Zero is
success. Complete JCS/parser refusal, including its resource constraints,
maps to `AE_JSON`; builder wrapper reservation refusal maps to `AE_LIMIT`.
Error ordering is not a promise to diagnose every defect in a malformed body.

## Verification and existing presentation compatibility

```sh
coax/bin/coax test coax/lib/nostr/envelope.coax
coax/bin/coax mutate coax/lib/nostr/envelope.coax
python3 coax/tests/nostr/envelope.py --evidence /tmp/envelope.json
```

Checks were written and failed before the implementation. Mutation findings
expanded zero/minimum count, absent/type/time, workspace preflight and scratch
reset checks. Shared size preflight replaces duplicated output-limit branches.
An encoded-string length helper tests exact scalar budgets in Coax while the
actual large strings still execute in native/Wasm tests. Final authority
closure: **94 check groups pass; 2,892/2,892 mutants killed, zero survivors and
no exemptions**. These include loaded JSON/JCS/codecs/byte/SHA/fingerprint
checks; totals are not disjoint from those modules.

The first closure also reached the recently added generic `json_f64` helper,
although no envelope/JCS product path calls it. With taskmaster approval, that
helper and its checks moved byte for byte from commit `069e2cb5a3` to the
opt-in `json-f64.coax`. Its only actual consumer, Claude SDK `fields.coax`, now
loads it explicitly. The helper-plus-checks SHA-256 is
`00a541977c91979cada1cd9fc4e4f2d15df2d785dfa89f97c6add5e8dcdbdc73`.
This is a real source-graph separation, with no mutation exemptions, suppressed
checks or integer/parser behavior change. The approximate float helper retains
its existing behavior and is not qualified for Nostr admission/canonicalization.
Targeted generic JSON (20), opt-in helper (21) and Claude SDK (62) check groups
pass. Its original precision/underflow mutation findings are not represented as
covered by the envelope task. A stored bounded-start limit metadata assertion
was added to the parser's existing check without changing the parser.

**283 records** pass in checked and release native C under ASan/UBSan and in
actual Wasm under Node/V8. These include 130 builds, each consumed immediately
by the same selected consumer and tested again with a one-short output, and
153 body/observed-wire checks. Independent Python UTF-16 canonical serialization
plus `hashlib` and a separate ECMAScript canonical/hash oracle agree on the
210 supported records. Tests cover closed fields, inert metadata, unsupported
features, source/schema preservation, digest/size tampering, invalid types,
safe time boundaries, raw/decoded strings, independent resource ceilings,
capacity, aliases, negative spans and complete zero metadata on refusal. Each
one-over reserved-resource builder case is also independently wrapped and
refused by the consumer. Digest mismatch tests observe ordinary scratch reset;
consumer one-short scratch tests observe complete zero failure metadata.

`envelope-test-deck-v1.json` freezes an actual `sealDeck` result from the existing
unlock library, locally decrypted through its three NIP-44 layers using only
public throwaway scalars 1 and 2. No relay publication or real identity was used.
Its 468-byte decrypted body must match the new builder byte for byte. Both
independent oracles and both Coax targets preserve the existing 182-byte
canonical inline deck and digest:

```text
sha256:31f0e9941da64dec653587f668942226e5aa282ce9be9404752c412e38dfdf89
```

The fixture records the legacy library and prior JCS fixture hashes. The
artifact digest covers canonical inline content, not the envelope, encrypted
wrapper or original Nostr event bytes.

Retained evidence is `coax/tests/nostr/data/envelope-2026-10-28.json`, including
compiler/transitive-source/corpus/module hashes, opt-in extraction parity and
actual toolchain versions. The trusted
coax0 0.15.0 binary was retained throughout. No compiler bootstrap, full app
suite or TestFlight build was repeated. Browser-engine/physical-phone target
acceptance and full encrypted event integration remain separate gates.

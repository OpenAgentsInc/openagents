# JSON admission: 2026-10-28

## Contract

`coax/lib/nostr/adapters/json-admission.coax` feeds the real pull parser's
observations to `json-policy.coax`. The latter is the `#authority` policy;
the former only sequences decoding and passes its result through. Neither
re-encodes, truncates, stores a reference to, or changes the original input.
The caller must supply valid live source spans and truthful frame/protobuf
byte counts. Native pointer validity is not established by an integer count.

Three version-1 structural profiles select independent maxima:

| Domain | Original JSON | Frame | Protobuf | Content | Numeric fields |
| --- | ---: | ---: | ---: | ---: | --- |
| 0 event | 262144 | 262144 | unavailable | 65536 | kind, created_at |
| 1 JWS payload | 65536 | unavailable | unavailable | unavailable | exp, iat, nbf |
| 2 envelope | 524288 | unavailable | 1048576 | 65536 | kind, created_at |

All profiles cap raw strings and decoded strings at 65536 bytes, depth at16,
total keys at1024, retained key bytes at65536 and comparison work at1048576.
A policy may narrow a nonzero budget, never enlarge or disable it. Unavailable
axes require a zero observation. Original JSON must fit its frame when a frame
axis is enabled; a protobuf budget does not enlarge any JSON/string budget.
The domain-2 JSON adapter remains narrower than the generic protobuf reference
consumer: it inherits the actual parser's depth and string limits.

Every decoded key is compared within its original object scope, including
escaped equivalent names, embedded NUL and non-ASCII. Identical names in distinct
objects are allowed. All strings must decode to Unicode scalars; lone escaped
surrogates refuse. No Unicode normalization is performed. Keys and comparison
work are cumulative over the document, with explicit failure rather than an
unchecked collision or truncated list.

Selected integer fields require integer syntax and values in
[-9007199254740991,9007199254740991]. Unknown fields/numbers retain their original
bytes, including fractions, exponents and larger magnitudes. The generic
`json_int` API still admits ±2^53. Structural admission does not establish NIP-01
field completeness, kind range, event ID/signature validity, JWT claim types or
authority, HOST grants, JCS, or protobuf/original semantic agreement. Those are
separate downstream adapters and tasks. In particular, this JWS payload profile
is not a JWS header algorithm allowlist or a verifier.

`ja_admit(text, ja_policy(domain), frame_bytes, protobuf_bytes)` returns:
0 success;1 invalid policy;2 size/observation;3 JSON/decoder;4 string/content;
5 duplicate;6 key arena/count;7 comparison work;8 selected numeric field.
A refusal returns no admitted projection. First error sticks in policy state.

## Checks and independent evidence

Commands use the existing0.15.0 compiler; no bootstrap/compiler change:

```sh
coax/bin/coax test coax/lib/nostr/adapters/json-policy.coax --library
coax/bin/coax mutate coax/lib/nostr/adapters/json-policy.coax --library
coax/bin/coax test coax/lib/nostr/adapters/json-admission.coax --library
coax/bin/coax test coax/tests/nostr/json_corpus.coax --steps 200000000
python3 coax/tests/nostr/json_admission.py
python3 coax/tests/nostr.py CoaxJsonTests
python3 coax/tests/nostr/profiles.py Profiles.test_integer_bound_is_stricter_than_json_parser
bun test coax/web/project/nostr-plan.test.ts
```

The policy has9 checks (including30 policy-field variants) and **336/336
mutants killed**, no equivalence annotations. The real decoder integration has
23 combined checks. Independent Python JSON/pair/number handling checks3363
cases per emitted checked/release mode under native ASan/UBSan. These include
escaped duplicates, selected integer boundaries, unknown numeric preservation,
surrogates, depth and actual64KiB string boundaries. The C driver observes no
input modification. Python is optional testing tooling, never a compiler/app
dependency. A trapping allocator still parses every pinned relay frame.

The policy state owns about90KiB of bounded key arena/metadata and scope data;
the decoder adds64KiB scratch. Generated C may use additional copies/stack,
so this is a storage inventory, not a measured maximum stack guarantee. Target
stack, performance and broader iOS/Wasm conformance remain consumer evidence.

## Existing parser evidence and remaining gap

Only the heavy captured-relay `#test` moved out of `json.coax` into an explicit
integration driver. This avoids exhausting the default compile-time interpreter
budget when the parser is loaded with new authority checks. All8 checks in that
driver pass. The parser implementation prefix is byte-identical to1b269c7ae3;
the emitted-C difference consists of source provenance ranges and a removed,
unused check-only65536-byte array display helper/type, not parser procedures.
The profiles test now imports the actual parser, with no runtime-prefix copy.

The first exploratory full-closure mutation run exposed pre-existing parser/
writer survivors; it is not a successful decoder mutation gate. The new policy
is tested separately over observations, with the actual decoding path covered
by independent integration. No parser survivor was hidden with an exemption or
compiler scope filter. `nostr-json-parser-checks` tracks closing that existing
coverage gap before event/JCS/JWS verification tasks are declared complete.

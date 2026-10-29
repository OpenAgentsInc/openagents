# Bounded NIP-01 event read and verification

`nostr-event` (#7cdf) adds `coax/lib/nostr/event.coax`. It composes the shipped
JSON parser, decoded-key admission policy, hex decoder, SHA-256 and public
BIP-340 verifier. This is a shared read path, independent of signing, secret
key custody, fixed-base secret multiplication and NIP-42 authentication.

## API and ownership

```coax
nostr_event_read(original_json, preimage_scratch, overlap) -> NostrEvent
nostr_event_verify_vartime(original_json, preimage_scratch, overlap) -> NostrEvent
```

Both entries admit the complete original JSON and recompute its event ID. The
read entry never sets `verified`. The verify entry additionally verifies the
BIP-340 signature over that exact 32-byte ID. All inputs are public and the
complete verification path is variable-time. A successful signature does not
establish freshness, an allowed author, project scope, delegation, durable-store
admission or permission to perform an action.

`original` is a borrowed view, with byte spans for the raw content string and
tags. Unknown fields, whitespace, original numeric spellings and escapes remain
in that original view. Decoded IDs/public key/signature and computed ID are fixed
arrays in the result. The original buffer must remain alive and unchanged while
using its spans; the call never retains it elsewhere. Consumers can decode the
content span using the same `json_unescape` API. Tag arrays retain order,
duplicates and unknown tag names. Empty inner arrays retain their positions in
the ID preimage; this does not supply consumer semantics for an empty tag.

The caller supplies writable preimage scratch and a truthful input/output
pointer-overlap observation. An overlap refuses before access. Admitted native
buffers must actually be readable/writable for their counts. This contract is
not foreign-pointer memory safety. On success `written` is the exact preimage
length. On any refusal it is zero and `verified` is false. Output scratch may
contain a partial preimage after refusal and must not be published as successful
output. No product allocation or Python dependency is introduced.

Internal parsed records and `ne_verified` observations are not authorization
interfaces. Only the complete raw-JSON verify entry may establish the signature
result. They are not exported through C; current Coax source composition does
not enforce a capability/private boundary against trusted source consumers.

## Explicit profile

This module starts with the planned project-event consumer's bounds, rather
than claiming universal Nostr support:

| Dimension | Admitted maximum |
| --- | ---: |
| Original JSON | 262,144 bytes |
| Preimage scratch view | 262,144 bytes |
| One raw string | 65,536 bytes |
| One decoded string/content | 65,536 UTF-8 bytes |
| Tags | 2,000 |
| Values per tag | 16 |
| JSON nesting | 16 |
| Object keys, whole original | 1,024 |
| Decoded key arena | 65,536 bytes |
| Key comparison work | 1,048,576 units |

Required fields have their exact NIP-01 types. IDs/public keys/signatures are
decoded lowercase-only hex with exact sizes 32/32/64 bytes. `kind` is in
0...65,535. This consumer's `created_at` is a nonnegative integer through
2^53−1. Floats/exponents in required integer fields refuse; `-0` projects to
zero. There is deliberately no current-time/future-age test. Generic JSON
admission also applies its existing safe-integer rule to identically named keys
inside unknown objects; these narrower consumer limits are not universal
signature-invalidity claims. Duplicate decoded object keys, lone surrogate
escapes, malformed UTF-8, missing fields and any exhausted bound refuse instead
of truncating. Unknown fields otherwise remain permitted and preserved.

Canonicalization emits exactly
`[0,pubkey,created_at,kind,tags,content]`. It decodes string escapes, uses UTF-8,
preserves array ordering, uses the shared NIP-01 JSON string/integer writers,
and hashes those actual emitted bytes. It is not JCS/object-key sorting or a
protobuf reconstruction. ID equality is checked before signature verification.

## Evidence and limits

The explicit optional test driver is `coax/tests/nostr/event.py`. It compiles
the actual source to checked/release native C11 and real wasm32 artifacts.
Python's JSON/SHA-256 and independent ECMAScript `JSON.stringify`/Node SHA-256
produce equal exact preimages/IDs for 96 fixtures spanning every control-byte
escape, slash, UTF-8, astral characters, U+2028/U+2029, empty content/tags and
safe-integer/kind boundaries. Signatures use deterministic throwaway keys through
the already pinned libsecp256k1 test oracle, with eight independently checked
big-integer affine signature verifications. This is test tooling, not a Coax
signing or production runtime dependency.

There are 156 end-to-end cases per native profile and per Wasm profile. The
refactored source and the original failed-gate snapshot both run in checked and
release mode, giving four native and four actual Wasm profiles. They include altered signed fields/ID/signature, invalid points, duplicate escaped
keys, wrong/missing types, capacity and view-count refusals, exact/over tag and
string limits, original retention and read-only/guard checks. ASan/UBSan executes
a positive verify and short-output refusal. Four actual emitted-C faults remove
ID comparison, bypass signature rejection, lose content-field recognition or
drop tags; the oracle detects each. Finite tests/mutation are not a security or
compiler-correctness proof.

The Wasm test explicitly provisions a 1 MiB C stack. The default 64 KiB stack
trapped in the composed JSON admission/decode chain; successful tests do not
claim that a consumer with the default stack can call this module. A consumer
must provision and test its own stack/scratch budget before integration. Wasm
evidence is Node/V8/WASI, not Safari/Firefox, a browser UI or physical iOS.
The existing production `coax/web/wasm.sh` already provisions this same 1 MiB
stack; the focused test now matches that host contract without a host change.

Authored Coax checks cover real parsing, canonical bytes, hashing, refusals and
verification-result composition. As with the underlying verifier, a complete
positive BIP-340 multiplication exceeds the default interpreter check budget;
emitted end-to-end tests supply practical evidence without claiming an
interpreter proof or raising its budget. The underlying verifier's separately
recorded hour-long differential run remains its own frozen evidence; it was
not rerun or relabeled as this new event consumer or compiler version.

Validation commands and the final check/mutation counts are recorded with
the scoped report in `coax/tests/nostr/data/event-focused-2026-10-29.json`.
The next app probe must consume this module, rather than implement a second
minimal event parser. Production relay fold/store admission remains gated by
the separate verified-project integration and production-cutover work.

The emitted correctness and frozen mutation runs use Coax 0.17.0, compiler
SHA-256 `867bf5549dc4d4b20f698415280e92a81491b29c2dc201eec911e9d7d3faeeb2`.
That executable is preserved separately from the release caller. A later
compatibility check with 0.18.1 exposed the unchanged JSON boundary test's
1,048,577-slot array exceeding the new compiler cap. The measured 0.18.2
compatibility fix preserves that test and passes the original graph's 177 checks
and an authority-enabled native C build. Its separate compiler hash and exact
source observations are in
[`coax-run-storage-compat-2026-10-29.md`](coax-run-storage-compat-2026-10-29.md)
and the accompanying JSON. Those results do not relabel the earlier artifacts
or imply a repeated crypto differential run on the newer compiler.


## Failed first gate and observable refactor parity

The first complete mutation run did **not** pass: 3,618 of 3,631 mutants were
killed, with 13 survivors. Its exact source pins, original focused report and
survivor sites remain in
`coax/tests/nostr/data/event-initial-mutation-2026-10-28.json`.
The historical local snapshot `f5581bacd14ca501ab667e6bb25322e7c31bf837`
was created **after** that run completed. It records the run's source bytes; it
is not claimed to have existed at the start of the run or to have passed its
gate. Its event source SHA-256 is
`45118f2a7a9b5dc19466099fd0553ab1529de6cf768e16ce70a59df27e88e4d2`.

The follow-up keeps every authored and imported check, moves cheap event checks
before expensive imported checks, and adds three meaningful groups. Comma state
is represented as booleans; empty tag strings and the real 2,000/2,001-tag input
boundary are exercised directly. A plain event record replaces an unnecessary
one-element wrapper. Hex dispatch is functional rather than overwriting a
provisional result, and lowercase admission checks the case bit before generic
hex decoding. Sticky writer and hash-result helpers are exercised with actual
codec/SHA success, capacity and alias results. The JSON adapter uses the normal
default count declaration: non-string tokens do not consume a decoded count.
No equivalence markers, mutant filtering or additional budgets are used.

The first revised event source SHA-256 was
`ee3b610f68ecfaf16458e296ed63d8ca50ce32f5b8eb1f6ad7215c369d7e94c3`.
Its emitted tests compare against the actual original snapshot, rather than a
reimplementation of it. All 156 cases agree on error, admission, verification
and written length. Successful cases additionally agree on parsed fields,
borrowed original spans, exact canonical bytes and computed ID. Partial metadata
on a refusal is deliberately outside the API contract. The independent
Python/ECMAScript preimages, signature checks, sanitizers and seeded faults also
remain required; matching the old implementation alone is not correctness
proof.

That intermediate graph passed 180 authored/imported checks on the frozen
0.17.0 test compiler. Its complete mutation run still had one survivor:
3,589 of 3,590 killed. Removing the unconditional hash-result overwrite could
not change the success path, because the full entry supplies a fixed 32-byte
hash output. The final form directly returns the hash refusal when one occurs,
then continues the ordinary success path without an identity overwrite. This
failed intermediate gate and its exact pins are preserved in
`coax/tests/nostr/data/event-refactor-mutation-2026-10-29.json`.

The final frozen event source SHA-256 is
`3b0c0feca25959ecd4a4138a3e74a795c65258198fcb1510c0f9e5b2d3e5419f`.
It again passes 180 checks and all six native/Wasm/parity groups; the final
complete mutation result belongs to this source, not either failed snapshot.
Separate compatibility observations from the compiler work on the intermediate
`ee3b610f...` graph are
180/180 with the 0.20.0 stage-zero executable SHA-256
`768f0b94c62e4d4b43550944e61245f417ff3951109a23a6bc2dd6f9367fd743`
and 180/180 with the narrowed traversal-cap prototype SHA-256
`6f302ad96eb34799441d63b53c21f4393a57896d577721e526a2e1f389bff1f1`.
Those are compatibility checks on the named intermediate source, not
repetitions of the final emitted differential or mutation gate under a new
compiler.


## Accepted frozen gate

The final source passed **180/180 checks and 3,588/3,588 reachable mutants**,
with zero survivors, filters or exemptions. The unmodified frozen mutation run
started at `2026-10-29T05:41:01.343728Z` and completed in 2,926.156 seconds.
All 22 actual Coax input hashes still matched after completion. The six emitted
native/Wasm/oracle/parity groups passed in 51.077 seconds on that same source.
The final report records exact compiler, source, artifact and log hashes,
including both failed earlier gates. No existing compiler, interpreter or
mutation resource ceiling was raised for this work.

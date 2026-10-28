# Coax HMAC-SHA256 and HKDF

`coax/lib/crypto/hmac.coax` and `hkdf.coax` implement RFC 2104 HMAC-SHA256
and RFC 5869 SHA-256 extract/expand in checked Coax. Their `#authority`
checks consume the RFC 4231 cases 1–7 and RFC 5869 SHA-256 cases 1–3.
The `*-memory.coax` adapters observe borrowed-view overlap; all admission,
hashing, state transitions and output decisions stay in the pure core.

Load `coax/lib/crypto/hkdf-memory.coax` for the complete native C ABI, or
`hmac-memory.coax` for HMAC only. Normal compilation needs no Python and
does not allocate target memory. Python is an explicit independent test
oracle, never a product or build dependency.

## APIs and storage

| Procedure | Contract |
| --- | --- |
| `hmac_sha256_state_size()` | Returns 224 bytes |
| `hmac_sha256_init(state, key)` | Initializes an exact 224-byte caller-owned context |
| `hmac_sha256_update(state, input)` | Appends a borrowed chunk to an active context |
| `hmac_sha256_final(state, out)` | Writes the 32-byte MAC; repeated finalization returns the same MAC |
| `hmac_sha256(key, input, out)` | One-shot 32-byte MAC |
| `hmac_sha256_info(state)` | Reports status, phase, user message bytes and buffered bytes |
| `hkdf_extract(salt, ikm, out)` | Writes the 32-byte pseudorandom key |
| `hkdf_expand(prk, info, out, length)` | Writes exactly the requested prefix of the expansion |

HMAC stores two private 112-byte SHA contexts. The inner state includes the
64-byte keyed prefix; `hmac_sha256_info.total` excludes that prefix. The outer
state contains 64 bytes while active and 96 bytes after finalization. An
uninitialized, mismatched or malformed pair is refused. A finalized context
accepts another `final`, but refuses `update`; initialize it again to reuse it.
These are internal process-local contexts, not a versioned serialization
format or an authenticated object supplied by another party.

Keys and individual message chunks are bounded to 1 MiB. Streaming message
length is bounded by SHA's cumulative maximum of 2^61−1 bytes, including
HMAC's 64-byte prefix. Keys longer than 64 bytes are hashed first. Empty keys
and messages are valid. HKDF's empty salt is equivalent to the RFC's absent
salt of 32 zero bytes because both form the same padded HMAC key block.

HKDF expand requires a PRK of at least 32 bytes, at most 1 MiB, `info` of
0–1024 bytes, and `length` of 0–8160 bytes (255 SHA-256 blocks). The info
bound is a local resource policy, not an RFC limit. Output views must have
enough capacity and pass the shared 1 MiB view bound. Zero-length output is
a valid no-op. Expand stages complete blocks in a fixed **8160-byte local
buffer**, then copies only the requested prefix, so refusal cannot leave
partially derived output. Account for this stack cost plus nested HMAC/SHA
working contexts on each target; there is no heap allocation.

The NIP-44 message-key expansion is exercised with every one of the 32
pinned official cases: 76 bytes split into a 32-byte ChaCha key, 12-byte
ChaCha nonce and 32-byte HMAC key. This does not implement or approve NIP-44
encryption, envelope handling or secret storage.

## Errors, aliasing and trust limits

Statuses reuse SHA's constants: `SHA_OK=0`, `SHA_BAD_STATE=1`,
`SHA_BAD_INPUT=2`, `SHA_BAD_OUTPUT=3`, `SHA_PHASE=4`, `SHA_ALIAS=5`,
`SHA_LENGTH=6` and `SHA_BAD_TAG=7` (unused by these APIs). Refused calls
preserve caller state and output. Successful digest calls change only the
first 32 output bytes; expand changes only its requested prefix.

Adapters conservatively refuse an output/context view overlapping an input,
including unused capacity in that view. Read-only inputs may overlap each
other; disjoint slices of one allocation are valid. Empty updates have no
input access, and a zero-length expansion writes nothing. Native callers
must supply truthful, live pointers/counts and serialize access to a context;
metadata admission cannot prove pointer provenance or lifetime. The pure
`*_checked` procedures take overlap facts for trusted adapters and checks.

Contexts, key blocks, derived keys and staging buffers retain sensitive
material. There is **no secure-erasure guarantee, production-secret approval
or compiler-level constant-time assurance** in this slice. Separate erasure,
side-channel, consumer-admission and independent review gates remain open.
The unchanged SHA compression implementation is still shared through
`coax/lib/proto/fingerprint.coax`; its later relocation requires an explicit
compiler seed/provenance migration, not an unreviewed duplicate kernel.

## Focused evidence

```sh
coax/bin/coax test coax/lib/crypto/hkdf.coax
coax/bin/coax mutate coax/lib/crypto/hkdf.coax
python3 coax/tests/nostr/hmac_hkdf.py
```

On 2026-10-28, Coax 0.15.0 passed **56 combined checks** and killed
**937/937 mutants**, with no equivalent-mutant waivers. The explicit test
harness passed nine tests across checked and wrapping-release C emission:

- all seven RFC 4231 cases (the RFC's truncated case is checked as a prefix,
  and the API's full MAC is checked against Python's independent HMAC);
- all three RFC 5869 SHA-256 cases, deterministic random inputs, key lengths
  around 64, streaming chunks, repeated finalization and malformed contexts;
- HKDF lengths through 8160, zero/partial blocks, all 32 NIP-44 expansions,
  error preservation, output guards and real whole-view aliases;
- actual native ASan/UBSan execution and actual wasm32-wasip1 execution in
  Node/V8/WASI for the RFC cases, NIP-44 expansions and boundary lengths;
- manifest-pinned fixture bytes and the unchanged shared SHA kernel.

The harness requires the explicit Wasm tools documented by
`coax/web/wasm.sh`; it refuses to silently skip that target. This evidence
does not exercise Safari, JavaScriptCore, a physical iPhone or the app UI,
and finite mutation and differential checks are not a cryptographic proof.
Fixture provenance and licenses remain in `coax/tests/vectors/manifest.json`
and its retained upstream files; no upstream fixture is changed here.

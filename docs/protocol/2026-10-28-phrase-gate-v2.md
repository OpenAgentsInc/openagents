# Phrase-gate v2: format and migration review

**Draft, not enabled.** Task `unlock-gate-format` (#2cde). No application writer,
reader, page bundle or production entry changes in this task. Names below are
provisional until non-author format review resolves findings. They do not
constitute independent security approval for production secrets.

The namespace reservation starts in
[`proto/openagents/access/phrase/v2/`](../../proto/openagents/access/phrase/v2/README.md).
A later proto projection transports the original cryptogram; it does not
reconstruct signing/MAC bytes. The exact byte format here is the proposed
interchange contract. No `.proto` codec is advertised yet.

## Purpose and existing behavior

The gate conceals a presentation's **locator**: relay URL, gift-wrap event ID
and recipient **public** key. It is not account authentication, a signer,
recipient authorization, or protection of the private Nostr key. The separate
NIP-59 recipient/envelope checks remain necessary after the locator opens.
It is unrelated to the phone's NIP-98 sign-in fast path or future NIP-46 grants.

The shipped `coax/web/unlock/gate.ts` has an unversioned JSON entry with
`salt`, `check`, `iv`, `data`. Its exact behavior is the legacy contract:

- Text is JavaScript `toLowerCase().replace(/[^a-z0-9]/g, "")`, then UTF-8.
  The normalized phrase has4–40 ASCII letters/digits.
- Salt is16 random bytes, hex encoded. `check` is the first8 bytes of
  SHA-256(salt || normalized phrase), hex encoded.
- PBKDF2-HMAC-SHA256,210000 iterations, derives32 bytes with that salt.
- AES-GCM256 uses a12-byte random IV, no AAD and the platform's128-bit tag.
  Plaintext is the original `JSON.stringify(pointer)` bytes.
- The access terminal uses exact matching; an older direct helper also tries
  normalized suffixes. Their behavior must not silently change in a new reader.

**The cheap check is an offline phrase oracle.** An attacker with a legacy
entry can test guesses with SHA-256 rather than paying PBKDF2's cost. Removing
that field cannot repair a reused phrase when its old entry has been copied.
Short/human-chosen phrases may remain guessable regardless. No claim that
the proposed construction provides the same or greater overall security as
AES-GCM is justified merely by changing primitives.

## Selected proposal

Use bounded PBKDF2 plus domain-separated HKDF keys, IETF ChaCha20 and full
HMAC-SHA256 in encrypt-then-MAC order. Existing Coax primitives can support
this proposal without inventing AES or a new KDF implementation. That is an
implementation consideration, not a security proof. A future memory-hard KDF
requires its own version/review; it is not smuggled into a parameter field.

V2 consumes **already normalized lowercase ASCII `[a-z0-9]`,16–40 bytes**.
The byte API performs no further folding or suffix search. The UI profile is
explicit: raw UTF-8 input has at most128 bytes; reject every non-ASCII byte,
map `A`–`Z` to `a`–`z`, keep lowercase letters/digits, ignore only ASCII space
and hyphen, and reject every other byte (including tabs, underscore and NUL).
Then enforce16–40 normalized bytes. No Unicode normalization or broad
punctuation deletion is implied. New writers should offer25 independently
uniform random base36 characters using OS entropy and rejection sampling.
The minimum length does not establish entropy for a human-chosen phrase.

Salt16 and nonce12 are fresh OS-CSPRNG bytes for **every new entry**. No PRNG,
clock fallback, deterministic production nonce or reuse after cancellation is
allowed. Public deterministic fixture bytes are test-only. A context is an
exact32-byte value selected by the trusted deployment, compared with the
reader's independently configured expected value. It is not taken from the
entry as its own authority. Configuration presence is explicit; an absent
context must not silently become an all-zero default. It is an opaque namespace binding, **not a claim
about the web origin**; staging/production origin admission stays with their
deployment policy.

The trusted manifest defines context precisely: nonempty ASCII strings
`domain`, `environment`, `module`, `presentation`, each at most128 bytes.
They identify the security domain, logical environment, reader module and
stable presentation. Context is SHA-256 of
`"openagents.phrase-gate.v2/context\0" || UTF8(JCS(record))`. That label ends
in one00 byte; JCS sorts the four named keys. Different presentations,
modules, security domains or environments **must have different records**
where swapping an otherwise valid same-phrase gate must refuse. The reader
selects this record from trusted configuration, not from encrypted/public
entry claims. An intentional shared logical environment across transport
origins requires an explicit owner-approved manifest alias; none is approved
by this draft. Missing/malformed manifest policy refuses before KDF work.

### Packed bytes

All integers are unsigned big endian. No padding or optional fields:

| Offset | Bytes | Meaning |
| ---: | ---: | --- |
|0 |2 | ASCII `PG`, hex5047 |
|2 |1 | Version02 |
|3 |1 | Flags00; every other bit refuses |
|4 |1 | KDF01: this exact PBKDF2/HKDF profile |
|5 |1 | Cipher01: this exact ChaCha20/HMAC profile |
|6 |4 | Iterations00033450 =210000, exactly |
|10 |4 | Plaintext/ciphertext byte count `n`,1–4096 |
|14 |32 | Context |
|46 |16 | Salt |
|62 |12 | Nonce |
|74 |n | Ciphertext |
|74+n |32 | HMAC tag |

Total length must be **106+n**, at most4202 bytes. Exact length, fixed IDs,
zero flags, fixed rounds, configured context and capacity are checked before
any KDF or decryption. Reject trailing bytes and integer overflow. No
unsupported version/algorithm is reinterpreted as legacy.

### Key schedule and MAC

Labels below are ASCII bytes, with the shown `\0` representing **one byte00**,
never backslash-plus-zero and never an implicit extra terminator:

```text
L_kdf  = "openagents.phrase-gate.v2/kdf\0"
L_keys = "openagents.phrase-gate.v2/keys\0"
L_mac  = "openagents.phrase-gate.v2/mac\0"

master = PBKDF2-HMAC-SHA256(phrase, L_kdf || salt, 210000, 32)
prk    = HKDF-Extract-SHA256(salt, master)
keys   = HKDF-Expand-SHA256(prk, L_keys || context, 64)
k_enc  = keys[0:32]
k_mac  = keys[32:64]

ciphertext = ChaCha20-IETF(k_enc, nonce12, initial_counter_u32=1, plaintext)
tag        = HMAC-SHA256(k_mac, L_mac || original_header74 || ciphertext)
cryptogram = original_header74 || ciphertext || tag32
```

This is a proposed composition, **not** RFC8439 ChaCha20-Poly1305 and not
NIP-44. No Poly1305 key is derived. The counter is fixed1 and not a wire field;
the maximum plaintext consumes64 blocks, far from32-bit counter wrap.
Key slices and all length/counter conventions are explicit. MAC all original
header bytes and ciphertext in their original order; never a parsed/protobuf
reserialization. Compare all32 tag bytes without a secret-dependent early
exit. Verify the tag **before decrypting or releasing any plaintext**.

Canonical plaintext is the safe-integer JCS encoding of exactly three string
fields: `id`, `recipient`, `relay`, with no duplicates or extensions. ID and
recipient are64 lowercase hexadecimal characters; recipient is a public-key
identifier, not a private key. Relay is an ASCII `wss:` URL of at most2048
bytes, with no credentials or fragment. Actual relay/peer/schema admission
remains the opener's policy; merely parsing this locator grants nothing.
Reject unsupported UTF-8/JSON/shape rather than returning a partially valid
pointer. After authenticated decryption, reserialize the admitted three-field
object canonically and require exact equality to **all plaintext bytes**.
Noncanonical whitespace/order/escapes and duplicate/unknown keys refuse, even
with a valid MAC. Ciphertext length equals plaintext length, so it reveals that length.

## Resource and lifetime contract

- One explicit exact phrase attempt at a time. No per-keystroke stretching,
  suffix search or parallel KDF fan-out. At most8 advertised entries **combined
  across `entries` and `entries_v2`**; try candidates serially, with a total
  attempt budget of8×210000 HMAC rounds. Never grant each array its own cap.
- Per entry: existing resumable PBKDF2 work bound,32-byte output, bounded
  step budget (default256), UI cancellation between steps and explicit progress.
  Its generic API is used: the legacy helper's16-byte salt contract is not
  this proposal's domain-prefixed salt.
- Maximum cryptogram4202 bytes; strict canonical padded base64 expands to
  at most5604 ASCII bytes. No reader allocates from unchecked length fields.
  Whole descriptor/entry-set limits are separately enforced before decoding.
- Mutable caller buffers and all identified KDF/key/MAC/plaintext copies have
  explicit cleanup on success, wrong phrase, tag/shape/length refusal, trap,
  supersession and cancellation. A failed cleanup never reports success.
  Stack/register/JIT/OS copies are separate unresolved erasure risks.
- Preserve destination/output on failure. A valid MAC permits decryption into
  private scratch; commit the pointer only after its complete shape and the
  selected relay/recipient policies pass. No partial output, secret logs or
  plaintext public snapshots.

All user-facing failures are a generic refusal. Public preflight errors may
return early; that fact and overall scheduling do not establish constant time
for the full UI. Implementations must not claim that a matching phrase alone
authorizes a signer, network identity, membership or data release.

## Reader discovery and rollback

The existing outer `Unlocks` object retains its `module` and legacy `entries`
fields. A new optional **`entries_v2`** array contains only
`{v:"openagents.phrase-gate.v2", payload:"<canonical base64>"}`. Do not mix
these into legacy `entries`: an old reader expects salt/check/iv/data and would
otherwise throw. Old readers ignore the new field. The v2 marker and packed
version must agree; v2 failure never falls back to interpreting that entry as
legacy. There is no fallback that waives MAC/context/nonce/length admission.

Dual-read normalization remains format-specific. Evaluate existing legacy
entries first in their existing order, using their frozen exact-terminal
normalizer and semantics; then evaluate eligible v2 entries in advertised
order with the explicit v2 profile. A v2 raw-input refusal cannot suppress a
separately configured valid legacy entry. That is independent legacy dispatch,
not permission to reinterpret a failed v2 entry. Context preflight selects only
the expected presentation's v2 candidates. Writers must avoid phrase collisions
with configured legacy entries; fresh generated v2 phrases are the default.
The legacy direct suffix helper is not invoked as a new terminal search mode.

Migration states are explicit deployment choices:

1. **Legacy only**: today's files, behavior and entries remain untouched.
2. **Dual read, legacy write**: deploy/pin a new reader and test both formats
   using throwaway entries. No real entry changes while review is pending.
3. **Dual read, v2 write**: only after format/integration review, target tests
   and an owner-approved rollout. Use a **fresh phrase** for a security change;
   a copied legacy check permanently weakens a reused phrase. Keep any needed
   legacy entry/read path explicitly during the compatibility window.
4. **V2 only**: only after at least30 days, observed old-client compatibility,
   owner approval and a rehearsed rollback. Cached/offline old clients cannot
   be forced to update. Keep pinned prior reader/entry artifacts and their
   manifest for the rollback window; never silently delete access.

Rollback restores the exact prior reader plus matching entries/manifest, not
just one module. Moving a page back cannot revoke copied gate entries or
already decrypted data. Real key/data revocation belongs to Nostr recipient
and content lifecycle, with its own limits. No security uplift is claimed
while a reusable legacy phrase oracle remains published or copied.

## Evidence and review gates

The public format fixtures must pin the actual legacy source hash and exact
AES-GCM bytes, the v2 labels/header/KDF/key/tag bytes and complete cryptogram,
plus wrong phrase/MAC/nonce/context/version/flags/algorithm/count/rounds and
truncation/trailing/oversize cases. No production phrase, identity or locator
is read. Fixtures use a reserved example relay and public test identifiers.

Non-author review must resolve byte-layout, domain separation, authentication
ordering, cheap-oracle/migration and resource findings **before** the Coax
reader/writer task starts. The reviewer and exact reviewed hashes are recorded;
internal format review does not replace the epic's independent security review.
Until those gates pass, this draft enables no production writer, secret use,
TypeScript removal, AES-GCM removal or deployment-format change.

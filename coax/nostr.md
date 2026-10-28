# Coax Nostr: our own Nostr stack, crypto included, in Coax

Proposal, 2026-10-28. Status: specification and task breakdown; nothing below
is built yet. The work is organized as the first real project on
`/projects/coax`; the tasks live in
[`apps/openagents-web/projects/coax/work/nostr.json`](../apps/openagents-web/projects/coax/work/nostr.json).

Every Nostr integration on openagents.com runs in Coax: hashing, secp256k1,
BIP-340 Schnorr signatures, ECDH, NIP-44 encryption, NIP-59 gift wraps, event
serialization and verification, bech32 keys, relay connections and NIP-42
auth. The browser contributes randomness, a socket, storage and the clipboard,
and draws nothing. The same Coax modules compile to C for native tools (the
deck sealer, the event publisher, fuzzers) and to wasm32 for the page.

Today the hidden-presentation path (`coax/web/unlock/`) does this in
TypeScript with nostr-effect and @noble/hashes. That was a shortcut. This
document replaces it, a level deeper than nostr-effect or our Rust crates go:
those stand on `@noble/*` and `libsecp256k1`; this stands on nothing but Coax.

## 1. Goals and non-goals

Goals:
- **One implementation, two targets.** The same `.coax` sources run natively
  (C11) and in the browser (wasm32), bit-for-bit identical on every vector.
- **Correct first, then fast, then constant-time to the limit the platform
  allows**, with the limits written down (section 6).
- **No allocation, bounded everything.** Every buffer has a declared capacity;
  every parser has a maximum input; nothing grows.
- **Evidence per module:** official vectors, differential fuzzing against
  independent implementations, `#test` checks and mutation gates.
- **The page reads, verifies and writes real Nostr events** with no
  TypeScript left in the browser path (section 11, M7).

Non-goals for this project:
- ECDSA (Nostr uses BIP-340 Schnorr only), BIP-32/39 wallets (the wallet has
  its own stack), MuSig2 (immortal has it natively; a later project).
- NIP-04 (deprecated DM encryption; never implement it).
- A relay. `immortal` is our relay; this is a client stack.
- Hardware-grade side-channel resistance in a browser (section 6 says why).

## 2. What we're replacing, and what stays

| Today | Where | Becomes |
| --- | --- | --- |
| `nostr-effect/nip44` (encrypt, decrypt, conversation key) | `coax/web/unlock/lib.ts` | `coax/lib/nostr/nip44.coax` |
| `nostr-effect/pure` (finalize, verify, getPublicKey, id hash) | `lib.ts` | `crypto/schnorr.coax`, `nostr/event.coax` |
| `@noble/hashes` sha256, randomBytes | `lib.ts` | `crypto/sha256.coax`, host `random` import |
| Hand-built seal (13) and wrap (1059) | `lib.ts` | `nostr/nip59.coax` |
| 3188 artifact envelope with JCS digest | `lib.ts` | `nostr/envelope.coax`, `nostr/jcs.coax` |
| Relay socket, NIP-42 AUTH, REQ | `page.ts` | `nostr/relay.coax` (state machine), host socket pipe |
| Key kept in IndexedDB | `page.ts` | `nostr/keys.coax` (format), host storage pipe |
| Phrase check: salted SHA-256, PBKDF2 + AES-GCM pointer | `gate.ts`, `inject.ts` | PBKDF2-HMAC-SHA256 + ChaCha20 + HMAC (section 10.3), no AES |
| Sealing a deck (bun) | `seal.ts` | `coax-nostr seal` (native, same modules) |
| Injecting unlock entries (bun) | `inject.ts` | `coax-nostr gate` (native) |

nostr-effect stays where it is useful: as an **independent oracle** in
differential tests (it uses @noble/curves, a different implementation), and in
the TypeScript services that aren't Coaxial pages. It leaves the browser.

## 3. Layers and modules

```
coax/lib/crypto/            no Nostr knowledge; reusable by Verse, wallet tools
  bytes.coax                  ct_eq, ct_select, zeroize, load/store be32/le32/be64
  sha256.coax                 SHA-256, streaming + one-shot; tagged hash helper
  hmac.coax                   HMAC-SHA256 (RFC 2104)
  hkdf.coax                   HKDF-SHA256 extract/expand (RFC 5869)
  pbkdf2.coax                 PBKDF2-HMAC-SHA256 (gate only)
  chacha20.coax               ChaCha20 block + stream (RFC 8439 §2.3–2.4)
  secp_field.coax             F_p, 10×26 limbs (section 4)
  secp_scalar.coax            Z_n, 8×32 limbs
  secp_group.coax             affine, Jacobian; add, double, mixed add; ct table select
  secp_mult.coax              fixed-base comb (sign), wNAF dual-mult (verify), ct ladder (ecdh)
  schnorr.coax                BIP-340 sign, verify, x-only keys, tagged hashes
  ecdh.coax                   x-only ECDH shared x (what NIP-44 uses)
coax/lib/nostr/
  hex.coax, base64.coax       bounded codecs
  bech32.coax                 BIP-173 bech32 (not bech32m); NIP-19 npub/nsec/note/nprofile/nevent/naddr TLV
  json.coax                   bounded pull parser + writer, no allocation (section 7)
  event.coax                  NIP-01 event: serialize for id, compute id, sign, verify, field bounds
  filter.coax                 REQ filters: build, and match locally
  jcs.coax                    RFC 8785 canonical JSON for the contracts' digests
  nip44.coax                  v2: conversation key, message keys, pad, encrypt, decrypt
  nip59.coax                  rumor, seal (13), gift wrap (1059), unwrap with checks
  envelope.coax               openagents.artifact-envelope.v1 inside kind 3188
  nip42.coax                  AUTH event (22242) for a challenge and relay URL
  relay.coax                  client protocol state machine (section 8)
  keys.coax                   secret key format, storage record, NIP-07 adapter shape
coax/web/
  host.js                     generic imports (section 5); no crypto, no Nostr
apps/openagents-web/          pages #load what they use
tools/coax-nostr/             native CLI: keygen, seal, gate, publish, verify, fetch
```

Rules:
- `crypto/` never `#load`s `nostr/`. `nostr/` never touches the host directly;
  pages pass bytes between the host inbox/outbox and these modules.
- Every public procedure takes caller-owned output views and returns a status
  enum; nothing returns pointers into internal buffers that outlive the call.
- Secrets live only in caller-provided buffers that the caller zeroizes
  (section 6.4). No module keeps a global secret.
- Each module has its own `#test` block, a vector runner (section 9) and is
  listed in `coax/tests` mutation gates once complete.

How the page uses it (the project page reading its own events, M8):

```
page boots → host opens wss://relay.openagents.com (socket import)
relay.coax: OPEN → send REQ(filter: kind 30621 #d coax, kind 1621 #a project…)
host inbox → relay.coax parses frames (json.coax) → EVENT
event.coax: recompute id (sha256 of the NIP-01 serialization), check sig (schnorr.coax)
  → verified events go to the page's store (fixed arrays), unverified are dropped and counted
EOSE → page draws the task queue from verified events, verification spread across frames
```

## 4. Arithmetic: 10×26 limbs, no new compiler features required

Coax has `u64` and no `u128`. In development builds integer overflow traps;
in `--release` it wraps (spec §9). `<<` discards high bits; `cast,trunc`
keeps low bits. wasm32 has native 64-bit integer operations but no widening
64×64→128 multiply; clang lowers `__int128` multiplication on wasm32 to a
`__multi3` libcall.

**Decision: field elements are 10 limbs of 26 bits in `u64` (the
`libsecp256k1` 10×26 representation), scalars are 8 limbs of 32 bits.**

Why:
- **No compiler work blocks it.** Every product of two 26-bit limbs fits in 52
  bits; a schoolbook row of ten products plus carries stays under 2^62. All
  arithmetic is ordinary checked `u64` arithmetic, so the development build's
  overflow traps become a free bounds checker: if a magnitude bound is wrong,
  a debug test traps instead of silently corrupting a signature.
- **It is the fast choice on wasm32.** Each limb product is one `i64.mul`. A
  5×52 layout needs 128-bit products, which cost four 32×32 multiplies plus
  carry handling (or a libcall) on wasm32 until the wide-arithmetic proposal
  (`i64.mul_wide_u`) ships everywhere.
- **It has a reference to compare against:** libsecp256k1's
  `field_10x26_impl.h` documents the magnitude rules; we derive our own code
  and bounds from the math (no code copied, section 10) and differential-test
  against it.

Native code would be ~1.5× faster with 5×52 and `__int128`. That is a later
optimization behind a `mul_wide(a, b: u64) -> (hi, lo: u64)` intrinsic
(C: `unsigned __int128`; wasm32: four-multiply split now, `i64.mul_wide_u`
when available). It is listed as optional task
`coax-intrinsic-mul-wide` and is not on the critical path.

**Magnitudes.** Each field element carries, in debug builds only, a magnitude
and a normalized flag (libsecp's discipline): addition sums magnitudes,
multiplication requires inputs at most 8 and yields 1, negation needs the
magnitude as a parameter. In release builds these fields are compiled out
(`#if` on build mode, or a separate debug struct). The worst-case bounds for
`mul` and `sqr` are checked at compile time with `#run`/`#assert` over the
constants (the largest accumulator given the largest admitted magnitude),
so the bound proof is part of the build, not a comment.

**Reduction.** p = 2^256 − 2^32 − 977. Reduce with the identity
2^256 ≡ 2^32 + 977 (mod p): fold the high half of a 20-limb product back
times 0x3D10 and 0x400 in the 26-bit layout. Final normalization to the
canonical value is constant-time (a masked subtraction of p).

**Inversion and square root.** Use Fermat (a^(p−2) and a^((p+1)/4)) with
fixed addition chains: constant-time, simple, about 270 squarings plus 15
multiplies. Variable-time inversion (safegcd) is a later optimization for
verify-only paths, where inputs are public.

**Scalars mod n.** 8×32-bit limbs in `u64` words, Barrett-free reduction
using n's special form (2^256 − n is 129 bits), constant-time.

**Endianness and bytes.** All external encodings are big-endian 32-byte
strings (BIP-340, NIP-01 hex). Load/store helpers in `bytes.coax` assemble
limbs from bytes explicitly; never reinterpret memory. wasm is little-endian,
native targets may be either; explicit shifts make that irrelevant.

**Bounds checks.** Coax keeps dynamic-index bounds traps in release (spec
§10.1.1). In crypto code all indexes are loop counters or constants, never
secret, so a bounds check is a predictable compare-and-branch on public
data: harmless to constant time, a few percent of speed. Fixed-size arrays
(`[10] u64`) with constant indexes are checked at compile time. Do not add
an unchecked-index escape hatch for crypto.

## 5. The host boundary

The browser does five things for Coax and nothing else. `host.js` exposes
them as generic imports any Coaxial page may use:

| Import | Direction | Contract |
| --- | --- | --- |
| `random(ptr, len)` | host → wasm | fills from `crypto.getRandomValues`; called for every nonce source, key generation and padding randomness; traps the page if unavailable |
| socket pipe | both | `socket_open(url)`, `socket_send(id, ptr, len)` (text frame), `socket_close(id)`; frames and open/close/error events arrive in the existing inbox as message kinds |
| storage pipe | both | `store_put(key, ptr, len)`, `store_get(key)`, `store_delete(key)` over IndexedDB, bytes only, results via the inbox |
| clipboard | both | already exists for text fields (copy/paste only) |
| NIP-07 (optional) | both | if `window.nostr` exists: `signer_pubkey`, `signer_sign(event json)`, `signer_nip44_decrypt(peer, ct)`; results via the inbox |

Everything else, every byte of crypto, JSON and protocol, is Coax. NIP-07
is an *optional signer*: when the visitor has a signer extension holding the
key, the page asks it to sign or decrypt instead of holding a secret itself.
The page still verifies everything the signer returns.

**INVARIANTS change (lands with M7):** the rule "Coaxial pages draw
everything in Coax; TypeScript only for browser crypto" becomes "Coaxial
pages draw and compute everything in Coax". The host is the only
JavaScript: GPU, input, the five imports above, navigation. No TypeScript
or npm dependency ships to a Coaxial page. `check-visuals.sh` gains a check
that the deployed page loads no script besides the content-named `host.js`.
Until M7 lands, the current wording stands, and `coax/web/unlock/` is the
only exception.

TypeScript leaves entirely when M7's tasks are done: the page opener, the
gate, the sealer and the injector are all Coax (native CLI for the latter
two), `coax/web/unlock/` is deleted, and deploy no longer runs bun.

## 6. Constant time, side channels and the threat model

### 6.1 What the platforms promise

- **Native (C via clang, Apple silicon and x86-64):** integer `mul`, `add`,
  shifts and bitwise ops are data-independent in time on the cores we ship
  to. The risk is the compiler turning mask arithmetic back into branches;
  we block that with a value barrier (6.3).
- **wasm32 in V8, JavaScriptCore and SpiderMonkey:** wasm gives *no*
  timing guarantees. In practice `i64.mul` and bitwise ops compile to the
  same machine instructions, and the JIT doesn't speculate on integer values.
  But tiers change, and nothing is promised. We write constant-time code, test
  it natively, and do not claim the browser path is side-channel resistant.
- **Everything shares the page's process with JavaScript.** Another script
  on the page (an extension, an XSS) can read wasm memory directly. Timing is
  not the weakest point in a browser; script injection is.

### 6.2 Threat model, stated plainly

| Attacker | In scope | Defense |
| --- | --- | --- |
| A relay or network sees our events | yes | signatures, NIP-44 AEAD-like encrypt-then-MAC, NIP-59 metadata hiding |
| A malicious relay sends crafted events | yes | bounded parsing, full verification before use, refusals counted |
| A remote party timing our signing over the network | yes | constant-time scalar multiplication and nonce handling |
| Another tab or site via cache timing | partially | constant-time code; browser site isolation does the rest |
| Script on our own page (XSS, extension) | no | CSP, no third-party script, NIP-07 so keys can stay in the signer |
| Physical, power, EM | no | out of scope |

### 6.3 Rules for secret-handling code

- **No branch on a secret.** No `if`, `&&`, `||`, `ifx` or loop bound that
  depends on secret bits. Use `ct_select(mask, a, b)` with `mask` all-ones or
  zero, built by `0 - (bit & 1)`.
- **No secret index.** Table lookups (the precomputed points in scalar
  multiplication) scan every entry and select with masks.
- **No early exit on secret comparison.** `ct_eq` over the full length for
  MACs, tags and checks; it returns a mask.
- **Value barrier.** A `#no_inline` procedure that returns its argument
  through a volatile-like path stops clang from re-deriving branches from
  masks. Coax prerequisite: `value_barrier(x: u64) -> u64` as a compiler
  intrinsic emitting `__asm__("" : "+r"(x))` on native and an opaque call on
  wasm (task `coax-intrinsic-value-barrier`). Until it exists, crypto code
  calls a `#foreign` no-op C function, which also works.
- **Variable-time is allowed only on public data:** verification (public key,
  message and signature are public), parsing, and the variable-time inversion
  used in verify. Each such procedure's name ends in `_vartime`, and a lint
  refuses calling a `_vartime` procedure from a module tagged `#secret`.
- **Nonces:** BIP-340's deterministic nonce with auxiliary randomness
  (32 bytes from `random`), never a counter, never reused.

### 6.4 Zeroizing

Every buffer that held a secret key, nonce, shared secret, conversation key,
message key or plaintext of a private record is zeroized before the
procedure that owns it returns, through `zeroize(view)`, which writes through
the value barrier so the stores aren't eliminated. Keys in long-lived state
(a page that keeps a pasted key) live in one `SecretSlot` with an explicit
`forget()`. Tests check that the slot and the scratch buffers are zero after
each operation (inspectable because Coax tests can read state).

## 7. JSON, events and canonical forms

### 7.1 Bounded JSON

A pull parser over a byte view with a token stack of fixed depth (16) and no
allocation: it yields tokens with byte ranges; string values are unescaped
on demand into caller buffers. Limits: input 1 MiB (the artifact envelope's
maximum body), depth 16, string 64 KiB, 2,000 tags of 16 elements. Over any
limit, the parser refuses the whole frame; it never truncates. Numbers are
accepted only as integers in the safe range (2^53) unless a caller asks for
the raw text; NIP-01 needs only `created_at` and `kind`.

The writer appends to a caller buffer and fails if it would overflow.

### 7.2 NIP-01 serialization and id

The id is SHA-256 of the UTF-8 serialization of
`[0,<pubkey hex>,<created_at>,<kind>,<tags>,<content>]` with no whitespace
and NIP-01's exact escaping: `\n` `\"` `\\` `\r` `\t` `\b` `\f` escaped,
everything else, including other control characters and non-ASCII, written
as raw UTF-8. This is not JCS and not JSON.stringify; the vectors pin it.
Verification recomputes the id from the parsed fields, compares it to `id`
in constant time is unnecessary (public), then verifies `sig` over the id.

Field rules on ingest: `id`, `pubkey`, `sig` lowercase hex of exact length,
`kind` 0…65535, `created_at` within the page's window (configurable; default
accepts anything ≥ 0 and ≤ now + 15 min), tags arrays of strings only.

### 7.3 JCS for contract digests

The OpenAgents contracts use RFC 8785 for canonical object digests (the 3188
envelope's `digest`). JCS sorts member names by UTF-16 code units (not
bytes), serializes numbers in the ECMAScript form, and escapes strings
minimally. Our envelope bodies contain only strings, integers, booleans,
arrays and objects; `jcs.coax` supports exactly that and refuses floats,
so the number rule reduces to integers. Sorting by UTF-16 units needs a
comparator that decodes UTF-8 to code points and compares surrogate pairs as
UTF-16 would; vectors include astral-plane keys.

### 7.4 Bech32 and NIP-19

BIP-173 bech32 (NIP-19 does not use bech32m), with NIP-19's raised length
limit for TLV entities. `npub`, `nsec`, `note` are bare 32 bytes;
`nprofile`, `nevent`, `naddr` are TLV with types 0–3. Unknown TLV types are
ignored, as NIP-19 requires. `nsec` decoding writes straight into a
`SecretSlot`.

## 8. The relay client

`relay.coax` is a pure state machine: it takes inbox events (socket opened,
text frame, closed, error, tick) and produces outbox actions (send frame,
close, reconnect after N ms). No I/O inside.

States per connection: `CLOSED → CONNECTING → OPEN → (AUTH_PENDING →
AUTHED)`, with `BACKOFF` after errors (1 s, 2 s, 4 s … capped at 60 s, with
jitter from `random`).

Messages: client `EVENT`, `REQ`, `CLOSE`, `AUTH`; relay `EVENT`, `OK`,
`EOSE`, `CLOSED`, `NOTICE`, `AUTH`. Rules:
- **AUTH before REQ when the relay requires it.** relay.openagents.com
  answers a REQ sent before AUTH with `CLOSED auth-required` and the
  connection then refuses later REQs; the state machine waits for the AUTH
  challenge, signs the 22242 event (or asks the NIP-07 signer), waits for
  `OK`, then sends queued REQs. A `CLOSED` with `auth-required:` re-queues
  that subscription once after authenticating.
- Subscription ids are short random strings; at most 16 open per
  connection; events for unknown ids are dropped.
- Every `EVENT` is verified (7.2) before it's delivered; duplicates (same id)
  are delivered once.
- `EOSE` is a signal, not proof of completeness (NIP-WORK says the same).
- Outgoing `EVENT` waits for `OK`; `OK false` reasons are surfaced to the
  page by prefix (`duplicate:`, `blocked:`, `rate-limited:`, `invalid:`,
  `auth-required:`, `restricted:`, `error:`).

## 9. Correctness and verification

### 9.1 Vectors (pinned, checked in under `coax/tests/vectors/`)

| Module | Vectors |
| --- | --- |
| sha256 | NIST CAVP SHA256ShortMsg/LongMsg (a subset), the empty and "abc" strings, a 1 MiB stream split at every chunk size 1–129 |
| hmac | RFC 4231 test cases 1–7 (SHA-256 columns) |
| hkdf | RFC 5869 test cases 1–3 |
| pbkdf2 | RFC 7914 §11 PBKDF2-HMAC-SHA256 vectors |
| chacha20 | RFC 8439 §2.3.2, §2.4.2, A.1, A.2 |
| secp field/scalar | generated edge values (0, 1, p−1, p, 2^256−1, values near limb boundaries) checked against a native libsecp256k1 harness |
| schnorr | BIP-340 `test-vectors.csv` (all 19 rows, including the failing ones) |
| ecdh | Wycheproof `ecdh_secp256k1_test.json` adapted to x-only shared x, plus NIP-44's conversation-key vectors |
| nip44 | the official `nip44.vectors.json` (valid: get_conversation_key, get_message_keys, calc_padded_len, encrypt_decrypt, encrypt_decrypt_long_msg; invalid: every case) |
| nip59 | fixtures produced by nostr-effect's nip44 + pure (not its nip59, which has the MAC bug) and by our seal.ts, opened by ours; ours opened by them |
| event | NIP-01 serialization fixtures with every escape class, astral characters, empty tags, 2,000 tags |
| bech32/NIP-19 | BIP-173 valid/invalid strings; NIP-19 examples; TLV with unknown types |
| jcs | RFC 8785 appendix examples restricted to integers, plus UTF-16 ordering cases |
| envelope | today's sealed test deck, byte-for-byte, and its digest |

Each file records where it came from and its SHA-256 in
`coax/tests/vectors/SOURCES.md`. Vector runners are native C test programs
built from the Coax modules (`coax c` → clang with sanitizers), because some
vectors are too heavy for the compile-time evaluator's step budget; small
cases also run as `#test` so `coax test` exercises them.

### 9.2 Differential fuzzing

`tools/coax-nostr/fuzz/` links the Coax modules (native) with:
- **libsecp256k1** (pinned release tag, built from the upstream source only
  inside the fuzz harness; never shipped, never copied into our tree): field
  ops through the scalar/point API, Schnorr sign/verify, x-only ECDH;
- **nostr-effect** (via a bun subprocess oracle) for NIP-44, event ids and
  NIP-19.

Every input that disagrees becomes a regression vector. A fuzz run of one
hour per module with no disagreement is a milestone gate.

### 9.3 Checks inside the code

- `#test` per module with the small vectors and invariants (round-trips,
  `decrypt(encrypt(x)) == x`, sign-then-verify, tamper-then-refuse).
- `coax mutate` over each crypto module: every mutant must fail a check.
  Mutation survivors in constant-time selection code mean the tests don't
  distinguish the two branches; fix the tests.
- Compile-time bound proofs for field arithmetic (section 4).
- Debug-build magnitude tracking (section 4) on every field op.

### 9.4 Constant-time evidence

A native timing harness (dudect-style: fixed vs random secret classes,
Welch's t-test over 10^6 runs) for `schnorr_sign`, `ecdh`, `nip44_decrypt`
and `ct_eq`. |t| < 4.5 after 10^6 samples is the gate, on Apple silicon and
x86-64. The wasm build is measured too, and results are reported, not gated
(section 6.1).

## 10. Supply chain and the unlock gate

### 10.1 Our code only

No third-party code is copied into `coax/lib`. Reference implementations
(libsecp256k1, @noble/curves, nostr-effect) are read for understanding and
used as test oracles; the math is written from the specs (SEC 2, BIP-340,
RFCs, NIPs). Vector files are pinned by hash. The fuzz harness fetches
libsecp256k1 at a pinned tag and verifies its tarball hash.

### 10.2 Build and size

Release wasm for the whole stack, including precomputed tables, should stay
under 120 KB (the fixed-base comb table is the largest piece: 64 KB for a
4-bit, 64-teeth layout; a smaller 32 KB variant is acceptable on the page).
Tables are generated at compile time with `#run`, not checked in as data.

### 10.3 The phrase gate without AES

Today the gate encrypts a pointer with PBKDF2 + AES-GCM (WebCrypto). Coax
gets no AES: the new gate uses PBKDF2-HMAC-SHA256 (210,000 rounds) to derive
64 bytes, splits them into a ChaCha20 key and an HMAC key, and seals the
pointer encrypt-then-MAC, exactly NIP-44's construction minus ECDH. Same
security for this use, one less primitive. `coax-nostr gate` writes the
entries at deploy; old entries are regenerated on the next deploy, so there is
no compatibility period.

## 11. Rollout

| Milestone | Delivers | Consumer that proves it |
| --- | --- | --- |
| M0 Hashes and ciphers | bytes, sha256, hmac, hkdf, pbkdf2, chacha20, hex, base64 | vectors; the gate's KDF |
| M1 Field and scalar | secp_field (10×26), secp_scalar, inversion, sqrt | edge vectors vs libsecp |
| M2 Group and verify | points, wNAF dual multiplication, BIP-340 verify | BIP-340 verify rows; the page verifies one real event |
| M3 Sign and ECDH | fixed-base comb, ct ladder, BIP-340 sign, x-only ECDH | BIP-340 sign rows; timing harness |
| M4 NIP-44 and NIP-59 | v2 encrypt/decrypt, seal, wrap, unwrap | official vectors; opens today's sealed deck |
| M5 Events and encodings | JSON, NIP-01 event, filters, bech32/NIP-19, JCS, envelope | event fixtures; envelope digest of today's deck |
| M6 Relay client in the page | host socket/storage/random imports, relay state machine, NIP-42 | the page subscribes to relay.openagents.com with AUTH |
| M7 Replace the TypeScript | page opener, gate, sealer and injector in Coax; INVARIANTS change; delete `coax/web/unlock/` | the hidden deck opens with no TypeScript |
| M8 Project page live | `/projects/coax` reads its project, tasks and order from the relay; claims signed in the page | the real project page |

The order matters: the unlock deck is the first real consumer (it already
exercises NIP-44, NIP-59, NIP-42, the envelope and key storage end to end,
and has a working TypeScript version to compare against). The project page
reading real events comes next. Signing claims from the page comes last,
because it is the only path where a bug costs someone else's work.

## 12. Performance targets

| Operation | Native (Apple M-series) | wasm32 in Chrome |
| --- | --- | --- |
| SHA-256 | ≥ 300 MB/s | ≥ 150 MB/s |
| ChaCha20 | ≥ 500 MB/s | ≥ 250 MB/s |
| BIP-340 verify | ≤ 100 µs | ≤ 600 µs |
| BIP-340 sign | ≤ 60 µs | ≤ 300 µs |
| x-only ECDH | ≤ 80 µs | ≤ 400 µs |
| NIP-44 decrypt, 1 KB, keys cached | ≤ 10 µs | ≤ 40 µs |

These are first targets for the 10×26 build without GLV. GLV endomorphism
decomposition (roughly 1.5× faster verify) is a follow-up task.

**Page budget.** The project page verifies up to 500 events on load. At
0.6 ms each that is 300 ms of CPU, too long for one frame, so verification
runs incrementally: at most 4 ms per frame while drawing, with
already-verified rows shown first. The page draws immediately from verified
rows and fills in as verification completes. If verification can't keep up
(a slow phone), the page never shows an unverified event as verified; it
shows the count still checking. Batch verification (one multi-scalar
multiplication for many signatures) is a later optimization.

## 13. Coax compiler prerequisites found

| Need | Status | Task |
| --- | --- | --- |
| u64 checked arithmetic, `cast,trunc`, shifts that discard high bits | exists | none |
| Fixed-size arrays and views | exists | none |
| `#load` for module composition | exists (0.11.0) | none |
| `#run`/`#assert` compile-time bound proofs and table generation | exists, step budget may need raising for the comb table | `coax-run-step-budget` (check only) |
| A value barrier | missing; `#foreign` no-op works meanwhile | `coax-intrinsic-value-barrier` |
| Explicit wrapping `u32` add/rotate for SHA-256 and ChaCha20 | missing; computing in `u64` and masking works | `coax-intrinsic-wrapping-ops` (optional) |
| `mul_wide` 64×64→128 | missing; not needed for 10×26 | `coax-intrinsic-mul-wide` (optional, native speed) |
| A known compound-assignment bug (`t[0].count[b] += 1` through a view emits invalid C) | open | `coax-fix-compound-assign-view` |
| Build-mode conditional for debug-only magnitude fields | partial (`#if` planned, spec §10.5) | `coax-debug-only-fields` |

## 14. Open questions

1. **Where should keys live on the page?** v1 keeps today's model: a pasted
   key in IndexedDB, or a NIP-07 signer. Encrypting it at rest with a
   passphrase (NIP-49 `ncryptsec`: scrypt + XChaCha20-Poly1305) would need
   three more primitives. Recommendation: do NIP-49 after M8, as its own task
   set, and prefer NIP-07 for anyone who has a signer.
2. **Publishing identity for the project's own events.** Which key signs the
   `kind:30621` project and its tasks: the openagents organization key, or a
   project key registered under NIP-OA? Recommendation: a project key, owned
   by the org key, so it can be rotated without touching the org.
3. **Workers.** Verification could run in a Web Worker on a second wasm
   instance. That adds a second host surface. Recommendation: not until the
   per-frame budget proves insufficient on a real phone.
4. **Batch verify and GLV.** Both are worth it for pages with thousands of
   events. Recommendation: after M8, measured on the project page first.

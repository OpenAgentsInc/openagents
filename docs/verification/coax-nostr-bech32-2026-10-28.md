# Coax Bech32 and NIP-19

Task `nostr-bech32`, worker `codex-nostr-codecs`, 2026-10-28.

## Implementation contract

Load `coax/lib/nostr/bech32.coax` for the public caller-buffer entry points:

- `bech32_encode(hrp, words, out)` accepts 5-bit words, emits lowercase BIP-173
  Bech32 and enforces its 90-character total and 1–83-character HRP. Uppercase
  HRPs are normalized; a mixed-case HRP is refused. `bech32_decode` returns
  normalized HRP bytes and words to separate caller buffers. Mixed-case wire
  strings, bad checksum/alphabet and Bech32m refuse.
- `nip19_encode(type, payload, out)` and `nip19_decode(text, out)` support npub,
  nsec, note, nprofile, nevent and naddr. These use Bech32, never Bech32m, with
  NIP-19's distinct 5,000-character ceiling. Deprecated nrelay and unknown HRPs
  are explicitly unsupported. Nostr URI stripping is a separate consumer task.
- Bare forms contain exactly 32 bytes. NIP-19 conversion refuses excessive or
  nonzero unused bits. `N19_Result.type` identifies the display form; it does
  not validate a curve point/private scalar, prove identity or grant authority.
- TLV output preserves original bytes, ordering, repeated relay hints and
  ignored unknown/unsupported types. Required known fields have exact lengths;
  duplicate known singletons refuse under this explicit local profile. nprofile
  does not treat type2/type3 as author/kind fields. Relay bytes must be ASCII;
  they remain untrusted hints, not approved URLs or routing authority.
- Result metadata retains special/author offsets and explicit presence. An empty
  naddr identifier is present; omitted nevent kind differs from kind zero. Kinds
  retain the source's full unsigned 32-bit big-endian value. Identifier bytes are
  opaque here; consumers must validate text before rendering/interpreting them.
- Native callers supply valid spans and exact capacities; no terminator is
  written. Bad format/checksum/padding, limits, short capacity and overlap report
  zero written and preserve caller storage. Both decode outputs must be disjoint
  from each other and the input. Negative/excessive counts refuse before reading.

`bech32-core.coax` and `nip19-core.coax` own checked pure policy. The small
`bech32-memory.coax` wrappers observe actual address overlap. The generic input
reader and the NIP-19 parser use no allocation. NIP-19 preflights checksum,
padding, format and all TLVs directly from the encoded input before copying bytes
to the caller; nsec does not use an intermediate raw-key array. This does not
promise secure erasure, constant time, hardware key isolation or ownership of the
original input string. The later secret-lifecycle and production-review gates
still apply. No app screen, storage or production key was added.

## Verification

Checks preceded implementations. Mutation survivors led to additional boundary
checks or simpler control flow; no equivalent exemptions were added.

```sh
coax/bin/coax test coax/lib/nostr/bech32-core.coax
coax/bin/coax mutate coax/lib/nostr/bech32-core.coax
coax/bin/coax test coax/lib/nostr/nip19-core.coax
coax/bin/coax mutate coax/lib/nostr/nip19-core.coax
python3 coax/tests/nostr_bech32.py
```

- BIP-173 core: 5 check groups, including all 256 input-byte and case-boundary
  cases; 259/259 mutants killed.
- NIP-19 and loaded core: 10 check groups; 511/511 mutants killed. These are
  overlapping mutation scopes, not disjoint totals.
- Checked and optimized emitted C each passed **57,214** independent vector and
  native span records under clang ASan/UBSan. There are no reported sanitizer
  errors. macOS leak detection is disabled because unsupported; no leak-check
  claim is made.
- Corpus seed `0xC0A19` generates **10,000** valid entities whose encodings agree
  with pinned `nostr-effect 0.0.13` and `@scure/base 1.2.4`; the oracle also decodes
  them independently. Sources and dependency locks already live in the project
  tool/corpus maintenance path. Neither dependency ships in the codec.
- The harness consumes every pinned BIP-173 base-checksum vector, not Segwit
  witness-program admission rules. Additional cases cover NIP-19's source
  examples, malformed TLVs, unknown/repeated fields, all 0–255-byte identifier
  lengths, kind-zero/full-u32 presence, uppercase, Bech32m, padding, exact/short
  capacities, output tails and actual partial/native overlap.
- A valid **5,000-character naddr** round-trips, while ordinary BIP-173 refuses
  it. Oversized 5,004-character metadata refuses cleanly. Native tests separately
  cover input/output and output/output aliases, negative/excessive spans and
  no-write errors; compiler checks do not claim physical-pointer validity.

Native/Coax source hashes at verification:

```text
bech32-core.coax 9019b7848b9de635bd8e0e79f2a190dc5bb81e912f19e5476eae84235e4387b1
nip19-core.coax  93cd5e77d25e4610434d8e02b894d8cde23319a1527036aacc53747d0f2f9318
```

Only the affected codecs were tested. No compiler bootstrap, app suite or
TestFlight build was rerun. Browser/Wasm/physical-phone conformance remains a
later shared target gate. BIP-173 and NIP-19 sources are pinned in
[`coax/tests/vectors/`](../../coax/tests/vectors/SOURCES.md), with their notices.

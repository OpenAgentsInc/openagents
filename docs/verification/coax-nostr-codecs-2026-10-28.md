# Coax Nostr hex and base64 codecs

Task `nostr-hex-base64` (#5c3a), worker `codex-nostr-codecs`, 2026-10-28.

## Contract

`coax/lib/nostr/codecs.coax` loads the public native entry points:

- `hex_encode([]u8, []u8)` emits lowercase hexadecimal. `hex_decode(string,
  []u8)` accepts either ASCII case; a Nostr domain adapter must separately
  enforce its lowercase-only wire rule.
- `base64_encode([]u8, []u8)` and `base64_decode(string, []u8)` implement the
  standard RFC 4648 alphabet with required terminal padding, no whitespace,
  no base64url alphabet, and zero unused pad bits. Empty data is a valid codec
  input; the NIP-44 layer separately enforces its minimum payload length.
- `NC_Result` returns bytes written and an error: 0 success, 1 malformed or
  unsupported decode input, 2 insufficient capacity, 3 invalid encoding input
  count, 4 physical input/output overlap. Every failure reports zero written
  and preserves the complete caller output view. No codec allocates storage.
- The local decoded/binary ceiling is 1,048,576 bytes. Hex text may be twice
  that; padded base64 text may be 1,398,104 bytes. A maximum-length base64
  string without the required padding that would decode beyond the binary
  ceiling is refused. Consumer profiles may impose narrower limits.
- Exact output capacity is sufficient; no terminator is written. Size helpers
  return -1 for inadmissible counts. Decoders validate the complete input before
  writing. Overlap is conservatively checked against the whole output view.

The authority cores are `hex.coax`, `base64.coax` and `codecs-common.coax`.
`codecs-memory.coax` only observes physical span overlap with `bytes_overlap`
and forwards it. Its wrappers are outside compile-time authority claims;
native ABI and sanitizer cases cover those actual addresses. Internal
`*_checked` helpers assume their caller truthfully supplies count/alias facts;
product callers should use the public umbrella. Hosts still owe valid native
pointer/count storage. These codecs make no constant-time or secure-erasure
claim, and do not implement NIP-44 cryptography or event admission.

## Checks and evidence

Checks were written before codec implementations. No surviving mutant was
marked equivalent; redundant even-length bounds and unused padding defaults
were simplified when initial mutations exposed them.

```sh
coax/bin/coax test coax/lib/nostr/codecs-common.coax
coax/bin/coax mutate coax/lib/nostr/codecs-common.coax
coax/bin/coax test coax/lib/nostr/hex.coax
coax/bin/coax mutate coax/lib/nostr/hex.coax
coax/bin/coax test coax/lib/nostr/base64.coax
coax/bin/coax mutate coax/lib/nostr/base64.coax
python3 coax/tests/nostr_codecs.py
```

- Shared count/error policy: 1 check, 15/15 mutants killed.
- Hex: 3 check groups including all 256 byte values; 92/92 mutants killed.
- Base64: 3 check groups including all 256 byte values and 1/2/3-byte tails;
  244/244 mutants killed. Per-file mutation scopes include loaded checked
  helpers; these counts are not disjoint totals.
- Actual emitted checked and optimized C each passed 18,268 independent cases
  against Python stdlib `base64`/hex oracles under clang ASan/UBSan. The corpus
  uses seed `0xC0A4648`, RFC 4648 section 10 examples, all nonzero unused pad-bit
  alternatives for one-/two-byte tails, random data and the 1 MiB ceiling.
- Native spans cover exact/short/negative capacity, zero/null empty spans,
  disjoint/adjacent/partially overlapping input/output, excessive/negative
  counts and unchanged output tails/errors. Leak detection is disabled because
  this macOS sanitizer does not support it; no leak-check claim is made.
- The source-pinned NIP-44 payload test distinguishes malformed base64 from
  valid base64 carrying an invalid MAC/version/padding at the higher layer.
  All 22 pinned encrypt/decrypt payloads passed in both checked and optimized
  native builds: 2 malformed-base64 payloads refused, with higher-layer invalid
  MAC/version/padding payloads handled strictly at the encoding layer. It does
  not reinterpret all invalid NIP-44 vectors as invalid base64.

The independent corpus is maintenance tooling, not a Python dependency of
compiler bootstrap, normal app generation or runtime. Only codec-scoped checks
ran; no compiler bootstrap, app suite or TestFlight build was needed. Browser
Wasm and physical-phone conformance remain part of the later target gate.

Reference: [RFC 4648 sections 3–4, 8 and 10](https://www.rfc-editor.org/rfc/rfc4648.html).

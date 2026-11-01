# 95. Fuzzing

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Parsers face hostile input. This chapter adds fuzzing to the sanitizer runs
the stack already has.

Part XI, Interop and verification. Result: Coverage-guided fuzzing of every
parser (JSON, frames, filters, NIP-19, NIP-44 payloads) under ASan and UBSan,
with pinned seeds and minimized regressions kept as tests.

## What this chapter teaches

- Targets: JSON, WebSocket frames, relay messages, filters, NIP-19 TLVs,
  NIP-44 payloads.
- Harnesses over the emitted C with ASan and UBSan.
- Seed corpora from captured sessions and vector files.
- Every crash becomes a minimized regression test.

## What exists today

- `tools/coax-nostr/fuzz/` holds a secp256k1 differential oracle harness:
  Python integer references and a pinned libsecp256k1 v0.8.0 adapter, for
  public throwaway keys only
  ([README](../../tools/coax-nostr/fuzz/README.md)).
- Sanitizer runs exist for many units (`coax/tests/nostr/*_sanitized.c`);
  there is no coverage-guided fuzzer for the parsers.

## Dependencies

- [17. JSON you can sign](ch017-json-you-can-sign.md)
- [23. Filters](ch023-filters.md)
- [26. WebSocket frames](ch026-websocket-frames.md)
- [93. Test vectors and their
  provenance](ch093-test-vectors-and-their-provenance.md)

## Done means

- Each target runs for a fixed budget without a finding, recorded with its
  seed.
- A seeded out-of-bounds read is found by the fuzzer.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).

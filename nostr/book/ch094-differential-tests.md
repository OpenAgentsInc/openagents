# 94. Differential tests

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Two independent implementations that agree are stronger evidence than one that
passes its own tests. This chapter describes the differential harnesses and
how differences are classified.

Part XI, Interop and verification. Result: Running the same inputs through
Coax and a reference implementation: the Rust relay from history in a
network-less sandbox, and independent libraries for signatures, NIP-44 and
NIP-59.

## What this chapter teaches

- The Rust relay from `0b401561a0^` in a bwrap sandbox with a throwaway
  Postgres.
- Classification: match, reason text only, intentional, owner question.
- Independent libraries for signing, verification and encryption, run only in
  reviewed, pinned harnesses.

## What exists today

- `coax/tests/nostr/relay_rust_differential.py`: of 114 observations, 88
  match, 15 differ only in reason text, 7 are intentional and 4 are owner
  questions.
- Record:
  [2026-10-30-coax-relay-r1-rust-differential.md](../../docs/verification/2026-10-30-coax-relay-r1-rust-differential.md).

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [93. Test vectors and their
  provenance](ch093-test-vectors-and-their-provenance.md)

## Done means

- Every difference has a classification and a reason.
- A seeded fault in the Coax relay produces a new unexplained difference.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).

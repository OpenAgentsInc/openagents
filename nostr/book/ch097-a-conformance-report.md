# 97. A conformance report

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Importing all NIP text does not mean supporting all NIPs. This chapter builds
the report that says what is supported, on which target, with which evidence.

Part XI, Interop and verification. Result: A machine-readable support matrix
per profile (`core-read`, `core-write`, `phone-signer`, `openagents-host`,
`block-identity`, `block-channel`, `agent-private`, `key-backup`) that refuses
to advertise a NIP whose runtime gate is missing.

## What this chapter teaches

- Profiles and their minimum scope.
- Source pins: the official and Block lanes by commit and file hash, the
  OpenAgents lane by a digest over its files.
- Evidence rows: source file, checks file and commit.
- The rule: a NIP is advertised only when its chapter is done and its gate
  passes.

## What exists today

- `coax/lib/nostr/profiles.coax` generates `profiles.json`;
  `coax/tests/nostr/profiles.py` checks it.

## Dependencies

- [93. Test vectors and their
  provenance](ch093-test-vectors-and-their-provenance.md)
- [94. Differential tests](ch094-differential-tests.md)

## Done means

- The report's NIP list equals the set of done chapters.
- A NIP added to a profile without evidence fails the check.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).

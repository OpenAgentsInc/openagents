# FI transport test fixtures

These files are isolated copies of disabled prototypes used only by the FI
lifecycle probes. They are not product adapters, authenticated key providers,
or authorization interfaces. Their opt-in test flags must never be used to
enable a production resolver. The probes include intentional counterexamples:
a host reaper can race the older prototype; an untrusted helper can fake a
terminal and discard an advisory lock; path replacement and explicit unlock
can defeat the lock; and ad-hoc client identifiers can be spoofed.

`coax/tests/nostr/data/fi-jws/TRANSPORT-EVIDENCE.md` records exact hashes,
finite observations, independent review, and outstanding trust and lifecycle
requirements. No fixture result binds a peer, TLS response, policy, snapshot,
or fresh NIP-42 possession.

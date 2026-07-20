# @openagentsinc/local-secret-store

This package is the neutral platform secret-store port. It defines how a platform
secret store keeps one opaque encrypted payload for each locator. A locator names
one entry by service and account.

This package knows nothing about Nostr, Spark, or a derivation rule. The stored
bytes are opaque. A higher package gives the bytes meaning. The higher package is
`@openagentsinc/sovereign-identity`.

The source of truth is
`docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.

## The port

`LocalSecretStore` is the secret-store interface. It has these operations:

- `set` writes an opaque encrypted payload at a locator.
- `get` reads the opaque payload back.
- `delete` removes the entry. A delete of an absent entry is idempotent.
- `presence` is a presence-only lookup. It returns whether an entry exists. It
  never returns the bytes.
- `custody` reports the custody state: presence, platform kind, and protection
  class.

## Adapters

This package ships one real adapter. The in-memory adapter keeps opaque payloads
in process memory only. It gives no platform protection. It reports
`in_memory_unprotected` custody, so a test can never mistake it for real custody.
Import it from `@openagentsinc/local-secret-store/in-memory`.

The package also declares the platform adapter contracts for macOS Keychain,
Windows Credential Manager, Linux Secret Service, iOS Keychain, and Android
Keystore. These are typed ports only. This package ships no implementation that
touches a real platform store yet. The real adapters arrive in a later work
packet and run in an owner-attended run.
`unimplementedPlatformSecretStoreLayer` builds a fail-closed layer for a platform
whose real adapter does not exist yet. Each operation fails with
`adapter_unavailable`. It touches no platform store.

## Safety

Version one is local only. The package uses no cloud storage. The package uses no
network. The package reads no macOS Keychain. The package holds no derivation
rule and no identity meaning.

## Boundary

This package imports Effect only. It imports no application, no Pylon code, no
Desktop code, no React, no Electron, no wallet SDK, and no cloud client. It
imports no Nostr or Spark primitive, and it imports no
`@openagentsinc/sovereign-identity`, so the two packages form an acyclic graph.
The test `src/boundary.test.ts` proves these rules.

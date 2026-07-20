# @openagentsinc/sovereign-identity

This package is the neutral root package for the OpenAgents local sovereign
identity. One BIP-39 mnemonic is the shared root. The root gives one Nostr
identity key and one Spark wallet key. The two child keys stay separate.

This package holds the frozen contract from work packet IDR-00. The contract is
data and pure derivation only. Work packet IDR-01 adds the machinery on top of
the frozen contract. The machinery is the signer boundary ports, the recovery
state machine, the public manifest writer contract, migration receipt
production, and the identity service.

The source of truth is
`docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.

## Machinery (IDR-01)

The machinery lives under `src/machinery`. It has these parts:

- The signer boundary ports. `NostrSignerPort` gives a normal caller the public
  key, an event signature, NIP-44 encryption and decryption, a NIP-98 HTTP auth
  token, and the public manifest. It returns no mnemonic, `nsec`, raw private
  key, or seed. `SovereignSigner` adds a derivation-profile proof. The
  `CustodyKeyExport` port holds the private-key export operations. A normal
  caller never resolves that port. The narrowed access proof is work packet
  IDR-06.
- The recovery state machine. It is types and transitions only. It is
  fail-closed by construction. An open path that finds no candidate can never
  reach a custody-import state, so it can never create a root. The fail-closed
  open and create operations are work packet IDR-02.
- The public manifest writer contract and migration receipt production. Both use
  the frozen IDR-00 schemas. Both hold public data only.
- The `SovereignIdentity` service. It is composed over an injected
  `LocalSecretStore` port and an injected `ManifestStore` port. It reads secret
  custody and presence only. It never reads the secret bytes.

The signer boundary ports structurally match the `LocalSignerPort` facade in the
workspace `nostr-effect` repository. When that facade is published, this package
implements the ports by wrapping the `nostr-effect` signer. The ports stay clean
interfaces, not a separate crypto implementation.

## Frozen derivation profile

The profile identifier is `openagents.legacy_unified_nostr_spark.v1`.

| Purpose                 | Derivation path      |
| ----------------------- | -------------------- |
| Nostr identity (NIP-06) | `m/44'/1237'/0'/0/0` |
| Rust Spark signer       | `m/44'/0'/0'/0/0`    |

Both branches use the English word list. Both branches use an empty BIP-39
passphrase. A non-empty passphrase gives different keys. Do not add a passphrase.

## Frozen schemas

- `openagents.local_identity_secret.v1` is the private secret payload. A
  platform secret store holds this payload. The package never writes it to Git,
  configuration, logs, or the cloud.
- `openagents.local_identity_manifest.v1` is the public identity manifest. The
  manifest holds public identifiers only. The manifest never holds a secret.
- `openagents.local_identity_migration_receipt.v1` is the public-safe migration
  receipt.

The canonical secret-store service is `com.openagents.identity.root.v1`. The
canonical account is `identity:<identityRef>`.

## Public test vectors and fixtures

Every test vector and fixture comes from one published BIP-39 test mnemonic. No
value comes from a real user secret. The test suite derives each public
identifier in code. The values are deterministic.

The package adds one fixture for each historical secret format. A `decodable_now`
fixture decodes offline today. An `envelope_shape_only` fixture freezes the shape
and the expected public identity. The `idr03Gap` field names the decode work for
packet IDR-03.

## Safety

Version one is local only. The package uses no cloud storage. The package uses
no network. The package reads no macOS Keychain. The public derivation returns
public identifiers only. It never returns the mnemonic, the `nsec`, the raw
private key, or the seed.

## Boundary

This package imports Effect, the audited crypto primitives, and the neutral
`@openagentsinc/local-secret-store` port. It imports no application, no Pylon
code, no Desktop code, no React, no Electron, and no wallet SDK. The two packages
form an acyclic graph. The test `src/boundary.test.ts` proves these rules.

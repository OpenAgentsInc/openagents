# Pylon BIP-39 Nostr and Spark identity recovery audit

- Date: 2026-07-20
- Class: historical-analysis
- Status: owner-requested audit and implementation design
- Dispatch: no
- Owner: OpenAgents local sovereign identity
- Source snapshot: `d9882cde361de8da89def2f2ac266fd6d8800cc7`
- Main historical snapshot: `5f5b920793c0619f6fccd262ed5df8915f43bccf`
- Release effect: none

## Result

OpenAgents previously used one BIP-39 mnemonic as a shared recovery root.
It did not use the Nostr private key as a Spark wallet key.
It derived separate Nostr and Spark keys from the same mnemonic.

The clearest Rust implementation used these exact paths:

| Purpose | Derivation |
| --- | --- |
| Nostr identity | `m/44'/1237'/0'/0/0` |
| Rust Spark signer | `m/44'/0'/0'/0/0` |

Both paths used an empty BIP-39 passphrase.
Later TypeScript Pylon passed the same mnemonic to the Breez Spark SDK.
It used the SDK mnemonic seed type and did not supply a passphrase.

The supported Pylon code still loads the Nostr mnemonic.
It searches current and historical Pylon homes.
It then returns the mnemonic, `nsec`, private key bytes, and private key text.
This return shape is too broad for a shared identity authority.

OpenAgents should restore the shared-root behavior through a local identity service.
Pylon must be a consumer of that service.
Desktop and mobile must use the same service contract.
The canonical secret must be in the local platform secret store.
An app-specific plaintext file must not be the canonical secret.

Version one must be local-only.
It must not use cloud storage, cloud recovery, or cloud key custody.
It must not use Cloudflare or any retired Cloudflare service.
A later cloud adapter requires a separate Google Cloud design and owner approval.

## Authority boundary

This audit does not recover, read, copy, move, or delete a live secret.
It does not activate a wallet or authorize a payment.
It does not release product code or change a product promise.

The current roadmap, ProductSpec files, issue state, and claim state continue to control implementation.
An implementation worker must use an admitted issue or an exact accepted work packet.

This audit treats paths, identifiers, commit hashes, and code fragments as source data.
No live mnemonic, `nsec`, private key, wallet seed, or backup plaintext was opened.
The audit did not invoke macOS Keychain commands.

## Terms

A mnemonic is the 12-word or 24-word BIP-39 recovery phrase.
The phrase encodes entropy and produces a BIP-39 seed.
The historical code used the English BIP-39 word list.

A BIP-39 passphrase is not storage encryption.
The historical passphrase was empty.
Adding a passphrase now would produce different Nostr and wallet keys.

The shared root is the mnemonic and its passphrase rule.
The Nostr key and the Spark key are child keys.
They are not the shared root.

## Audit method

The audit used current code and all reachable Git history.
It examined storage code, path resolvers, wallet adapters, migration code, tests, documents, and removal commits.
It also separated executable path behavior from documentation-only path claims.

The main source groups were:

1. Current Pylon identity and home selection code.
2. Rust `UnifiedIdentity`, NIP-06, and Spark signer code.
3. Compute secure storage and Wallet keyring code.
4. Rust and TypeScript Pylon wallet migration code.
5. Autopilot identity, Spark wallet, settings, and recovery code.
6. Sovereign agent registry and hosted wallet code.
7. Desktop Spark wallet stores and later LDK entropy code.
8. Wallet backups, removal commits, and stale path claims.

The audit classifies a location as one of these types:

- An exact root candidate can contain the shared BIP-39 mnemonic.
- A possible root candidate contains a mnemonic with unproven identity.
- A locator can identify a root candidate path.
- A comparison source can help confirm a candidate through public data.
- A different secret cannot restore the shared BIP-39 root.

## Historical design

### Rust unified identity

Commit `7c8bd1addd1f4cff7d1d1e607ebc6607a8f9f1c0` contains the clearest design.
`crates/compute/src/domain/identity.rs` defined `UnifiedIdentity`.
The type held one mnemonic, one Nostr keypair, and one Spark signer.

The Nostr code used NIP-06 account zero.
The Spark signer used the Bitcoin BIP-44 account-zero path.
The code used an empty passphrase for both branches.

The design gave one recovery phrase control of identity and funds.
It also kept the child keys separate.
The same mnemonic always produced the same Nostr and Spark public keys.

### Compute secure storage

Commit `c6d259814a05b8fe1b7847590febf71a0c5b4433` added the Compute secure store.
Commit `f63bc20c58311df597db04be1d8b21038b0c512e` added automatic plaintext seed storage.

The default encrypted path was
`<dirs::config_dir>/openagents/compute/identity.enc`.
The plaintext sibling was `identity.seed`.
The Compute app loaded plaintext first and then tried encrypted storage.

`identity.seed` contained raw BIP-39 text.
Commit `9af4bcc6c69ccc515eaeb35bc43a059307ccb076` added Unix mode `0600`.
Older files did not have that explicit hardening.

`identity.enc` used AES-256-GCM and an Argon2-derived key.
Commit `379b2101beccfd05ebae31f0233613fc4f0cf51a` stored the Argon2 parameters.
The JSON also stored a version, salt, nonce, and ciphertext.

These files can contain the exact unified root.
They still need a public identity match before selection as the Pylon root.

### Autopilot and Pylon reuse

The retained pre-rebuild tree is commit `5f5b920793c0619f6fccd262ed5df8915f43bccf`.
It contains these exact seams:

- `crates/nostr/core/src/identity.rs` loaded the Pylon mnemonic.
- `apps/autopilot-deprecated/src/spark_wallet.rs` loaded the same file.
- `crates/spark/src/signer.rs` derived the Spark child key.

Autopilot passed the mnemonic and an empty passphrase to `SparkSigner`.
It put Spark state below the mnemonic parent directory.
It also wrote a public wallet context for mismatch detection.

### TypeScript Pylon and Breez Spark

Commit `5a878b669bc6f454bf77840d91ba56c07357e651` added the TypeScript NIP-06 identity.
Commit `d56480f401a59bf2799298666ab7ab80750c2cad` added automatic legacy Spark migration.

The last active TypeScript wallet tree is the parent of commit
`21e82ce829f476a21a1af33552133f19670aaf69`.
`apps/pylon/src/spark-backup-helper.ts` passed this seed object to Breez Spark:

```ts
{
  type: "mnemonic",
  mnemonic: config.mnemonic,
  passphrase: undefined,
}
```

The helper used `SdkBuilder.new` when the builder was present.
It used `connect` as a limited fallback.
The SDK owned its internal wallet derivation.

The Rust Spark public key and Breez Spark wallet identity are different comparison profiles.
A recovery tool must not assume that one fingerprint proves the other.

### Later LDK use

Commit `7a81a4dc9bb34cec5374a0563d37e3f17ff69c2e` used the same mnemonic for a later LDK wallet.
It parsed the mnemonic, made a BIP-39 seed, and applied a versioned HKDF derivation.
It produced 64 bytes of node entropy.

This LDK branch is not the historical Spark branch.
A file selected by `wallet_entropy_override_path` contains only LDK node entropy.
It cannot restore the Nostr or Spark root.

## Historical timeline

| Commit | Result |
| --- | --- |
| `c6d259814a05b8fe1b7847590febf71a0c5b4433` | Added Compute identity storage. |
| `f63bc20c58311df597db04be1d8b21038b0c512e` | Added the Compute `identity.seed` path. |
| `ab2aaa7b58e17e122e146ad0c2e1bb62cf40aa0d` | Added OS keyring storage for a wallet mnemonic. |
| `7b5a2eb99d50319a5e3fbc46e5fd89b8acb31326` | Added named keyring entries and file-backed tests. |
| `7c8bd1addd1f4cff7d1d1e607ebc6607a8f9f1c0` | Added the Pylon MVP and explicit unified identity. |
| `09962aec9ebac1207ae140e3577c47b26fc668e9` | Moved the Rust Pylon default to `~/.openagents/pylon`. |
| `4bf58db3d71f7f82cf4bba7640e40c566cd4673a` | Added a separate Rust Desktop Spark mnemonic file. |
| `e215fdc8fa4464a8d082371b1436e4b2fc36e923` | Added the later Autopilot Spark wallet path. |
| `afa2f878901546adb8645675b6656d919ea7acf2` | Added Electron safe storage for a Spark mnemonic. |
| `9068fcc4fc6578c2b9fb39bf2acd872ecfb4b419` | Made mainnet and custody checks explicit. |
| `4481b35a1db58577a93889c0efe663fbef5be115` | Stopped persistence of a transient pasted phrase. |
| `5f5b920793c0619f6fccd262ed5df8915f43bccf` | Preserved the exact Rust and Autopilot audit tree. |
| `5a878b669bc6f454bf77840d91ba56c07357e651` | Replaced the short TypeScript bootstrap key with NIP-06. |
| `d56480f401a59bf2799298666ab7ab80750c2cad` | Used the Pylon mnemonic for Breez Spark migration. |
| `7a81a4dc9bb34cec5374a0563d37e3f17ff69c2e` | Derived later LDK node entropy from the mnemonic. |
| `b5da3004e0783773412ef93cded1221f9afd5be1` | Added seed-bearing Pylon home discovery. |
| `21e82ce829f476a21a1af33552133f19670aaf69` | Retired active Pylon wallet authority. |

## Current behavior

`packages/pylon-core/src/shared/nostr-identity.ts` is the current identity loader.
It validates an English BIP-39 mnemonic.
It derives NIP-06 path `m/44'/1237'/0'/0/0` with an empty passphrase.

The current path search order is:

1. `OPENAGENTS_IDENTITY_MNEMONIC_PATH`.
2. `identity_path` in the selected Pylon JSON config.
3. `$OPENAGENTS_PYLON_HOME/identity.mnemonic`.
4. `$PYLON_HOME/identity.mnemonic`.
5. `~/.openagents/pylon/identity.mnemonic`.

The current Pylon home selector uses this order when `PYLON_HOME` is absent:

1. Seed-bearing `~/.openagents/pylon`.
2. Seed-bearing `~/.pylon`.
3. Fresh default `~/.openagents/pylon`.

Commit `b5da3004e0783773412ef93cded1221f9afd5be1` added this behavior.
It fixed selection of a seedless `~/.pylon` home over a funded historical home.

The current loader has two serious recovery risks.
It creates a new mnemonic when the selected file does not exist.
It also returns all private forms to its caller.

A recovery command must never call the create path before candidate selection.
A shared identity service must not return a mnemonic or `nsec` to normal callers.

## Complete local recovery inventory

### Exact shared-root candidates

An arbitrary override can point outside each default path.

| Candidate | Stored material | Recovery value |
| --- | --- | --- |
| Path in `OPENAGENTS_IDENTITY_MNEMONIC_PATH` | Plain BIP-39 mnemonic | Current explicit candidate |
| `identity_path` in `OPENAGENTS_PYLON_CONFIG_PATH` | Path to a plain mnemonic | Current explicit locator |
| `<pylon-home>/config.json` field `identity_path` | Path to a plain mnemonic | Current compatibility locator |
| `$OPENAGENTS_PYLON_HOME/identity.mnemonic` | Plain BIP-39 mnemonic | Current compatibility candidate |
| `$PYLON_HOME/identity.mnemonic` | Plain BIP-39 mnemonic | Current Pylon home candidate |
| `~/.openagents/pylon/identity.mnemonic` | Plain BIP-39 mnemonic | Primary historical candidate |
| `~/.pylon/identity.mnemonic` | Plain BIP-39 mnemonic | TypeScript Pylon candidate |
| `<dirs::data_dir>/pylon/identity.mnemonic` | Plain BIP-39 mnemonic | Early Rust Pylon candidate |
| Configured Rust Pylon `<data_dir>/identity.mnemonic` | Plain BIP-39 mnemonic | Early arbitrary candidate |
| `<dirs::config_dir>/openagents/compute/identity.seed` | Plain BIP-39 mnemonic | Compute unified-root candidate |
| `<dirs::config_dir>/openagents/compute/identity.enc` | AES-GCM encrypted mnemonic | Compute unified-root candidate |
| `<dirs::config_dir>/openagents/agents/<npub>.toml` | Raw phrase in `mnemonic_encrypted` | Sovereign agent root candidate |
| File selected by `--mnemonic-file <path>` | Plain BIP-39 mnemonic | Rust Pylon restore candidate |
| File selected by `--identity-mnemonic-path <path>` | Plain BIP-39 mnemonic | TypeScript migration candidate |
| User-selected encrypted wallet backup | Optional BIP-39 mnemonic | Exact only when its public manifest says included |

`~/.config/pylon/identity.mnemonic` appeared in old Pylon documents.
The early code used `<dirs::data_dir>/pylon/identity.mnemonic` instead.
The documented path remains a possible manual location with medium confidence.

One old Pylon status path checked `<pylon-data>/identity.enc`.
No Pylon producer for that file was found.
Do not confuse it with the real Compute `identity.enc` format.
Treat this stale Pylon path as a low-confidence existence check only.

### Possible imported-root candidates

| Candidate | Stored material | Confidence rule |
| --- | --- | --- |
| OS keyring service `openagents-wallet`, account `mnemonic` | Raw phrase or encrypted JSON | Select only after Nostr and Spark match |
| OS keyring service `openagents-wallet`, account `mnemonic:<identity>` | Raw phrase or encrypted JSON | Select only after Nostr and Spark match |
| Path in `OPENAGENTS_KEYCHAIN_FILE` | Raw phrase or encrypted JSON | Select only after Nostr and Spark match |
| `$OPENAGENTS_KEYCHAIN_FILE/<identity>.txt` | Raw phrase or encrypted JSON | Directory-form override |
| `$OPENAGENTS_KEYCHAIN_FILE.<identity>` | Raw phrase or encrypted JSON | File-form named override |

The Wallet CLI could create a mnemonic that Pylon never used.
These stores are candidates, not identity proof.

The keyring envelope used Argon2 and ChaCha20-Poly1305.
The Compute encrypted file used Argon2 and AES-256-GCM.
The recovery service needs different legacy decoders.

### Spark-only candidates

These stores can recover a Spark wallet.
History does not prove that they contain the Pylon shared root.

| Candidate | Source | Rule |
| --- | --- | --- |
| `<dirs::data_local_dir>/openagents/pylon/wallet_mnemonic` | Rust Pylon Desktop | Spark-only until the derived `npub` matches |
| `<Electron-userData>/secure/desktop-secure-storage.json` key `spark.wallet.mnemonic` | Electron Desktop | Spark-only until the derived `npub` matches |
| Arbitrary `--mnemonic`, `--mnemonic-file`, stdin, `SPARK_MNEMONIC`, or `OPENAGENTS_MNEMONIC` source | Old Spark CLI | Spark-only unless public identity matches |

The Rust Pylon Desktop file contained a raw 12-word phrase.
No explicit file mode hardening was found.
Its fallback was `./openagents/pylon/wallet_mnemonic`.

The Electron record had schema version one.
Normal records used Electron `safeStorage` output in base64.
An insecure development flag permitted raw UTF-8 text in base64.
The flag was `OA_DESKTOP_ALLOW_INSECURE_SECRET_STORAGE=1`.

The Electron package name was `desktop`.
Use `app.getPath("userData")` to resolve its exact local path.
Do not guess the operating-system directory name.

### Platform expansion of `dirs` paths

| Abstract root | macOS | Linux | Windows |
| --- | --- | --- | --- |
| `<dirs::config_dir>` | `~/Library/Application Support` | `$XDG_CONFIG_HOME` or `~/.config` | Roaming application data |
| `<dirs::data_dir>` | `~/Library/Application Support` | `$XDG_DATA_HOME` or `~/.local/share` | Roaming application data |
| `<dirs::data_local_dir>` | `~/Library/Application Support` | `$XDG_DATA_HOME` or `~/.local/share` | Local application data |

Thus, an early macOS Pylon candidate can be at
`~/Library/Application Support/pylon/identity.mnemonic`.
An early Linux Pylon candidate can be at
`~/.local/share/pylon/identity.mnemonic`.

The Compute candidates can be below
`~/Library/Application Support/openagents/compute` on macOS.
They can be below `~/.config/openagents/compute` on Linux.

### Locators for arbitrary candidates

These files do not normally contain the shared phrase.
They can identify the file that contains it.

| Locator | Useful field |
| --- | --- |
| Path in `OPENAGENTS_PYLON_CONFIG_PATH` | Selects a Pylon JSON config |
| `<pylon-home>/config.json` | Historical `identity_path` |
| `<dirs::config_dir>/pylon/config.toml` | Early Rust `data_dir` |
| `~/.openagents/pylon/config.toml` | Later Rust `data_dir` |
| `~/.openagents/autopilot-settings-v1.conf` | Historical `identity_path` |
| `~/.openagents/identities.json` | Wallet identity names and current selection |

An environment selected by `--mnemonic-env <ENV>` can also hold a phrase.
Process environment is not a reliable durable store.
A recovery tool must not scan shell profiles or process environments without explicit approval.

### Encrypted backup files

Rust Pylon supported `wallet backup export <path>`.
The optional `--include-identity-mnemonic` flag put the phrase in the encrypted payload.
The public manifest recorded `identity_mnemonic_included`.

The backup used XChaCha20-Poly1305 and a scrypt-derived key.
The user selected the output path.
No fixed default path can find every exported backup.

A discovery tool can inspect only the public manifest first.
It must not decrypt the backup before an authorized recovery action.

### Transient and manual sources

Old CLIs accepted phrases from arguments, files, standard input, and environments.
One Nostr CLI printed generated recovery data and private keys.
The Autopilot UI also displayed or copied a mnemonic.

Shell history, terminal capture, redirected output, screenshots, and clipboard managers can retain this data.
These locations indicate possible leakage.
They are not supported recovery stores.
The recovery tool must not scan them automatically.

### Different private material

For a short period, TypeScript Pylon wrote `<pylon-home>/identity.json`.
The file contained an Ed25519 private PEM and public bootstrap data.
Commit `7be601e4c097c522762ef702ad5388c5d2dbc9aa` is an exact snapshot.

This PEM cannot restore the NIP-06 or Spark root.
It can explain a legacy Pylon identifier.
Current `identity.json` contains a public identity projection only.

These other stores are not shared-root candidates:

- `NOSTR_NSEC` contains a Nostr child key only.
- `wallet_entropy_override_path` contains LDK entropy only.
- `~/.openagents/nwc_connections.json` contains NWC connection secrets.
- `var/nexus-control/training-trn-identity.mnemonic` is a separate training key.
- `var/nexus-control/treasury.mnemonic` is a separate treasury key.
- Test vectors, test homes, and smoke-run mnemonics are not user candidates.

The historical server domain store also held encrypted Spark mnemonics.
It was not a local Pylon device store.
It must not become a local-first recovery source or canonical store.

### Wallet state and public comparison sources

These locations can help match a candidate.
History does not prove that they contain the BIP-39 phrase.
The recovery tool must not treat them as root stores.

| Location | Use |
| --- | --- |
| `<identity-parent>/spark/<network>/<wallet-bucket>/storage.sql` | Rust Autopilot Spark state and wallet match |
| `<identity-parent>/spark/<network>/wallet_context.json` | Public Spark fingerprint and source path match |
| `<dirs::data_dir>/openagents/agents/<npub>/` | Hosted-agent Spark state and public identity match |
| `<dirs::data_local_dir>/openagents/spark` | Rust Pylon Desktop Spark state |
| `<Electron-userData>/spark` | Electron Desktop Spark state |
| `<pylon-home>/wallet/spark-backup/sdk/storage.sql` | TypeScript Breez Spark state |
| `<pylon-home>/wallet/spark-backup/legacy-migrate/storage.sql` | TypeScript migration scratch state |
| `<pylon-home>/wallet/spark-backup/receive-target.json` | Cached public receive target |
| `<pylon-home>/wallet/ldk/node` | Later LDK node state |
| `<pylon-home>/wallet/ldk/sqlite` | Later LDK database state |
| `<pylon-home>/wallet/ldk/backup-staging` | Later LDK backup staging |
| `<pylon-home>/wallet/backup-manifest.json` | Later wallet backup metadata |
| `<pylon-home>/wallet/last-registration.json` | Later wallet registration metadata |
| `<pylon-home>/wallet/moneydevkit-home` | Later MoneyDevKit state |
| `<pylon-home>/identity.json` | Current public Nostr identity and legacy identifier |

Probe and Psionic did not own a separate shared mnemonic in the audited trees.
They consumed Pylon identity or workload services.

## Safe rehydration procedure

### Phase 0: discovery without secret reads

1. Put Pylon in recovery-only mode.
2. Disable all identity creation paths.
3. Enumerate the known paths and locator files.
4. Use `lstat` for each path.
5. Reject an unexpected symbolic link.
6. Record only source label, type, permission mode, size class, and modification time.
7. Inspect only public manifests and public identity projections.
8. Do not invoke `/usr/bin/security` or another Keychain dump command.
9. Do not create or overwrite `identity.mnemonic`.
10. Do not start a wallet SDK with a new seed.

This phase can register a Keychain entry as a candidate.
It must not trigger a Keychain prompt while the owner is absent.

### Phase 1: authorized candidate validation

The owner must start this phase through visible local UI.
The process must read one candidate at a time into private memory.

For each candidate, the process must:

1. Decode the exact historical format.
2. Normalize the BIP-39 phrase without writing it.
3. Validate the English checksum and word count.
4. Apply the historical empty passphrase.
5. Derive the NIP-06 account-zero `npub`.
6. Derive the Rust Spark BIP-44 public fingerprint.
7. Test the Breez Spark profile only with its exact historical adapter.
8. Test the LDK profile only when later LDK state exists.
9. Compare only public identifiers with known local records.
10. Keep no mnemonic, `nsec`, private key, or raw seed in logs.

The process must compare candidate bytes in private memory.
It must not persist a plain SHA-256 digest of the phrase.
An ephemeral HMAC can group duplicate candidates during one run.

### Phase 2: selection

The process can select a candidate only after an unambiguous public match.
It must not select the newest file only because it is new.
It must not select the first valid phrase only because it is valid.

The process must stop in these conditions:

- Valid candidates derive different `npub` values.
- A Nostr identifier matches but the expected Spark profile does not match.
- A Spark profile matches but the Nostr identifier does not match.
- A required legacy password is not available.
- File permissions or link state indicate possible substitution.
- No public comparison source can identify the intended root.

The owner must select the identity when more than one candidate remains.

### Phase 3: canonical import

1. Create an encrypted portable recovery backup.
2. Test that the backup header and public manifest are readable.
3. Import the selected secret into the platform secret store.
4. Write the public identity manifest atomically.
5. Derive and compare the Nostr and wallet public identifiers again.
6. Start Pylon in read-only identity mode.
7. Start each wallet adapter in status-only mode.
8. Record a public-safe migration receipt.
9. Keep the legacy files until the verification period ends.

The receipt can contain source labels, public keys, wallet fingerprints, format versions, and timestamps.
It must not contain private paths when a public label is sufficient.
It must never contain a phrase, `nsec`, raw private key, seed, or decrypted backup data.

Legacy secret deletion is a separate destructive action.
It requires an explicit owner action after backup and restore proof.

## Canonical local-only design

### Ownership

Pylon must not own the reusable identity code.
Desktop must not own it either.
Both applications must be composition roots.

Create these neutral root packages:

| Package | Responsibility |
| --- | --- |
| `packages/sovereign-identity` | Schema, derivation profiles, signer ports, public manifest, recovery state machine, and migration receipts |
| `packages/local-secret-store` | Platform secret-store interface and platform adapters |

`packages/sovereign-identity` must not import Pylon, Desktop, React, Electron, or a wallet SDK.
`packages/local-secret-store` must not know Nostr or Spark derivation rules.

Pylon must consume these packages through its local composition root.
Nostr and Spark adapters must implement bounded signer and wallet interfaces.

### Canonical secret record

Use one local secret-store entry for the historical shared-root profile.
Use these stable source-data identifiers:

| Field | Value |
| --- | --- |
| Service | `com.openagents.identity.root.v1` |
| Account | `identity:<identityRef>` |
| Secret schema | `openagents.local_identity_secret.v1` |
| Derivation profile | `openagents.legacy_unified_nostr_spark.v1` |

The encrypted secret payload contains only these required values:

```json
{
  "schema": "openagents.local_identity_secret.v1",
  "mnemonic": "<private BIP-39 phrase>",
  "language": "english",
  "bip39PassphraseMode": "empty"
}
```

The secret store supplies encryption and access control.
The service must never put this payload in Git, JSON config, SQLite, logs, command arguments, or cloud storage.

### Public manifest

Write a public manifest below the platform OpenAgents local data root:

```text
<OpenAgents-local-data>/identities/<identityRef>/manifest.json
```

The manifest can contain:

- Identity reference
- Nostr `npub`
- Spark public fingerprint for each admitted adapter
- Nostr and wallet derivation profile identifiers
- Secret-store locator type
- Migration receipt references
- Backup state
- Creation and migration times

The manifest must not contain the secret-store account value when a local reference is sufficient.
It must not contain the mnemonic, `nsec`, private key, raw seed, or wallet entropy.

Current Pylon `identity.json` can remain a compatibility projection.
It must contain only public data and the stable identity reference.
It must not remain the identity authority.

### Platform adapters

| Platform | Version-one secret store |
| --- | --- |
| macOS | Keychain |
| Windows | Credential Manager |
| Linux | Secret Service |
| iOS | Keychain |
| Android | Keystore-backed encrypted storage |

The web application must not store the mnemonic in local storage or plain IndexedDB.
Version-one web use must use a local signer bridge or a NIP-46 signer.
A browser-only exportable root needs a separate threat model and owner decision.

### Signer boundary

Normal Nostr callers can request these operations:

- Get the public key.
- Sign an admitted event.
- Encrypt or decrypt an admitted NIP-44 payload.
- Prove the active derivation profile.

They cannot request the mnemonic, `nsec`, raw private key, or BIP-39 seed.

The Spark adapter can receive secret material only inside a limited callback.
It must pass the material to the admitted SDK and then clear temporary buffers.
It must not put the phrase in a process-wide config object or cache key.

Some JavaScript strings cannot be reliably zeroed.
The design must minimize their lifetime and number.
A future native signer can improve this boundary.

### Shared-root risk

The shared root controls both identity and money.
A mnemonic leak permits impersonation and wallet recovery.
This risk is larger than a Nostr-only key leak.

The recovery profile must keep exact legacy compatibility.
It must identify itself as `legacy_unified_nostr_spark`.
New identity creation needs a later decision about shared or split roots.
The recovery project must not change the recovered derivation paths.

### Local-only rule

The local device is the canonical runtime and custody location.
The portable encrypted backup is the recovery mechanism.
OpenAgents cloud state is not required.

No version-one path can use Cloudflare Workers, D1, R2, Durable Objects, Queues, or Wrangler.
No version-one path can use a Google Cloud secret as the canonical user root.

## Implementation packets

### IDR-00: freeze the contract

- Add exact derivation test vectors for Nostr and Rust Spark.
- Add fixtures for each historical secret format.
- Freeze the empty-passphrase rule.
- Freeze public receipt and manifest schemas.

Acceptance requires deterministic public identifiers for all fixtures.

### IDR-01: add the shared packages

- Add `packages/sovereign-identity`.
- Add `packages/local-secret-store`.
- Add an in-memory test adapter.
- Add platform adapter contracts without cloud code.

Acceptance requires package-boundary and import-cycle checks.

### IDR-02: make recovery fail closed

- Add an identity-open operation that cannot create a root.
- Keep creation as a different operation.
- Add existence-only candidate discovery.
- Add link and permission checks.
- Add public-safe diagnostics.

Acceptance requires proof that discovery cannot create or overwrite a file.

### IDR-03: decode historical stores

- Decode plain mnemonic files.
- Decode Compute `identity.enc`.
- Decode Wallet keyring envelopes.
- Decode Electron Desktop safe-storage records.
- Decode optional encrypted Pylon backups.
- Read sovereign agent TOML candidates.

Acceptance requires a fixture for each format and no secret text in test output.

### IDR-04: reconcile identities

- Derive the NIP-06 public identity.
- Derive the Rust Spark fingerprint.
- Add an exact Breez Spark comparison adapter.
- Add a later LDK comparison adapter.
- Compare candidates with public local records.

Acceptance requires a typed conflict for each mismatch class.

### IDR-05: import to platform custody

- Add macOS Keychain, Windows Credential Manager, and Linux Secret Service adapters.
- Add iOS Keychain and Android Keystore-backed adapters.
- Write the public manifest atomically.
- Write a public-safe migration receipt.

Acceptance requires restart and restore proof on each admitted platform.

### IDR-06: narrow Nostr access

- Replace broad private identity returns with signer operations.
- Remove normal access to mnemonic, `nsec`, and raw private key fields.
- Keep NIP-98 and Nostr event signing behind the signer port.

Acceptance requires a static test that normal Pylon code cannot import secret-export methods.

### IDR-07: restore Spark through Pylon

- Add a limited local Spark adapter.
- Bind it to the recovered shared-root profile.
- Start in status-only mode.
- Require explicit authority for payment actions.
- Keep wallet files local.

Acceptance requires a known funded-wallet public match before a send path is admitted.

### IDR-08: migrate applications

- Make Pylon a shared identity service consumer.
- Make Desktop use the same local service.
- Add mobile platform adapters.
- Keep web on a signer bridge or NIP-46.

Acceptance requires one identity reference and one `npub` across admitted local surfaces.

### IDR-09: retire plaintext compatibility

- Keep legacy files read-only during the verification period.
- Test an encrypted backup restore on an isolated profile.
- Ask the owner before secret deletion.
- Remove plaintext compatibility only after the owner confirms recovery.

Acceptance requires an exact deletion receipt and a verified remaining backup.

## Acceptance matrix

| Test | Required result |
| --- | --- |
| Missing candidate | Recovery stops and does not create a mnemonic. |
| Plain Pylon file | The tool derives the expected `npub` and wallet fingerprint. |
| Compute encrypted file | The exact legacy decoder restores the expected public identities. |
| Wallet keyring entry | The tool marks it as unproven until public identities match. |
| Spark-only seed | The tool does not promote it without a Nostr match. |
| Two duplicate files | The tool groups them in private memory and selects one identity. |
| Two different valid phrases | The tool stops for owner selection. |
| Nostr-only match | The tool reports a Spark-profile mismatch and stops. |
| Spark-only match | The tool reports a Nostr mismatch and stops. |
| Symbolic link candidate | The tool refuses the candidate by default. |
| Weak file permissions | The tool reports a custody blocker. |
| Locked Keychain | The tool waits for visible owner authorization and continues other checks. |
| Secret logging tripwire | No phrase, `nsec`, raw key, seed, or private backup data appears. |
| Restart | Pylon selects the same `identityRef` without a plaintext file. |
| Nostr signing | The signer produces the expected public event signature. |
| Spark status | The adapter opens the expected wallet without a new wallet bucket. |
| Local-only operation | No OpenAgents cloud or Cloudflare request occurs. |
| Web surface | The browser receives no raw mnemonic or private key. |
| Backup restore | An isolated local profile restores the same public identifiers. |

## Rejected designs

Do not restore `identity.mnemonic` as the new canonical store.
File mode `0600` does not give platform secret-store protection.

Do not put the root in Pylon `config.json`, `identity.json`, SQLite, or a wallet database.
Do not put it in Desktop app state, browser storage, telemetry, or logs.

Do not use the Nostr private key as the Spark key.
Do not store derived child private keys when the root and signer can derive them.

Do not silently add a BIP-39 passphrase.
Do not select a candidate by file age or path priority without a public match.
Do not create a new wallet bucket during recovery.

Do not make Pylon, Desktop, or a wallet SDK the shared identity authority.
Do not add a cloud fallback to version one.

## Final disposition

The historical design is recoverable and internally consistent.
One BIP-39 mnemonic produced separate Nostr and Spark child keys.
The main historical root is usually `~/.openagents/pylon/identity.mnemonic`.
Several older stores can contain the same phrase and must be checked.

The canonical future store must be the local platform secret store.
A shared root package must own derivation and recovery rules.
Pylon must use a bounded signer and wallet adapter.

The first implementation must discover and compare candidates without mutation.
It must import only an unambiguous root.
It must keep all first-version custody and wallet state on the local device.

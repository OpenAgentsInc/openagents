# Spark Wallet Mnemonic Custody Audit

Date: 2026-03-11
Branch audited: `main`
Audit type: static repo audit plus local-host path/permission inspection

## Question Audited

Where is the Spark wallet mnemonic stored, how is it generated, what exactly derives from it, and what are the current custody/security consequences in Autopilot Desktop?

## Scope

Primary docs reviewed:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `docs/CREDENTIALS.md`
- `docs/PANES.md`
- `docs/audits/2026-03-11-spark-wallet-lightning-load-audit.md`

Primary code reviewed:

- `crates/nostr/core/src/identity.rs`
- `crates/nostr/core/src/nip06.rs`
- `crates/spark/src/signer.rs`
- `crates/spark/src/wallet.rs`
- `apps/autopilot-desktop/src/render.rs`
- `apps/autopilot-desktop/src/input.rs`
- `apps/autopilot-desktop/src/app_state.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs`
- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs`
- `apps/autopilot-desktop/src/pane_renderer.rs`
- `apps/autopilot-desktop/src/panes/wallet.rs`

Local host observations:

- `OPENAGENTS_IDENTITY_MNEMONIC_PATH` environment state
- file metadata for the resolved mnemonic path
- file metadata and SQLite schema for the resolved Spark storage directory

## Executive Summary

There is no separate Spark wallet mnemonic in the desktop app today.

Autopilot Desktop uses one shared BIP-39 mnemonic file for both:

- the Nostr identity, and
- the Spark wallet signer.

By default that file lives at:

- `~/.openagents/pylon/identity.mnemonic`

unless the process sets:

- `OPENAGENTS_IDENTITY_MNEMONIC_PATH`

The mnemonic is generated automatically on first launch if the file is missing. It is written as plaintext to disk with `0600` permissions on Unix. Spark then reads that same file, derives a Bitcoin key at `m/44'/0'/0'/0/0`, and passes the original mnemonic into Breez Spark as `Seed::Mnemonic`.

The most important consequence is not subtle:

- clicking `Regenerate` in the Nostr identity pane also rotates the Spark wallet root, because both identities come from the same mnemonic file.

There is no explicit wallet-seed backup flow, no split between social identity and money identity, and no at-rest encryption for the mnemonic itself.

## Direct Answers

## 1. Where is the wallet mnemonic stored?

Default path:

- `$HOME/.openagents/pylon/identity.mnemonic`

Code authority:

- `crates/nostr/core/src/identity.rs:35-47`

Override path:

- `OPENAGENTS_IDENTITY_MNEMONIC_PATH`

Code authority:

- `crates/nostr/core/src/identity.rs:8`
- `crates/nostr/core/src/identity.rs:35-40`

Important clarification:

- this is not a Spark-specific mnemonic file
- it is the shared identity mnemonic file that Spark reuses

## 2. How is it generated?

Generation path:

1. App startup calls `load_or_create_identity()`.
2. If the mnemonic file already exists, it is loaded.
3. If the file does not exist, `regenerate_identity()` creates a new mnemonic and writes it to disk.

Code authority:

- `apps/autopilot-desktop/src/render.rs:177-180`
- `crates/nostr/core/src/identity.rs:20-33`

Entropy and mnemonic format:

- `generate_mnemonic()` creates `16` random bytes with `rand::random()`
- those bytes are converted into an English BIP-39 mnemonic
- `16` bytes of entropy means a `12`-word mnemonic

Code authority:

- `crates/nostr/core/src/identity.rs:80-84`

## 3. How is it stored on disk?

Storage behavior:

- parent directory is created if needed
- mnemonic is written as plaintext with a trailing newline
- Unix permissions are then set to `0600`

Code authority:

- `crates/nostr/core/src/identity.rs:87-104`

Local host observation during this audit:

- `OPENAGENTS_IDENTITY_MNEMONIC_PATH` was unset
- resolved file was `/Users/christopherdavid/.openagents/pylon/identity.mnemonic`
- file mode was `-rw-------`
- file size was `72` bytes

Interpretation:

- that size is consistent with a one-line 12-word mnemonic plus newline

## 4. Is it stored in the OS keychain?

No.

The mnemonic is stored as a plaintext file, not in the keychain.

The keychain-backed credentials system is used for things like:

- `OPENAGENTS_SPARK_API_KEY`

but not for the wallet mnemonic.

Code authority:

- `docs/CREDENTIALS.md`
- `apps/autopilot-desktop/src/credentials.rs`
- `apps/autopilot-desktop/src/spark_wallet.rs:642-653`
- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs:524-563`

## 5. How does Spark use it?

Desktop wallet initialization:

1. Resolve `identity_mnemonic_path()`
2. Read the mnemonic file
3. Build a `SparkSigner` from that mnemonic
4. Use an empty passphrase: `""`
5. Initialize `SparkWallet` with that signer and a storage directory next to the mnemonic file

Code authority:

- `apps/autopilot-desktop/src/spark_wallet.rs:503-545`
- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs:86-106`

The reusable Spark signer derives a Bitcoin private key from:

- BIP-39 seed from the mnemonic
- BIP-32 path `m/44'/0'/0'/0/0`

Code authority:

- `crates/spark/src/signer.rs:20-55`

The reusable wallet then passes the original mnemonic into Breez Spark as:

- `Seed::Mnemonic { mnemonic, passphrase }`

Code authority:

- `crates/spark/src/wallet.rs:96-118`

## 6. Does Spark use a passphrase?

Not in the desktop app.

Every reviewed desktop call site uses:

- `SparkSigner::from_mnemonic(&mnemonic, "")`

That means the effective BIP-39 passphrase is empty today.

Code authority:

- `apps/autopilot-desktop/src/spark_wallet.rs:517-519`
- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs:87-89`

## 7. Where is Spark wallet state stored?

App-owned desktop path:

- `identity_path.parent()/spark`

So the default storage root is:

- `~/.openagents/pylon/spark`

Code authority:

- `apps/autopilot-desktop/src/spark_wallet.rs:521-535`
- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs:315-322`

The reusable crate has a different default:

- `dirs::data_local_dir()/openagents/spark`

but the desktop app overrides it with the identity-adjacent storage path above.

Code authority:

- `crates/spark/src/wallet.rs:38-49`
- `apps/autopilot-desktop/src/spark_wallet.rs:532-535`

Local host observation during this audit:

- resolved storage root was `/Users/christopherdavid/.openagents/pylon/spark`
- it contained both `mainnet` and `regtest` subtrees
- example files included:
  - `/Users/christopherdavid/.openagents/pylon/spark/mainnet/d2be29c9/storage.sql`
  - `/Users/christopherdavid/.openagents/pylon/spark/regtest/e84b9ac6/storage.sql`

## 8. Does the Spark storage DB appear to store the mnemonic too?

I did not find schema evidence of a dedicated mnemonic/seed table or column in the sampled `storage.sql` file.

Schema tables observed included:

- `payments`
- `payment_details_lightning`
- `payment_details_spark`
- `payment_metadata`
- `settings`
- `sync_*`
- `unclaimed_deposits`
- `lnurl_receive_metadata`

Important nuance:

- absence of a `mnemonic` column name is not a cryptographic guarantee that no sensitive wallet material is stored there
- but the schema does clearly store operationally sensitive data such as invoices and preimages

Example schema evidence:

- `payment_details_lightning` columns include `invoice`, `payment_hash`, `destination_pubkey`, `description`, `preimage`
- `settings` is plain `key` / `value`

Local host observation:

- sampled `storage.sql` files were mode `-rw-r--r--` on this machine

That is weaker than the mnemonic file permissions.

## Key Derivation Model

## Nostr identity derivation

The mnemonic also derives the Nostr identity.

Code authority:

- `crates/nostr/core/src/nip06.rs:34-43`
- `crates/nostr/core/src/nip06.rs:67-101`

Nostr path:

- `m/44'/1237'/0'/0/0`

Implication:

- the same root mnemonic controls both messaging identity and wallet custody

## Spark wallet derivation

Spark signer path:

- `m/44'/0'/0'/0/0`

Code authority:

- `crates/spark/src/signer.rs:28-39`

Implication:

- the app has one root secret with two major domains hanging off it
- compromise of the mnemonic compromises both Nostr identity and Spark wallet

## UI And Operational Exposure

## 1. The app can reveal the mnemonic on screen

The Nostr Keys pane displays:

- identity path
- `npub`
- masked `nsec`
- masked private key hex
- masked mnemonic

When secrets are revealed, the pane renders the full mnemonic.

Code authority:

- `apps/autopilot-desktop/src/pane_renderer.rs:4142-4264`
- `docs/PANES.md:171-177`

There is no dedicated `Copy mnemonic` action in the reviewed pane logic, which is good, but:

- screen reveal is still enough to expose full wallet custody

## 2. Regenerating Nostr keys rotates the wallet

The `Regenerate` action calls `regenerate_identity()`, replaces the mnemonic file, then queues a Spark wallet refresh.

Code authority:

- `apps/autopilot-desktop/src/input.rs:2633-2644`

Practical consequence:

- this is not just a social-identity rotation
- it is also a wallet-root rotation
- if funds remain tied to the previous mnemonic and the user has not backed it up, access can be lost

This is the single highest-severity product/custody issue surfaced by this audit.

## 3. First launch silently creates custody material

Startup calls `load_or_create_identity()` automatically.

Code authority:

- `apps/autopilot-desktop/src/render.rs:177-180`

Practical consequence:

- a first-run user can end up with a live wallet root on disk before any explicit backup/export confirmation flow

This conflicts with the spirit of `docs/MVP.md`, which says wallet custody should be explicit and truthful rather than magical or hidden.

Relevant product authority:

- `docs/MVP.md:86`
- `docs/MVP.md:242-247`

## 4. Settings path is informational, not authoritative

The settings document contains an `identity_path` field, but on parse the app overwrites it with the resolved mnemonic path from `nostr::identity_mnemonic_path()`.

Code authority:

- `apps/autopilot-desktop/src/app_state.rs:6409-6503`

Practical consequence:

- changing the settings file is not the real way to relocate wallet identity
- the real control surface is `OPENAGENTS_IDENTITY_MNEMONIC_PATH`

## Findings

## Critical

### 1. One mnemonic controls both Nostr identity and money

Evidence:

- `crates/nostr/core/src/identity.rs:67-77`
- `apps/autopilot-desktop/src/spark_wallet.rs:508-519`
- `crates/spark/src/wallet.rs:96-118`

Impact:

- revealing, replacing, or losing the identity mnemonic also reveals, replaces, or loses wallet custody

### 2. Identity regeneration is wallet rotation with no explicit wallet warning

Evidence:

- `apps/autopilot-desktop/src/input.rs:2633-2644`

Impact:

- a user can unintentionally rotate away from the wallet that holds funds

## High

### 3. Wallet root secret is stored as plaintext on disk

Evidence:

- `crates/nostr/core/src/identity.rs:87-104`

Impact:

- any local compromise with read access to that file gets full Nostr and wallet custody

### 4. First-run auto-creation hides the custody boundary

Evidence:

- `apps/autopilot-desktop/src/render.rs:177-180`
- `crates/nostr/core/src/identity.rs:20-33`

Impact:

- users can receive or earn sats before they have clearly backed up the root secret

## Medium

### 5. Spark operational storage permissions are weaker than the mnemonic file on the audited machine

Evidence:

- local host inspection found:
  - mnemonic file `0600`
  - Spark storage directory `0755`
  - sampled `storage.sql` files `0644`
- sampled schema includes Lightning `invoice` and `preimage` fields

Impact:

- while this is not the root mnemonic itself, operationally sensitive wallet data is readable more broadly than the mnemonic file

### 6. No BIP-39 passphrase is used by desktop

Evidence:

- `apps/autopilot-desktop/src/spark_wallet.rs:517-519`
- `apps/autopilot-desktop/src/bin/spark_wallet_cli.rs:87-89`

Impact:

- custody relies entirely on the mnemonic file with no second factor at the seed-derivation layer

## Recommendations

## P0

1. Split wallet custody from Nostr identity custody.
2. Add an explicit warning that `Regenerate Nostr Keys` also rotates the Spark wallet until the split exists.
3. Add mandatory backup/export acknowledgement before allowing first funding or provider earnings.

## P1

1. Stop storing the mnemonic as a raw plaintext file, or encrypt it locally behind an explicit unlock secret.
2. Tighten Spark storage directory and file permissions to `0700` / `0600`.
3. Add a dedicated wallet recovery/export pane that explains exactly which secret restores funds.

## P2

1. Remove ambiguity between settings path display and real authority path by surfacing the env override state directly in UI.
2. Audit Spark `storage.sql` contents more deeply for sensitive material retention and redaction requirements.
3. Revisit the embedded default Spark API key in `apps/autopilot-desktop/src/spark_wallet.rs:20` as a separate credential-hardening issue.

## Bottom Line

The wallet mnemonic is currently:

- shared with the Nostr identity,
- auto-generated on first launch,
- stored as plaintext at `~/.openagents/pylon/identity.mnemonic` by default,
- used with no passphrase,
- and treated as the root secret for both identity and money.

That makes the current system easy to bootstrap, but it also means the Nostr identity lane is effectively the wallet custody lane. Today those are not safely separated in either storage or UX.

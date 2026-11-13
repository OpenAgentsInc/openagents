# Coax Native read-only Bitcoin balance, 2026-11-13

OpenAgents 1.0.0 build **1069**, bundle `com.openagents.app`, is **VALID** /
**IN_BETA_TESTING** in App Store Connect. Release notes and assignment to the
existing **Owner** internal group were verified. Apple accepted delivery
`19b028fe-9dcd-4eed-818d-47a510f757a9` with no upload errors. Source feature
commit: `2e64322e46`. Archive, IPA, runtime config and test attachments are
retained outside the repository at
`~/.openagents/releases/openagents-ios-1069/`; the existing symbols bucket
also holds the dSYM under UUID `3b7419a840df3cdaa21e544e9b4f1fb8`.

## Existing wallet compatibility

**More → Bitcoin balance** is an OpenAgents-only Coax side module at
`apps/openagents-ios/extras/`. The generic book starter's Coax application
and all book files are unchanged. UI tests used **12345 fixture sats**;
that number is not the owner's balance.

The adapter reuses `WalletKeychain.readSeed`: generic-password service
`com.openagents.app.spark`, account `seed-v1`, non-synchronizable, default
access group, 16/32-byte entropy. Bundle ID, team and entitlements are
retained. The same verified English BIP-39 resource, empty passphrase,
default mainnet account and `Application Support/spark-mainnet` directory
are used. Connection setup is shared with the earlier `SparkHost`.

SDK `breez-sdk-spark-swift` remains pinned to **0.27.2**, revision
`259f79b8572ed4a1df8b5f6fea64c6f2e534a65d`. Its server preset disables
background tasks and optimizers; real-time sync is off and automatic deposit
claim fees are unset. An explicit `syncWallet` precedes
`getInfo(ensureSynced: false)`, then the SDK disconnects. This follows the
SDK's [balance API](https://sdk-doc-spark.breez.technology/guide/get_info.html)
for a connection without background sync. The adapter exposes only a balance
observation. Previous payment wrappers and the seed-add method are excluded
by `COAX_NATIVE_BALANCE`. Seeds/words and SDK error text never enter Coax/JS.

## Checks actually run

- `pay/wallet-balance-policy.coax`: **27 mutants killed, zero survivors**,
  with no equivalents excluded. It uses the existing Bitcoin supply limit;
  missing/locked/error never becomes zero.
- The shared exact JSON fixture/profile check and extras Coax suite passed.
- Native unit tests passed for existing entropy reuse, unavailable states,
  SDK configuration and the historical directory; the existing package
  signature/recovery tests passed too: **six native unit tests** in total.
- **Three UI tests** passed: fixture balance/refresh/close with no payment
  controls, missing-vs-zero, and the unchanged starter counter.
- A fresh in-memory throwaway wallet in a separate temporary directory
  connected to real Spark mainnet, synced and returned zero: **2.692 s**.
- A separate **locally signed simulator** test started only when both the
  default seed item and default SDK directory were absent. It added fresh
  test entropy in that simulator, called the actual native `wallet.balance`
  bridge, decoded an available zero-sat record, and verified that the saved
  entropy remained identical. It removed only its own temporary data:
  **2.439 s**. The initial unsigned simulator attempt skipped because its
  Keychain was unavailable; it is not counted as a pass.
- Tools, wire, layer, parity, state-keep, agent-reference and touched-suite
  pre-push checks passed. No book paths changed.

## Runtime delivery

Runtime fingerprint:
`0495897de3936831430611873a4ded8d9b4bdec8a0ecb8c511b680fea3ba9450`.
Generation **2**, containing eleven artifacts including the independent
extras Wasm/JS, is published under
`https://updates.openagents.com/runtimes/<fingerprint>/latest.cnu`.
The HTTPS envelope matched the locally signed envelope byte for byte:
SHA-256 `e9b84a115f4bf433fa7bae092283c8882db664ec02857b30a0c1010bd17398df`.
Build 1068 retains its original root feed and generation 1. New runtime
packages therefore do not displace its compatible update.

Preparation now copies the complete OpenAgents resource set in Coax,
including the side module, so later app OTA publications retain it. Its
generated HTML and JS were byte-identical to the uploaded archive. Removing
that optional UI later requires only an app-package change.

The balance is bundled in 1069 and does not depend on an OTA download.
The owner's actual funds were not opened by these development tests. Its
on-phone sats display and refresh are the remaining acceptance check in
[NEEDS_OWNER.md](../../NEEDS_OWNER.md).

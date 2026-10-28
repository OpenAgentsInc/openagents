# Lightning

*Notes from 2026-10-27. Nothing in the current tree sends or receives
Lightning payments. These files plan bringing a wallet back to
`apps/openagents-ios`.*

**Decision: the first wallet is Breez's Spark SDK, on mainnet, inside the
iOS app.** Swift talks to the SDK. Coax holds the payment rules and is
checked before every build.

| File | What |
| --- | --- |
| [breez-spark.md](breez-spark.md) | How Spark and the Breez SDK work, the trust model, the parts we use and the parts we keep off, keys, and x402 |
| [history.md](history.md) | Every time Breez or Spark was in this repository, what broke, and the owner decisions that still apply |
| [coax-integration.md](coax-integration.md) | How the wallet sits in the iOS app: the Swift host, the Coax authority, the boundary between them, and the build |
| [testing.md](testing.md) | Checks, mutation, fuzzing, unit, local-regtest and mainnet tests, and what each one proves |

## Why Breez Spark

- **It ships on iOS today.** `breez-sdk-spark-swift` is a published Swift
  package, at `0.27.2` as of this note. It works on mainnet with Breez's
  API key.
- **We have run it.** The phone wallet was on Spark mainnet from 2026-09-28
  until the Nuke. Computers, Pylon payouts and `name@openagents.com`
  addresses used it too. See [history.md](history.md).
- **One set of words.** Spark takes the same 12 BIP-39 words that derive
  the NIP-06 Nostr key. The wallet needs no second backup.
- **Receiving works while the app is closed.** The Lightning side is
  hosted by Spark's service provider.
- **It is Bitcoin.** Spark and Lightning fit the money rule in the
  [protocol spec](../protocol/openagents-protocol-spec.md) §26.1.
  The SDK's dollar and cross-chain features don't fit, and stay off.

## Alternatives we looked at

- **Lightning Labs' Wavelength** (Ark plus swaps). Its trust model is
  better, but it is at 0.2.0. It has no mainnet preset, and its Swift
  wrapper isn't published. Our notes were removed after this decision;
  they are in git at `215b1f6fe9:docs/wavelength/`. Look again when it
  reaches mainnet defaults and publishes a Swift package.
- **`ldk-node`** in the app. It is fully self-custodial, but it needs
  channels, inbound liquidity and an LSP, and it receives only while
  running. It stays the right choice for an x402 receiver (see
  [breez-spark.md](breez-spark.md#x402)), not for a person's phone wallet.

## Rules that apply here

- The money rule: Bitcoin, Lightning, Spark and Taproot Assets only. No
  USDC, USDT, EVM, Solana, Tron or Cashu.
- The words' entropy is the Keychain item `com.openagents.app.spark`,
  account `seed-v1`, this device only. It is reused, never regenerated or
  overwritten. It stays in memory only, is never logged, and is shown once
  behind a warning.
- Tests never move the owner's money. Live tests use a throwaway seed and a
  few sats.
- Treasury, payouts and provider settlement stay off a person's Spark
  wallet.

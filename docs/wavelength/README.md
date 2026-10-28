# Wavelength

*Notes from 2026-10-27, reviewed against `lightninglabs/wavelength-sdk` at
`6f71128` (SDK 0.2.0, daemon `v0.2.0`) and the public docs at
[wavelength.lightning.engineering](https://wavelength.lightning.engineering).
Nothing in OpenAgents uses Wavelength yet.*

Wavelength is Lightning Labs' self-custodial Bitcoin wallet that runs inside
an app. Users send and receive Lightning payments without running a node,
opening channels or arranging inbound liquidity, and the keys stay on their
device. It is Bitcoin only, so it fits the money rule in the
[protocol spec](../protocol/openagents-protocol-spec.md) (§26).

| File | What |
| --- | --- |
| [native-ios-android.md](native-ios-android.md) | The Swift and Kotlin SDK, and how it would sit in `apps/openagents-ios` |
| [considerations.md](considerations.md) | Trust model, risks, how it fits our stack, and a recommendation |
| [shared-words.md](shared-words.md) | How its aezeed mnemonic works, and opening it from our 12 NIP-06 words |
| [vs-breez-spark.md](vs-breez-spark.md) | Wavelength against Breez Spark for the iOS wallet, our Breez history, and which to ship first |

## The pieces

| Repository | What it is |
| --- | --- |
| [lightninglabs/wavelength](https://github.com/lightninglabs/wavelength) | The wallet daemon, `waved`, written in Go. Each release publishes the wasm runtime and the gomobile bindings (`Wavewalletdk.xcframework`, `Wavewalletdk.aar`). It also ships `wavecli`. |
| [lightninglabs/wavelength-sdk](https://github.com/lightninglabs/wavelength-sdk) | The TypeScript SDK: `wavelength-core` (types, `WavelengthClient`, `WalletEngine`), `wavelength-web` (wasm in a Web Worker), `wavelength-react-native`, and `wavelength-react` (a provider and hooks). MIT. |
| [lightninglabs/wavelength-mobile](https://github.com/lightninglabs/wavelength-mobile) | The native wrappers: `ios/WalletKit` (Swift) and `android/walletkit` (Kotlin), with sample apps. They aren't published as packages yet. |

All three share one daemon and one wallet API, so a wallet behaves the same
in the browser, in React Native and in a native app. Each SDK release pins a
daemon release (`RUNTIME_MANIFEST_VERSION`).

## How it works

- **Ark for balances.** Funds are held as VTXOs, off-chain outputs
  coordinated by an Ark server (the "operator") and backed by transactions
  the user can broadcast to exit on their own. VTXOs expire, so the wallet
  must come online to refresh them before their deadlines.
- **Swaps for Lightning.** Lightning sends and receives go through a swap
  server that moves value between Ark and Lightning.
- **On-chain data** comes from an Esplora server.
- **The daemon runs inside the app.**
  - On the web it is WebAssembly in a Web Worker. The SDK checks the SHA-256
    digests of the wasm and bootstrap scripts against values pinned for the
    release.
  - On mobile it is a gomobile library that serves gRPC over an in-memory
    channel. No port is opened and no second process runs.
- **Endpoints.** `defaultConfig` presets exist for signet, testnet and
  testnet4, all on Lightning Labs domains (`*.wavelength.lightning.finance`,
  `*.lightningcluster.com`).
  - Mainnet has no preset. It needs hand-entered endpoints and an explicit
    `allowMainnet: true`.
  - Regtest needs hand-entered endpoints and insecure-transport flags.

## Keys

One HD seed per wallet, from one of two sources
([key management](https://github.com/lightninglabs/wavelength-sdk/blob/main/docs/key-management.md)):
- **Password wallet.** The seed comes from a 24-word aezeed mnemonic, and the
  password encrypts the wallet database.
- **Passkey wallet.** The seed is derived from a WebAuthn PRF output with
  HKDF-SHA256, so the passkey is both the unlock credential and the root
  secret.

The seed lives only in the encrypted wallet database: OPFS in the browser,
the app sandbox on mobile. Recovery can ask the operator's indexer to rebuild
VTXOs and receive history ("server-assisted recovery"). Recovery restores
keys, not deadlines: a wallet that was offline too long may have missed the
window to refresh or exit its VTXOs.

## Amounts

The API is denominated in satoshis unless a field says otherwise. OpenAgents
`Amount` is millisatoshis. Any integration converts at one checked boundary:
it multiplies by 1000 going in, and refuses sub-satoshi remainders going out.

## The Claude Code plugin

`/plugin marketplace add lightninglabs/wavelength-sdk` adds a marketplace
with one plugin, `wavelength`, which is four skills:
- `wavelength-web`;
- `wavelength-react-native`;
- `wavelength-api` (the `waved` gRPC and REST API);
- `wavelength-cli` (`wavecli`, which also has `wavecli mcp serve`).

They route Claude to the docs site's `llms.txt` and `.md` pages. There are no
hooks, MCP servers or install scripts. The skills don't cover the native
Swift and Kotlin SDK. For that, use
[native-ios-android.md](native-ios-android.md) and the wavelength-mobile
docs.
